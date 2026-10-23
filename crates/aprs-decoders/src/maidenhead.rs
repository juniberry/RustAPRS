//! Maidenhead grid locator (grid square) parsing.

use crate::decoded::{Coordinates, Maidenhead};

/// Parse a Maidenhead grid locator from the start of `input`.
///
/// Recognizes the 4-character (`GGnn`) and 6-character (`GGnnXX`) forms. The
/// field letters are `A`–`R`, the square digits are `0`–`9`, and the subsquare
/// letters are `A`–`X` (all accepted in either case). Returns the decoded
/// locator and the remaining bytes, or `None` if the input does not start with
/// a valid locator.
pub fn parse(input: &[u8]) -> Option<(Maidenhead, &[u8])> {
    let f1 = field_letter(*input.first()?)?;
    let f2 = field_letter(*input.get(1)?)?;
    let s1 = ascii_digit(*input.get(2)?)?;
    let s2 = ascii_digit(*input.get(3)?)?;

    let mut grid = [b' '; 6];
    grid[0] = f1;
    grid[1] = f2;
    grid[2] = s1;
    grid[3] = s2;

    // Optional subsquare pair (two letters, A–X).
    let (sub_lon, sub_lat, precision) = match (input.get(4), input.get(5)) {
        (Some(&a), Some(&b)) => match (subsquare_letter(a), subsquare_letter(b)) {
            (Some(la), Some(lb)) => {
                grid[4] = a.to_ascii_uppercase();
                grid[5] = b.to_ascii_uppercase();
                (la, lb, 6u8)
            }
            _ => (0, 0, 4u8),
        },
        _ => (0, 0, 4u8),
    };

    let lon = -180.0
        + (f1 - b'A') as f64 * 20.0
        + (s1 - b'0') as f64 * 2.0
        + sub_lon as f64 * (5.0 / 60.0);
    let lat = -90.0
        + (f2 - b'A') as f64 * 10.0
        + (s2 - b'0') as f64 * 1.0
        + sub_lat as f64 * (2.5 / 60.0);

    // Center of the grid cell.
    let (latitude, longitude) = if precision == 6 {
        (lat + (2.5 / 60.0) / 2.0, lon + (5.0 / 60.0) / 2.0)
    } else {
        (lat + 0.5, lon + 1.0)
    };

    Some((
        Maidenhead {
            grid,
            precision,
            coordinates: Coordinates {
                latitude,
                longitude,
            },
        },
        &input[precision as usize..],
    ))
}

fn field_letter(c: u8) -> Option<u8> {
    let c = c.to_ascii_uppercase();
    (b'A'..=b'R').contains(&c).then_some(c)
}

fn subsquare_letter(c: u8) -> Option<u8> {
    let c = c.to_ascii_uppercase();
    if (b'A'..=b'X').contains(&c) {
        Some(c - b'A')
    } else {
        None
    }
}

fn ascii_digit(c: u8) -> Option<u8> {
    if c.is_ascii_digit() {
        Some(c)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_six_char_locator() {
        // IO91SX is the London area.
        let (m, rest) = parse(b"IO91SX/G").unwrap();
        assert_eq!(m.precision, 6);
        assert_eq!(&m.grid[..6], b"IO91SX");
        assert!((m.coordinates.latitude - 51.979).abs() < 0.01);
        assert!((m.coordinates.longitude + 0.458).abs() < 0.01);
        assert_eq!(rest, b"/G");
    }

    #[test]
    fn parses_four_char_locator() {
        let (m, rest) = parse(b"IO91/G").unwrap();
        assert_eq!(m.precision, 4);
        assert_eq!(&m.grid[..4], b"IO91");
        assert_eq!(rest, b"/G");
    }

    #[test]
    fn accepts_lowercase() {
        let (m, _) = parse(b"io91sx").unwrap();
        assert_eq!(&m.grid[..6], b"IO91SX");
    }

    #[test]
    fn rejects_non_locator() {
        assert!(parse(b"Hello").is_none());
        assert!(parse(b"9O91SX").is_none());
    }
}
