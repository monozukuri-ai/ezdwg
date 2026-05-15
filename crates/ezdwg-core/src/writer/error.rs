use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

pub type WriterResult<T> = Result<T>;

pub fn unsupported(message: impl Into<String>) -> DwgError {
    DwgError::new(ErrorKind::Unsupported, message)
}

pub fn not_implemented(message: impl Into<String>) -> DwgError {
    DwgError::new(ErrorKind::NotImplemented, message)
}
