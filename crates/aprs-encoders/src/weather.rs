//! Positionless weather report encoding (`_` data type, §12).

use aprs_core::Error;

use crate::format::write_padded;

/// Input for a positionless weather report.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeatherInput<'a> {
    /// Month/Day/Hour/Minute timestamp (8 bytes), optional.
    pub timestamp: Option<&'a [u8]>,
    /// Wind direction in degrees (0–360).
    pub wind_direction: Option<u16>,
    /// Sustained wind speed in mph.
    pub wind_speed_mph: Option<u16>,
    /// Wind gust in mph.
    pub gust_mph: Option<u16>,
    /// Temperature in degrees Fahrenheit.
    pub temperature_f: Option<i16>,
    /// Relative humidity in percent (0–100).
    pub humidity: Option<u8>,
    /// Barometric pressure in tenths of millibar.
    pub pressure_mbar_tenths: Option<u16>,
    /// Rain since midnight (hundredths of an inch).
    pub rain_hundredths_in: Option<u16>,
    /// Rain in the last hour (hundredths of an inch).
    pub rain_last_hour_hundredths_in: Option<u16>,
    /// Rain in the last 24 hours (hundredths of an inch).
    pub rain_24h_hundredths_in: Option<u16>,
}

/// Encode a positionless weather report (`_`) into `out`, returning bytes
/// written. Fields are emitted in the canonical tag order, omitting any that
/// are `None`.
pub fn encode(input: &WeatherInput<'_>, out: &mut [u8]) -> Result<usize, Error> {
    let mut pos = 0;
    out[pos] = b'_';
    pos += 1;

    if let Some(ts) = input.timestamp {
        if out.len() < pos + ts.len() {
            return Err(Error::BufferTooSmall);
        }
        out[pos..pos + ts.len()].copy_from_slice(ts);
        pos += ts.len();
    }

    let tag = |field: u8, digits: u32, width: usize, out: &mut [u8], pos: &mut usize| {
        if out.len() < *pos + 1 + width {
            return Err(Error::BufferTooSmall);
        }
        out[*pos] = field;
        *pos += 1;
        write_padded(digits, width, &mut out[*pos..*pos + width]);
        *pos += width;
        Ok(())
    };

    if let Some(d) = input.wind_direction {
        tag(b'c', d as u32, 3, out, &mut pos)?;
    }
    if let Some(s) = input.wind_speed_mph {
        tag(b's', s as u32, 3, out, &mut pos)?;
    }
    if let Some(g) = input.gust_mph {
        tag(b'g', g as u32, 3, out, &mut pos)?;
    }
    if let Some(t) = input.temperature_f {
        if out.len() < pos + 4 {
            return Err(Error::BufferTooSmall);
        }
        out[pos] = b't';
        pos += 1;
        if t < 0 {
            out[pos] = b'-';
            write_padded((-t) as u32, 2, &mut out[pos + 1..pos + 3]);
        } else {
            write_padded(t as u32, 3, &mut out[pos..pos + 3]);
        }
        pos += 3;
    }
    if let Some(r) = input.rain_last_hour_hundredths_in {
        tag(b'r', r as u32, 3, out, &mut pos)?;
    }
    if let Some(p) = input.rain_24h_hundredths_in {
        tag(b'p', p as u32, 3, out, &mut pos)?;
    }
    if let Some(p) = input.rain_hundredths_in {
        tag(b'P', p as u32, 3, out, &mut pos)?;
    }
    if let Some(h) = input.humidity {
        tag(b'h', h as u32, 2, out, &mut pos)?;
    }
    if let Some(b) = input.pressure_mbar_tenths {
        tag(b'b', b as u32, 5, out, &mut pos)?;
    }

    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded};

    fn with_decoded(info: &[u8], f: impl FnOnce(Decoded<'_>)) {
        let mut line = [0u8; 256];
        line[0] = b'A';
        line[1] = b'>';
        line[2] = b'B';
        line[3] = b':';
        let n = info.len();
        line[4..4 + n].copy_from_slice(info);
        let frame = tnc2::parse(&line[..4 + n]).unwrap();
        f(decode_packet(&frame.to_packet()).unwrap());
    }

    #[test]
    fn roundtrip_weather() {
        let mut ts = [0u8; 8];
        crate::format::timestamp_mdhm(10, 9, 5, 56, &mut ts).unwrap();

        let input = WeatherInput {
            timestamp: Some(&ts),
            wind_direction: Some(220),
            wind_speed_mph: Some(4),
            gust_mph: Some(5),
            temperature_f: Some(77),
            humidity: Some(50),
            pressure_mbar_tenths: Some(10135),
            rain_last_hour_hundredths_in: Some(0),
            rain_24h_hundredths_in: Some(0),
            rain_hundredths_in: Some(0),
        };

        let mut out = [0u8; 128];
        let n = encode(&input, &mut out).unwrap();
        assert_eq!(&out[..n], b"_10090556c220s004g005t077r000p000P000h50b10135");

        with_decoded(&out[..n], |d| match d {
            Decoded::Weather(w) => {
                assert_eq!(w.wind_direction, Some(220));
                assert_eq!(w.wind_speed_mph, Some(4));
                assert_eq!(w.temperature_f, Some(77));
                assert_eq!(w.humidity, Some(50));
                assert_eq!(w.pressure_mbar_tenths, Some(10135));
            }
            other => panic!("expected weather, got {other:?}"),
        });
    }

    #[test]
    fn negative_temperature() {
        let input = WeatherInput {
            temperature_f: Some(-5),
            humidity: Some(100),
            ..Default::default()
        };
        let mut out = [0u8; 64];
        let n = encode(&input, &mut out).unwrap();
        assert_eq!(&out[..n], b"_t-05h00");
    }
}
