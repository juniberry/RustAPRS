//! Mic-E position decoding.
//!
//! Mic-E packs the latitude into the six bytes of the AX.25 *destination*
//! address and the longitude, speed, course, and symbol into the information
//! field. This module follows Chapter 10 ("Mic-E Data Format") of the APRS
//! Protocol Reference.
//!
//! # Destination address (latitude)
//!
//! ```text
//! byte 0: degrees tens       (+ position comment bit A)
//! byte 1: degrees units      (+ position comment bit B)
//! byte 2: minutes tens       (+ position comment bit C)
//! byte 3: minutes units      (+ N/S indicator)
//! byte 4: hundredths tens    (+ longitude offset indicator)
//! byte 5: hundredths units   (+ W/E indicator)
//! ```
//!
//! Each byte encodes a digit 0–9 as an ASCII character:
//!
//! | char | digit | flag |
//! |------|-------|------|
//! | `0`-`9` | 0–9 | "south/east/+0" or comment bit 0 |
//! | `A`-`J` | 0–9 | comment bit 1 (Custom) |
//! | `P`-`Y` | 0–9 | "north/west/+100" or comment bit 1 (Std) |
//! | `K`,`L`,`Z` | (space) | position ambiguity |
//!
//! # Information field
//!
//! ```text
//! byte 0: data type id (` or ')
//! byte 1: longitude degrees + 28
//! byte 2: longitude minutes + 28
//! byte 3: longitude hundredths + 28
//! byte 4: SP+28  (speed, hundreds/tens of knots)
//! byte 5: DC+28  (speed units + course hundreds)
//! byte 6: SE+28  (course tens/units of degrees)
//! byte 7: symbol code
//! byte 8: symbol table id
//! byte 9+: status text
//! ```

use aprs_core::address::CALLSIGN_LEN;
use aprs_core::base91::base91_decode;
use aprs_core::Error;

use crate::decoded::{Coordinates, Maidenhead, Position, PositionEncoding, SymbolTable};
use crate::maidenhead;

/// Decode a Mic-E position from the raw destination bytes and the information
/// field (including the leading data-type byte).
pub fn decode<'a>(destination_raw: &[u8], info: &'a [u8]) -> Result<Position<'a>, Error> {
    let dt = *info.first().ok_or(Error::Empty)?;
    if !matches!(dt, b'`' | b'\'') {
        return Err(Error::UnknownDataType(dt));
    }
    if destination_raw.len() < CALLSIGN_LEN {
        return Err(Error::Truncated);
    }
    let dest = &destination_raw[..CALLSIGN_LEN];

    // --- Latitude from the destination address ---
    // 'K', 'L' and 'Z' encode a blanked (ambiguous) digit.
    let (d0, b0) = lat_digit(dest[0])?;
    let (d1, b1) = lat_digit(dest[1])?;
    let (d2, b2) = lat_digit(dest[2])?;
    let (d3, b3) = lat_digit(dest[3])?;
    let (d4, b4) = lat_digit(dest[4])?;
    let (d5, b5) = lat_digit(dest[5])?;

    let lat_deg = (d0 as u16 * 10 + d1 as u16) as f64;
    let lat_min = (d2 as u16 * 10 + d3 as u16) as f64;
    let lat_hund = (d4 as u16 * 10 + d5 as u16) as f64;
    let ambiguity = b0 as u8 + b1 as u8 + b2 as u8 + b3 as u8 + b4 as u8 + b5 as u8;

    // Flags are packed by character range: 'P'-'Z' means North/+100/West.
    let north = matches!(dest[3], b'P'..=b'Z');
    let long_offset_100 = matches!(dest[4], b'P'..=b'Z');
    let west = matches!(dest[5], b'P'..=b'Z');

    let mut latitude = lat_deg + (lat_min + lat_hund / 100.0) / 60.0;
    if !north {
        latitude = -latitude;
    }

    // --- Information field ---
    let body = &info[1..];
    if body.len() < 8 {
        return Err(Error::Truncated);
    }

    // Longitude degrees (with the +100 offset and 80/190 fold handling).
    let mut lon_deg = (body[0] as i32) - 28;
    if long_offset_100 {
        lon_deg += 100;
    }
    if (180..=189).contains(&lon_deg) {
        lon_deg -= 80;
    } else if (190..=199).contains(&lon_deg) {
        lon_deg -= 190;
    }

    // Longitude minutes (subtract 60 when the encoded value is >= 60).
    let mut lon_min = (body[1] as i32) - 28;
    if lon_min >= 60 {
        lon_min -= 60;
    }
    let lon_hund = (body[2] as i32) - 28;

    let mut longitude = lon_deg as f64 + (lon_min as f64 + lon_hund as f64 / 100.0) / 60.0;
    if west {
        longitude = -longitude;
    }

    // Speed and course (three bytes, with the dual-encoding adjustments).
    let sp = (body[3] as i32) - 28;
    let dc = (body[4] as i32) - 28;
    let se = (body[5] as i32) - 28;

    let mut speed_tens = sp;
    if speed_tens >= 80 {
        speed_tens -= 80;
    }
    let speed_units = dc / 10;
    let mut speed = speed_tens * 10 + speed_units;
    if speed >= 800 {
        speed -= 800;
    }

    let dc_remainder = dc % 10;
    let course_hundreds = if dc_remainder >= 4 {
        dc_remainder - 4
    } else {
        dc_remainder
    };
    let mut course = course_hundreds * 100 + se;
    if course >= 400 {
        course -= 400;
    }

    let symbol_code = body[6];
    let symbol_table = SymbolTable::from_byte(body[7]);

    // The remainder is status text: an optional device/messaging prefix, an
    // optional base-91 altitude, an optional Maidenhead locator, and free text,
    // with an optional trailing device identifier suffix.
    let status = parse_status_text(&body[8..]);

    Ok(Position {
        coordinates: Coordinates {
            latitude,
            longitude,
        },
        encoding: PositionEncoding::MicE,
        symbol_table,
        symbol_code,
        comment: status.comment,
        timestamp: None,
        ambiguity,
        messaging: status.messaging,
        device: status.device,
        course: Some(course as u16),
        speed_knots: Some(speed as u16),
        altitude_feet: status.altitude_feet,
        phg: None,
        radio_range_miles: None,
        compression_type: None,
        maidenhead: status.maidenhead,
    })
}

