use std::{
    fmt::{Display, Formatter},
    io,
    io::Error,
};

/// Wrapper for all errors that can occur in `legion-sync`.
#[derive(Debug)]
pub enum ErrorKind {
    IoError(io::Error),
    NetSyncError(net_sync::error::ErrorKind),
}

impl Display for ErrorKind {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::IoError(e) => write!(fmt, "IO error occurred: {:?}", e),
            ErrorKind::NetSyncError(e) => {
                write!(fmt, "Network synchronisation error occurred: {:?}", e)
            }
        }
    }
}

impl From<io::Error> for ErrorKind {
    fn from(error: Error) -> Self {
        ErrorKind::IoError(error)
    }
}
