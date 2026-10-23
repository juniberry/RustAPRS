//! Position report encoding (uncompressed and base-91 compressed).

use aprs_core::base91::base91_encode_fixed;
use aprs_core::Error;

use crate::format;

/// Encode an uncompressed position report (`!` without messaging, `=` with).
///
/// Writes into `out` and returns the number of bytes written. `lat`/`lon` are
/// in decimal degrees; `symbol_table` and `symbol_code` are the display symbol.
pub fn encode_uncompressed(
    messaging: bool,
    lat: f64,
    lon: f64,
    symbol_table: u8,
    symbol_code: u8,
    comment: &[u8],
    out: &mut [u8],
) -> Result<usize, Error> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }

    // 1 (DTI) + 7 (lat) + 1 (N/S) + 1 (table) + 8 (lon) + 1 (E/W) + 1 (symbol)
    const HEADER: usize = 20;
    if out.len() < HEADER + comment.len() {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    out[pos] = if messaging { b'=' } else { b'!' };
    pos += 1;

    pos += encode_uncompressed_body(lat, lon, symbol_table, symbol_code, &mut out[pos..])?;

    out[pos..pos + comment.len()].copy_from_slice(comment);
    pos += comment.len();

    Ok(pos)
}

/// Encode the uncompressed position body (19 bytes): `DDMM.MMh/DDDMM.MMl`
/// followed by the symbol code. Shared by position and object/item encoders.
pub fn encode_uncompressed_body(
    lat: f64,
    lon: f64,
    symbol_table: u8,
    symbol_code: u8,
    out: &mut [u8],
) -> Result<usize, Error> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }
    if out.len() < 19 {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    pos += format::ddmm(lat, 2, &mut out[pos..])?;
    out[pos] = if lat >= 0.0 { b'N' } else { b'S' };
    pos += 1;

    out[pos] = symbol_table;
    pos += 1;

    pos += format::ddmm(lon, 3, &mut out[pos..])?;
    out[pos] = if lon >= 0.0 { b'E' } else { b'W' };
    pos += 1;

    out[pos] = symbol_code;
    pos += 1;

    Ok(pos)
}

/// Encode a compressed position report (`!` without messaging, `=` with).
///
/// The compressed format is a fixed 13-byte field (symbol table, 4 base-91
/// latitude digits, 4 base-91 longitude digits, symbol code, and `csT`). When
/// `course_speed` is `Some((course, speed))`, `cs` carries the compressed
/// course/speed; otherwise `cs` is blank (space) and no course/speed is sent.
#[allow(clippy::too_many_arguments)]
pub fn encode_compressed(
    messaging: bool,
    lat: f64,
    lon: f64,
    symbol_table: u8,
    symbol_code: u8,
    course_speed: Option<(u16, u16)>,
    comment: &[u8],
    out: &mut [u8],
) -> Result<usize, Error> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }

    // 1 (DTI) + 13 (compressed field) + comment.
    if out.len() < 14 + comment.len() {
        return Err(Error::BufferTooSmall);
    }

    let lat_val = (380926.0 * (90.0 - lat) + 0.5) as u32;
    let lon_val = (190463.0 * (180.0 + lon) + 0.5) as u32;

    let mut pos = 0;
    out[pos] = if messaging { b'=' } else { b'!' };
    pos += 1;
    out[pos] = symbol_table;
    pos += 1;
    pos += base91_encode_fixed(lat_val, 4, &mut out[pos..]).unwrap();
    pos += base91_encode_fixed(lon_val, 4, &mut out[pos..]).unwrap();
    out[pos] = symbol_code;
    pos += 1;

    match course_speed {
        Some((course, speed)) => {
            let c = (course / 4).min(89) as u8;
            let s = speed_to_s(speed);
            out[pos] = c + 33;
            out[pos + 1] = s + 33;
            out[pos + 2] = b'['; // RMC, current fix, software origin
        }
        None => {
            // `c` = space marks "no course/speed"; `s`/`T` are non-space fillers
            // (following the spec's `> sT` example) so trailing bytes are kept.
            out[pos] = b' ';
            out[pos + 1] = b's';
            out[pos + 2] = b'T';
        }
    }
    pos += 3;

    out[pos..pos + comment.len()].copy_from_slice(comment);
    pos += comment.len();

    Ok(pos)
}

