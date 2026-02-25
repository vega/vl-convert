use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontsourceError {
    #[error("Font not found: \"{0}\"")]
    FontNotFound(String),

    #[error("Invalid font ID: \"{0}\". Must match [a-z0-9][a-z0-9_-]*")]
    InvalidFontId(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Failed to determine cache directory")]
    NoCacheDir,
}
