use http::StatusCode;
use log::error;
use reqwest::Client;
use std::io::Write;
use tokio::task;
use usvg::{ImageHrefResolver, ImageKind, Options};

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

/// A shorthand for [ImageHrefResolver]'s string function.
/// This isn't exposed publicly by usvg, so copied here
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

/// Custom image url string resolver that handles downloading remote files
/// (The default usvg implementation only supports local image files)
pub fn custom_string_resolver() -> ImageHrefStringResolverFn {
    let default_string_resolver = ImageHrefResolver::default_string_resolver();
    Box::new(move |href: &str, opts: &Options| {
        if href.starts_with("http://") || href.starts_with("https://") {
            // parse as file to extract the extension
            let href_path = std::path::Path::new(href);
            let extension = href_path
                .extension()
                .and_then(|ext| ext.to_str().map(|ext| format!(".{}", ext)))
                .unwrap_or("".to_string());

            // Download image to temporary file with reqwest
            let bytes: Option<_> = task::block_in_place(move || {
                IMAGE_TOKIO_RUNTIME.block_on(async {
                    if let Ok(response) = REQWEST_CLIENT.get(href).send().await {
                        // Check status code.
                        match response.status() {
                            StatusCode::OK => response.bytes().await.ok(),
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
                                None
                            }
                        }
                    } else {
                        None
                    }
                })
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
