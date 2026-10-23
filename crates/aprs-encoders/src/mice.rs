//! Mic-E position encoding (§10).
//!
//! Mic-E packs the latitude into the 6-byte destination address and the
//! longitude, speed, course, and symbol into the information field. Encoding is
//! split across two output buffers: `dest` (6 bytes) and `info`.

use aprs_core::Error;

/// Encode a Mic-E position into the destination address (6 bytes) and the
/// information field, returning `(dest_len, info_len)`.
///
/// Uses the "current GNSS data" data type (` `` `) and the M1 "En Route"
/// position comment. `lat`/`lon` are in decimal degrees; `speed_knots` and
/// `course` (degrees) are optional motion data; `status` is optional status
/// text (may be empty).
#[allow(clippy::too_many_arguments)]
pub fn encode(
    lat: f64,
    lon: f64,
    speed_knots: u16,
    course: u16,
    symbol_table: u8,
    symbol_code: u8,
    status: &[u8],
    dest: &mut [u8],
    info: &mut [u8],
) -> Result<(usize, usize), Error> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }
    if dest.len() < 6 || info.len() < 9 + status.len() {
        return Err(Error::BufferTooSmall);
    }

    // Latitude digits.
    let lat_total = (lat.abs() * 6000.0 + 0.5) as u32;
    let lat_deg = lat_total / 6000;
    let lat_min = (lat_total % 6000) / 100;
    let lat_hund = lat_total % 100;
    let d0 = lat_deg / 10;
    let d1 = lat_deg % 10;
    let d2 = lat_min / 10;
    let d3 = lat_min % 10;
    let d4 = lat_hund / 10;
    let d5 = lat_hund % 10;

    // Longitude degrees/minutes/hundredths and the +100 offset flag.
    let lon_total = (lon.abs() * 6000.0 + 0.5) as u32;
    let lon_deg = lon_total / 6000;
    let lon_min = (lon_total % 6000) / 100;
    let lon_hund = lon_total % 100;
    let (lon_deg_byte, offset_100) = encode_lon_deg(lon_deg);

    let north = lat >= 0.0;
    let west = lon < 0.0;

    // Destination address (M1 "En Route": comment bits A=1, B=1, C=0).
    dest[0] = b'P' + d0 as u8;
    dest[1] = b'P' + d1 as u8;
    dest[2] = b'0' + d2 as u8;
    dest[3] = if north { b'P' } else { b'0' } + d3 as u8;
    dest[4] = if offset_100 { b'P' } else { b'0' } + d4 as u8;
    dest[5] = if west { b'P' } else { b'0' } + d5 as u8;

    // Information field.
    let (sp, dc, se) = encode_speed_course(speed_knots, course);
    let mut pos = 0;
    info[pos] = b'`'; // current GNSS data
    pos += 1;
    info[pos] = lon_deg_byte;
    pos += 1;
    info[pos] = encode_lon_min(lon_min);
    pos += 1;
    info[pos] = (lon_hund + 28) as u8;
    pos += 1;
    info[pos] = sp;
    info[pos + 1] = dc;
    info[pos + 2] = se;
    pos += 3;
    info[pos] = symbol_code;
    info[pos + 1] = symbol_table;
    pos += 2;
    info[pos..pos + status.len()].copy_from_slice(status);
    pos += status.len();

    Ok((6, pos))
}

/// Encode longitude degrees (0–179) into the `d+28` byte, returning the byte and
/// whether the +100 degree longitude offset applies.
fn encode_lon_deg(lon_deg: u32) -> (u8, bool) {
    match lon_deg {
        0..=9 => ((lon_deg + 118) as u8, true),
        10..=99 => ((lon_deg + 28) as u8, false),
        100..=109 => ((lon_deg + 8) as u8, true),
        _ => ((lon_deg - 72) as u8, true),
    }
}

/// Encode longitude minutes (0–59) into the `m+28` byte.
fn encode_lon_min(lon_min: u32) -> u8 {
    if lon_min < 10 {
        (lon_min + 88) as u8
    } else {
        (lon_min + 28) as u8
    }
}

/// Encode speed (knots) and course (degrees) into the `SP+28`, `DC+28`, and
/// `SE+28` bytes.
fn encode_speed_course(speed_knots: u16, course: u16) -> (u8, u8, u8) {
    let speed = speed_knots.min(799);
    let speed_tens = speed / 10;
    let speed_units = speed % 10;
    let course = course.min(359);
    let course_hundreds = course / 100;
    let course_tens_units = course % 100;

    let sp = (speed_tens + 28) as u8;
    let dc = (speed_units * 10 + course_hundreds + 28) as u8;
    let se = (course_tens_units + 28) as u8;
    (sp, dc, se)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded, PositionEncoding};

    #[test]
    fn destination_flags_match_spec() {
        // 33°25.64'N, offset +100, west -> bytes 3–5 carry N / +100 / W. (The
        // spec example uses comment M3 1/0/0, so bytes 0–2 differ from our M1.)
        let mut dest = [0u8; 6];
        let mut info = [0u8; 32];
        let (dl, _) = encode(
            33.4273, -112.129, 20, 251, b'/', b'j', b"", &mut dest, &mut info,
        )
        .unwrap();
        assert_eq!(dl, 6);
        assert_eq!(&dest[3..6], b"UVT");
    }

    #[test]
    fn roundtrip_mic_e() {
        let mut dest = [0u8; 6];
        let mut info = [0u8; 64];
        let (dl, il) = encode(
            33.4273, -112.129, 20, 251, b'/', b'j', b"", &mut dest, &mut info,
        )
        .unwrap();

        // Reconstruct a full TNC2 packet and decode it.
        let mut line = [0u8; 256];
        // Source "A>DEST:" then info.
        line[0] = b'A';
        line[1] = b'>';
        let d = dl;
        line[2..2 + d].copy_from_slice(&dest[..d]);
        line[2 + d] = b':';
        let base = 3 + d;
        line[base..base + il].copy_from_slice(&info[..il]);
        let total = base + il;
        let frame = tnc2::parse(&line[..total]).unwrap();

        match decode_packet(&frame.to_packet()).unwrap() {
            Decoded::Position(p) => {
                assert_eq!(p.encoding, PositionEncoding::MicE);
                assert!((p.coordinates.latitude - 33.4273).abs() < 1e-3);
                assert!((p.coordinates.longitude + 112.129).abs() < 1e-3);
                assert_eq!(p.course, Some(251));
                assert_eq!(p.speed_knots, Some(20));
            }
            other => panic!("expected Mic-E position, got {other:?}"),
        }
    }
}
