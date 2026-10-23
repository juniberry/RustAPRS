//! Status report encoding (`>` data type).

use aprs_core::Error;

/// Encode a plain `>` status report into `out`, returning bytes written.
pub fn encode(text: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    let total = 1 + text.len();
    if out.len() < total {
        return Err(Error::BufferTooSmall);
    }
    out[0] = b'>';
    out[1..1 + text.len()].copy_from_slice(text);
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aprs_core::tnc2;
    use aprs_decoders::{decode_packet, Decoded};

    #[test]
    fn roundtrip_status() {
        let mut out = [0u8; 64];
        let n = encode(b"Hello world", &mut out).unwrap();
        assert_eq!(&out[..n], b">Hello world");

        let mut line = [0u8; 256];
        line[0] = b'A';
        line[1] = b'>';
        line[2] = b'B';
        line[3] = b':';
        line[4..4 + n].copy_from_slice(&out[..n]);
        let frame = tnc2::parse(&line[..4 + n]).unwrap();
        match decode_packet(&frame.to_packet()).unwrap() {
            Decoded::Status(s) => {
                assert!(s.locator.is_none());
                assert_eq!(s.text, b"Hello world");
            }
            other => panic!("expected status, got {other:?}"),
        }
    }
}
