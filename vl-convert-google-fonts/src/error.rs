use thiserror::Error;

use crate::types::GoogleFontUsage;

/// A result whose error includes usage recorded before the failure.
pub type GoogleFontsResult<T> = Result<T, GoogleFontsFailure>;

/// A Google Fonts error together with the work performed before it occurred.
#[derive(Debug)]
pub struct GoogleFontsFailure {
    /// The cause of the failure.
    pub error: GoogleFontsError,
    /// Resolution and download activity recorded before the failure.
    pub usage: GoogleFontUsage,
}

impl GoogleFontsFailure {
    pub(crate) fn new(error: GoogleFontsError, usage: GoogleFontUsage) -> Self {
        Self { error, usage }
    }

    /// Return the underlying error, discarding usage information.
    pub fn into_error(self) -> GoogleFontsError {
        self.error
    }
}

impl std::fmt::Display for GoogleFontsFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for GoogleFontsFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Failures in font resolution, downloading, or caching.
#[derive(Debug, Error)]
pub enum GoogleFontsError {
    /// Google Fonts did not provide the requested family.
    #[error("Font not found: \"{0}\"")]
    FontNotFound(String),

    /// The family could not be represented as a valid normalized font ID.
    #[error("Invalid font ID: \"{0}\". Must match [a-z0-9][a-z0-9_-]*")]
    InvalidFontId(String),

    /// Requested variants could not be resolved for the family.
    #[error("Requested variants not available for font \"{font_id}\": {unavailable:?}")]
    VariantsNotAvailable {
        /// Normalized family identifier.
        font_id: String,
        /// Requested variants that could not be resolved.
        unavailable: Vec<crate::types::VariantRequest>,
    },

    /// A request explicitly supplied an empty variant list.
    #[error("No variants requested (empty list)")]
    NoVariantsRequested,

    /// HTTP client setup or a network request failed.
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// A server returned an unsuccessful HTTP status.
    #[error("HTTP status error ({status}) for URL: {url}")]
    HttpStatus {
        /// URL of the failed request.
        url: String,
        /// HTTP status code returned by the server.
        status: u16,
    },

    /// A filesystem operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The configured cache directory is not an absolute path.
    #[error("Cache directory must be an absolute path, got: {0:?}")]
    RelativeCacheDir(std::path::PathBuf),

    /// Downloaded data did not have a recognized TTF or OTF header.
    #[error("Unexpected font format from Google Fonts (expected TTF/OTF, got {detected}): {url}")]
    UnexpectedFontFormat {
        /// URL that returned the data.
        url: String,
        /// Detected format name or unrecognized header bytes.
        detected: String,
    },

    /// An internal operation failed, such as synchronization or CSS parsing.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl GoogleFontsError {
    pub(crate) fn with_usage(self, usage: impl Into<GoogleFontUsage>) -> GoogleFontsFailure {
        GoogleFontsFailure::new(self, usage.into())
    }

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
