#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
/// The errors that may occur.
pub enum EasypackError {
    /// A generic IO Error
    IoError(std::io::Error),
    /// When the input file is wrong and unreadable.
    InvalidFileError(String),
    /// If the record to be read is too big for the desired architecture.
    RecordTooBig(String),
    /// If the record name is too long.
    RecordNameTooBig(String),
    /// If the same record name is used twice.
    RecordSameName(String),
    /// Internal error.
    InternalError(String),
}

impl std::fmt::Display for EasypackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{self:?}"))
    }
}

impl From<std::io::Error> for EasypackError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
impl std::convert::From<std::num::TryFromIntError> for EasypackError {
    fn from(e: std::num::TryFromIntError) -> Self {
        Self::InternalError(format!("{e}"))
    }
}

impl std::error::Error for EasypackError {}

pub type Result<T> = std::result::Result<T, EasypackError>;
