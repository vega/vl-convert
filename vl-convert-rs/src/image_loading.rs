use std::io::Write;
use usvg::{ImageHrefResolver, ImageKind, Options};

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
            if let Ok(get_result) = reqwest::blocking::get(href) {
                if let Ok(bytes) = get_result.bytes() {
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
        }

        // Delegate to default implementation
        default_string_resolver(href, opts)
    })
}
