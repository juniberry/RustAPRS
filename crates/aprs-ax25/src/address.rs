//! AX.25 address field encoding/decoding.
//!
//! Each AX.25 address occupies seven octets: six callsign bytes with each
//! character shifted left one bit (so `'N'` → `0x9C`), followed by an SSID
//! octet. The SSID octet is laid out (bit 7 to bit 0) as:
//!
//! ```text
//!   C/H   R R   S S I D   E
//!    7    6 5   4 3 2 1   0
//! ```
//!
//! - Bit 7: the `C` bit (command/response, destination/source subfields) or the
//!   `H` bit ("has been repeated", repeater subfields).
//! - Bits 6–5: reserved, set to 1 per AX.25 2.2.
//! - Bits 4–1: the four-bit SSID (0–15).
//! - Bit 0: the HDLC address extension bit (1 on the last octet of the address
//!   field).

use aprs_core::address::{Address, CALLSIGN_LEN};

/// Number of octets in an encoded AX.25 address.
pub const ADDRESS_LEN: usize = CALLSIGN_LEN + 1;

/// Bit 7 of the SSID octet: command/response (destination/source subfields).
pub const C_BIT: u8 = 0x80;
/// Bit 7 of the SSID octet: "has been repeated" (repeater subfields).
pub const H_BIT: u8 = 0x80;
/// Bit 0 of the SSID octet: HDLC address extension bit (1 = last octet).
pub const EXTENSION_BIT: u8 = 0x01;
/// Reserved bits (6–5) of the SSID octet, set to 1 per AX.25 2.2.
const RESERVED_BITS: u8 = 0x60;

/// Encode an [`Address`] into its 7-octet wire representation.
///
/// `is_last` sets the address extension bit (bit 0), marking this as the final
/// octet of the address field.
pub fn encode_address(address: &Address, is_last: bool) -> [u8; ADDRESS_LEN] {
    let mut out = [0u8; ADDRESS_LEN];
    for (i, &c) in address.callsign.iter().enumerate() {
        out[i] = c << 1;
    }
    let mut ssid_octet = RESERVED_BITS | (address.ssid << 1);
    if is_last {
        ssid_octet |= EXTENSION_BIT;
    }
    out[CALLSIGN_LEN] = ssid_octet;
    out
}

/// Decode a 7-octet address field, returning the [`Address`] and whether the
/// address extension bit (last address) was set.
pub fn decode_address(bytes: &[u8; ADDRESS_LEN]) -> Option<(Address, bool)> {
    let mut callsign = [b' '; CALLSIGN_LEN];
    for (i, &octet) in bytes[..CALLSIGN_LEN].iter().enumerate() {
        // Each callsign byte stores the character in the upper seven bits.
        if octet & 0x01 != 0 {
            return None; // extension bit unexpectedly set in a callsign octet
        }
        callsign[i] = octet >> 1;
    }
    let ssid_octet = bytes[CALLSIGN_LEN];
    let ssid = (ssid_octet >> 1) & 0x0F;
    let is_last = ssid_octet & EXTENSION_BIT != 0;
    let address = Address { callsign, ssid };
    Some((address, is_last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_known_example() {
        // "N0CALL" (SSID 0) as the last address: extension bit set, SSID in
        // bits 4–1, reserved bits (6–5) = 1.
        let addr = Address::parse(b"N0CALL").unwrap();
        let encoded = encode_address(&addr, true);
        assert_eq!(encoded, [0x9C, 0x60, 0x86, 0x82, 0x98, 0x98, 0x61]);
    }

    #[test]
    fn encode_non_last_has_no_extension_bit() {
        let addr = Address::parse(b"N0CALL").unwrap();
        let encoded = encode_address(&addr, false);
        assert_eq!(encoded[6], 0x60); // reserved (6–5) = 1, SSID = 0, ext = 0
    }

    #[test]
    fn encode_decode_roundtrip() {
        for ssid in [0u8, 1, 9, 15] {
            let addr = Address::new(b"K7ABC", ssid).unwrap();
            for last in [false, true] {
                let encoded = encode_address(&addr, last);
                let (decoded, is_last) = decode_address(&encoded).unwrap();
                assert_eq!(decoded, addr);
                assert_eq!(is_last, last);
            }
        }
    }

    #[test]
    fn decodes_spec_ssid_octet() {
        // Spec §3.12.2: A14 = 0x61 = C/H=0, RR=11, SSID=0000, extension=1.
        let mut bytes = [0x40u8; ADDRESS_LEN]; // ' ' << 1
        bytes[0] = 0x9C; // N
        bytes[1] = 0x60; // 0
        bytes[2] = 0x86; // C
        bytes[3] = 0x82; // A
        bytes[4] = 0x98; // L
        bytes[5] = 0x98; // L
        bytes[6] = 0x61;
        let (addr, is_last) = decode_address(&bytes).unwrap();
        assert_eq!(addr, Address::parse(b"N0CALL").unwrap());
        assert!(is_last);
    }

    #[test]
    fn rejects_set_extension_bit() {
        let mut bytes = [0x40u8; ADDRESS_LEN]; // ' ' << 1
        bytes[0] = 0x9D; // 'N'<<1 with LSB set
        assert!(decode_address(&bytes).is_none());
    }
}
