use std::{
    fmt::{Display, Formatter},
    io,
    io::Error,
};

/// Wrapper for all errors that can occur in `crossterm`.
#[derive(Debug)]
pub enum ErrorKind {
    IoError(io::Error),
}

impl Display for ErrorKind {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::IoError(e) => write!(fmt, "Serialization error occurred: {:?}", e),
        }
    }
}

impl From<io::Error> for ErrorKind {
    fn from(error: Error) -> Self {
        ErrorKind::IoError(error)
    }
}
