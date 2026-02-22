use crate::converter::ACCESS_DENIED_MARKER;
use backon::{ExponentialBuilder, Retryable};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::url::Url;
use log::{error, info, warn};
use reqwest::{Client, StatusCode};
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
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
    static ref REQWEST_CLIENT_FOLLOW_REDIRECTS: Client = reqwest::ClientBuilder::new()
        .user_agent(VL_CONVERT_USER_AGENT)
        .build()
        .expect("Failed to construct reqwest client");
    static ref REQWEST_CLIENT_NO_REDIRECTS: Client = reqwest::ClientBuilder::new()
        .user_agent(VL_CONVERT_USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to construct reqwest client");
}

thread_local! {
    static IMAGE_ACCESS_POLICY: RefCell<Option<ImageAccessPolicy>> = const { RefCell::new(None) };
    static IMAGE_ACCESS_ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAccessPolicy {
    pub allow_http_access: bool,
    pub filesystem_root: Option<PathBuf>,
    pub allowed_base_urls: Option<Vec<String>>,
}

pub fn with_image_access_policy<T>(
    policy: ImageAccessPolicy,
    f: impl FnOnce() -> T,
) -> (T, Vec<String>) {
    let previous_policy = IMAGE_ACCESS_POLICY.with(|slot| slot.replace(Some(policy)));
    let previous_errors = IMAGE_ACCESS_ERRORS.with(|slot| std::mem::take(&mut *slot.borrow_mut()));

    let result = f();

    let access_errors = IMAGE_ACCESS_ERRORS.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
    IMAGE_ACCESS_POLICY.with(|slot| {
        *slot.borrow_mut() = previous_policy;
    });
    IMAGE_ACCESS_ERRORS.with(|slot| {
        *slot.borrow_mut() = previous_errors;
    });

    (result, access_errors)
}

fn push_access_error(message: String) {
    IMAGE_ACCESS_ERRORS.with(|slot| {
        slot.borrow_mut().push(message);
    });
}

fn current_access_policy() -> Option<ImageAccessPolicy> {
    IMAGE_ACCESS_POLICY.with(|slot| slot.borrow().clone())
}

fn access_denied_message(detail: impl AsRef<str>) -> String {
    format!("{ACCESS_DENIED_MARKER}: {}", detail.as_ref())
}

fn normalize_http_url_for_allowlist(uri: &str) -> Result<String, AnyError> {
    Ok(Url::parse(uri)?.to_string())
}

fn is_url_allowed(uri: &str, allowed_base_urls: &[String]) -> bool {
    let Ok(normalized_uri) = normalize_http_url_for_allowlist(uri) else {
        return false;
    };
    allowed_base_urls
        .iter()
        .any(|allowed_url| normalized_uri.starts_with(allowed_url))
}

fn resolve_local_href_path(href: &str, opts: &Options) -> Result<PathBuf, AnyError> {
    if href.starts_with("file://") {
        let url = Url::parse(href)?;
        return url
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URL path: {href}"));
    }
    Ok(opts.get_abs_path(Path::new(href)))
}

fn resolve_path_for_policy_check(path: &Path) -> Result<PathBuf, AnyError> {
    if path.exists() {
        return crate::converter::portable_canonicalize(path).map_err(|err| {
            anyhow!(
                "Failed to resolve local image path {}: {}",
                path.display(),
                err
            )
        });
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = crate::converter::portable_canonicalize(parent).map_err(|err| {
        anyhow!(
            "Failed to resolve local image parent path {}: {}",
            parent.display(),
            err
        )
    })?;
    let Some(file_name) = path.file_name() else {
        bail!(
            "Failed to resolve local image path {}: missing file name",
            path.display()
        );
    };
    Ok(canonical_parent.join(file_name))
}

fn ensure_path_is_under_root(path: &Path, root: &Path) -> Result<PathBuf, AnyError> {
    let resolved_path = resolve_path_for_policy_check(path)?;
    if !resolved_path.starts_with(root) {
        let detail = format!(
            "filesystem access denied for image path {} (outside filesystem_root {})",
            resolved_path.display(),
            root.display()
        );
        bail!("{}", access_denied_message(detail));
    }
    Ok(resolved_path)
}

enum HttpFetchOutcome {
    Success {
        bytes: Vec<u8>,
        content_type: Option<String>,
    },
    AccessDenied {
        message: String,
    },
    Failed,
}

async fn fetch_http_with_policy(
    href: &str,
    allowed_base_urls: Option<&[String]>,
) -> Result<HttpFetchOutcome, reqwest::Error> {
    if let Some(allowed_base_urls) = allowed_base_urls {
        if !is_url_allowed(href, allowed_base_urls) {
            return Ok(HttpFetchOutcome::AccessDenied {
                message: access_denied_message(format!("External data url not allowed: {href}")),
            });
        }
    }

    let response = if allowed_base_urls.is_some() {
        REQWEST_CLIENT_NO_REDIRECTS.get(href).send().await?
    } else {
        REQWEST_CLIENT_FOLLOW_REDIRECTS.get(href).send().await?
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|h| h.to_str().ok().map(|c| c.to_string()));

    if allowed_base_urls.is_some() && status.is_redirection() {
        return Ok(HttpFetchOutcome::AccessDenied {
            message: access_denied_message(format!(
                "Redirected HTTP URLs are not allowed when allowed_base_urls is configured: {href}"
            )),
        });
    }

    match status {
        StatusCode::OK => {
            let bytes = response.bytes().await?;
            Ok(HttpFetchOutcome::Success {
                bytes: bytes.to_vec(),
                content_type,
            })
        }
        s if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS => {
            // Transient HTTP error — signal for retry.
            Err(response.error_for_status().unwrap_err())
        }
        s => {
            // Permanent HTTP error — log and short-circuit without retrying.
            let body = match response.bytes().await {
                Ok(bytes) => String::from_utf8_lossy(bytes.as_ref()).to_string(),
                Err(_) => String::new(),
            };
            error!(
                "Failed to load image from url {} with status code {:?}\n{}",
                href, s, body
            );
            Ok(HttpFetchOutcome::Failed)
        }
    }
}

/// Custom image url string resolver that handles downloading remote files
/// (The default usvg implementation only supports local image files)
pub fn custom_string_resolver() -> usvg::ImageHrefStringResolverFn<'static> {
    let default_string_resolver = ImageHrefResolver::default_string_resolver();

    Box::new(move |href: &str, opts: &Options| {
        info!("Resolving image: {href}");
        let policy = current_access_policy();

        if href.starts_with("data:") {
            return default_string_resolver(href, opts);
        }

        if href.starts_with("http://") || href.starts_with("https://") {
            if let Some(policy) = policy.as_ref() {
                if !policy.allow_http_access {
                    push_access_error(access_denied_message(format!(
                        "HTTP access denied by converter policy for image URL: {href}"
                    )));
                    return None;
                }

                if let Some(allowed_base_urls) = &policy.allowed_base_urls {
                    if !is_url_allowed(href, allowed_base_urls) {
                        push_access_error(access_denied_message(format!(
                            "External data url not allowed: {href}"
                        )));
                        return None;
                    }
                }
            }

            // Download image to temporary file with reqwest, retrying on transient errors
            let allowed_base_urls = policy
                .as_ref()
                .and_then(|policy| policy.allowed_base_urls.as_deref());
            let fetch_outcome = task::block_in_place(move || {
                IMAGE_TOKIO_RUNTIME.block_on(async {
                    let result =
                        (|| async { fetch_http_with_policy(href, allowed_base_urls).await })
                            .retry(
                                ExponentialBuilder::default()
                                    .with_min_delay(Duration::from_millis(500))
                                    .with_max_delay(Duration::from_secs(10))
                                    .with_max_times(4),
                            )
                            .when(|e| {
                                // Retry on network errors (no status) and transient HTTP errors.
                                e.status()
                                    .map(|s| {
                                        s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS
                                    })
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
                        Ok(outcome) => outcome,
                        Err(e) => {
                            error!("Failed to load image from url {}: {}", href, e);
                            HttpFetchOutcome::Failed
                        }
                    }
                })
            });

            let (bytes, content_type) = match fetch_outcome {
                HttpFetchOutcome::Success {
                    bytes,
                    content_type,
                } => (bytes, content_type),
                HttpFetchOutcome::AccessDenied { message } => {
                    push_access_error(message);
                    return None;
                }
                HttpFetchOutcome::Failed => {
                    return None;
                }
            };

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
            return None;
        }

        if let Some(policy) = policy {
            let Some(filesystem_root) = policy.filesystem_root.as_ref() else {
                push_access_error(access_denied_message(format!(
                    "Filesystem access denied by converter policy for image path: {href}"
                )));
                return None;
            };

            let local_path = match resolve_local_href_path(href, opts) {
                Ok(local_path) => local_path,
                Err(err) => {
                    push_access_error(format!(
                        "Failed to resolve local image path from href {href}: {err}"
                    ));
                    return None;
                }
            };

            let allowed_path = match ensure_path_is_under_root(&local_path, filesystem_root) {
                Ok(path) => path,
                Err(err) => {
                    push_access_error(format!("{err}"));
                    return None;
                }
            };

            if let Some(path_str) = allowed_path.to_str() {
                return default_string_resolver(path_str, opts);
            }
            push_access_error(format!(
                "{ACCESS_DENIED_MARKER}: Filesystem access denied for non-utf8 image path: {}",
                allowed_path.display()
            ));
            return None;
        }

        // Delegate to default implementation
        default_string_resolver(href, opts)
    })
}
