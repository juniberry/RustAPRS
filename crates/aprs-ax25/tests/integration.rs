//! End-to-end integration: encode → AX.25 frame → decode round-trips.
//!
//! Exercises the full pipeline across `aprs-encoders`, `aprs-ax25`, and
//! `aprs-decoders`: a payload is encoded, assembled into an AX.25 frame,
//! disassembled, and decoded back to the original typed value.

use aprs_ax25::{decode_frame, encode_frame, Frame, MAX_DIGIPEATERS};
use aprs_core::address::Address;
use aprs_core::packet::Packet;
use aprs_decoders::{decode_packet, Decoded, PositionEncoding};

fn addr(callsign: &[u8], ssid: u8) -> Address {
    Address::new(callsign, ssid).unwrap()
}

/// Decode a frame's info field as a full packet (for Mic-E, the destination
/// carries the latitude).
fn decode_frame_info<'a>(frame: &'a Frame<'a>) -> Decoded<'a> {
    let packet = Packet::new(
        frame.source,
        frame.destination,
        &frame.destination.callsign,
        frame.info,
    );
    decode_packet(&packet).unwrap()
}

#[test]
fn uncompressed_position_roundtrip() {
    let mut info = [0u8; 64];
    let n = aprs_encoders::position::encode_uncompressed(
        false, 49.0583, -72.0292, b'/', b'-', b"Test", &mut info,
    )
    .unwrap();

    let frame = Frame {
        destination: addr(b"APRS", 0),
        source: addr(b"N0CALL", 9),
        digipeaters: [Address::default(); MAX_DIGIPEATERS],
        digi_count: 0,
        info: &info[..n],
    };

    let mut bytes = [0u8; 128];
    let m = encode_frame(&frame, &mut bytes).unwrap();
    let decoded = decode_frame(&bytes[..m]).unwrap();
    assert_eq!(decoded.destination, frame.destination);
    assert_eq!(decoded.source, frame.source);

    match decode_frame_info(&decoded) {
        Decoded::Position(p) => {
            assert_eq!(p.encoding, PositionEncoding::Uncompressed);
            assert!((p.coordinates.latitude - 49.0583).abs() < 1e-4);
            assert!((p.coordinates.longitude + 72.0292).abs() < 1e-4);
            assert_eq!(p.comment, b"Test");
        }
        other => panic!("expected position, got {other:?}"),
    }
}

#[test]
fn compressed_position_roundtrip() {
    let mut info = [0u8; 64];
    let n = aprs_encoders::position::encode_compressed(
        false,
        49.5,
        -72.75,
        b'/',
        b'>',
        Some((88, 36)),
        b"",
        &mut info,
    )
    .unwrap();

    let frame = Frame {
        destination: addr(b"APRS", 0),
        source: addr(b"N0CALL", 0),
        digipeaters: [Address::default(); MAX_DIGIPEATERS],
        digi_count: 0,
        info: &info[..n],
    };

    let mut bytes = [0u8; 128];
    let m = encode_frame(&frame, &mut bytes).unwrap();
    let decoded = decode_frame(&bytes[..m]).unwrap();

    match decode_frame_info(&decoded) {
        Decoded::Position(p) => {
            assert_eq!(p.encoding, PositionEncoding::Compressed);
            assert!((p.coordinates.latitude - 49.5).abs() < 1e-3);
            assert!((p.coordinates.longitude + 72.75).abs() < 1e-3);
            assert_eq!(p.course, Some(88));
        }
        other => panic!("expected compressed position, got {other:?}"),
    }
}

#[test]
fn mice_position_roundtrip() {
    let mut dest = [0u8; 6];
    let mut info = [0u8; 64];
    let (dl, il) = aprs_encoders::mice::encode(
        33.4273, -112.129, 20, 251, b'/', b'j', b"", &mut dest, &mut info,
    )
    .unwrap();

    // The Mic-E destination is the latitude encoding, still a valid address.
    let frame = Frame {
        destination: Address::new(&dest[..dl], 0).unwrap(),
        source: addr(b"N0CALL", 0),
        digipeaters: [Address::default(); MAX_DIGIPEATERS],
        digi_count: 0,
        info: &info[..il],
    };

    let mut bytes = [0u8; 128];
    let m = encode_frame(&frame, &mut bytes).unwrap();
    let decoded = decode_frame(&bytes[..m]).unwrap();

    match decode_frame_info(&decoded) {
        Decoded::Position(p) => {
            assert_eq!(p.encoding, PositionEncoding::MicE);
            assert!((p.coordinates.latitude - 33.4273).abs() < 1e-3);
            assert!((p.coordinates.longitude + 112.129).abs() < 1e-3);
            assert_eq!(p.course, Some(251));
        }
        other => panic!("expected Mic-E position, got {other:?}"),
    }
}

#[test]
fn kiss_encapsulated_roundtrip() {
    // Full path: position → frame → KISS → wire → KISS → frame → position.
    let mut info = [0u8; 64];
    let n = aprs_encoders::position::encode_uncompressed(
        false, 49.0583, -72.0292, b'/', b'-', b"", &mut info,
    )
    .unwrap();

    let frame = Frame {
        destination: addr(b"APRS", 0),
        source: addr(b"N0CALL", 0),
        digipeaters: [Address::default(); MAX_DIGIPEATERS],
        digi_count: 0,
        info: &info[..n],
    };

    let mut ax25 = [0u8; 128];
    let ax25_len = encode_frame(&frame, &mut ax25).unwrap();

    // Wrap in KISS, then unwrap.
    let mut wire = [0u8; 256];
    let wire_len =
        aprs_ax25::encode_kiss(aprs_ax25::CMD_DATA, &ax25[..ax25_len], &mut wire).unwrap();
    let mut ax25_2 = [0u8; 128];
    let (cmd, ax25_2_len) = aprs_ax25::decode_kiss(&wire[..wire_len], &mut ax25_2).unwrap();
    assert_eq!(cmd, aprs_ax25::CMD_DATA);

    let decoded = decode_frame(&ax25_2[..ax25_2_len]).unwrap();
    match decode_frame_info(&decoded) {
        Decoded::Position(p) => {
            assert!((p.coordinates.latitude - 49.0583).abs() < 1e-4);
            assert!((p.coordinates.longitude + 72.0292).abs() < 1e-4);
        }
        other => panic!("expected position, got {other:?}"),
    }
}
