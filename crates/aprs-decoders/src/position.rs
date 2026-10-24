//! Position report decoding (uncompressed and base-91 compressed).

use aprs_core::base91::{base91_decode, base91_digit};
use aprs_core::Error;

use crate::data_extension;
use crate::decoded::{
    CompressionOrigin, CompressionType, Coordinates, NmeaSource, Position, PositionEncoding,
    SymbolTable, Timestamp,
};
use crate::parse::{degrees_minutes, digits};

/// Minimum body length (after the data-type byte) for an uncompressed position
/// without a timestamp: 7 (lat) + 1 (N/S) + 1 (table) + 8 (lon) + 1 (E/W)
/// + 1 (symbol code) = 19.
const UNCOMPRESSED_LEN: usize = 19;

/// Decode a position report from the full information field (including the
/// leading data-type byte).
pub fn decode(info: &[u8]) -> Result<Position<'_>, Error> {
    let dt = *info.first().ok_or(Error::Empty)?;

    // `!`/`=` carry no timestamp; `/`/`@` always carry a `DDHHMMz` timestamp.
    let (messaging, with_ts) = match dt {
        b'!' => (false, false),
        b'=' => (true, false),
        b'/' => (false, true),
        b'@' => (true, true),
        _ => return Err(Error::UnknownDataType(dt)),
    };

    let mut body = &info[1..];
    let timestamp = if with_ts {
        let (ts, rest) = parse_timestamp(body)?;
        body = rest;
        Some(ts)
    } else {
        None
    };

    // A leading digit indicates an uncompressed `DDMM.MM` coordinate; any
    // other byte is a Symbol Table Identifier and therefore a compressed
    // position (§9: "the presence of the leading Symbol Table Identifier
    // instead of a digit indicates ... a compressed Position Report").
    match body.first() {
        Some(b) if b.is_ascii_digit() => decode_uncompressed(body, timestamp, messaging),
        Some(_) => decode_compressed(body, timestamp, messaging),
        None => Err(Error::Truncated),
    }
}

/// Parse a 7-byte `DDHHMMz`/`DDHHMM/` (Day/Hour/Minute) or `HHMMSSh`
/// (Hour/Minute/Second) timestamp, returning the timestamp and the remaining
/// body. The suffix selects the format: `z`/`Z` (UTC DHM), `/` (local DHM), or
/// `h`/`H` (UTC HMS).
pub fn parse_timestamp(body: &[u8]) -> Result<(Timestamp, &[u8]), Error> {
    if body.len() < 7 {
        return Err(Error::Truncated);
    }
    let kind = body[6];
    match kind {
        b'h' | b'H' => {
            let hour = digits(&body[0..2]).ok_or(Error::InvalidNumber)? as u8;
            let minute = digits(&body[2..4]).ok_or(Error::InvalidNumber)? as u8;
            let second = digits(&body[4..6]).ok_or(Error::InvalidNumber)? as u8;
            Ok((
                Timestamp::HMS {
                    hour,
                    minute,
                    second,
                },
                &body[7..],
            ))
        }
        b'z' | b'Z' | b'/' => {
            let day = digits(&body[0..2]).ok_or(Error::InvalidNumber)? as u8;
            let hour = digits(&body[2..4]).ok_or(Error::InvalidNumber)? as u8;
            let minute = digits(&body[4..6]).ok_or(Error::InvalidNumber)? as u8;
            Ok((
                Timestamp::DHM {
                    day,
                    hour,
                    minute,
                    utc: kind != b'/',
                },
                &body[7..],
            ))
        }
        _ => Err(Error::InvalidCharacter),
    }
}

/// Parse an 8-byte `MMDDHHMM` (Month/Day/Hour/Minute) timestamp with no
/// suffix, used by positionless weather reports. Returns the timestamp and the
/// remaining body.
pub fn parse_mdhm(body: &[u8]) -> Result<(Timestamp, &[u8]), Error> {
    if body.len() < 8 {
        return Err(Error::Truncated);
    }
    let month = digits(&body[0..2]).ok_or(Error::InvalidNumber)? as u8;
    let day = digits(&body[2..4]).ok_or(Error::InvalidNumber)? as u8;
    let hour = digits(&body[4..6]).ok_or(Error::InvalidNumber)? as u8;
    let minute = digits(&body[6..8]).ok_or(Error::InvalidNumber)? as u8;
    Ok((
        Timestamp::MDHM {
            month,
            day,
            hour,
            minute,
        },
        &body[8..],
    ))
}

