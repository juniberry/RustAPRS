//! AX.25 address field encoding/decoding.
//!
//! Each AX.25 address occupies seven octets: six callsign bytes with each
//! character shifted left one bit (so `'N'` → `0x9C`), followed by an SSID
//! octet. The SSID octet packs the SSID into bits 0–3, two reserved bits
//! (set to 1) in bits 4–5, the command/response `C` bit in bit 6, and the
//! `H` bit (1 = last address in the field) in bit 7.
//!
//! Per the AX.25 2.2 specification, the reserved bits are set to 1, giving a
//! base of `0x30` for the SSID octet.

use aprs_core::address::{Address, CALLSIGN_LEN};

/// Number of octets in an encoded AX.25 address.
pub const ADDRESS_LEN: usize = CALLSIGN_LEN + 1;

/// Bit 6 of the SSID octet: command/response flag.
pub const C_BIT: u8 = 0x40;
/// Bit 7 of the SSID octet: "has been repeated" / last-address flag.
pub const H_BIT: u8 = 0x80;

/// Encode an [`Address`] into its 7-octet wire representation.
///
/// `is_last` sets the `H` bit, marking this as the final address in the field.
pub fn encode_address(address: &Address, is_last: bool) -> [u8; ADDRESS_LEN] {
    let mut out = [0u8; ADDRESS_LEN];
    for (i, &c) in address.callsign.iter().enumerate() {
        out[i] = c << 1;
    }
    let mut ssid_octet = 0x30 | address.ssid;
    if is_last {
        ssid_octet |= H_BIT;
    }
    out[CALLSIGN_LEN] = ssid_octet;
    out
}

/// Decode a 7-octet address field, returning the [`Address`] and whether the
/// `H` bit (last address) was set.
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
    let ssid = ssid_octet & 0x0F;
    let is_last = ssid_octet & H_BIT != 0;
    let address = Address { callsign, ssid };
    Some((address, is_last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_known_example() {
        // "N0CALL" as the destination (last address): a well-known AX.25
        // encoding example.
        let addr = Address::parse(b"N0CALL").unwrap();
        let encoded = encode_address(&addr, true);
        assert_eq!(encoded, [0x9C, 0x60, 0x86, 0x82, 0x98, 0x98, 0xB0]);
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
    fn rejects_set_extension_bit() {
        let mut bytes = [0x40u8; ADDRESS_LEN]; // ' ' << 1
        bytes[0] = 0x9D; // 'N'<<1 with LSB set
        assert!(decode_address(&bytes).is_none());
    }
}
