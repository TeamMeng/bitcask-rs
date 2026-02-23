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

    #[error("failed to delete the database directory")]
    FailedToDeleteDatabaseDir,

    #[error("failed to delete file")]
    FailedToDeleteFile,

    #[error("failed to rename")]
    FailedToRename,

    #[error("failed to get merge file")]
    FailedToGetMergeFile,

    #[error("the database directory maybe corrupted")]
    DataDirectoryCorrupted,

    #[error("read data file eof")]
    ReadDataFileEOF,

    #[error("index update failed")]
    IndexUpdateFailed,

    #[error("invalid path")]
    InvalidPath,

    #[error("decode error")]
    DecodeError,

    #[error("decode error")]
    EncodeError,

    #[error("get value failed")]
    GetValueFailed,

    #[error("invalid crc value, log record maybe corrupted")]
    InvalidLogRecordCrc,

    #[error("iterator next error")]
    IteratorNextError,

    #[error("exceed the max batch num")]
    ExceedMaxBatchNum,

    #[error("merge is in progress, try again later")]
    MergeInProgress,

    #[error("parse error")]
    ParseError,

    #[error("can not use write batch, seq file not exists")]
    UnableToUserWriteBatch,

    #[error("the database directory is used by another process")]
    DatabaseIsUsing,

    #[error("failed to unlock file")]
    FailedToUnlockFile,

    #[error("invalid merge ratio, must between 0 and 1")]
    InvalidMergeRatio,

    #[error("do not reach the merge ratio")]
    MergeRatioUnreached,

    #[error("disk space is not enough of merge")]
    MergeNoEnoughSpace,
}
