use crate::converter::ACCESS_DENIED_MARKER;
use crate::data_ops::AllowedBaseUrlPattern;
use backon::{BlockingRetryable, ExponentialBuilder};
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::url::Url;
use log::{info, warn};
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use usvg::{ImageHrefResolver, Options};

pub(crate) static VL_CONVERT_USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

lazy_static! {
    static ref BLOCKING_CLIENT: reqwest::blocking::Client = reqwest::blocking::ClientBuilder::new()
        .user_agent(VL_CONVERT_USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(crate::data_ops::REQUEST_TIMEOUT)
        // Redirects are followed manually so every hop is checked against the
        // access policy before it is fetched.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to construct blocking reqwest client");
}

thread_local! {
    static IMAGE_ACCESS_POLICY: RefCell<Option<ImageAccessPolicy>> = const { RefCell::new(None) };
    static IMAGE_ACCESS_ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAccessPolicy {
    /// Parsed allowlist patterns (passed directly to `is_access_allowed()`).
    pub allowed_base_urls: Option<Vec<AllowedBaseUrlPattern>>,
    /// Directory that relative image hrefs resolve against, installed as usvg's
    /// `resources_dir`. Derived from a filesystem `base_url`. Resolution only:
    /// every local read is still authorized by `allowed_base_urls`.
    pub resources_dir: Option<PathBuf>,
}

struct PolicyScopeGuard {
    previous_policy: Option<ImageAccessPolicy>,
    previous_errors: Vec<String>,
}

impl PolicyScopeGuard {
    fn install(policy: ImageAccessPolicy) -> Self {
        let previous_policy = IMAGE_ACCESS_POLICY.with(|slot| slot.replace(Some(policy)));
        let previous_errors =
            IMAGE_ACCESS_ERRORS.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
        Self {
            previous_policy,
            previous_errors,
        }
    }

    fn drain_access_errors(&mut self) -> Vec<String> {
        IMAGE_ACCESS_ERRORS.with(|slot| std::mem::take(&mut *slot.borrow_mut()))
    }
}

impl Drop for PolicyScopeGuard {
    fn drop(&mut self) {
        IMAGE_ACCESS_POLICY.with(|slot| {
            *slot.borrow_mut() = self.previous_policy.take();
        });
        IMAGE_ACCESS_ERRORS.with(|slot| {
            *slot.borrow_mut() = std::mem::take(&mut self.previous_errors);
        });
    }
}

pub fn with_image_access_policy<T>(
    policy: ImageAccessPolicy,
    f: impl FnOnce() -> T,
) -> (T, Vec<String>) {
    let mut guard = PolicyScopeGuard::install(policy);
    let result = f();
    let access_errors = guard.drain_access_errors();
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

fn is_url_allowed(uri: &str, patterns: &Option<Vec<AllowedBaseUrlPattern>>) -> bool {
    crate::data_ops::is_access_allowed(uri, patterns)
}

fn resolve_local_href_path(href: &str, opts: &Options) -> Result<PathBuf, AnyError> {
    if href.starts_with("file://") {
        let url = Url::parse(href)?;
        return url
            .to_file_path()
            .map_err(|_| anyhow!("Invalid file URL path: {href}"));
    }
    let path = Path::new(href);
    if path.is_relative() && opts.resources_dir.is_none() {
        bail!("Cannot resolve relative image path '{href}' without a filesystem-backed base_url");
    }
    Ok(opts.get_abs_path(path))
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

fn fetch_http_blocking(
    href: &str,
    allowed_base_urls: &Option<Vec<AllowedBaseUrlPattern>>,
) -> Result<HttpFetchOutcome, reqwest::Error> {
    if !is_url_allowed(href, allowed_base_urls) {
        return Ok(HttpFetchOutcome::AccessDenied {
            message: access_denied_message(format!("External image url not allowed: {href}")),
        });
    }

    // Follow redirects by hand so each destination passes the same check as
    // the URL in the specification.
    let mut current = href.to_string();
    let mut hops = 0usize;
    // One deadline covers the whole redirect chain.
    let deadline = std::time::Instant::now() + crate::data_ops::REQUEST_TIMEOUT;
    let response = loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            warn!("Timed out while loading image from {href}");
            return Ok(HttpFetchOutcome::Failed);
        }
        let response = BLOCKING_CLIENT.get(&current).timeout(remaining).send()?;
        if !response.status().is_redirection() {
            break response;
        }
        hops += 1;
        if hops > crate::data_ops::MAX_REDIRECTS {
            warn!("Too many HTTP redirects while loading image from {href}");
            return Ok(HttpFetchOutcome::Failed);
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok());
        let Some(next) = crate::data_ops::redirect_target(location, &current) else {
            warn!("HTTP redirect from {current} has no usable Location header");
            return Ok(HttpFetchOutcome::Failed);
        };
        let is_http = next.starts_with("http://") || next.starts_with("https://");
        if !is_http || !is_url_allowed(&next, allowed_base_urls) {
            return Ok(HttpFetchOutcome::AccessDenied {
                message: access_denied_message(format!(
                    "External image url not allowed: {next} (redirected from {current})"
                )),
            });
        }
        current = next;
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok().map(|c| c.to_string()));

    match status {
        StatusCode::OK => {
            let bytes = response.bytes()?;
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
            let body = match response.bytes() {
                Ok(bytes) => String::from_utf8_lossy(bytes.as_ref()).to_string(),
                Err(_) => String::new(),
            };
            warn!(
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
                if !is_url_allowed(href, &policy.allowed_base_urls) {
                    push_access_error(access_denied_message(format!(
                        "External image url not allowed: {href}"
                    )));
                    return None;
                }
            }

            // Download image using blocking reqwest on a dedicated thread to avoid
            // interfering with the worker pool's single-threaded Tokio runtime.
            let allowed_base_urls = policy
                .as_ref()
                .and_then(|policy| policy.allowed_base_urls.clone());
            let fetch_outcome = std::thread::scope(|s| {
                s.spawn(move || {
                    let result = (|| fetch_http_blocking(href, &allowed_base_urls))
                        .retry(
                            ExponentialBuilder::default()
                                .with_min_delay(Duration::from_millis(500))
                                .with_max_delay(Duration::from_secs(10))
                                .with_max_times(4),
                        )
                        .when(|e| {
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
                        .call();

                    match result {
                        Ok(outcome) => outcome,
                        Err(e) => {
                            warn!("Failed to load image from url {}: {}", href, e);
                            HttpFetchOutcome::Failed
                        }
                    }
                })
                .join()
                .expect("Image fetch thread panicked")
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
            let extension: Cow<'static, str> = href_path
                .extension()
                .and_then(|ext| ext.to_str().map(|ext| Cow::Owned(format!(".{}", ext))))
                .unwrap_or({
                    // Fall back to extension based on content type
                    if let Some(content_type) = &content_type {
                        match content_type.as_str() {
                            "image/jpeg" => Cow::Borrowed(".jpg"),
                            "image/png" => Cow::Borrowed(".png"),
                            "image/gif" => Cow::Borrowed(".gif"),
                            "image/svg+xml" => Cow::Borrowed(".svg"),
                            _ => Cow::Borrowed(""),
                        }
                    } else {
                        Cow::Borrowed("")
                    }
                });

            // Create the temporary file (maybe with an extension)
            let mut builder = tempfile::Builder::new();
            builder.suffix(extension.as_ref());
            if let Ok(mut temp_file) = builder.tempfile() {
                // Write image contents to temp file and call default string resolver
                // with temporary file path
                if temp_file.write_all(&bytes).is_ok() {
                    let temp_href = temp_file.path();
                    if let Some(temp_href) = temp_href.to_str() {
                        return default_string_resolver(temp_href, opts);
                    }
                }
            }
            return None;
        }

        if let Some(policy) = policy {
            let local_path = match resolve_local_href_path(href, opts) {
                Ok(local_path) => local_path,
                Err(err) => {
                    push_access_error(format!(
                        "Failed to resolve local image path from href {href}: {err}"
                    ));
                    return None;
                }
            };

            // Same rule as data loading: the canonical path must fall under an
            // allowlisted directory (or match `*`). `base_url` only resolves.
            let allowed_path =
                match crate::data_ops::authorize_local_path(&local_path, &policy.allowed_base_urls)
                {
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

/// Infer MIME type from image bytes (magic bytes) or content-type header.
fn infer_image_mime(bytes: &[u8], content_type: Option<&str>) -> &'static str {
    // Check magic bytes first
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "image/png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF8") {
        return "image/gif";
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
        return "image/svg+xml";
    }
    // Fall back to content-type header
    if let Some(ct) = content_type {
        if ct.starts_with("image/") {
            // Return a static str for common types
            if ct.starts_with("image/png") {
                return "image/png";
            }
            if ct.starts_with("image/jpeg") {
                return "image/jpeg";
            }
            if ct.starts_with("image/gif") {
                return "image/gif";
            }
            if ct.starts_with("image/webp") {
                return "image/webp";
            }
            if ct.starts_with("image/svg+xml") {
                return "image/svg+xml";
            }
        }
    }
    "application/octet-stream"
}

/// Fetch an HTTP image URL (policy-checked), returning `(mime, base64)`.
///
/// Returns an error if the URL is denied by policy or the fetch fails.
pub(crate) fn fetch_and_encode_image_http(
    href: &str,
    allowed_base_urls: &Option<Vec<AllowedBaseUrlPattern>>,
) -> Result<(String, String), AnyError> {
    use base64::Engine;

    if !is_url_allowed(href, allowed_base_urls) {
        bail!(
            "{}",
            access_denied_message(format!("External image url not allowed: {href}"))
        );
    }

    let outcome = std::thread::scope(|s| {
        let handle = s.spawn(move || {
            (|| fetch_http_blocking(href, allowed_base_urls))
                .retry(
                    ExponentialBuilder::default()
                        .with_min_delay(Duration::from_millis(500))
                        .with_max_delay(Duration::from_secs(10))
                        .with_max_times(4),
                )
                .when(|e| {
                    e.status()
                        .map(|s| s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS)
                        .unwrap_or(true)
                })
                .call()
        });
        handle.join().expect("Image fetch thread panicked")
    });

    match outcome {
        Ok(HttpFetchOutcome::Success {
            bytes,
            content_type,
        }) => {
            let mime = infer_image_mime(&bytes, content_type.as_deref());
            if mime == "application/octet-stream" {
                bail!(
                    "Response from {href} is not a recognized image format \
                     (content-type: {})",
                    content_type.as_deref().unwrap_or("none")
                );
            }
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            Ok((mime.to_string(), b64))
        }
        Ok(HttpFetchOutcome::AccessDenied { message }) => {
            bail!("{message}")
        }
        Ok(HttpFetchOutcome::Failed) => {
            bail!("Failed to fetch image from URL: {href}")
        }
        Err(e) => {
            bail!("Failed to fetch image from URL {href}: {e}")
        }
    }
}

/// Resolve a local image path (relative or file://), authorize it against
/// `allowed_base_urls`, read the file, and return `(mime, base64)`.
///
/// Relative paths are resolved against `resources_dir` (derived from the
/// converter's `base_url`); absolute paths and `file://` URLs need no root.
pub(crate) fn resolve_and_read_local_image(
    href: &str,
    allowed_base_urls: &Option<Vec<AllowedBaseUrlPattern>>,
    resources_dir: Option<&Path>,
) -> Result<(String, String), AnyError> {
    use base64::Engine;

    // Resolve the path
    let abs_path = if href.starts_with("file://") {
        let url = Url::parse(href)?;
        url.to_file_path()
            .map_err(|_| anyhow!("Invalid file URL path: {href}"))?
    } else {
        let path = Path::new(href);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            match resources_dir {
                Some(dir) => dir.join(path),
                None => bail!(
                    "Cannot resolve relative image path '{href}' without a filesystem-backed base_url"
                ),
            }
        }
    };

    // Same authorization as data loading and the usvg resolver.
    let abs_path = crate::data_ops::authorize_local_path(&abs_path, allowed_base_urls)?;

    // Read the file
    let bytes = std::fs::read(&abs_path).map_err(|e| {
        anyhow!(
            "Failed to read local image file {}: {e}",
            abs_path.display()
        )
    })?;

    let mime = infer_image_mime(&bytes, None);
    if mime == "application/octet-stream" {
        bail!(
            "File {} is not a recognized image format",
            abs_path.display()
        );
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok((mime.to_string(), b64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn test_policy(label: &str) -> ImageAccessPolicy {
        ImageAccessPolicy {
            allowed_base_urls: Some(Vec::new()),
            resources_dir: Some(PathBuf::from(format!("/tmp/{label}"))),
        }
    }

    #[test]
    fn with_image_access_policy_restores_previous_state_after_panic() {
        let (panic_result, outer_errors) = with_image_access_policy(test_policy("outer"), || {
            push_access_error("outer-error".to_string());
            catch_unwind(AssertUnwindSafe(|| {
                let _ = with_image_access_policy(test_policy("inner"), || {
                    push_access_error("inner-error".to_string());
                    panic!("boom");
                });
            }))
        });

        assert!(panic_result.is_err());
        assert_eq!(outer_errors, vec!["outer-error".to_string()]);
        assert_eq!(current_access_policy(), None);
        assert!(IMAGE_ACCESS_ERRORS.with(|slot| slot.borrow().is_empty()));
    }
}
