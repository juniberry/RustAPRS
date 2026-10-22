//! Weather report decoding.
//!
//! Weather data appears either as a positionless report (`_` data type with an
//! optional timestamp) or appended to a position report's comment. This module
//! decodes the positionless form and the common tag set:
//!
//! | Tag | Field | Digits |
//! |-----|-------|--------|
//! | `c` | wind direction (deg)  | 3 |
//! | `s` | wind speed (mph)      | 3 |
//! | `g` | gust (mph)            | 3 |
//! | `t` | temperature (F, signed) | 3 |
//! | `h` | humidity (%)          | 2 |
//! | `b` | pressure (tenths mbar) | 5 |
//! | `r` | rain last hour (hundredths in) | 3 |
//! | `p` | rain last 24h (hundredths in)  | 3 |
//! | `P` | rain since midnight (hundredths in) | 3 |

use aprs_core::Error;

use crate::decoded::Weather;
use crate::parse::digits;
use crate::position::parse_mdhm;

/// Decode a positionless weather report (`_` or `*`) from the full information
/// field.
pub fn decode(info: &[u8]) -> Result<Weather, Error> {
    let dt = *info.first().ok_or(Error::Empty)?;
    if dt != b'_' {
        return Err(Error::UnknownDataType(dt));
    }

    let body = &info[1..];

    // Positionless weather carries an 8-character `MMDDHHMM` (MDHM) timestamp.
    let (timestamp, body) = match parse_mdhm(body) {
        Ok((ts, rest)) => (Some(ts), rest),
        Err(_) => (None, body),
    };

    let mut weather = Weather {
        timestamp,
        wind_direction: None,
        wind_speed_mph: None,
        gust_mph: None,
        temperature_f: None,
        humidity: None,
        pressure_mbar_tenths: None,
        rain_hundredths_in: None,
        rain_last_hour_hundredths_in: None,
        rain_24h_hundredths_in: None,
    };

    let mut i = 0;
    while i < body.len() {
        let (field_len, setter) = match body[i] {
            b'c' => (3, 1usize),
            b's' => (3, 2usize),
            b'g' => (3, 3usize),
            b't' => (3, 4usize),
            b'h' => (2, 5usize),
            b'b' => (5, 6usize),
            b'r' => (3, 7usize),
            b'p' => (3, 8usize),
            b'P' => (3, 9usize),
            _ => {
                i += 1;
                continue;
            }
        };

        let start = i + 1;
        let end = match start.checked_add(field_len) {
            Some(e) if e <= body.len() => e,
            _ => break,
        };
        let value = &body[start..end];

        match setter {
            1 => weather.wind_direction = parse_u16(value),
            2 => weather.wind_speed_mph = parse_u16(value),
            3 => weather.gust_mph = parse_u16(value),
            4 => weather.temperature_f = parse_signed3(value),
            5 => weather.humidity = parse_humidity(value),
            6 => weather.pressure_mbar_tenths = parse_u16(value),
            7 => weather.rain_last_hour_hundredths_in = parse_u16(value),
            8 => weather.rain_24h_hundredths_in = parse_u16(value),
            9 => weather.rain_hundredths_in = parse_u16(value),
            _ => unreachable!(),
        }

        i = end;
    }

    Ok(weather)
}

fn parse_u16(value: &[u8]) -> Option<u16> {
    u16::try_from(digits(value)?).ok()
}

/// Temperature is a 3-character signed value, e.g. `075`, `-05`, `+12`.
fn parse_signed3(value: &[u8]) -> Option<i16> {
    let (neg, digits) = match value.first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let magnitude = parse_u16(digits)? as i16;
    Some(if neg { -magnitude } else { magnitude })
}

/// Humidity is two digits (0–99); a value of `00` conventionally represents
/// 100%.
fn parse_humidity(value: &[u8]) -> Option<u8> {
    let v = parse_u16(value)? as u8;
    Some(if v == 0 { 100 } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_tags() {
        // month 10, day 09, 05:56 UTC, wind 220/004, gust 5, temp 77, humidity
        // 50, pressure 1013.5, no rain.
        let w = decode(b"_10090556c220s004g005t077r000p000P000h50b10135").unwrap();
        let ts = w.timestamp.unwrap();
        assert_eq!(
            ts,
            crate::decoded::Timestamp::MDHM {
                month: 10,
                day: 9,
                hour: 5,
                minute: 56,
            }
        );
        assert_eq!(w.wind_direction, Some(220));
        assert_eq!(w.wind_speed_mph, Some(4));
        assert_eq!(w.gust_mph, Some(5));
        assert_eq!(w.temperature_f, Some(77));
        assert_eq!(w.humidity, Some(50));
        assert_eq!(w.pressure_mbar_tenths, Some(10135));
        assert_eq!(w.rain_last_hour_hundredths_in, Some(0));
    }

    #[test]
    fn negative_temperature() {
        let w = decode(b"_10090556c000s000t-05h00").unwrap();
        assert_eq!(w.temperature_f, Some(-5));
        assert_eq!(w.humidity, Some(100));
    }

    #[test]
    fn missing_timestamp_is_ok() {
        let w = decode(b"_t075h50").unwrap();
        assert!(w.timestamp.is_none());
        assert_eq!(w.temperature_f, Some(75));
    }
}
