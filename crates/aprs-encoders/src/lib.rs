//! `aprs-encoders` — allocation-free APRS payload encoders.
//!
//! Encoders are pure functions that write into a caller-provided byte buffer
//! and return the number of bytes written. They depend only on [`aprs_core`]
//! and `core`, making them suitable for embedded targets.
//!
//! The encoders mirror [`aprs_decoders`]: every encoder is exercised against
//! the corresponding decoder in a round-trip test.

#![no_std]
#![forbid(unsafe_code)]

pub mod format;
pub mod message;
pub mod mice;
pub mod object;
pub mod position;
pub mod status;
pub mod telemetry;
pub mod weather;

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded, PositionEncoding};

    #[test]
    fn position_roundtrip() {
        let mut out = [0u8; 64];
        let n = position::encode_uncompressed(false, 49.0583, -72.0292, b'/', b'-', b"", &mut out)
            .unwrap();

        let mut line = [0u8; 256];
        line[0] = b'A';
        line[1] = b'>';
        line[2] = b'B';
        line[3] = b':';
        line[4..4 + n].copy_from_slice(&out[..n]);
        let frame = tnc2::parse(&line[..4 + n]).unwrap();
        match decode_packet(&frame.to_packet()).unwrap() {
            Decoded::Position(p) => {
                assert_eq!(p.encoding, PositionEncoding::Uncompressed);
                assert!((p.coordinates.latitude - 49.0583).abs() < 1e-4);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
