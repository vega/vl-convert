use log::{error, info};
use reqwest::{Client, StatusCode};
use std::io::Write;
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
            // Download image to temporary file with reqwest
            let (bytes, content_type): (Option<_>, Option<_>) = task::block_in_place(move || {
                IMAGE_TOKIO_RUNTIME.block_on(async {
                    if let Ok(response) = REQWEST_CLIENT.get(href).send().await {
                        let content_type = response.headers().get("Content-Type")
                            .and_then(|h| h.to_str().ok().map(|c| c.to_string()));

                        // Check status code.
                        match response.status() {
                            StatusCode::OK => (response.bytes().await.ok(), content_type),
                            status => {
                                let msg = response
                                    .bytes()
                                    .await
                                    .map(|b| String::from_utf8_lossy(b.as_ref()).to_string());
                                if let Ok(msg) = msg {
                                    error!(
                                        "Failed to load image from url {} with status code {:?}\n{}",
                                        href, status, msg
                                    );
                                } else {
                                    error!(
                                        "Failed to load image from url {} with status code {:?}",
                                        href, status
                                    );
                                }
                                (None, None)
                            }
                        }
                    } else {
                        (None, None)
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
