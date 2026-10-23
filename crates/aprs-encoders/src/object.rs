//! Object (`;`) and item (`)`) encoding (§11).

use aprs_core::Error;

use crate::position::encode_uncompressed_body;

/// Encode an object or item report into `out`, returning bytes written.
///
/// `name` is the object/item name (1–9 chars, space-padded for objects);
/// `live` selects `*`/`!` (live) versus `_` (killed); `timestamp` is the
/// required 7-byte timestamp for objects (ignored for items). The position is
/// uncompressed.
#[allow(clippy::too_many_arguments)]
pub fn encode(
    is_item: bool,
    name: &[u8],
    live: bool,
    timestamp: Option<&[u8]>,
    lat: f64,
    lon: f64,
    symbol_table: u8,
    symbol_code: u8,
    comment: &[u8],
    out: &mut [u8],
) -> Result<usize, Error> {
    let name_len = name.len();
    if is_item {
        if !(3..=9).contains(&name_len) {
            return Err(Error::InvalidCharacter);
        }
    } else if name_len > 9 {
        return Err(Error::InvalidCharacter);
    }

    // Object requires a timestamp; item must not have one.
    let ts_len = if is_item { 0 } else { 7 };
    if !is_item && timestamp.is_none() {
        return Err(Error::Truncated);
    }

    let total = 1 + 9 + 1 + ts_len + 19 + comment.len();
    if out.len() < total {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    out[pos] = if is_item { b')' } else { b';' };
    pos += 1;

    // Name (9 chars, space-padded on the right).
    for i in 0..9 {
        out[pos + i] = if i < name_len { name[i] } else { b' ' };
    }
    pos += 9;

    // Live/killed marker.
    out[pos] = if live {
        if is_item {
            b'!'
        } else {
            b'*'
        }
    } else {
        b'_'
    };
    pos += 1;

    // Object timestamp.
    if !is_item {
        out[pos..pos + 7].copy_from_slice(timestamp.unwrap());
        pos += 7;
    }

    pos += encode_uncompressed_body(lat, lon, symbol_table, symbol_code, &mut out[pos..])?;

    out[pos..pos + comment.len()].copy_from_slice(comment);
    pos += comment.len();

    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded};

    fn with_decoded(info: &[u8], f: impl FnOnce(Decoded<'_>)) {
        let mut line = [0u8; 256];
        line[0] = b'A';
        line[1] = b'>';
        line[2] = b'B';
        line[3] = b':';
        let n = info.len();
        line[4..4 + n].copy_from_slice(info);
        let frame = tnc2::parse(&line[..4 + n]).unwrap();
        f(decode_packet(&frame.to_packet()).unwrap());
    }

    #[test]
    fn roundtrip_object() {
        let mut out = [0u8; 128];
        let ts = b"111111z";
        let n = encode(
            false,
            b"OBJECT",
            true,
            Some(ts),
            49.0583,
            -72.0292,
            b'/',
            b'r',
            b"Test",
            &mut out,
        )
        .unwrap();
        assert_eq!(&out[..n], b";OBJECT   *111111z4903.50N/07201.75WrTest");

        with_decoded(&out[..n], |d| match d {
            Decoded::Object(o) => {
                assert!(!o.is_item);
                assert!(o.live);
                assert_eq!(&o.name, b"OBJECT   ");
                let p = o.position.unwrap();
                assert!((p.coordinates.latitude - 49.0583).abs() < 1e-4);
                assert_eq!(p.comment, b"Test");
            }
            other => panic!("expected object, got {other:?}"),
        });
    }

    #[test]
    fn roundtrip_item() {
        let mut out = [0u8; 128];
        let n = encode(
            true, b"ITEM", true, None, 49.0583, -72.0292, b'/', b'r', b"", &mut out,
        )
        .unwrap();
        assert_eq!(&out[..n], b")ITEM     !4903.50N/07201.75Wr");

        with_decoded(&out[..n], |d| match d {
            Decoded::Object(o) => {
                assert!(o.is_item);
                assert!(o.live);
                assert_eq!(&o.name, b"ITEM     ");
            }
            other => panic!("expected item, got {other:?}"),
        });
    }

    #[test]
    fn object_requires_timestamp() {
        let mut out = [0u8; 128];
        assert!(
            encode(false, b"OBJECT", true, None, 49.0, -72.0, b'/', b'r', b"", &mut out).is_err()
        );
    }
}
