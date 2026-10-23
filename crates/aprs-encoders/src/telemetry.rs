//! Telemetry report encoding (`T#...`).

use aprs_core::Error;

use crate::format::write_padded;

/// Encode a `T#` telemetry report into `out`, returning bytes written.
///
/// `sequence` is a 3-digit sequence number; `analog` holds the five analog
/// values (0–999); `digital` packs the eight binary channel bits.
pub fn encode(
    sequence: u32,
    analog: &[u16; 5],
    digital: u8,
    out: &mut [u8],
) -> Result<usize, Error> {
    if sequence > 999 {
        return Err(Error::InvalidNumber);
    }

    // `T#NNN,AAA,AAA,AAA,AAA,AAA,BBBBBBBB`
    const TOTAL: usize = 2 + 3 + 1 + (3 + 1) * 5 + 8;
    if out.len() < TOTAL {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    out[pos] = b'T';
    out[pos + 1] = b'#';
    pos += 2;

    write_padded(sequence, 3, &mut out[pos..pos + 3]);
    pos += 3;

    for a in analog {
        out[pos] = b',';
        pos += 1;
        write_padded(*a as u32, 3, &mut out[pos..pos + 3]);
        pos += 3;
    }

    out[pos] = b',';
    pos += 1;
    for bit in (0..8).rev() {
        out[pos] = if digital & (1 << bit) != 0 {
            b'1'
        } else {
            b'0'
        };
        pos += 1;
    }

    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded, TelemetrySequence};

    #[test]
    fn roundtrip_telemetry() {
        let mut out = [0u8; 64];
        let n = encode(5, &[199, 0, 255, 73, 123], 0b0110_1001, &mut out).unwrap();
        assert_eq!(&out[..n], b"T#005,199,000,255,073,123,01101001");

        let mut line = [0u8; 256];
        line[0] = b'A';
        line[1] = b'>';
        line[2] = b'B';
        line[3] = b':';
        line[4..4 + n].copy_from_slice(&out[..n]);
        let frame = tnc2::parse(&line[..4 + n]).unwrap();
        match decode_packet(&frame.to_packet()).unwrap() {
            Decoded::Telemetry(t) => {
                assert_eq!(t.sequence, TelemetrySequence::Number(5));
                assert_eq!(t.analog, [199.0, 0.0, 255.0, 73.0, 123.0]);
                assert_eq!(t.digital, 0b0110_1001);
            }
            other => panic!("expected telemetry, got {other:?}"),
        }
    }
}
