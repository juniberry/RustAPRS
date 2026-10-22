//! Small numeric parsing helpers (no `std`, no allocation).

/// Parse an ASCII digit sequence into a `u32`. Returns `None` on any non-digit
/// or an empty input.
pub fn digits(input: &[u8]) -> Option<u32> {
    if input.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for &b in input {
        if !b.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((b - b'0') as u32)?;
    }
    Some(value)
}

/// Parse a digit sequence, treating spaces as blanked (zero) low-order digits
/// for position ambiguity. Returns `(value, blanked_count)`.
pub fn digits_ambiguous(input: &[u8]) -> Option<(u32, u32)> {
    let mut value: u32 = 0;
    let mut blanked: u32 = 0;
    for &b in input {
        match b {
            b'0'..=b'9' => value = value * 10 + (b - b'0') as u32,
            b' ' => {
                value *= 10;
                blanked += 1;
            }
            _ => return None,
        }
    }
    Some((value, blanked))
}

/// Parse an unsigned decimal (integer or fractional, e.g. `18`, `18.2`, `4.10`)
/// into an `f32`.
pub fn parse_float(input: &[u8]) -> Option<f32> {
    let (neg, rest) = match input.first() {
        Some(b'-') => (true, &input[1..]),
        Some(b'+') => (false, &input[1..]),
        _ => (false, input),
    };
    let mut whole: f32 = 0.0;
    let mut frac: f32 = 0.0;
    let mut frac_div: f32 = 1.0;
    let mut seen_dot = false;
    let mut any = false;
    for &b in rest {
        match b {
            b'0'..=b'9' => {
                any = true;
                let d = (b - b'0') as f32;
                if seen_dot {
                    frac_div *= 10.0;
                    frac = frac * 10.0 + d;
                } else {
                    whole = whole * 10.0 + d;
                }
            }
            b'.' => {
                if seen_dot {
                    return None;
                }
                seen_dot = true;
            }
            _ => return None,
        }
    }
    if any {
        let v = whole + frac / frac_div;
        Some(if neg { -v } else { v })
    } else {
        None
    }
}

/// Parse a degrees/minutes field of the form `DDMM.MM` (2 degree digits) or
/// `DDDMM.MM` (3 degree digits) into decimal degrees, tolerating trailing
/// spaces (position ambiguity).
///
/// `input` is the raw field bytes (including the decimal point); `deg_len`
/// selects 2 or 3 degree digits. Returns `(degrees, blanked_digits)`.
pub fn degrees_minutes(input: &[u8], deg_len: usize) -> Option<(f64, u8)> {
    let (deg, blank_deg) = digits_ambiguous(&input[..deg_len])?;
    let (min, blank_min) = digits_ambiguous(&input[deg_len..deg_len + 2])?;
    // Skip the '.', then the two fractional-minute digits.
    let (frac, blank_frac) = digits_ambiguous(&input[deg_len + 3..deg_len + 5])?;
    let value = deg as f64 + (min as f64 + frac as f64 / 100.0) / 60.0;
    Some((value, (blank_deg + blank_min + blank_frac) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_digits() {
        assert_eq!(digits(b"007"), Some(7));
        assert_eq!(digits(b""), None);
        assert_eq!(digits(b"12a"), None);
    }

    #[test]
    fn parses_degrees_minutes() {
        // 49 degrees 03.50 minutes = 49 + 3.5/60
        let (v, blank) = degrees_minutes(b"4903.50", 2).unwrap();
        assert!((v - 49.0583333).abs() < 1e-6, "got {v}");
        assert_eq!(blank, 0);
        // 72 degrees 01.75 minutes
        let (v, _) = degrees_minutes(b"07201.75", 3).unwrap();
        assert!((v - 72.0291666).abs() < 1e-6, "got {v}");
    }

    #[test]
    fn degrees_minutes_ambiguity() {
        // "3924.7 " -> 39 + 24.70/60, one blanked digit.
        let (v, blank) = degrees_minutes(b"3924.7 ", 2).unwrap();
        assert!((v - 39.4116666).abs() < 1e-6, "got {v}");
        assert_eq!(blank, 1);
    }
}
