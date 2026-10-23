//! APRS data extensions (§7): course/speed, PHG, and radio range.
//!
//! These 7-byte fields may follow a position report's symbol code and precede
//! any free-form comment. Recognized extensions are stripped from the comment
//! and returned as typed values.

use crate::decoded::Phg;
use crate::parse::digits;

/// Parsed data extensions from a position/object comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extensions {
    /// Course over ground in degrees (from `CSE/SPD`); `None` if 000 or absent.
    pub course: Option<u16>,
    /// Speed in knots (from `CSE/SPD`); `None` if 000 or absent.
    pub speed_knots: Option<u16>,
    /// Power/height/gain/directivity (from `PHGphgd`).
    pub phg: Option<Phg>,
    /// Pre-calculated radio range in miles (from `RNGrrrr`).
    pub radio_range_miles: Option<u16>,
}

/// Parse data extensions from the start of a comment, returning the parsed
/// values and the remaining (unconsumed) comment bytes.
pub fn parse(comment: &[u8]) -> (Extensions, &[u8]) {
    let mut ext = Extensions {
        course: None,
        speed_knots: None,
        phg: None,
        radio_range_miles: None,
    };
    let mut rest = comment;

    // CSE/SPD: `DDD/DDD` (course/speed, in knots).
    if rest.len() >= 7
        && rest[..3].iter().all(u8::is_ascii_digit)
        && rest[3] == b'/'
        && rest[4..7].iter().all(u8::is_ascii_digit)
    {
        ext.course = digits(&rest[..3])
            .and_then(|v| u16::try_from(v).ok())
            .filter(|&v| v != 0);
        ext.speed_knots = digits(&rest[4..7])
            .and_then(|v| u16::try_from(v).ok())
            .filter(|&v| v != 0);
        rest = &rest[7..];
    }

    if let Some(phg) = parse_phg(rest) {
        ext.phg = Some(phg);
        rest = &rest[7..];
    }

    if let Some(range) = parse_rng(rest) {
        ext.radio_range_miles = Some(range);
        rest = &rest[7..];
    }

    (ext, rest)
}

/// Parse a `PHGphgd` field. The power code is squared to watts; the height code
/// gives `10 × 2^h` feet (and may exceed `9` for balloons/aircraft); the gain
/// code is dBi; the directivity code gives `d × 45` degrees.
fn parse_phg(bytes: &[u8]) -> Option<Phg> {
    if bytes.len() < 7 || &bytes[..3] != b"PHG" {
        return None;
    }
    let p = digit(bytes[3])?;
    let h = height_code(bytes[4])?;
    let g = digit(bytes[5])?;
    let d = digit(bytes[6])?;

    Some(Phg {
        power_watts: (p * p) as u16,
        height_feet: 10u32.checked_shl(h).unwrap_or(u32::MAX),
        gain_dbi: g as u8,
        directivity_deg: (d * 45) as u16,
    })
}

/// Parse an `RNGrrrr` field: a pre-calculated omni radio range in miles.
fn parse_rng(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 7 || &bytes[..3] != b"RNG" {
        return None;
    }
    digits(&bytes[3..7]).and_then(|v| u16::try_from(v).ok())
}

fn digit(c: u8) -> Option<u32> {
    if c.is_ascii_digit() {
        Some((c - b'0') as u32)
    } else {
        None
    }
}

/// Height code: `0`–`9`, then `:` (10), `;` (11), and so on for larger heights.
fn height_code(c: u8) -> Option<u32> {
    if c >= b'0' {
        Some((c - b'0') as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_course_speed() {
        let (ext, rest) = parse(b"088/036Hello");
        assert_eq!(ext.course, Some(88));
        assert_eq!(ext.speed_knots, Some(36));
        assert_eq!(rest, b"Hello");
    }

    #[test]
    fn zero_course_speed_is_unknown() {
        let (ext, _) = parse(b"000/000");
        assert_eq!(ext.course, None);
        assert_eq!(ext.speed_knots, None);
    }

    #[test]
    fn parses_phg() {
        // PHG5132: 25 W, 20 ft, 3 dBi, east (90°).
        let (ext, rest) = parse(b"PHG5132comment");
        let phg = ext.phg.unwrap();
        assert_eq!(phg.power_watts, 25);
        assert_eq!(phg.height_feet, 20);
        assert_eq!(phg.gain_dbi, 3);
        assert_eq!(phg.directivity_deg, 90);
        assert_eq!(rest, b"comment");
    }

    #[test]
    fn parses_range() {
        let (ext, _) = parse(b"RNG0050");
        assert_eq!(ext.radio_range_miles, Some(50));
    }

    #[test]
    fn parses_chained_extensions() {
        let (ext, rest) = parse(b"088/036PHG5130RNG0050 tail");
        assert_eq!(ext.course, Some(88));
        assert_eq!(ext.speed_knots, Some(36));
        assert_eq!(ext.phg.unwrap().power_watts, 25);
        assert_eq!(ext.radio_range_miles, Some(50));
        assert_eq!(rest, b" tail");
    }

    #[test]
    fn no_extensions_leaves_comment_intact() {
        let (ext, rest) = parse(b"just a comment 088/036");
        assert!(ext.course.is_none());
        assert!(ext.phg.is_none());
        assert_eq!(rest, b"just a comment 088/036");
    }
}
