//! Telemetry (`T#...`) decoding.

use aprs_core::Error;

use crate::decoded::{Telemetry, TelemetrySequence};
use crate::parse::{digits, parse_float};

/// Decode a `T#` telemetry report from the full information field.
pub fn decode(info: &[u8]) -> Result<Telemetry<'_>, Error> {
    if info.get(..2) != Some(b"T#") {
        return Err(Error::UnknownDataType(info[0]));
    }

    let body = &info[2..];

    // Sequence number: a decimal number, or the literal `MIC` marker. `MIC` may
    // be followed by a comma or directly by the first analog value.
    let (sequence, body) = if body.starts_with(b"MIC") {
        let rest = &body[3..];
        let rest = match rest.first() {
            Some(b',') => &rest[1..],
            _ => rest,
        };
        (TelemetrySequence::Mic, rest)
    } else {
        let (seq, rest) = split_comma(body).ok_or(Error::Truncated)?;
        (
            TelemetrySequence::Number(digits(seq).ok_or(Error::InvalidNumber)?),
            rest,
        )
    };

    // Five analog channels, comma separated.
    let mut analog = [0.0f32; 5];
    let mut body = body;
    for slot in analog.iter_mut() {
        let (field, rest) = split_comma(body).ok_or(Error::Truncated)?;
        *slot = parse_float(field).ok_or(Error::InvalidNumber)?;
        body = rest;
    }

    // Digital field (eight binary digits), optionally followed by a comment.
    let (digital, comment) = if body.len() >= 8 && body[..8].iter().all(|&b| b == b'0' || b == b'1')
    {
        let bits = body[..8]
            .iter()
            .fold(0u8, |acc, &b| (acc << 1) | (b - b'0'));
        let comment = match body.get(8) {
            Some(b',') => &body[9..],
            Some(_) => &body[8..],
            None => &[],
        };
        (bits, comment)
    } else {
        // Rare decimal digital value (no comment).
        (digits(body).ok_or(Error::InvalidNumber)? as u8, &[][..])
    };

    Ok(Telemetry {
        sequence,
        analog,
        digital,
        comment,
    })
}

fn split_comma(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let pos = input.iter().position(|&b| b == b',')?;
    Some((&input[..pos], &input[pos + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_telemetry() {
        let t = decode(b"T#005,199,000,255,073,123,01101001").unwrap();
        assert_eq!(t.sequence, TelemetrySequence::Number(5));
        assert_eq!(t.analog, [199.0, 0.0, 255.0, 73.0, 123.0]);
        assert_eq!(t.digital, 0b0110_1001);
    }

    #[test]
    fn parses_decimal_telemetry() {
        let t = decode(b"T#196,18.2,4.1,84.0,12.0,15.0,00000000").unwrap();
        assert_eq!(t.sequence, TelemetrySequence::Number(196));
        assert_eq!(t.analog[0], 18.2);
        assert_eq!(t.analog[1], 4.1);
    }

    #[test]
    fn parses_mic_sequence() {
        let t = decode(b"T#MIC,1,2,3,4,5,00000000").unwrap();
        assert_eq!(t.sequence, TelemetrySequence::Mic);
    }

    #[test]
    fn parses_mic_sequence_no_comma() {
        let t = decode(b"T#MIC199,000,255,073,123,01101001").unwrap();
        assert_eq!(t.sequence, TelemetrySequence::Mic);
        assert_eq!(t.analog[0], 199.0);
        assert_eq!(t.digital, 0b0110_1001);
    }

    #[test]
    fn parses_trailing_comment() {
        let t = decode(b"T#001,1,16,0,43,13.60,00000000,@ 18 39 20 22 10 2026").unwrap();
        assert_eq!(t.sequence, TelemetrySequence::Number(1));
        assert_eq!(t.analog[4], 13.60);
        assert_eq!(t.digital, 0);
        assert_eq!(t.comment, b"@ 18 39 20 22 10 2026");
    }

    #[test]
    fn rejects_non_telemetry() {
        assert!(decode(b"!4903.50N/07201.75W-").is_err());
    }
}
