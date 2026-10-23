//! `aprs-decoders` — allocation-free APRS payload decoders.
//!
//! Decoders are pure functions over byte slices: they borrow variable-length
//! text from the input and return typed results. They depend only on
//! [`aprs_core`] and `core`, making them suitable for embedded targets.

#![no_std]
#![forbid(unsafe_code)]

pub mod data_extension;
pub mod decoded;
pub mod maidenhead;
pub mod message;
pub mod mice;
pub mod object;
pub mod parse;
pub mod position;
pub mod status;
pub mod telemetry;
pub mod thirdparty;
pub mod weather;

pub use decoded::{
    CompressionOrigin, CompressionType, Coordinates, Decoded, Maidenhead, Message, NmeaSource,
    Object, Phg, Position, PositionEncoding, Status, SymbolTable, Telemetry, TelemetrySequence,
    ThirdParty, Timestamp, Weather,
};

use aprs_core::{DataType, Error, Packet};

/// Trim trailing carriage-return / line-feed bytes that some senders append to
/// the information field. These are not valid APRS content; without trimming,
/// they would leak into trailing text fields (message IDs, comments, status
/// text) and corrupt terminal output.
fn trim_trailing_crlf(mut info: &[u8]) -> &[u8] {
    while matches!(info.last(), Some(b'\r') | Some(b'\n')) {
        info = &info[..info.len() - 1];
    }
    info
}

/// Decode a packet's information field (the APRS payload).
///
/// Mic-E packets additionally encode their latitude in the destination address,
/// so they are not decodable from the information field alone; use
/// [`decode_packet`] for those.
pub fn decode(info: &[u8]) -> Result<Decoded<'_>, Error> {
    let info = trim_trailing_crlf(info);
    let dt = DataType::from_byte(*info.first().ok_or(Error::Empty)?);
    match dt {
        DataType::Position | DataType::PositionWithTimestamp => {
            Ok(Decoded::Position(position::decode(info)?))
        }
        DataType::Message => Ok(Decoded::Message(message::decode(info)?)),
        DataType::Object | DataType::Item => Ok(Decoded::Object(object::decode(info)?)),
        DataType::Status => Ok(Decoded::Status(status::decode(info)?)),
        DataType::GridLocator => Ok(Decoded::Maidenhead(status::decode_grid(info)?)),
        DataType::Telemetry => Ok(Decoded::Telemetry(telemetry::decode(info)?)),
        DataType::Weather => Ok(Decoded::Weather(weather::decode(info)?)),
        DataType::ThirdParty => Ok(Decoded::ThirdParty(thirdparty::decode(info)?)),
        DataType::MicECurrent | DataType::MicEOld => Err(Error::Unsupported),
        other => Ok(Decoded::Unsupported(other)),
    }
}

/// Decode a full packet, including Mic-E (which needs the destination).
pub fn decode_packet<'a>(packet: &Packet<'a>) -> Result<Decoded<'a>, Error> {
    let info = trim_trailing_crlf(packet.info);
    let dt = DataType::from_byte(*info.first().ok_or(Error::Empty)?);
    if matches!(dt, DataType::MicECurrent | DataType::MicEOld) {
        return Ok(Decoded::Position(mice::decode(
            packet.destination_raw,
            info,
        )?));
    }
    decode(info)
}

/// Decode the inner payload of a third-party packet, routing Mic-E through the
/// inner destination (which carries the Mic-E latitude).
pub fn decode_third_party<'a>(tp: &ThirdParty<'a>) -> Result<Decoded<'a>, Error> {
    let info = trim_trailing_crlf(tp.info);
    let dt = DataType::from_byte(*info.first().ok_or(Error::Empty)?);
    if matches!(dt, DataType::MicECurrent | DataType::MicEOld) {
        return Ok(Decoded::Position(mice::decode(tp.destination, info)?));
    }
    decode(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_status() {
        match decode(b">Hello world").unwrap() {
            Decoded::Status(s) => {
                assert!(s.locator.is_none());
                assert_eq!(s.text, b"Hello world");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn mic_e_requires_packet() {
        assert_eq!(decode(b"`abc"), Err(Error::Unsupported));
    }

    #[test]
    fn trims_trailing_crlf() {
        match decode(b":HKN      :hmepkpnp{48\r\n").unwrap() {
            Decoded::Message(m) => {
                assert_eq!(m.text, b"hmepkpnp");
                assert_eq!(m.id, Some(&b"48"[..]));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
