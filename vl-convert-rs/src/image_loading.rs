use backon::{ExponentialBuilder, Retryable};
use log::{error, info, warn};
use reqwest::{Client, StatusCode};
use std::io::Write;
use std::time::Duration;
use tokio::task;
use usvg::{ImageHrefResolver, Options};

static VL_CONVERT_USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

lazy_static! {
    static ref IMAGE_TOKIO_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
    static ref REQWEST_CLIENT: Client = reqwest::ClientBuilder::new()
        .user_agent(VL_CONVERT_USER_AGENT)
        .build()
        .expect("Failed to construct reqwest client");
}

/// Custom image url string resolver that handles downloading remote files
/// (The default usvg implementation only supports local image files)
pub fn custom_string_resolver() -> usvg::ImageHrefStringResolverFn<'static> {
    let default_string_resolver = ImageHrefResolver::default_string_resolver();

    Box::new(move |href: &str, opts: &Options| {
        info!("Resolving image: {href}");
        if href.starts_with("http://") || href.starts_with("https://") {
            // Download image to temporary file with reqwest, retrying on transient errors
            let (bytes, content_type): (Option<_>, Option<_>) = task::block_in_place(move || {
                IMAGE_TOKIO_RUNTIME.block_on(async {
                    let result = (|| async {
                        let response = REQWEST_CLIENT.get(href).send().await?;
                        let status = response.status();
                        let content_type = response
                            .headers()
                            .get("Content-Type")
                            .and_then(|h| h.to_str().ok().map(|c| c.to_string()));

                        match status {
                            StatusCode::OK => {
                                let bytes = response.bytes().await?;
                                Ok((Some(bytes), content_type))
                            }
                            s if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS => {
                                // Transient HTTP error — signal for retry
                                Err(response.error_for_status().unwrap_err())
                            }
                            s => {
                                // Permanent HTTP error — log and short-circuit without retrying
                                let body = response
                                    .bytes()
                                    .await
                                    .map(|b| String::from_utf8_lossy(&b).to_string())
                                    .unwrap_or_default();
                                error!(
                                    "Failed to load image from url {} with status code {:?}\n{}",
                                    href, s, body
                                );
                                Ok((None, None))
                            }
                        }
                    })
                    .retry(
                        ExponentialBuilder::default()
                            .with_min_delay(Duration::from_millis(500))
                            .with_max_delay(Duration::from_secs(10))
                            .with_max_times(4),
                    )
                    .when(|e| {
                        // Retry on network errors (no status) and transient HTTP errors
                        e.status()
                            .map(|s| s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS)
                            .unwrap_or(true)
                    })
                    .notify(|err, dur| {
                        warn!(
                            "Retrying image load from {} in {:.1}s: {}",
                            href,
                            dur.as_secs_f32(),
                            err
                        );
                    })
                    .await;

                    match result {
                        Ok((bytes, content_type)) => (bytes, content_type),
                        Err(e) => {
                            error!("Failed to load image from url {}: {}", href, e);
                            (None, None)
                        }
                    }
                })
            });

            // Compute file extension, which usvg uses to infer the image type
            let href_path = std::path::Path::new(href);
            let extension = href_path
                .extension()
                .and_then(|ext| ext.to_str().map(|ext| format!(".{}", ext)))
                .unwrap_or_else(|| {
                    // Fall back to extension based on content type
                    if let Some(content_type) = &content_type {
                        match content_type.as_str() {
                            "image/jpeg" => ".jpg".to_string(),
                            "image/png" => ".png".to_string(),
                            "image/gif" => ".gif".to_string(),
                            "image/svg+xml" => ".svg".to_string(),
                            _ => String::new(),
                        }
                    } else {
                        String::new()
                    }
                });

            if let Some(bytes) = bytes {
                // Create the temporary file (maybe with an extension)
                let mut builder = tempfile::Builder::new();
                builder.suffix(extension.as_str());
                if let Ok(mut temp_file) = builder.tempfile() {
                    // Write image contents to temp file and call default string resolver
                    // with temporary file path
                    if temp_file.write(bytes.as_ref()).ok().is_some() {
                        let temp_href = temp_file.path();
                        if let Some(temp_href) = temp_href.to_str() {
                            return default_string_resolver(temp_href, opts);
                        }
                    }
                }
            }
        }

        // Delegate to default implementation
        default_string_resolver(href, opts)
    })
}
