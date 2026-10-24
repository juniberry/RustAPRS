//! Mic-E position comment (message) types (§10, "Mic-E Position Comments").
//!
//! The three message bits A, B and C ride in the first three bytes of a Mic-E
//! destination address. A bit is a *standard* 1 (`P`–`Z`), a *custom* 1
//! (`A`–`K`) or a 0 (`0`–`9`, `L`). Shared by the encoder and the decoder.

/// A Mic-E position comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MicEMessage {
    /// M0, bits `111`.
    OffDuty,
    /// M1, bits `110`.
    EnRoute,
    /// M2, bits `101`.
    InService,
    /// M3, bits `100`.
    Returning,
    /// M4, bits `011`.
    Committed,
    /// M5, bits `010`.
    Special,
    /// M6, bits `001`.
    Priority,
    /// Custom position comment C0–C6 (the number; bits `111` to `001`).
    Custom(u8),
    /// All three bits 0.
    Emergency,
    /// Standard and custom 1s mixed: the spec calls the type "unknown".
    Unknown,
}

impl MicEMessage {
    /// The three bits A, B, C (A the most significant) and whether the 1s are
    /// custom ones, or `None` for [`MicEMessage::Unknown`] or a custom number
    /// above 6.
    pub fn bits(self) -> Option<(u8, bool)> {
        Some(match self {
            MicEMessage::OffDuty => (0b111, false),
            MicEMessage::EnRoute => (0b110, false),
            MicEMessage::InService => (0b101, false),
            MicEMessage::Returning => (0b100, false),
            MicEMessage::Committed => (0b011, false),
            MicEMessage::Special => (0b010, false),
            MicEMessage::Priority => (0b001, false),
            MicEMessage::Custom(n) if n <= 6 => (7 - n, true),
            MicEMessage::Emergency => (0, false),
            _ => return None,
        })
    }

    /// The message from its three bits (A the most significant) as decoded.
    pub fn from_bits(bits: u8, standard: bool, custom: bool) -> Self {
        let bits = bits & 0b111;
        match (standard, custom) {
            (true, true) => MicEMessage::Unknown,
            (false, false) => MicEMessage::Emergency,
            (false, true) => MicEMessage::Custom(7 - bits),
            (true, false) => [
                MicEMessage::Emergency,
                MicEMessage::Priority,
                MicEMessage::Special,
                MicEMessage::Committed,
                MicEMessage::Returning,
                MicEMessage::InService,
                MicEMessage::EnRoute,
                MicEMessage::OffDuty,
            ][bits as usize],
        }
    }

    /// The spec's name ("En Route", "Custom-3", "Emergency", ...).
    pub fn name(self) -> &'static str {
        match self {
            MicEMessage::OffDuty => "Off Duty",
            MicEMessage::EnRoute => "En Route",
            MicEMessage::InService => "In Service",
            MicEMessage::Returning => "Returning",
            MicEMessage::Committed => "Committed",
            MicEMessage::Special => "Special",
            MicEMessage::Priority => "Priority",
            MicEMessage::Custom(0) => "Custom-0",
            MicEMessage::Custom(1) => "Custom-1",
            MicEMessage::Custom(2) => "Custom-2",
            MicEMessage::Custom(3) => "Custom-3",
            MicEMessage::Custom(4) => "Custom-4",
            MicEMessage::Custom(5) => "Custom-5",
            MicEMessage::Custom(6) => "Custom-6",
            MicEMessage::Custom(_) | MicEMessage::Unknown => "Unknown",
            MicEMessage::Emergency => "Emergency",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip() {
        for bits in 1..8u8 {
            let m = MicEMessage::from_bits(bits, true, false);
            assert_eq!(m.bits(), Some((bits, false)));
            let c = MicEMessage::from_bits(bits, false, true);
            assert_eq!(c, MicEMessage::Custom(7 - bits));
            assert_eq!(c.bits(), Some((bits, true)));
        }
        assert_eq!(
            MicEMessage::from_bits(0, false, false),
            MicEMessage::Emergency
        );
        assert_eq!(
            MicEMessage::from_bits(0b101, true, true),
            MicEMessage::Unknown
        );
        assert_eq!(MicEMessage::Unknown.bits(), None);
        assert_eq!(MicEMessage::Custom(7).bits(), None);
        assert_eq!(
            MicEMessage::from_bits(0b100, true, false).name(),
            "Returning"
        );
    }
}
