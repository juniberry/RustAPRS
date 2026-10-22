//! AX.25 frame check sequence (FCS).
//!
//! AX.25 uses the CCITT CRC-16 (polynomial `0x1021`, initial value `0xFFFF`,
//! no bit reflection, no final XOR — commonly known as CRC-16/CCITT-FALSE).
//! The FCS is computed over the addresses, control, PID, and information bytes,
//! and is transmitted least-significant byte first.

/// CRC-16/CCITT polynomial.
const POLY: u16 = 0x1021;

/// Compute the running FCS over `data`, starting from `seed`.
pub fn fcs_update(seed: u16, data: &[u8]) -> u16 {
    let mut crc = seed;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Compute the FCS of `data` (initial value `0xFFFF`).
pub fn fcs(data: &[u8]) -> u16 {
    fcs_update(0xFFFF, data)
}

/// Check a complete frame, whose final two bytes are the FCS in AX.25
/// (little-endian) byte order.
pub fn fcs_valid(frame: &[u8]) -> bool {
    if frame.len() < 2 {
        return false;
    }
    let (data, fcs_bytes) = frame.split_at(frame.len() - 2);
    let stored = u16::from_le_bytes([fcs_bytes[0], fcs_bytes[1]]);
    fcs(data) == stored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_check_value() {
        // CRC-16/CCITT-FALSE check value for "123456789" is 0x29B1.
        assert_eq!(fcs(b"123456789"), 0x29B1);
    }

    #[test]
    fn valid_frame_roundtrip() {
        let frame = b"test frame";
        let crc = fcs(frame);
        let mut full = [0u8; 12];
        full[..10].copy_from_slice(frame);
        full[10..12].copy_from_slice(&crc.to_le_bytes());
        assert!(fcs_valid(&full));
    }

    #[test]
    fn corrupted_frame_fails() {
        let frame = b"test frame";
        let crc = fcs(frame);
        let mut full = [0u8; 12];
        full[..10].copy_from_slice(frame);
        full[10..12].copy_from_slice(&crc.to_le_bytes());
        full[2] ^= 0x01;
        assert!(!fcs_valid(&full));
    }
}
