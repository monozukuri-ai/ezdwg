use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    Format,
    Decode,
    Resolve,
    Unsupported,
    NotImplemented,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Io => "io",
            Self::Format => "format",
            Self::Decode => "decode",
            Self::Resolve => "resolve",
            Self::Unsupported => "unsupported",
            Self::NotImplemented => "not_implemented",
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone)]
pub struct DwgError {
    pub kind: ErrorKind,
    pub message: String,
    pub offset: Option<u64>,
}

impl DwgError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: None,
        }
    }

    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotImplemented, message)
    }
}

impl fmt::Display for DwgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(
                f,
                "{} error: {} (offset {})",
                self.kind, self.message, offset
            ),
            None => write!(f, "{} error: {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for DwgError {}

impl From<std::io::Error> for DwgError {
    fn from(err: std::io::Error) -> Self {
        DwgError::new(ErrorKind::Io, err.to_string())
    }
}
