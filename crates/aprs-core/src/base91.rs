//! Base-91 encoding used by APRS for compressed positions and course/speed.

/// Number of symbols in the APRS base-91 alphabet.
pub const BASE91_ALPHABET_SIZE: u32 = 91;

/// Convert a single base-91 character (ASCII `!`..`{`, i.e. `0x21`..`0x7B`)
/// into its numeric value in the range `0..91`.
///
/// Returns `None` for any byte outside the valid alphabet.
#[inline]
pub fn base91_digit(c: u8) -> Option<u32> {
    if matches!(c, 0x21..=0x7B) {
        Some((c - 0x21) as u32)
    } else {
        None
    }
}

/// Decode a big-endian base-91 integer: the first byte is the most
/// significant digit (matching the APRS compressed-position encoding).
///
/// Returns `None` if any byte is outside the alphabet or if the value would
/// overflow a `u32`.
pub fn base91_decode(digits: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for &c in digits {
        let d = base91_digit(c)?;
        value = value.checked_mul(BASE91_ALPHABET_SIZE)?.checked_add(d)?;
    }
    Some(value)
}

/// The full range of a 4-digit base-91 value: `91^4 - 1`.
pub const BASE91_4_MAX: u32 = 68_574_960;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_digits() {
        assert_eq!(base91_digit(b'!'), Some(0));
        assert_eq!(base91_digit(b'"'), Some(1));
        assert_eq!(base91_digit(b'{'), Some(90));
        assert_eq!(base91_digit(b' '), None); // below range
        assert_eq!(base91_digit(b'|'), None); // above range
        assert_eq!(base91_digit(0x00), None);
    }

    #[test]
    fn big_endian_decode() {
        // "!!" = 0 * 91 + 0 = 0
        assert_eq!(base91_decode(b"!!"), Some(0));
        // "!\"" = 0 * 91 + 1 = 1
        assert_eq!(base91_decode(b"!\""), Some(1));
        // "\"!" = 1 * 91 + 0 = 91
        assert_eq!(base91_decode(b"\"!"), Some(91));
    }

    #[test]
    fn overflow_is_none() {
        // 8 digits of the highest value would overflow u32.
        assert_eq!(base91_decode(b"{{{{{{{{"), None);
    }
}
