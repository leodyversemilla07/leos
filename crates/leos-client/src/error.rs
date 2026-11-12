//! The errors the on-device core can produce.

/// Anything that can go wrong while talking to the Leos API.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The API rejected the request.
    #[error("{message}")]
    Api {
        /// The API's machine readable error code.
        code: String,
        /// The API's human readable message.
        message: String,
    },
    /// The API could not be reached.
    #[error("could not reach Leos: {0}")]
    Network(String),
    /// The API sent something the client could not read.
    #[error("unexpected response from Leos: {0}")]
    Decode(String),
    /// The operation needs a signed in user.
    #[error("you are not signed in")]
    NotSignedIn,
    /// The request was not valid before it was even sent.
    #[error("{0}")]
    InvalidRequest(String),
}

impl ClientError {
    /// The machine readable code, so the UI can react to it.
    pub fn code(&self) -> &str {
        match self {
            ClientError::Api { code, .. } => code,
            ClientError::Network(_) => "network_error",
            ClientError::Decode(_) => "decode_error",
            ClientError::NotSignedIn => "not_signed_in",
            ClientError::InvalidRequest(_) => "invalid_request",
        }
    }

    /// Whether retrying the same request could work.
    pub fn is_retryable(&self) -> bool {
        match self {
            ClientError::Network(_) => true,
            ClientError::Api { .. } => false,
            _ => false,
        }
    }
}

/// A result from the on-device core.
pub type Result<T> = std::result::Result<T, ClientError>;