/// Decode an uncompressed position body (the data type and any timestamp have
/// already been consumed).
pub fn decode_uncompressed(
    body: &[u8],
    timestamp: Option<Timestamp>,
    messaging: bool,
) -> Result<Position<'_>, Error> {
    if body.len() < UNCOMPRESSED_LEN {
        return Err(Error::Truncated);
    }

    let (mut lat, lat_blank) = degrees_minutes(&body[0..7], 2).ok_or(Error::InvalidCoordinate)?;
    let (mut lon, lon_blank) = degrees_minutes(&body[9..17], 3).ok_or(Error::InvalidCoordinate)?;

    let lat_sign = match body[7] {
        b'N' | b'n' => 1.0,
        b'S' | b's' => -1.0,
        _ => return Err(Error::InvalidCoordinate),
    };
    let lon_sign = match body[17] {
        b'E' | b'e' => 1.0,
        b'W' | b'w' => -1.0,
        _ => return Err(Error::InvalidCoordinate),
    };
    lat *= lat_sign;
    lon *= lon_sign;

    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }

    let symbol_table = SymbolTable::from_byte(body[8]);
    let (ext, comment) = data_extension::parse(&body[19..]);

    Ok(Position {
        coordinates: Coordinates {
            latitude: lat,
            longitude: lon,
        },
        encoding: PositionEncoding::Uncompressed,
        symbol_table,
        symbol_code: body[18],
        comment,
        timestamp,
        ambiguity: lat_blank.max(lon_blank),
        messaging,
        device: None,
        course: ext.course,
        speed_knots: ext.speed_knots,
        altitude_feet: None,
        phg: ext.phg,
        radio_range_miles: ext.radio_range_miles,
        compression_type: None,
        maidenhead: None,
        mic_e_message: None,
    })
}

/// Decode a compressed position body (the data-type byte and any timestamp
/// have been consumed; the first byte is the symbol-table identifier).
pub fn decode_compressed(
    body: &[u8],
    timestamp: Option<Timestamp>,
    messaging: bool,
) -> Result<Position<'_>, Error> {
    if body.len() < 13 {
        return Err(Error::Truncated);
    }

    let symbol_table = SymbolTable::from_byte(body[0]);
    let lat_enc = base91_decode(&body[1..5]).ok_or(Error::InvalidBase91)?;
    let lon_enc = base91_decode(&body[5..9]).ok_or(Error::InvalidBase91)?;
    let symbol_code = body[9];
    let c = body[10];
    let s = body[11];
    let t_val = (body[12] as i32 - 33).max(0) as u8;

    let latitude = 90.0 - lat_enc as f64 / 380926.0;
    let longitude = -180.0 + lon_enc as f64 / 190463.0;

    // The compression-type (T) byte is not meaningful when `c` is a space.
    let compression_type = (c != b' ').then_some(CompressionType {
        gps_fix_current: t_val & 0x20 != 0,
        nmea_source: match (t_val >> 3) & 0x3 {
            0 => NmeaSource::Other,
            1 => NmeaSource::Gll,
            2 => NmeaSource::Gga,
            _ => NmeaSource::Rmc,
        },
        origin: match t_val & 0x7 {
            0 => CompressionOrigin::Compressed,
            1 => CompressionOrigin::TncBText,
            2 => CompressionOrigin::Software,
            3 => CompressionOrigin::Tbd,
            4 => CompressionOrigin::Kpc3,
            5 => CompressionOrigin::Pico,
            6 => CompressionOrigin::OtherTracker,
            _ => CompressionOrigin::Digipeater,
        },
    });

    // The cs bytes encode course/speed, pre-calculated radio range, or
    // altitude. `c == ' '` means none of these are present.
    let (course, speed_knots, altitude_feet, cs_range) = if c == b' ' {
        (None, None, None, None)
    } else if c == b'{' {
        // '{' (base-91 value 90): compressed radio range (range = 2 * 1.08^s).
        let s_val = base91_digit(s).ok_or(Error::InvalidBase91)? as i32;
        (None, None, None, Some((2.0 * exp(1.08, s_val)) as u16))
    } else if (t_val & 0x18) == 0x10 {
        // GGA sentence: cs carries altitude = 1.002^(c*91+s) feet.
        let c_val = base91_digit(c).ok_or(Error::InvalidBase91)? as i32;
        let s_val = base91_digit(s).ok_or(Error::InvalidBase91)? as i32;
        let cs = c_val * 91 + s_val;
        (None, None, Some(exp(1.002, cs) as i32), None)
    } else {
        // Course/speed: course = c*4, speed = 1.08^s - 1 (c in 0..89).
        let c_val = base91_digit(c).ok_or(Error::InvalidBase91)?;
        let s_val = base91_digit(s).ok_or(Error::InvalidBase91)?;
        let course = (c_val != 0).then(|| (c_val * 4) as u16);
        let speed = (s_val != 0).then(|| (exp(1.08, s_val as i32) - 1.0) as u16);
        (course, speed, None, None)
    };

    // Data extensions (PHG, RNG, CSE/SPD) may follow the compressed field.
    let (ext, comment) = data_extension::parse(&body[13..]);
    let radio_range_miles = cs_range.or(ext.radio_range_miles);

    Ok(Position {
        coordinates: Coordinates {
            latitude,
            longitude,
        },
        encoding: PositionEncoding::Compressed,
        symbol_table,
        symbol_code,
        comment,
        timestamp,
        ambiguity: 0,
        messaging,
        device: None,
        course,
        speed_knots,
        altitude_feet,
        phg: ext.phg,
        radio_range_miles,
        compression_type,
        maidenhead: None,
        mic_e_message: None,
    })
}

