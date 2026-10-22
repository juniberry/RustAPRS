//! Corpus-driven decode tests.
//!
//! Loads a sample TNC2 corpus, asserts golden values for a handful of packets,
//! and soaks every line through the decoder to ensure none panic and most are
//! recognized.

use aprs_core::tnc2;
use aprs_decoders::{decode_packet, Decoded, PositionEncoding, SymbolTable};

const CORPUS: &str = include_str!("../../../corpus/sample.tnc2");

fn packets() -> impl Iterator<Item = (&'static str, aprs_core::Packet<'static>)> {
    CORPUS.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let frame = tnc2::parse(line.as_bytes())
            .unwrap_or_else(|e| panic!("failed to parse TNC2 line {line:?}: {e}"));
        Some((line, frame.to_packet()))
    })
}

#[test]
fn golden_positions() {
    for (line, packet) in packets() {
        if line.starts_with("N0CALL-9>APRS") {
            match decode_packet(&packet).unwrap() {
                Decoded::Position(p) => {
                    assert_eq!(p.encoding, PositionEncoding::Uncompressed);
                    assert_eq!(p.symbol_table, SymbolTable(b'/'));
                    assert_eq!(p.symbol_code, b'-');
                    assert!((p.coordinates.latitude - 49.0583333).abs() < 1e-5);
                    assert!((p.coordinates.longitude + 72.0291666).abs() < 1e-5);
                }
                other => panic!("expected position, got {other:?}"),
            }
        }
    }
}

#[test]
fn golden_message_and_telemetry() {
    for (line, packet) in packets() {
        let decoded = decode_packet(&packet).unwrap();
        if line.contains(":BLN1     :test message") {
            match decoded {
                Decoded::Message(m) => {
                    assert_eq!(&m.addressee, b"BLN1     ");
                    assert_eq!(m.text, b"test message");
                    assert_eq!(m.id, Some(&b"123"[..]));
                }
                other => panic!("expected message, got {other:?}"),
            }
        } else if line.starts_with("N0CALL>APRS:T#") {
            match decoded {
                Decoded::Telemetry(t) => {
                    assert_eq!(t.sequence, aprs_decoders::TelemetrySequence::Number(5));
                    assert_eq!(t.analog, [199.0, 0.0, 255.0, 73.0, 123.0]);
                    assert_eq!(t.digital, 0b0110_1001);
                }
                other => panic!("expected telemetry, got {other:?}"),
            }
        }
    }
}

#[test]
fn soak_no_panics_and_high_recognition() {
    let mut total = 0usize;
    let mut recognized = 0usize;
    let mut unsupported = 0usize;
    let mut errors = 0usize;

    for (_line, packet) in packets() {
        total += 1;
        match decode_packet(&packet) {
            Ok(Decoded::Unsupported(_)) => unsupported += 1,
            Ok(_) => recognized += 1,
            Err(_) => errors += 1,
        }
    }

    assert_eq!(total, 20, "corpus should contain 20 packets");
    // Every v1 data type except the intentionally-unsupported ones
    // (capability, query, grid locator, third-party, user-defined) must decode.
    assert_eq!(errors, 0, "no packet should produce a decode error");
    assert_eq!(unsupported, 5, "five types are not yet decoded");
    assert_eq!(recognized, total - unsupported);
}
