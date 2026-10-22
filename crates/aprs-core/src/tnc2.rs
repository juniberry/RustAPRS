//! Parsing of APRS-IS "TNC2 monitor format" lines.
//!
//! A TNC2 line has the shape:
//!
//! ```text
//! SRC>DEST,PATH1,PATH2:INFO
//! ```
//!
//! where `SRC` is the source station, `DEST` the destination (often a
//! software identifier such as `APRS` or a Mic-E encoded latitude), and the
//! comma-separated `PATH` fields are digipeaters. Everything after the first
//! `:` is the APRS information field.

use crate::address::Address;
use crate::error::Error;

/// A parsed TNC2 monitor-format frame.
#[derive(Debug, Clone, Copy)]
pub struct Tnc2Frame<'a> {
    pub source: Address,
    pub destination: Address,
    /// Raw destination bytes as received (for Mic-E latitude decoding).
    pub destination_raw: &'a [u8],
    pub info: &'a [u8],
    /// Raw comma-separated digipeater list (may be empty).
    path: &'a [u8],
}

impl<'a> Tnc2Frame<'a> {
    /// Iterate over the digipeater addresses in path order.
    pub fn digipeaters(&self) -> DigipeaterIter<'a> {
        DigipeaterIter {
            remaining: self.path,
        }
    }

    /// Convenience accessor mirroring [`crate::packet::Packet`].
    pub fn to_packet(&self) -> crate::packet::Packet<'a> {
        crate::packet::Packet::new(
            self.source,
            self.destination,
            self.destination_raw,
            self.info,
        )
    }
}

/// Parse a TNC2 monitor-format line into a [`Tnc2Frame`].
pub fn parse(input: &[u8]) -> Result<Tnc2Frame<'_>, Error> {
    let input = trim_end(input);
    if input.is_empty() {
        return Err(Error::Empty);
    }

    let gt = input
        .iter()
        .position(|&b| b == b'>')
        .ok_or(Error::Truncated)?;
    let source = Address::parse(&input[..gt]).ok_or(Error::InvalidCharacter)?;

    let rest = &input[gt + 1..];
    let (header, info) = match rest.iter().position(|&b| b == b':') {
        Some(colon) => (&rest[..colon], &rest[colon + 1..]),
        None => (rest, &[] as &[u8]),
    };

    let (dest_bytes, path) = match header.iter().position(|&b| b == b',') {
        Some(comma) => (&header[..comma], &header[comma + 1..]),
        None => (header, &[] as &[u8]),
    };

    let destination = Address::parse(dest_bytes).ok_or(Error::InvalidCharacter)?;

    Ok(Tnc2Frame {
        source,
        destination,
        destination_raw: dest_bytes,
        info,
        path,
    })
}

fn trim_end(mut input: &[u8]) -> &[u8] {
    while let Some((&last, rest)) = input.split_last() {
        if last == b'\r' || last == b'\n' || last == b' ' {
            input = rest;
        } else {
            break;
        }
    }
    input
}

/// Iterator over the digipeater fields of a TNC2 header.
#[derive(Debug, Clone)]
pub struct DigipeaterIter<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for DigipeaterIter<'a> {
    type Item = Result<Address, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let (field, rest) = match self.remaining.iter().position(|&b| b == b',') {
            Some(comma) => (&self.remaining[..comma], &self.remaining[comma + 1..]),
            None => (self.remaining, &[] as &[u8]),
        };
        self.remaining = rest;

        // Strip a trailing '*' that marks a digipeater as "used" (heard).
        let field = match field.strip_suffix(b"*") {
            Some(f) => f,
            None => field,
        };

        Some(Address::parse(field).ok_or(Error::InvalidCharacter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_frame() {
        let f = parse(b"N0CALL-9>APRS,WIDE1-1,WIDE2-1:!4903.50N/07201.75W-").unwrap();
        assert_eq!(f.source, Address::parse(b"N0CALL-9").unwrap());
        assert_eq!(f.destination, Address::parse(b"APRS").unwrap());
        assert_eq!(f.info, b"!4903.50N/07201.75W-");

        let mut digs = f.digipeaters();
        assert_eq!(
            digs.next().unwrap().unwrap(),
            Address::parse(b"WIDE1-1").unwrap()
        );
        assert_eq!(
            digs.next().unwrap().unwrap(),
            Address::parse(b"WIDE2-1").unwrap()
        );
        assert!(digs.next().is_none());
    }

    #[test]
    fn strips_used_marker() {
        let f = parse(b"A>B,WIDE1-1*,WIDE2-2*:>hi").unwrap();
        let mut digs = f.digipeaters();
        assert_eq!(
            digs.next().unwrap().unwrap(),
            Address::parse(b"WIDE1-1").unwrap()
        );
        assert_eq!(
            digs.next().unwrap().unwrap(),
            Address::parse(b"WIDE2-2").unwrap()
        );
        assert!(digs.next().is_none());
    }

    #[test]
    fn no_digipeaters() {
        let f = parse(b"A>B:hi").unwrap();
        assert_eq!(f.digipeaters().count(), 0);
    }

    #[test]
    fn missing_info_is_ok() {
        let f = parse(b"A>B").unwrap();
        assert!(f.info.is_empty());
    }

    #[test]
    fn handles_crlf() {
        let f = parse(b"A>B:hi\r\n").unwrap();
        assert_eq!(f.info, b"hi");
    }
}
