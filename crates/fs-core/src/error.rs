use std::fmt;

/// Every error an emulated filesystem can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotFound,
    AlreadyExists,
    InvalidPath,
    InvalidName,
    DiskFull,
    DirectoryFull,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    FileTooLarge,
    InvalidGeometry(String),
    CorruptImage(String),
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => write!(f, "not found"),
            Error::AlreadyExists => write!(f, "already exists"),
            Error::InvalidPath => write!(f, "invalid path"),
            Error::InvalidName => write!(f, "invalid name"),
            Error::DiskFull => write!(f, "disk full"),
            Error::DirectoryFull => write!(f, "directory full"),
            Error::NotADirectory => write!(f, "not a directory"),
            Error::IsADirectory => write!(f, "is a directory"),
            Error::DirectoryNotEmpty => write!(f, "directory not empty"),
            Error::FileTooLarge => write!(f, "file too large"),
            Error::InvalidGeometry(msg) => write!(f, "invalid geometry: {msg}"),
            Error::CorruptImage(msg) => write!(f, "corrupt image: {msg}"),
            Error::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_human_readable() {
        assert_eq!(Error::NotFound.to_string(), "not found");
        assert_eq!(
            Error::InvalidGeometry("too small".into()).to_string(),
            "invalid geometry: too small"
        );
    }

    #[test]
    fn implements_std_error() {
        fn takes(_: &dyn std::error::Error) {}
        takes(&Error::DiskFull);
    }
}
