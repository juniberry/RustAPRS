//! Mic-E position encoding (§10).
//!
//! Mic-E packs the latitude into the 6-byte destination address and the
//! longitude, speed, course, and symbol into the information field. Encoding is
//! split across two output buffers: `dest` (6 bytes) and `info`.

use aprs_core::base91::base91_encode_fixed;
use aprs_core::{Error, MicEMessage};

/// Input for a Mic-E position report.
#[derive(Debug, Clone, Copy)]
pub struct MicEInput<'a> {
    /// Latitude in decimal degrees (north positive).
    pub latitude: f64,
    /// Longitude in decimal degrees (east positive).
    pub longitude: f64,
    /// Speed in knots (0–799; higher values are capped at 799).
    pub speed_knots: u16,
    /// Course in degrees: 1–360 (360 = due north), 0 = unknown. Values above
    /// 360 wrap.
    pub course: u16,
    pub symbol_table: u8,
    pub symbol_code: u8,
    /// The position comment carried in the destination's message bits.
    pub message: MicEMessage,
    /// Position ambiguity: 0 (exact) to 4 trailing latitude digits blanked.
    /// The same digits of the longitude are not sent (zeros), as §10 applies
    /// the latitude's ambiguity to the longitude.
    pub ambiguity: u8,
    /// Altitude in feet, sent as the `xxx}` base-91 field (meters).
    pub altitude_feet: Option<i32>,
    /// Status text (may be empty).
    pub status: &'a [u8],
}

/// Encode a Mic-E position into the destination address (6 bytes) and the
/// information field, returning `(dest_len, info_len)`.
///
/// Uses the "current GNSS data" data type (` `` `) and the M1 "En Route"
/// position comment, with no ambiguity or altitude: see [`encode_report`] for
/// the rest. `lat`/`lon` are in decimal degrees; `speed_knots` and `course`
/// (degrees, 0 = unknown, 360 = north) are optional motion data; `status` is
/// optional status text (may be empty).
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
    encode_report(
        &MicEInput {
            latitude: lat,
            longitude: lon,
            speed_knots,
            course,
            symbol_table,
            symbol_code,
            message: MicEMessage::EnRoute,
            ambiguity: 0,
            altitude_feet: None,
            status,
        },
        dest,
        info,
    )
}

