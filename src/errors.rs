use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("failed to read from data file")]
    Read,

    #[error("failed to write data file")]
    Write,

    #[error("failed to write data file")]
    Sync,

    #[error("failed to open data file")]
    Open,

    #[error("the key is empty")]
    KeyIsEmpty,

    #[error("key is not found in database")]
    KeyNotFound,

    #[error("data file not found in database")]
    DataFileNotFound,
}
