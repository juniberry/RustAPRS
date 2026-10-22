//! `aprs-decoders` — allocation-free APRS payload decoders.
//!
//! Decoders are pure functions over byte slices: they borrow variable-length
//! text from the input and return typed results. They depend only on
//! [`aprs_core`] and `core`, making them suitable for embedded targets.

#![no_std]
#![forbid(unsafe_code)]

pub mod decoded;
pub mod message;
pub mod mice;
pub mod object;
pub mod parse;
pub mod position;
pub mod telemetry;
pub mod weather;

pub use decoded::{
    Coordinates, Decoded, Message, Object, Position, PositionEncoding, SymbolTable, Telemetry,
    TelemetrySequence, Timestamp, Weather,
};

use aprs_core::{DataType, Error, Packet};

/// Decode a packet's information field (the APRS payload).
///
/// Mic-E packets additionally encode their latitude in the destination address,
/// so they are not decodable from the information field alone; use
/// [`decode_packet`] for those.
pub fn decode(info: &[u8]) -> Result<Decoded<'_>, Error> {
    let dt = DataType::from_byte(*info.first().ok_or(Error::Empty)?);
    match dt {
        DataType::Position | DataType::PositionWithTimestamp => {
            Ok(Decoded::Position(position::decode(info)?))
        }
        DataType::Message => Ok(Decoded::Message(message::decode(info)?)),
        DataType::Object | DataType::Item => Ok(Decoded::Object(object::decode(info)?)),
        DataType::Status => Ok(Decoded::Status(&info[1..])),
        DataType::Telemetry => Ok(Decoded::Telemetry(telemetry::decode(info)?)),
        DataType::Weather => Ok(Decoded::Weather(weather::decode(info)?)),
        DataType::MicECurrent | DataType::MicEOld => Err(Error::Unsupported),
        other => Ok(Decoded::Unsupported(other)),
    }
}

/// Decode a full packet, including Mic-E (which needs the destination).
pub fn decode_packet<'a>(packet: &Packet<'a>) -> Result<Decoded<'a>, Error> {
    let dt = DataType::from_byte(*packet.info.first().ok_or(Error::Empty)?);
    if matches!(dt, DataType::MicECurrent | DataType::MicEOld) {
        return Ok(Decoded::Position(mice::decode(
            packet.destination_raw,
            packet.info,
        )?));
    }
    decode(packet.info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_status() {
        match decode(b">Hello world").unwrap() {
            Decoded::Status(s) => assert_eq!(s, b"Hello world"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn mic_e_requires_packet() {
        assert_eq!(decode(b"`abc"), Err(Error::Unsupported));
    }
}