/// Compute `base^pow` via repeated multiplication (no `std` `powi`).
fn exp(base: f64, pow: i32) -> f64 {
    let mut v = 1.0f64;
    for _ in 0..pow {
        v *= base;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncompressed_no_timestamp() {
        let p = decode(b"!4903.50N/07201.75W-Test 001").unwrap();
        assert_eq!(p.encoding, PositionEncoding::Uncompressed);
        assert_eq!(p.symbol_table, SymbolTable(b'/'));
        assert_eq!(p.symbol_code, b'-');
        assert_eq!(p.comment, b"Test 001");
        assert!(p.timestamp.is_none());
        assert!(!p.messaging);
        assert!((p.coordinates.latitude - 49.0583333).abs() < 1e-6);
        assert!((p.coordinates.longitude + 72.0291666).abs() < 1e-6);
    }

    #[test]
    fn uncompressed_with_timestamp() {
        let p = decode(b"/092345z4903.50N/07201.75W-").unwrap();
        assert_eq!(
            p.timestamp,
            Some(Timestamp::DHM {
                day: 9,
                hour: 23,
                minute: 45,
                utc: true,
            })
        );
        assert!((p.coordinates.latitude - 49.0583333).abs() < 1e-6);
    }

    #[test]
    fn messaging_flag() {
        assert!(decode(b"=4903.50N/07201.75W-").unwrap().messaging);
        assert!(!decode(b"!4903.50N/07201.75W-").unwrap().messaging);
    }

    #[test]
    fn southern_hemisphere() {
        let p = decode(b"!3327.50S/07043.00W-").unwrap();
        assert!(p.coordinates.latitude < 0.0);
        assert!(p.coordinates.longitude < 0.0);
    }

    #[test]
    fn truncated_is_error() {
        assert_eq!(decode(b"!4903.50N").unwrap_err(), Error::Truncated);
    }

    #[test]
    fn compressed_roundtrip_reference() {
        // A known compressed packet: `!/5L!!<*e7>7P[` decodes to roughly
        // 49.5000 N, 72.7500 W (canonical example from the APRS spec).
        let p = decode(b"!/5L!!<*e7>7P[").unwrap();
        assert_eq!(p.encoding, PositionEncoding::Compressed);
        assert_eq!(p.symbol_code, b'>');
        assert!(p.course.is_some() && p.speed_knots.is_some());
    }

    #[test]
    fn compressed_lat_lon_are_sane() {
        let p = decode(b"!/5L!!<*e7>7P[").unwrap();
        assert!((48.0..52.0).contains(&p.coordinates.latitude));
        assert!((-74.0..-71.0).contains(&p.coordinates.longitude));
    }

    #[test]
    fn compressed_non_slash_symbol_table() {
        // Symbol table `b` (overlay/alternate), still a compressed position.
        let p = decode(b"!b:)YB:>:b#  !").unwrap();
        assert_eq!(p.encoding, PositionEncoding::Compressed);
        assert_eq!(p.symbol_table, SymbolTable(b'b'));
        assert_eq!(p.symbol_code, b'#');
        assert!((40.0..41.0).contains(&p.coordinates.latitude));
        assert!((-80.0..-79.0).contains(&p.coordinates.longitude));
    }

    #[test]
    fn compressed_with_timestamp() {
        // `@` carries an HMS timestamp, followed by a compressed position.
        let p = decode(b"@183834h/9Fl/;DN?- sT").unwrap();
        assert_eq!(p.encoding, PositionEncoding::Compressed);
        assert_eq!(
            p.timestamp,
            Some(Timestamp::HMS {
                hour: 18,
                minute: 38,
                second: 34,
            })
        );
        assert_eq!(p.symbol_code, b'-');
    }

    #[test]
    fn compressed_gga_altitude_and_type() {
        // Spec example: GGA sentence, altitude = 1.002^(c*91+s) = 10004 ft.
        let p = decode(b"=/5L!!<*e7OS]S").unwrap();
        assert_eq!(p.altitude_feet, Some(10004));
        let ct = p.compression_type.unwrap();
        assert!(ct.gps_fix_current);
        assert_eq!(ct.nmea_source, NmeaSource::Gga);
        assert_eq!(ct.origin, CompressionOrigin::Software);
    }

    #[test]
    fn compressed_radio_range() {
        // Spec example: c = '{', range = 2 * 1.08^s ≈ 20 miles.
        let p = decode(b"=/5L!!<*e7>{?!").unwrap();
        assert_eq!(p.radio_range_miles, Some(20));
        assert_eq!(p.course, None);
        assert_eq!(p.speed_knots, None);
    }
}