/// Encode a Mic-E position report into the destination address (6 bytes) and
/// the information field, returning `(dest_len, info_len)`.
///
/// The information field is ` `` ` (current GNSS data), longitude, speed and
/// course, symbol, then the altitude (`xxx}`) when given, then the status
/// text. Status text that a decoder would misread (a leading device or
/// telemetry prefix `` ` `` `'` `>` `]`, or an `xxx}` altitude in its first
/// five bytes when no altitude is sent) is sent behind one or two spaces.
pub fn encode_report(
    input: &MicEInput<'_>,
    dest: &mut [u8],
    info: &mut [u8],
) -> Result<(usize, usize), Error> {
    let (lat, lon) = (input.latitude, input.longitude);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(Error::InvalidCoordinate);
    }
    if input.ambiguity > 4 {
        return Err(Error::InvalidNumber);
    }
    let (bits, custom) = input.message.bits().ok_or(Error::Unsupported)?;
    if dest.len() < 6 {
        return Err(Error::BufferTooSmall);
    }

    // Degrees and hundredths of minutes; trailing digits dropped for
    // ambiguity. Longitude 180 (or 179.99999 rounding up) is not encodable:
    // send 179°59.99'.
    let blank = [1, 10, 100, 1000, 10_000][input.ambiguity as usize];
    let lat_total = (lat.abs() * 6000.0 + 0.5) as u32;
    let lat_total = lat_total - lat_total % 6000 % blank;
    let lat_deg = lat_total / 6000;
    let lat_hmin = lat_total % 6000;
    let lon_total = ((lon.abs() * 6000.0 + 0.5) as u32).min(180 * 6000 - 1);
    let lon_total = lon_total - lon_total % 6000 % blank;
    let lon_deg = lon_total / 6000;
    let lon_min = (lon_total % 6000) / 100;
    let lon_hund = lon_total % 100;
    let (lon_deg_byte, offset_100) = encode_lon_deg(lon_deg);

    // Destination address: each byte a latitude digit, its flag the message
    // bit (bytes 0–2), N/S, the +100 offset or W/E (bytes 3–5).
    let digits = [
        lat_deg / 10,
        lat_deg % 10,
        lat_hmin / 1000,
        lat_hmin / 100 % 10,
        lat_hmin / 10 % 10,
        lat_hmin % 10,
    ];
    let flags = [
        bits & 0b100 != 0,
        bits & 0b010 != 0,
        bits & 0b001 != 0,
        lat >= 0.0,
        offset_100,
        lon < 0.0,
    ];
    for i in 0..6 {
        let blanked = i >= 6 - input.ambiguity as usize;
        let custom_one = custom && i < 3 && flags[i];
        dest[i] = match (blanked, flags[i], custom_one) {
            (false, _, true) => b'A' + digits[i] as u8,
            (false, true, false) => b'P' + digits[i] as u8,
            (false, false, _) => b'0' + digits[i] as u8,
            (true, _, true) => b'K',
            (true, true, false) => b'Z',
            (true, false, _) => b'L',
        };
    }

    // Status text protection.
    let status = input.status;
    let looks_like_altitude = |s: &[u8]| {
        s.len() >= 4 && s[3] == b'}' && s[..3].iter().all(|&c| (0x21..=0x7B).contains(&c))
    };
    let pad = if input.altitude_feet.is_some() {
        0
    } else if looks_like_altitude(status) {
        2
    } else if looks_like_altitude(status.get(1..).unwrap_or(&[]))
        || matches!(status.first(), Some(b'`' | b'\'' | b'>' | b']' | 0x1d))
    {
        1
    } else {
        0
    };
    let alt_len = if input.altitude_feet.is_some() { 4 } else { 0 };
    if info.len() < 9 + alt_len + pad + status.len() {
        return Err(Error::BufferTooSmall);
    }

    // Information field.
    let (sp, dc, se) = encode_speed_course(input.speed_knots, input.course);
    info[0] = b'`'; // current GNSS data
    info[1] = lon_deg_byte;
    info[2] = encode_lon_min(lon_min);
    info[3] = (lon_hund + 28) as u8;
    info[4] = sp;
    info[5] = dc;
    info[6] = se;
    info[7] = input.symbol_code;
    info[8] = input.symbol_table;
    let mut pos = 9;
    if let Some(feet) = input.altitude_feet {
        // Meters (rounded) above 10 km below mean sea level.
        let half = if feet < 0 { -5000 } else { 5000 };
        let meters = (feet as i64 * 3048 + half) / 10_000;
        let v = (meters + 10_000).clamp(0, 91 * 91 * 91 - 1) as u32;
        base91_encode_fixed(v, 3, &mut info[pos..pos + 3]).ok_or(Error::BufferTooSmall)?;
        info[pos + 3] = b'}';
        pos += 4;
    }
    info[pos..pos + pad].fill(b' ');
    pos += pad;
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
/// `SE+28` bytes. Course 0 means unknown and 360 due north (§10), so both are
/// sent as they are; above 360 wraps to 1–360.
fn encode_speed_course(speed_knots: u16, course: u16) -> (u8, u8, u8) {
    let speed = speed_knots.min(799);
    let speed_tens = speed / 10;
    let speed_units = speed % 10;
    let course = if course > 360 {
        (course - 1) % 360 + 1
    } else {
        course
    };
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

    fn input(lat: f64, lon: f64) -> MicEInput<'static> {
        MicEInput {
            latitude: lat,
            longitude: lon,
            speed_knots: 0,
            course: 0,
            symbol_table: b'/',
            symbol_code: b'>',
            message: MicEMessage::EnRoute,
            ambiguity: 0,
            altitude_feet: None,
            status: b"",
        }
    }

    /// Encode, then decode with this workspace's decoder (fields that don't
    /// borrow the buffer).
    fn decoded(input: &MicEInput) -> Decodedish {
        let mut dest = [0u8; 6];
        let mut info = [0u8; 64];
        let (_, il) = encode_report(input, &mut dest, &mut info).unwrap();
        let p = aprs_decoders::mice::decode(&dest, &info[..il]).unwrap();
        Decodedish {
            coordinates: p.coordinates,
            course: p.course,
            speed_knots: p.speed_knots,
            altitude_feet: p.altitude_feet,
            mic_e_message: p.mic_e_message,
        }
    }

    struct Decodedish {
        coordinates: aprs_decoders::Coordinates,
        course: Option<u16>,
        speed_knots: Option<u16>,
        altitude_feet: Option<i32>,
        mic_e_message: Option<MicEMessage>,
    }

    #[test]
    fn longitude_180_is_sent_as_179_59_99() {
        // Rounding 179.99999 (or 180) up to 180 degrees has no Mic-E encoding:
        // it used to come out as byte 108, which decodes as 100 degrees.
        for lon in [179.99999, 180.0, -180.0] {
            let p = decoded(&input(0.0, lon));
            let want = 179.0 + 59.99 / 60.0;
            assert!(
                (p.coordinates.longitude.abs() - want).abs() < 1e-9,
                "{lon}: {}",
                p.coordinates.longitude
            );
        }
    }

    #[test]
    fn course_zero_unknown_and_360_north() {
        let mut i = input(45.0, -75.0);
        i.speed_knots = 10;
        assert_eq!(decoded(&i).course, None);
        i.course = 360; // was clamped to 359
        assert_eq!(decoded(&i).course, Some(360));
        i.course = 361;
        assert_eq!(decoded(&i).course, Some(1));
        i.speed_knots = 900;
        assert_eq!(decoded(&i).speed_knots, Some(799));
    }

    #[test]
    fn every_message_type() {
        let mut dest = [0u8; 6];
        let mut info = [0u8; 32];
        let mut i = input(33.4273333, -112.129);
        for (m, first3) in [
            (MicEMessage::OffDuty, b"SSR"),
            (MicEMessage::EnRoute, b"SS2"),
            (MicEMessage::InService, b"S3R"),
            (MicEMessage::Returning, b"S32"),
            (MicEMessage::Committed, b"3SR"),
            (MicEMessage::Special, b"3S2"),
            (MicEMessage::Priority, b"33R"),
            (MicEMessage::Emergency, b"332"),
            (MicEMessage::Custom(0), b"DDC"),
            (MicEMessage::Custom(6), b"33C"),
        ] {
            i.message = m;
            encode_report(&i, &mut dest, &mut info).unwrap();
            assert_eq!(&dest[..3], first3, "{m:?}");
            assert_eq!(&dest[3..], b"UVT");
            assert_eq!(decoded(&i).mic_e_message, Some(m));
        }
        i.message = MicEMessage::Unknown;
        assert_eq!(
            encode_report(&i, &mut dest, &mut info),
            Err(Error::Unsupported)
        );
    }

    #[test]
    fn ambiguity_blanks_both_coordinates() {
        // §10's example: T4SQZZ, and the longitude's last two digits ignored.
        let mut dest = [0u8; 6];
        let mut info = [0u8; 32];
        let mut i = input(44.5228, -112.129);
        i.message = MicEMessage::InService;
        i.ambiguity = 2;
        encode_report(&i, &mut dest, &mut info).unwrap();
        assert_eq!(&dest, b"T4SQZZ");
        assert_eq!(&info[1..4], b"(_\x1c"); // 112 07, hundredths not sent
        i.ambiguity = 4;
        encode_report(&i, &mut dest, &mut info).unwrap();
        assert_eq!(&dest, b"T4ZZZZ");
        i.ambiguity = 5;
        assert_eq!(
            encode_report(&i, &mut dest, &mut info),
            Err(Error::InvalidNumber)
        );
    }

    #[test]
    fn altitude_field() {
        // §10: 200 feet = 61 m = "4T}
        let mut dest = [0u8; 6];
        let mut info = [0u8; 32];
        let mut i = input(33.4273333, -112.129);
        i.altitude_feet = Some(200);
        i.status = b"hi";
        let (_, il) = encode_report(&i, &mut dest, &mut info).unwrap();
        assert_eq!(&info[9..il], b"\"4T}hi");
        assert_eq!(decoded(&i).altitude_feet, Some(200));
    }

    #[test]
    fn status_text_protected() {
        // Text that a decoder would take for a device prefix or an altitude
        // goes out behind one or two spaces and reads back as written.
        let mut dest = [0u8; 6];
        let mut info = [0u8; 64];
        for (status, pad) in [
            (&b"`x"[..], 1),
            (b"'x", 1),
            (b">x", 1),
            (b"]x=", 1),
            (b"abc}", 2),
            (b"xabc}", 1),
            (b"Hello", 0),
            (b"} ok", 0),
        ] {
            let mut i = input(45.0, -75.0);
            i.status = status;
            let (_, il) = encode_report(&i, &mut dest, &mut info).unwrap();
            assert_eq!(&info[9..9 + pad], &b"  "[..pad], "{status:?}");
            assert_eq!(&info[9 + pad..il], status);
            let p = aprs_decoders::mice::decode(&dest, &info[..il]).unwrap();
            assert_eq!(p.comment.trim_ascii_start(), status);
            assert_eq!(
                (p.altitude_feet, p.device, p.messaging),
                (None, None, false)
            );
        }
    }
}
