use alloc::string::String;
use core::fmt;

#[derive(Debug)]
pub enum Error {
    Io(alloc::io::Error),
    FastbootError(String),
    FastbootProtocolError(&'static str),
    AdbProtocolError(&'static str),
    AdbRejected(String),
    AdbAuthRequired,
    InvalidMagic { expected: u32, actual: u32 },
    ChecksumMismatch { expected: u32, actual: u32 },
    PayloadTooLarge { max: u32, actual: u32 },
    Utf8Error,
    Custom(&'static str),
    Message(String),
}

impl From<alloc::io::Error> for Error {
    fn from(err: alloc::io::Error) -> Self {
        Error::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::FastbootError(msg) => write!(f, "Fastboot error: {msg}"),
            Error::FastbootProtocolError(msg) => write!(f, "Fastboot protocol error: {msg}"),
            Error::AdbProtocolError(msg) => write!(f, "ADB protocol error: {msg}"),
            Error::AdbRejected(msg) => write!(f, "ADB request rejected: {msg}"),
            Error::AdbAuthRequired => {
                write!(f, "ADB authentication required (device is unauthorized)")
            }
            Error::InvalidMagic { expected, actual } => {
                write!(
                    f,
                    "ADB invalid magic: expected {expected:#010x}, got {actual:#010x}"
                )
            }
            Error::ChecksumMismatch { expected, actual } => {
                write!(
                    f,
                    "ADB checksum mismatch: expected {expected:#010x}, got {actual:#010x}"
                )
            }
            Error::PayloadTooLarge { max, actual } => {
                write!(f, "Payload too large: {actual} > max {max}")
            }
            Error::Utf8Error => write!(f, "UTF-8 decode error"),
            Error::Custom(msg) => write!(f, "{msg}"),
            Error::Message(msg) => write!(f, "{msg}"),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;