/// The parsed components of a Mic-E status text field.
struct StatusText<'a> {
    comment: &'a [u8],
    messaging: bool,
    altitude_feet: Option<i32>,
    maidenhead: Option<Maidenhead>,
    device: Option<&'a [u8]>,
}

/// Parse the Mic-E status text (everything after the symbol table identifier).
///
/// The leading device/messaging prefix, the optional `xxx}` base-91 altitude,
/// the optional Maidenhead locator, and the trailing device identifier suffix
/// are removed from the comment.
fn parse_status_text(text: &[u8]) -> StatusText<'_> {
    let mut rest = text;
    let mut messaging = false;

    // Device/messaging prefix: `` ` `` = messaging capable, `'` = not capable,
    // `>`/`]` = Kenwood (no messaging indication).
    if let Some(&prefix) = rest.first() {
        match prefix {
            b'`' => {
                messaging = true;
                rest = &rest[1..];
            }
            b'\'' => {
                rest = &rest[1..];
            }
            b'>' | b']' => {
                rest = &rest[1..];
            }
            _ => {}
        }
    }

    // Altitude: three base-91 characters followed by `}`, in meters relative to
    // 10 km below mean sea level.
    let mut altitude_feet = None;
    if rest.len() >= 4 && rest[3] == b'}' {
        if let Some(v) = base91_decode(&rest[..3]) {
            let meters = v as i32 - 10_000;
            altitude_feet = Some((meters as f64 * 3.280_84) as i32);
            rest = &rest[4..];
        }
    }

    // Maidenhead locator: a 4- or 6-character grid square, optionally followed
    // by a symbol (`/G`).
    let mut maidenhead = None;
    if let Some((m, r)) = maidenhead::parse(rest) {
        maidenhead = Some(m);
        rest = r;
    }

    // Device identifier suffix (e.g. `|3`, `_0`, `=`).
    let (comment, device) = strip_device_suffix(rest);

    StatusText {
        comment,
        messaging,
        altitude_feet,
        maidenhead,
        device,
    }
}

/// Known Mic-E device identifier suffixes, as listed in APRS 1.2c §10. The
/// full, maintained table lives in the `aprs-deviceid` project; this covers the
/// common trackers and radios described in the spec.
const DEVICE_SUFFIXES_2: [&[u8]; 5] = [b"_(", b"_0", b"_3", b"|3", b"|4"];
const DEVICE_SUFFIXES_1: [&[u8]; 3] = [b"=", b"^", b"&"];

/// Strip a known device identifier suffix from the end of the status text,
/// returning the remaining comment and the device bytes (or `None`).
fn strip_device_suffix(text: &[u8]) -> (&[u8], Option<&[u8]>) {
    for suffix in DEVICE_SUFFIXES_2 {
        if text.ends_with(suffix) {
            return (&text[..text.len() - suffix.len()], Some(suffix));
        }
    }
    for suffix in DEVICE_SUFFIXES_1 {
        if text.ends_with(suffix) {
            return (&text[..text.len() - suffix.len()], Some(suffix));
        }
    }
    (text, None)
}

