//! KISS framing.
//!
//! KISS is a minimal serial encapsulation that carries an AX.25 frame between a
//! host and a TNC. A KISS frame is:
//!
//! ```text
//! FEND  COMMAND  [DATA]  FEND
//! ```
//!
//! where `FEND` (`0xC0`) delimits the frame, `COMMAND` selects the port (high
//! nibble) and command (low nibble, `0` = data), and `DATA` is the AX.25 frame.
//! The two reserved bytes are escaped: `0xC0` → `0xDB 0xDC`, `0xDB` → `0xDB
//! 0xDD`.

use aprs_core::Error;

/// Frame end marker.
pub const FEND: u8 = 0xC0;
/// Frame escape marker.
pub const FESC: u8 = 0xDB;
/// Transposed frame end (escaped `FEND`).
pub const TFEND: u8 = 0xDC;
/// Transposed frame escape (escaped `FESC`).
pub const TFESC: u8 = 0xDD;
/// Data-frame command for port 0.
pub const CMD_DATA: u8 = 0x00;
/// Return command: exit KISS and return control to the host/TNC's higher-level
/// program (e.g. the normal command interpreter). Per the KISS specification,
/// `FF` is sent as a frame of its own: `FEND 0xFF FEND`. Only meaningful for
/// TNCs that incorporate KISS alongside other firmware (e.g. Kantronics,
/// AEA); not used by dedicated KISS-only TNCs.
pub const CMD_RETURN: u8 = 0xFF;

/// Encode a KISS frame into `out`, returning the number of bytes written.
///
/// `command` is the KISS command byte (port in the high nibble); `data` is the
/// payload (an AX.25 frame).
pub fn encode(command: u8, data: &[u8], out: &mut [u8]) -> Result<usize, Error> {
    // Worst case: FEND + command + 2 * data + FEND.
    if out.len() < 2 + 1 + 2 * data.len() {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    out[pos] = FEND;
    pos += 1;
    out[pos] = command;
    pos += 1;
    for &b in data {
        match b {
            FEND => {
                out[pos] = FESC;
                out[pos + 1] = TFEND;
                pos += 2;
            }
            FESC => {
                out[pos] = FESC;
                out[pos + 1] = TFESC;
                pos += 2;
            }
            _ => {
                out[pos] = b;
                pos += 1;
            }
        }
    }
    out[pos] = FEND;
    pos += 1;

    Ok(pos)
}

/// Decode a KISS payload (`COMMAND DATA`, without the `FEND` delimiters),
/// unescaping into `out` and returning `(command, data_len)`.
pub fn decode_payload(payload: &[u8], out: &mut [u8]) -> Result<(u8, usize), Error> {
    if payload.is_empty() {
        return Err(Error::Truncated);
    }

    let command = payload[0];
    let mut i = 1;
    let mut o = 0;

    while i < payload.len() {
        let b = payload[i];
        i += 1;

        match b {
            FESC => {
                if i >= payload.len() {
                    return Err(Error::Truncated);
                }
                let escaped = payload[i];
                i += 1;
                let unescaped = match escaped {
                    TFEND => FEND,
                    TFESC => FESC,
                    _ => return Err(Error::InvalidCharacter),
                };
                if out.len() < o + 1 {
                    return Err(Error::BufferTooSmall);
                }
                out[o] = unescaped;
                o += 1;
            }
            _ => {
                if out.len() < o + 1 {
                    return Err(Error::BufferTooSmall);
                }
                out[o] = b;
                o += 1;
            }
        }
    }

    Ok((command, o))
}

/// Decode a single KISS frame from `bytes` (with leading/trailing `FEND`),
/// unescaping into `out` and returning `(command, data_len)`.
pub fn decode(bytes: &[u8], out: &mut [u8]) -> Result<(u8, usize), Error> {
    if bytes.len() < 2 || bytes[0] != FEND {
        return Err(Error::InvalidCharacter);
    }

    let end = bytes[1..]
        .iter()
        .position(|&b| b == FEND)
        .map(|p| p + 1)
        .ok_or(Error::Truncated)?;

    decode_payload(&bytes[1..end], out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_plain() {
        let mut enc = [0u8; 64];
        let n = encode(CMD_DATA, b"hello", &mut enc).unwrap();
        assert_eq!(&enc[..n], b"\xC0\x00hello\xC0");

        let mut dec = [0u8; 64];
        let (cmd, len) = decode(&enc[..n], &mut dec).unwrap();
        assert_eq!(cmd, CMD_DATA);
        assert_eq!(&dec[..len], b"hello");
    }

    #[test]
    fn roundtrip_with_escaping() {
        // Data containing FEND and FESC must be escaped and recovered.
        let data = [0xC0, 0xDB, 0x01, 0x02];
        let mut enc = [0u8; 64];
        let n = encode(CMD_DATA, &data, &mut enc).unwrap();

        let mut dec = [0u8; 64];
        let (_, len) = decode(&enc[..n], &mut dec).unwrap();
        assert_eq!(&dec[..len], &data);
    }

    #[test]
    fn missing_fend_is_truncated() {
        let mut dec = [0u8; 64];
        assert_eq!(decode(b"\xC0\x00abc", &mut dec), Err(Error::Truncated));
    }

    #[test]
    fn bad_escape_is_invalid() {
        let mut dec = [0u8; 64];
        assert_eq!(
            decode(b"\xC0\x00\xDB\x01\xC0", &mut dec),
            Err(Error::InvalidCharacter)
        );
    }
}
