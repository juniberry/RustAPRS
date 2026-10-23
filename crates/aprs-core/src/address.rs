//! AX.25 address representation (strict RF form).
//!
//! An AX.25 address is a six-character callsign plus a 4-bit numeric SSID in
//! the range `0..=15`. The callsign is space-padded on the right and, on the
//! wire, each character is left-shifted one bit. The SSID is a separate 4-bit
//! field, *not* stored as ASCII digits.
//!
//! This library targets native RF/AX.25. It does not model the more permissive
//! APRS-IS identifier rules (3–9 character totals, 1–2 character alphanumeric
//! SSIDs, gateway prefixes such as `ER-`/`IRLP-`). Such identifiers cannot be
//! represented in a native AX.25 address and, on the air, must instead travel
//! via third-party encapsulation.

use core::fmt;

/// Maximum length of a callsign (always six characters, space padded).
pub const CALLSIGN_LEN: usize = 6;

/// An AX.25 address: a six-character callsign plus a 4-bit numeric SSID.
///
/// The callsign is stored as six uppercase ASCII bytes, space padded on the
/// right. This mirrors the on-air AX.25 encoding and keeps the type trivially
/// copyable for embedded use.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    pub callsign: [u8; CALLSIGN_LEN],
    /// Numeric SSID in the range `0..=15`.
    pub ssid: u8,
}

impl Address {
    /// Create an address from a callsign and a numeric SSID (`0..=15`).
    ///
    /// The callsign is uppercased and space-padded to six characters. Returns
    /// `None` if the SSID exceeds 15, the callsign exceeds six characters, or
    /// the callsign contains a non-alphanumeric character.
    pub const fn new(callsign: &[u8], ssid: u8) -> Option<Self> {
        if ssid > 15 || callsign.len() > CALLSIGN_LEN {
            return None;
        }
        let mut cs = [b' '; CALLSIGN_LEN];
        let mut i = 0;
        while i < callsign.len() {
            let c = callsign[i];
            if !c.is_ascii_alphanumeric() {
                return None;
            }
            cs[i] = c.to_ascii_uppercase();
            i += 1;
        }
        Some(Address { callsign: cs, ssid })
    }

    /// Return the callsign as a byte slice with trailing space padding removed.
    pub fn callsign_trimmed(&self) -> &[u8] {
        let mut end = self.callsign.len();
        while end > 0 && self.callsign[end - 1] == b' ' {
            end -= 1;
        }
        &self.callsign[..end]
    }

    /// Parse a `CALLSIGN` or `CALLSIGN-N` field from bytes, where `N` is a
    /// numeric SSID in the range `0..=15`.
    pub fn parse(input: &[u8]) -> Option<Self> {
        if input.is_empty() {
            return None;
        }
        let (cs, ssid) = match input.iter().position(|&b| b == b'-') {
            Some(dash) => (&input[..dash], Some(&input[dash + 1..])),
            None => (input, None),
        };
        let callsign = parse_callsign(cs)?;

        let ssid = match ssid {
            None => 0,
            Some(b"") => return None,
            Some(s) => {
                let mut n = 0u8;
                for &d in s {
                    if !d.is_ascii_digit() {
                        return None;
                    }
                    n = n.checked_mul(10)?.checked_add(d - b'0')?;
                }
                if n > 15 {
                    return None;
                }
                n
            }
        };

        Some(Address { callsign, ssid })
    }
}

/// Parse, uppercase, and space-pad a six-character callsign.
fn parse_callsign(input: &[u8]) -> Option<[u8; CALLSIGN_LEN]> {
    if input.is_empty() || input.len() > CALLSIGN_LEN {
        return None;
    }
    let mut cs = [b' '; CALLSIGN_LEN];
    for (i, &c) in input.iter().enumerate() {
        if !c.is_ascii_alphanumeric() {
            return None;
        }
        cs[i] = c.to_ascii_uppercase();
    }
    Some(cs)
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(core::str::from_utf8(self.callsign_trimmed()).unwrap_or(""))?;
        if self.ssid != 0 {
            write!(f, "-{}", self.ssid)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_callsign() {
        let a = Address::parse(b"N0CALL").unwrap();
        assert_eq!(a.ssid, 0);
        assert_eq!(a.callsign_trimmed(), b"N0CALL");
    }

    #[test]
    fn parses_numeric_ssid() {
        let a = Address::parse(b"N0CALL-9").unwrap();
        assert_eq!(a.ssid, 9);
        assert_eq!(a.callsign_trimmed(), b"N0CALL");
    }

    #[test]
    fn parses_zero_ssid() {
        let a = Address::parse(b"N0CALL-0").unwrap();
        assert_eq!(a.ssid, 0);
    }

    #[test]
    fn uppercases_and_pads() {
        let a = Address::parse(b"ab1cd").unwrap();
        assert_eq!(a.callsign, *b"AB1CD ");
        assert_eq!(a.callsign_trimmed(), b"AB1CD");
    }

    #[test]
    fn rejects_bad_ssid() {
        assert!(Address::parse(b"N0CALL-16").is_none());
        assert!(Address::parse(b"N0CALL-40").is_none());
        assert!(Address::parse(b"N0CALL-").is_none());
    }

    #[test]
    fn rejects_alphanumeric_ssid() {
        // Alphanumeric SSIDs are an APRS-IS extension, not valid on RF.
        assert!(Address::parse(b"N0CALL-A").is_none());
        assert!(Address::parse(b"N0CALL-B").is_none());
    }

    #[test]
    fn rejects_oversized_callsign() {
        assert!(Address::parse(b"N0CALLS").is_none());
        assert!(Address::parse(b"WINLINK").is_none());
    }

    #[test]
    fn rejects_bad_callsign_char() {
        assert!(Address::parse(b"N0 CALL").is_none());
    }
}