/// Decode a single destination-address byte into a latitude digit.
///
/// Returns `(digit, is_blanked)` where `is_blanked` marks the position-ambiguity
/// characters `K`, `L`, and `Z` (which encode a blanked/space digit).
fn lat_digit(c: u8) -> Result<(u8, bool), Error> {
    match c {
        b'0'..=b'9' => Ok((c - b'0', false)),
        b'A'..=b'J' => Ok((c - b'A', false)),
        b'P'..=b'Y' => Ok((c - b'P', false)),
        b'K' | b'L' | b'Z' => Ok((0, true)),
        _ => Err(Error::InvalidCharacter),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Destination "S32UVT": latitude 33°25.64'N, longitude offset +100, West.
    // Information `(_fn "Oj/`: longitude 112°7.74'W, 20 knots, 251°, jeep symbol.
    // This matches the worked examples in APRS101 Chapter 10.
    #[test]
    fn decodes_spec_example() {
        let dest = b"S32UVT";
        // APRS101 Ch.10 example: '(_fn "Oj/ — apostrophe data type, then
        // d+28='(', m+28='_', h+28='f', SP+28='n', DC+28='"', SE+28='O',
        // symbol code='j', symbol table='/'.
        let info: &[u8] = b"'(_fn\"Oj/";
        let p = decode(dest, info).unwrap();

        assert_eq!(p.encoding, PositionEncoding::MicE);
        assert!(
            (p.coordinates.latitude - 33.4273333).abs() < 1e-5,
            "lat {}",
            p.coordinates.latitude
        );
        assert!(
            (p.coordinates.longitude + 112.129).abs() < 1e-4,
            "lon {}",
            p.coordinates.longitude
        );
        assert_eq!(p.speed_knots, Some(20));
        assert_eq!(p.course, Some(251));
        assert_eq!(p.symbol_code, b'j');
        assert_eq!(p.symbol_table, SymbolTable(b'/'));
    }

    #[test]
    fn decodes_captured_packet() {
        // WF5X-9>TRSX1U,...:`oY}lzI>/]"6&}=
        // destination "TRSX1U" => 42°38.15'N, +0 offset, West.
        let dest = b"TRSX1U";
        let info: &[u8] = b"`oY}lzI>/]";
        let p = decode(dest, info).unwrap();
        assert!(
            (p.coordinates.latitude - 42.6358333).abs() < 1e-4,
            "lat {}",
            p.coordinates.latitude
        );
        // 'o'(111)-28=83 lon degrees; 'Y'(89)-28=61 => 1 min; '}'(125)-28=97 hundredths.
        // West: negative longitude.
        assert!(p.coordinates.longitude < 0.0);
    }

    #[test]
    fn rejects_non_mice() {
        let info: &[u8] = b"!4903.50N/07201.75W-";
        assert!(decode(b"S32UVT", info).is_err());
    }

    #[test]
    fn decodes_position_ambiguity() {
        // "T4SQZZ": hundredths blanked (ZZ) => ambiguity 2, 44°31'N.
        let dest = b"T4SQZZ";
        let info: &[u8] = b"`(_fn\"Oj/";
        let p = decode(dest, info).unwrap();
        assert_eq!(p.ambiguity, 2);
        assert!((p.coordinates.latitude - 44.5166666).abs() < 1e-5);
    }

    #[test]
    fn status_text_messaging_and_altitude() {
        // Backtick prefix = messaging capable; `"4T}` = 200 feet altitude.
        let dest = b"S32UVT";
        let info = b"'(_fn\"Oj/`\"4T}";
        let p = decode(dest, info).unwrap();
        assert!(p.messaging);
        assert_eq!(p.altitude_feet, Some(200));
        assert!(p.comment.is_empty());
    }

    #[test]
    fn status_text_no_messaging() {
        // Apostrophe prefix = not messaging capable.
        let dest = b"S32UVT";
        let info = b"'(_fn\"Oj/'\"4T}";
        let p = decode(dest, info).unwrap();
        assert!(!p.messaging);
        assert_eq!(p.altitude_feet, Some(200));
    }

    #[test]
    fn status_text_kenwood_prefix() {
        // Kenwood `>` prefix, free-text comment preserved.
        let dest = b"S32UVT";
        let info = b"'(_fn\"Oj/>Hello";
        let p = decode(dest, info).unwrap();
        assert!(!p.messaging);
        assert_eq!(p.comment, b"Hello");
    }

    #[test]
    fn status_text_device_suffix() {
        // Byonics TinyTrack 3: ' prefix (no messaging) + altitude + `|3` suffix.
        let dest = b"S32UVT";
        let info = b"'(_fn\"Oj/'\"4T}hello|3";
        let p = decode(dest, info).unwrap();
        assert!(!p.messaging);
        assert_eq!(p.altitude_feet, Some(200));
        assert_eq!(p.device, Some(&b"|3"[..]));
        assert_eq!(p.comment, b"hello");
    }
}
