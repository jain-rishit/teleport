use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),

    #[error("MTP error: {0}")]
    Mtp(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