/// Find the base-91 `s` digit whose compressed speed `1.08^s - 1` best matches
/// `speed` (in knots).
fn speed_to_s(speed: u16) -> u8 {
    let target = speed as f64;
    let mut v = 1.0f64; // 1.08^s
    let mut best_s = 0u8;
    let mut best_diff = f64::MAX;
    for s in 0..=89u8 {
        let diff = (v - 1.0 - target).abs();
        if diff < best_diff {
            best_diff = diff;
            best_s = s;
        }
        v *= 1.08;
    }
    best_s
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded, PositionEncoding};

    /// Wrap `info` in a minimal TNC2 line and pass the decoded result to `f`.
    fn with_decoded(info: &[u8], f: impl FnOnce(aprs_decoders::Decoded<'_>)) {
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
    fn roundtrip_uncompressed() {
        let mut out = [0u8; 64];
        let n = encode_uncompressed(
            false,
            49.0583333,
            -72.0291666,
            b'/',
            b'-',
            b"Test",
            &mut out,
        )
        .unwrap();
        let info = &out[..n];
        assert_eq!(info, b"!4903.50N/07201.75W-Test");

        with_decoded(info, |decoded| match decoded {
            Decoded::Position(p) => {
                assert_eq!(p.encoding, PositionEncoding::Uncompressed);
                assert!((p.coordinates.latitude - 49.0583333).abs() < 1e-5);
                assert!((p.coordinates.longitude + 72.0291666).abs() < 1e-5);
                assert_eq!(p.symbol_code, b'-');
                assert_eq!(p.comment, b"Test");
            }
            other => panic!("expected position, got {other:?}"),
        });
    }

    #[test]
    fn southern_western_hemisphere() {
        let mut out = [0u8; 64];
        let n = encode_uncompressed(false, -33.4583333, -70.7166667, b'/', b'-', b"", &mut out)
            .unwrap();
        assert_eq!(&out[..n], b"!3327.50S/07043.00W-");
    }

    #[test]
    fn messaging_flag() {
        let mut out = [0u8; 64];
        let n = encode_uncompressed(true, 0.0, 0.0, b'/', b'-', b"", &mut out).unwrap();
        assert_eq!(&out[..n], b"=0000.00N/00000.00E-");
    }

    #[test]
    fn buffer_too_small() {
        let mut out = [0u8; 8];
        assert_eq!(
            encode_uncompressed(false, 49.058, -72.029, b'/', b'-', b"", &mut out),
            Err(Error::BufferTooSmall)
        );
    }

    #[test]
    fn rejects_out_of_range() {
        let mut out = [0u8; 64];
        assert!(encode_uncompressed(false, 91.0, 0.0, b'/', b'-', b"", &mut out).is_err());
        assert!(encode_uncompressed(false, 0.0, 181.0, b'/', b'-', b"", &mut out).is_err());
    }

    #[test]
    fn roundtrip_compressed() {
        // The spec's canonical example: 49°30'N, 72°45'W, 88°, ~36 kn.
        let mut out = [0u8; 64];
        let n = encode_compressed(
            false,
            49.5,
            -72.75,
            b'/',
            b'>',
            Some((88, 36)),
            b"",
            &mut out,
        )
        .unwrap();
        // Prefix matches the spec's `!/5L!!` (DTI + table + latitude).
        assert_eq!(&out[..6], b"!/5L!!");

        with_decoded(&out[..n], |d| match d {
            Decoded::Position(p) => {
                assert_eq!(p.encoding, PositionEncoding::Compressed);
                assert!((p.coordinates.latitude - 49.5).abs() < 1e-3);
                assert!((p.coordinates.longitude + 72.75).abs() < 1e-3);
                assert_eq!(p.course, Some(88));
                assert!(p.speed_knots.is_some());
            }
            other => panic!("expected compressed position, got {other:?}"),
        });
    }

    #[test]
    fn compressed_no_course_speed() {
        let mut out = [0u8; 64];
        let n = encode_compressed(false, 49.5, -72.75, b'/', b'>', None, b"", &mut out).unwrap();
        with_decoded(&out[..n], |d| match d {
            Decoded::Position(p) => {
                assert_eq!(p.encoding, PositionEncoding::Compressed);
                assert_eq!(p.course, None);
                assert_eq!(p.speed_knots, None);
            }
            other => panic!("expected compressed position, got {other:?}"),
        });
    }
}
