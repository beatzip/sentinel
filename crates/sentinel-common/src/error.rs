use thiserror::Error;

#[derive(Error, Debug)]
pub enum SentinelError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid demo format: {0}")]
    InvalidDemoFormat(String),

    #[error("Missing data: {0}")]
    MissingData(String),

    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Feature computation failed: {0}")]
    FeatureComputation(String),

    #[error("Analysis error: {0}")]
    Analysis(String),
}

pub type Result<T> = std::result::Result<T, SentinelError>;
