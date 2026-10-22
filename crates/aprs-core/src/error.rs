//! Error type shared across the APRS crates.

use core::fmt;

/// Errors produced while parsing or decoding APRS data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The input was empty and no data type could be determined.
    Empty,
    /// The leading byte is not a recognized APRS data type identifier.
    UnknownDataType(u8),
    /// Input ended before a field could be fully parsed.
    Truncated,
    /// A field contained a character outside its expected range.
    InvalidCharacter,
    /// A coordinate value was out of its valid range.
    InvalidCoordinate,
    /// A numeric field could not be parsed.
    InvalidNumber,
    /// A base-91 encoded value contained an invalid character.
    InvalidBase91,
    /// The frame was recognized but this decoder does not support it (yet).
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Empty => write!(f, "empty input"),
            Error::UnknownDataType(b) => write!(f, "unknown data type identifier 0x{b:02X}"),
            Error::Truncated => write!(f, "input truncated"),
            Error::InvalidCharacter => write!(f, "invalid character"),
            Error::InvalidCoordinate => write!(f, "coordinate out of range"),
            Error::InvalidNumber => write!(f, "invalid numeric field"),
            Error::InvalidBase91 => write!(f, "invalid base-91 character"),
            Error::Unsupported => write!(f, "recognized but unsupported"),
        }
    }
}
