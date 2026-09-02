use std::error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Broad category for a conversion failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Io,
    InvalidArguments,
    InvalidSfc,
    InvalidTemplate,
    InvalidScript,
    InvalidOutput,
    Validation,
}

/// A path-aware converter error.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    path: Option<PathBuf>,
    message: String,
    source: Option<io::Error>,
}

impl Error {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            path: None,
            message: message.into(),
            source: None,
        }
    }

    #[must_use]
    pub fn at(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub(crate) fn io(error: io::Error, path: impl AsRef<Path>) -> Self {
        Self {
            kind: ErrorKind::Io,
            path: Some(path.as_ref().to_path_buf()),
            message: error.to_string(),
            source: Some(error),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}: {}", path.display(), self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn error::Error + 'static))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
