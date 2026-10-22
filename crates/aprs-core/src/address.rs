//! AX.25 address representation.

use core::fmt;

/// Maximum length of a callsign (always six characters, space padded).
pub const CALLSIGN_LEN: usize = 6;

/// An SSID.
///
/// AX.25 uses a 4-bit numeric SSID (`0..=15`). APRS-IS additionally uses a
/// single alphanumeric SSID (`-A` through `-Z`, and lowercase) to distinguish
/// non-radio stations such as D-STAR/DMR/YSF gateways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ssid {
    /// Numeric SSID (`0..=15`), the AX.25 wire format.
    Numeric(u8),
    /// Single alphanumeric SSID (an ASCII letter), APRS-IS convention.
    Alphanumeric(u8),
}

impl Ssid {
    /// The numeric value for AX.25 wire encoding. Alphanumeric SSIDs have no
    /// wire representation and map to `0`.
    pub const fn wire(self) -> u8 {
        match self {
            Ssid::Numeric(n) => n,
            Ssid::Alphanumeric(_) => 0,
        }
    }
}

impl fmt::Display for Ssid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ssid::Numeric(n) => write!(f, "{n}"),
            Ssid::Alphanumeric(c) => write!(f, "{}", *c as char),
        }
    }
}

/// An AX.25 address: a six-character callsign plus an SSID.
///
/// The callsign is stored as six uppercase ASCII bytes, space padded on the
/// right. This mirrors the on-air AX.25 encoding and keeps the type trivially
/// copyable for embedded use.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    pub callsign: [u8; CALLSIGN_LEN],
    pub ssid: Ssid,
}

impl Address {
    /// Create an address from a callsign and a numeric SSID (`0..=15`).
    ///
    /// The callsign is uppercased and space-padded to six characters.
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
        Some(Address {
            callsign: cs,
            ssid: Ssid::Numeric(ssid),
        })
    }

    /// Return the callsign as a byte slice with trailing space padding removed.
    pub fn callsign_trimmed(&self) -> &[u8] {
        let mut end = self.callsign.len();
        while end > 0 && self.callsign[end - 1] == b' ' {
            end -= 1;
        }
        &self.callsign[..end]
    }

    /// Parse a `CALLSIGN`, `CALLSIGN-N`, or `CALLSIGN-X` field from bytes.
    ///
    /// The SSID may be numeric (`0..=15`) or a single alphanumeric character.
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
            None => Ssid::Numeric(0),
            Some(b"") => return None,
            Some(s) => {
                if s.len() == 1 && s[0].is_ascii_alphabetic() {
                    Ssid::Alphanumeric(s[0].to_ascii_uppercase())
                } else {
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
                    Ssid::Numeric(n)
                }
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
        if !matches!(self.ssid, Ssid::Numeric(0)) {
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
        assert_eq!(a.ssid, Ssid::Numeric(0));
        assert_eq!(a.callsign_trimmed(), b"N0CALL");
    }

    #[test]
    fn parses_numeric_ssid() {
        let a = Address::parse(b"N0CALL-9").unwrap();
        assert_eq!(a.ssid, Ssid::Numeric(9));
        assert_eq!(a.callsign_trimmed(), b"N0CALL");
    }

    #[test]
    fn parses_alphanumeric_ssid() {
        let a = Address::parse(b"K8ETN-B").unwrap();
        assert_eq!(a.ssid, Ssid::Alphanumeric(b'B'));
        assert_eq!(a.callsign_trimmed(), b"K8ETN");
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
        assert!(Address::parse(b"N0CALL-").is_none());
        assert!(Address::parse(b"N0CALL-AB").is_none());
    }

    #[test]
    fn rejects_bad_callsign_char() {
        assert!(Address::parse(b"N0 CALL").is_none());
    }
}
