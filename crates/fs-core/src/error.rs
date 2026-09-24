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
    /// A raw byte range that does not lie inside the disk.
    OutOfBounds {
        offset: u64,
        len: u64,
        disk_len: u64,
    },
    /// An ext3 volume whose journal holds transactions not yet replayed:
    /// every mutation is refused until `recover` runs.
    NeedsRecovery,
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
            Error::OutOfBounds {
                offset,
                len,
                disk_len,
            } => write!(
                f,
                "out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk"
            ),
            Error::NeedsRecovery => write!(f, "needs recovery"),
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
        assert_eq!(
            Error::OutOfBounds {
                offset: 3,
                len: 2,
                disk_len: 4
            }
            .to_string(),
            "out of bounds: 2 bytes at offset 3 run past the end of the 4-byte disk"
        );
        assert_eq!(Error::NeedsRecovery.to_string(), "needs recovery");
    }

    #[test]
    fn implements_std_error() {
        fn takes(_: &dyn std::error::Error) {}
        takes(&Error::DiskFull);
    }
}
