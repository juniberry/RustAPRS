//! AX.25 UI-frame assembly and disassembly.
//!
//! A complete frame (excluding the `0x7E` flags and bit-stuffing, which belong
//! to the modem/KISS layer) has the shape:
//!
//! ```text
//! DST(7)  SRC(7)  [DIGI(7)]*  CTRL(0x03)  PID(0xF0)  INFO...  FCS(2)
//! ```
//!
//! The `H` bit of each 7-octet address marks the final address in the field.
//! The two-octet FCS (CRC-16/CCITT-FALSE) is transmitted least-significant
//! byte first.

use aprs_core::address::Address;
use aprs_core::Error;

use crate::address::{decode_address, encode_address, ADDRESS_LEN, C_BIT};
use crate::fcs::{fcs, fcs_valid};

/// Maximum number of digipeaters permitted by AX.25.
pub const MAX_DIGIPEATERS: usize = 8;

/// Control field value for UI (unnumbered information) frames.
pub const CONTROL_UI: u8 = 0x03;
/// Protocol ID for "no layer 3 protocol" (APRS).
pub const PID_NO_L3: u8 = 0xF0;

/// A decoded/encodable AX.25 UI-frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    pub destination: Address,
    pub source: Address,
    /// Digipeater addresses; `digi_count` entries are valid.
    pub digipeaters: [Address; MAX_DIGIPEATERS],
    pub digi_count: usize,
    /// The information field (APRS payload).
    pub info: &'a [u8],
}

impl<'a> Frame<'a> {
    /// The valid digipeater slice.
    pub fn digipeaters(&self) -> &[Address] {
        &self.digipeaters[..self.digi_count]
    }
}

/// Encode a [`Frame`] into `out`, returning the number of bytes written.
pub fn encode_frame(frame: &Frame<'_>, out: &mut [u8]) -> Result<usize, Error> {
    if frame.digi_count > MAX_DIGIPEATERS {
        return Err(Error::InvalidCharacter);
    }

    let size = 7 + 7 + 7 * frame.digi_count + 2 + frame.info.len() + 2;
    if out.len() < size {
        return Err(Error::BufferTooSmall);
    }

    let mut pos = 0;
    // Destination: a UI frame is a command, so the C bit is set.
    let mut dest = encode_address(&frame.destination, false);
    dest[ADDRESS_LEN - 1] |= C_BIT;
    out[pos..pos + ADDRESS_LEN].copy_from_slice(&dest);
    pos += ADDRESS_LEN;
    out[pos..pos + ADDRESS_LEN]
        .copy_from_slice(&encode_address(&frame.source, frame.digi_count == 0));
    pos += ADDRESS_LEN;

    for i in 0..frame.digi_count {
        let is_last = i == frame.digi_count - 1;
        out[pos..pos + ADDRESS_LEN]
            .copy_from_slice(&encode_address(&frame.digipeaters[i], is_last));
        pos += ADDRESS_LEN;
    }

    out[pos] = CONTROL_UI;
    out[pos + 1] = PID_NO_L3;
    pos += 2;

    out[pos..pos + frame.info.len()].copy_from_slice(frame.info);
    pos += frame.info.len();

    let crc = fcs(&out[..pos]);
    out[pos..pos + 2].copy_from_slice(&crc.to_le_bytes());
    pos += 2;

    Ok(pos)
}

/// Decode a complete frame from `bytes`, verifying the FCS.
pub fn decode_frame(bytes: &[u8]) -> Result<Frame<'_>, Error> {
    // Minimum: dest(7) + source(7) + control(1) + pid(1) + fcs(2).
    if bytes.len() < 18 {
        return Err(Error::Truncated);
    }
    if !fcs_valid(bytes) {
        return Err(Error::InvalidFcs);
    }

    let mut pos = 0;
    let (destination, _) = decode_address(addr(bytes, pos)?).ok_or(Error::InvalidCharacter)?;
    pos += ADDRESS_LEN;
    let (source, mut last) = decode_address(addr(bytes, pos)?).ok_or(Error::InvalidCharacter)?;
    pos += ADDRESS_LEN;

    let mut digipeaters = [Address::default(); MAX_DIGIPEATERS];
    let mut digi_count = 0;
    while !last && digi_count < MAX_DIGIPEATERS {
        let (digi, is_last) = decode_address(addr(bytes, pos)?).ok_or(Error::InvalidCharacter)?;
        digipeaters[digi_count] = digi;
        digi_count += 1;
        pos += ADDRESS_LEN;
        last = is_last;
    }

    // Control + PID (APRS always uses UI / no-layer-3).
    if bytes.get(pos) != Some(&CONTROL_UI) || bytes.get(pos + 1) != Some(&PID_NO_L3) {
        return Err(Error::InvalidCharacter);
    }
    pos += 2;

    let info = &bytes[pos..bytes.len() - 2];

    Ok(Frame {
        destination,
        source,
        digipeaters,
        digi_count,
        info,
    })
}

fn addr(bytes: &[u8], pos: usize) -> Result<&[u8; ADDRESS_LEN], Error> {
    bytes
        .get(pos..pos + ADDRESS_LEN)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(callsign: &[u8], ssid: u8) -> Address {
        Address::new(callsign, ssid).unwrap()
    }

    #[test]
    fn roundtrip_no_digipeaters() {
        let frame = Frame {
            destination: addr(b"APRS", 0),
            source: addr(b"N0CALL", 9),
            digipeaters: [Address::default(); MAX_DIGIPEATERS],
            digi_count: 0,
            info: b"!4903.50N/07201.75W-",
        };

        let mut out = [0u8; 128];
        let n = encode_frame(&frame, &mut out).unwrap();

        let decoded = decode_frame(&out[..n]).unwrap();
        assert_eq!(decoded.destination, frame.destination);
        assert_eq!(decoded.source, frame.source);
        assert_eq!(decoded.digi_count, 0);
        assert_eq!(decoded.info, frame.info);
    }

    #[test]
    fn roundtrip_with_digipeaters() {
        let mut digis = [Address::default(); MAX_DIGIPEATERS];
        digis[0] = addr(b"WIDE1", 1);
        digis[1] = addr(b"WIDE2", 1);
        let frame = Frame {
            destination: addr(b"APRS", 0),
            source: addr(b"N0CALL", 0),
            digipeaters: digis,
            digi_count: 2,
            info: b":BLN1     :test",
        };

        let mut out = [0u8; 128];
        let n = encode_frame(&frame, &mut out).unwrap();

        let decoded = decode_frame(&out[..n]).unwrap();
        assert_eq!(decoded.digi_count, 2);
        assert_eq!(decoded.digipeaters()[0], addr(b"WIDE1", 1));
        assert_eq!(decoded.digipeaters()[1], addr(b"WIDE2", 1));
        assert_eq!(decoded.info, frame.info);
    }

    #[test]
    fn rejects_bad_fcs() {
        let frame = Frame {
            destination: addr(b"APRS", 0),
            source: addr(b"N0CALL", 0),
            digipeaters: [Address::default(); MAX_DIGIPEATERS],
            digi_count: 0,
            info: b"test",
        };
        let mut out = [0u8; 128];
        let n = encode_frame(&frame, &mut out).unwrap();
        out[0] ^= 0x01;
        assert_eq!(decode_frame(&out[..n]), Err(Error::InvalidFcs));
    }

    #[test]
    fn rejects_too_short() {
        assert_eq!(decode_frame(b"short"), Err(Error::Truncated));
    }
}
