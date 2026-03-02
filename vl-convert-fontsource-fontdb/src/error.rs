use thiserror::Error;

#[derive(Debug, Error)]
pub enum FontsourceFontdbError {
    #[error("Font not found: \"{0}\"")]
    FontNotFound(String),

    #[error("Invalid font ID: \"{0}\". Must match [a-z0-9][a-z0-9_-]*")]
    InvalidFontId(String),

    #[error("Requested variants not available for font \"{font_id}\": {unavailable:?}")]
    VariantsNotAvailable {
        font_id: String,
        unavailable: Vec<crate::types::VariantRequest>,
    },

    #[error("No variants requested (empty list)")]
    NoVariantsRequested,

    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("HTTP status error ({status}) for URL: {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl FontsourceFontdbError {
    pub(crate) fn is_retryable(&self) -> bool {
        match self {
            Self::Http(_) => true,
            Self::HttpStatus { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        }
    }

    pub(crate) fn from_reqwest(url: &str, err: reqwest::Error) -> Self {
        match err.status() {
            Some(status) => Self::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            },
            None => Self::Http(err.to_string()),
        }
    }
}
