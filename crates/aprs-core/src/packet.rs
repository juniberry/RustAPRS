//! A minimal APRS packet: source, destination, and information field.

use crate::address::Address;

/// A decoded APRS packet, independent of how it was transported (AX.25, KISS,
/// or APRS-IS TNC2 text).
///
/// The `info` field is the APRS payload (the AX.25 information field after the
/// PID byte). Decoders operate on this slice.
///
/// The destination matters for Mic-E packets, where the latitude is encoded in
/// the destination callsign rather than the information field. Because that
/// encoding uses lowercase bytes (`a`–`j`) that [`Address`] normalization
/// would uppercase, `destination_raw` preserves the original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet<'a> {
    pub source: Address,
    pub destination: Address,
    /// Raw destination bytes as received (6 bytes, space padded for AX.25).
    pub destination_raw: &'a [u8],
    pub info: &'a [u8],
}

impl<'a> Packet<'a> {
    pub const fn new(
        source: Address,
        destination: Address,
        destination_raw: &'a [u8],
        info: &'a [u8],
    ) -> Self {
        Packet {
            source,
            destination,
            destination_raw,
            info,
        }
    }
}
