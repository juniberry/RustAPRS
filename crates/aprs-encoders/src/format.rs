//! Fixed-width numeric and coordinate formatting helpers.

use aprs_core::Error;

/// Format an absolute coordinate value as `DDMM.MM` (`deg_digits == 2`) or
/// `DDDMM.MM` (`deg_digits == 3`), writing into `out` and returning the number
/// of bytes written. The sign is not included (the caller adds `N`/`S`/`E`/`W`).
pub fn ddmm(value: f64, deg_digits: usize, out: &mut [u8]) -> Result<usize, Error> {
    // Total hundredths of a minute: `abs(deg) * 60 * 100`, rounded to nearest
    // (the `+ 0.5` avoids the `libm`-only `f64::round`).
    let total = (value.abs() * 6000.0 + 0.5) as u32;
    let degrees = total / 6000;
    let rem = total % 6000;
    let minutes = rem / 100;
    let hundredths = rem % 100;

    // Reject a degree count that cannot fit the requested width.
    let max_deg = 10u32.pow(deg_digits as u32);
    if degrees >= max_deg {
        return Err(Error::InvalidCoordinate);
    }

    let need = deg_digits + 2 + 1 + 2;
    if out.len() < need {
        return Err(Error::BufferTooSmall);
    }

    write_padded(degrees, deg_digits, &mut out[..deg_digits]);
    write_padded(minutes, 2, &mut out[deg_digits..deg_digits + 2]);
    out[deg_digits + 2] = b'.';
    write_padded(hundredths, 2, &mut out[deg_digits + 3..deg_digits + 5]);
    Ok(need)
}

/// Write `value` as a zero-padded decimal of exactly `width` digits into `out`.
pub fn write_padded(mut value: u32, width: usize, out: &mut [u8]) {
    for i in (0..width).rev() {
        out[i] = (value % 10) as u8 + b'0';
        value /= 10;
    }
}

/// Format a Day/Hour/Minute timestamp (`DDHHMMz` for UTC or `DDHHMM/` local).
pub fn timestamp_dhm(
    day: u8,
    hour: u8,
    minute: u8,
    utc: bool,
    out: &mut [u8],
) -> Result<usize, Error> {
    if out.len() < 7 {
        return Err(Error::BufferTooSmall);
    }
    write_padded(day as u32, 2, &mut out[0..2]);
    write_padded(hour as u32, 2, &mut out[2..4]);
    write_padded(minute as u32, 2, &mut out[4..6]);
    out[6] = if utc { b'z' } else { b'/' };
    Ok(7)
}

/// Format an Hour/Minute/Second timestamp (`HHMMSSh`).
pub fn timestamp_hms(hour: u8, minute: u8, second: u8, out: &mut [u8]) -> Result<usize, Error> {
    if out.len() < 7 {
        return Err(Error::BufferTooSmall);
    }
    write_padded(hour as u32, 2, &mut out[0..2]);
    write_padded(minute as u32, 2, &mut out[2..4]);
    write_padded(second as u32, 2, &mut out[4..6]);
    out[6] = b'h';
    Ok(7)
}

/// Format a Month/Day/Hour/Minute timestamp (8 digits, no suffix).
pub fn timestamp_mdhm(
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    out: &mut [u8],
) -> Result<usize, Error> {
    if out.len() < 8 {
        return Err(Error::BufferTooSmall);
    }
    write_padded(month as u32, 2, &mut out[0..2]);
    write_padded(day as u32, 2, &mut out[2..4]);
    write_padded(hour as u32, 2, &mut out[4..6]);
    write_padded(minute as u32, 2, &mut out[6..8]);
    Ok(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_latitude() {
        let mut out = [0u8; 8];
        let n = ddmm(49.0583333, 2, &mut out).unwrap();
        assert_eq!(&out[..n], b"4903.50");
    }

    #[test]
    fn formats_longitude() {
        let mut out = [0u8; 8];
        let n = ddmm(72.0291666, 3, &mut out).unwrap();
        assert_eq!(&out[..n], b"07201.75");
    }

    #[test]
    fn rounds_hundredths() {
        let mut out = [0u8; 8];
        // 49.0583333... -> 4903.50
        let n = ddmm(49.058333333, 2, &mut out).unwrap();
        assert_eq!(&out[..n], b"4903.50");
    }

    #[test]
    fn formats_timestamps() {
        let mut out = [0u8; 8];
        assert_eq!(timestamp_dhm(9, 23, 45, true, &mut out).unwrap(), 7);
        assert_eq!(&out[..7], b"092345z");

        assert_eq!(timestamp_hms(23, 45, 17, &mut out).unwrap(), 7);
        assert_eq!(&out[..7], b"234517h");

        assert_eq!(timestamp_mdhm(10, 9, 23, 45, &mut out).unwrap(), 8);
        assert_eq!(&out[..8], b"10092345");
    }
}
