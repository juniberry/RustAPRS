//! Status report (`>`) and Maidenhead grid locator beacon (`[`) decoding.

use aprs_core::Error;

use crate::decoded::{Maidenhead, Status};
use crate::maidenhead;

/// Decode a `>` status report, extracting a leading Maidenhead locator when the
/// report begins with one (§16).
pub fn decode(info: &[u8]) -> Result<Status<'_>, Error> {
    if info.first() != Some(&b'>') {
        return Err(Error::UnknownDataType(info[0]));
    }
    let body = &info[1..];
    let (locator, text) = match maidenhead::parse(body) {
        Some((m, rest)) => (Some(m), rest),
        None => (None, body),
    };
    Ok(Status { locator, text })
}

/// Decode a `[` Maidenhead grid locator beacon (obsolete, §5).
pub fn decode_grid(info: &[u8]) -> Result<Maidenhead, Error> {
    if info.first() != Some(&b'[') {
        return Err(Error::UnknownDataType(info[0]));
    }
    maidenhead::parse(&info[1..])
        .map(|(m, _)| m)
        .ok_or(Error::InvalidCharacter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_with_locator() {
        let s = decode(b">IO91SX/G My house").unwrap();
        let m = s.locator.unwrap();
        assert_eq!(&m.grid[..6], b"IO91SX");
        assert_eq!(s.text, b"/G My house");
    }

    #[test]
    fn status_without_locator() {
        let s = decode(b">Hello world").unwrap();
        assert!(s.locator.is_none());
        assert_eq!(s.text, b"Hello world");
    }

    #[test]
    fn grid_beacon() {
        let m = decode_grid(b"[IO91SX]").unwrap();
        assert_eq!(&m.grid[..6], b"IO91SX");
    }
}
