//! APRS message encoding (`:` data type).

use aprs_core::Error;

/// Encode a `:` message into `out`, returning the number of bytes written.
///
/// `addressee` is the 9-character (space-padded) destination; `text` is the
/// message body; `id` is the optional `{id}` message identifier.
pub fn encode(
    addressee: &[u8; 9],
    text: &[u8],
    id: Option<&[u8]>,
    out: &mut [u8],
) -> Result<usize, Error> {
    let id_len = id.map_or(0, |i| 1 + i.len());
    let total = 1 + 9 + 1 + text.len() + id_len;
    if out.len() < total {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    out[pos] = b':';
    pos += 1;
    out[pos..pos + 9].copy_from_slice(addressee);
    pos += 9;
    out[pos] = b':';
    pos += 1;
    out[pos..pos + text.len()].copy_from_slice(text);
    pos += text.len();
    if let Some(id) = id {
        out[pos] = b'{';
        pos += 1;
        out[pos..pos + id.len()].copy_from_slice(id);
        pos += id.len();
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
    fn roundtrip_message() {
        let mut out = [0u8; 64];
        let addressee = *b"BLN1     ";
        let n = encode(&addressee, b"test message", Some(b"123"), &mut out).unwrap();
        assert_eq!(&out[..n], b":BLN1     :test message{123");

        with_decoded(&out[..n], |d| match d {
            Decoded::Message(m) => {
                assert_eq!(&m.addressee, b"BLN1     ");
                assert_eq!(m.text, b"test message");
                assert_eq!(m.id, Some(&b"123"[..]));
            }
            other => panic!("expected message, got {other:?}"),
        });
    }

    #[test]
    fn no_id() {
        let mut out = [0u8; 64];
        let addressee = *b"N0CALL   ";
        let n = encode(&addressee, b"hi", None, &mut out).unwrap();
        assert_eq!(&out[..n], b":N0CALL   :hi");
    }

    #[test]
    fn buffer_too_small() {
        let mut out = [0u8; 4];
        let addressee = *b"N0CALL   ";
        assert!(encode(&addressee, b"hello", None, &mut out).is_err());
    }
}
