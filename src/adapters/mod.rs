pub mod kollio;
pub mod uni;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("UNI decision is not Accepted: {0}")]
    UniNotAccepted(String),
    #[error("adapter input is missing {0}")]
    MissingField(&'static str),
    #[error("adapter input is invalid: {0}")]
    InvalidInput(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl AdapterError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UniNotAccepted(_) => "UNI_NOT_ACCEPTED",
            Self::MissingField(_) => "MISSING_FIELD",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
        }
    }
}

pub(crate) fn digest_json(value: &serde_json::Value) -> Result<String, AdapterError> {
    use sha2::{Digest, Sha256};

    let canonical = serde_json::to_vec(value)?;
    Ok(format!("sha256:{:x}", Sha256::digest(canonical)))
}
