//! Object (`;`) and item (`)`) decoding (§11).
//!
//! An **Object Report** has a fixed 9-character name, a `*` (live) or `_`
//! (killed) marker, and a mandatory 7-byte timestamp. An **Item Report** has a
//! variable-length (3–9 character) name terminated by `!` (live) or `_`
//! (killed), and carries no timestamp. Both may carry an uncompressed or
//! compressed position.

use aprs_core::Error;

use crate::decoded::Object;
use crate::position::{decode_compressed, decode_uncompressed, parse_timestamp};

/// Decode an object (`;`) or item (`)`) from the full information field.
pub fn decode(info: &[u8]) -> Result<Object<'_>, Error> {
    let dt = *info.first().ok_or(Error::Empty)?;
    let is_item = match dt {
        b';' => false,
        b')' => true,
        _ => return Err(Error::UnknownDataType(dt)),
    };

    let (name, live, rest) = if is_item {
        parse_item(info)?
    } else {
        parse_object(info)?
    };

    // Object always carries a timestamp; item never does.
    let (timestamp, rest) = if is_item {
        (None, rest)
    } else {
        let (ts, r) = parse_timestamp(rest)?;
        (Some(ts), r)
    };

    // A leading digit indicates uncompressed `DDMM.MM` coordinates; any other
    // byte is a symbol-table identifier and therefore a compressed position.
    let position = match rest.first() {
        Some(b) if b.is_ascii_digit() => decode_uncompressed(rest, timestamp, false).ok(),
        Some(_) => decode_compressed(rest, timestamp, false).ok(),
        None => None,
    };

    match position {
        Some(position) => Ok(Object {
            name,
            live,
            is_item,
            position: Some(position),
            comment: position.comment,
        }),
        None => Ok(Object {
            name,
            live,
            is_item,
            position: None,
            comment: rest,
        }),
    }
}

/// Parse an object: `;` + 9-char name + `*`/`_` marker.
fn parse_object(info: &[u8]) -> Result<([u8; 9], bool, &[u8]), Error> {
    if info.len() < 11 {
        return Err(Error::Truncated);
    }
    let mut name = [b' '; 9];
    name.copy_from_slice(&info[1..10]);
    let live = match info[10] {
        b'*' => true,
        b'_' => false,
        _ => return Err(Error::InvalidCharacter),
    };
    Ok((name, live, &info[11..]))
}

/// Parse an item: `)` + variable (3–9 char) name terminated by `!`/`_`.
fn parse_item(info: &[u8]) -> Result<([u8; 9], bool, &[u8]), Error> {
    let term = info[1..]
        .iter()
        .position(|&b| b == b'!' || b == b'_')
        .ok_or(Error::InvalidCharacter)?;
    // `term` is the number of name characters between the `)` and the marker.
    if !(3..=9).contains(&term) {
        return Err(Error::InvalidCharacter);
    }
    let mut name = [b' '; 9];
    name[..term].copy_from_slice(&info[1..1 + term]);
    let live = info[1 + term] == b'!';
    Ok((name, live, &info[term + 2..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_object_with_position() {
        let o = decode(b";OBJECT   *111111z4903.50N/07201.75WrTest").unwrap();
        assert_eq!(&o.name, b"OBJECT   ");
        assert!(o.live);
        assert!(!o.is_item);
        let p = o.position.unwrap();
        assert!((p.coordinates.latitude - 49.0583333).abs() < 1e-6);
        assert_eq!(p.symbol_code, b'r');
        assert_eq!(p.comment, b"Test");
    }

    #[test]
    fn killed_object() {
        let o = decode(b";OBJECT   _111111z_Gone").unwrap();
        assert!(!o.live);
        assert!(o.position.is_none());
        assert_eq!(o.comment, b"_Gone");
    }

    #[test]
    fn item_is_flagged_and_variable_name() {
        let o = decode(b")VE3IXY!4324.26N/08031.87WrTest").unwrap();
        assert!(o.is_item);
        assert_eq!(&o.name, b"VE3IXY   ");
        assert!(o.live);
        let p = o.position.unwrap();
        assert!((p.coordinates.latitude - 43.4043333).abs() < 1e-6);
        assert_eq!(p.symbol_code, b'r');
        assert_eq!(p.comment, b"Test");
    }

    #[test]
    fn item_with_compressed_position() {
        let o = decode(b")Food!\\9\\=R;#SMR sTFood/Gas/Hotel").unwrap();
        assert!(o.is_item);
        assert_eq!(&o.name, b"Food     ");
        assert!(o.live);
        let p = o.position.unwrap();
        assert_eq!(p.symbol_table, crate::decoded::SymbolTable(b'\\'));
        assert_eq!(p.symbol_code, b'R');
    }

    #[test]
    fn killed_item() {
        let o = decode(b")AID #2_4903.50N/07201.75WA").unwrap();
        assert!(o.is_item);
        assert!(!o.live);
        assert_eq!(&o.name, b"AID #2   ");
    }
}
