//! Errors at the camera and V4L2 boundary. JSON/CLI still print `Display`.

use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Busy,
    Timeout { cid: Option<u32> },
    Io {
        message: String,
        source: Option<io::Error>,
    },
    Protocol(String),
    Invalid(String),
    Disconnected(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            source: None,
        }
    }

    pub fn io_ctx(source: io::Error, ctx: impl fmt::Display) -> Self {
        Self::Io {
            message: format!("{ctx}: {source}"),
            source: Some(source),
        }
    }

    /// Write would block or a write is already in flight. Do not drop the client.
    pub fn is_transient_write(&self) -> bool {
        match self {
            Self::Busy => true,
            Self::Io { source: Some(e), .. } => matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
            ),
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => write!(f, "busy"),
            Self::Timeout { cid: Some(cid) } => write!(f, "timeout waiting for cid {cid}"),
            Self::Timeout { cid: None } => write!(f, "timeout"),
            Self::Io { message, .. } => write!(f, "{message}"),
            Self::Protocol(s) | Self::Invalid(s) => write!(f, "{s}"),
            Self::Disconnected(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io {
                source: Some(e), ..
            } => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        let message = e.to_string();
        Self::Io {
            message,
            source: Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use std::error::Error as _;
    use std::io;

    #[test]
    fn busy_displays_as_busy() {
        assert_eq!(Error::Busy.to_string(), "busy");
    }

    #[test]
    fn timeout_display_unchanged() {
        assert_eq!(
            Error::Timeout { cid: Some(23) }.to_string(),
            "timeout waiting for cid 23"
        );
    }

    #[test]
    fn io_source_is_the_os_error() {
        let src = io::Error::from_raw_os_error(libc::ECONNREFUSED);
        let err = Error::io_ctx(src, "connect 192.168.129.10:12345");
        assert!(err.to_string().starts_with("connect 192.168.129.10:12345:"));
        assert!(err.source().is_some());
    }

    #[test]
    fn would_block_is_transient_write() {
        let e = Error::io_ctx(
            io::Error::from(io::ErrorKind::WouldBlock),
            "write",
        );
        assert!(e.is_transient_write());
        assert!(!Error::Disconnected("camera closed the socket").is_transient_write());
    }
}
