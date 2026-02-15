use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppError {
    #[error("failed to read from data file")]
    FailedToReadFromDataFile,

    #[error("failed to write data file")]
    FailedToWriteDataFile,

    #[error("failed to write data file")]
    FailedToSyncDataFile,

    #[error("failed to open data file")]
    FailedToOpenDataFile,

    #[error("the key is empty")]
    KeyIsEmpty,

    #[error("key is not found in database")]
    KeyNotFound,

    #[error("file is not found in data file")]
    FileNotFound,

    #[error("data file not found in database")]
    DataFileNotFound,

    #[error("database dir can not be empty")]
    DirPathIsEmpty,

    #[error("database data file size must be greater than 0")]
    DataFileSizeTooSmall,

    #[error("failed to create the database directory")]
    FailedToCreateDatabaseDir,

    #[error("failed to read the database directory")]
    FailedToReadDatabaseDir,

    #[error("the database directory maybe corrupted")]
    DataDirectoryCorrupted,

    #[error("read data file eof")]
    ReadDataFileEOF,

    #[error("index update failed")]
    IndexUpdateFailed,
}
