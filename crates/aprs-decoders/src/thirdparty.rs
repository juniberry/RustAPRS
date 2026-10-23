//! Third-party encapsulated traffic decoding (`}`), §17.
//!
//! A third-party packet carries a complete original APRS packet inside its
//! information field. The structure is:
//!
//! ```text
//! } <TNC-2 header> : <original information field>
//! ```
//!
//! where the TNC-2 header is `src>dst,network,gateway*`. The inner source is a
//! printable-ASCII string (1–9 chars) and is *not* constrained to the AX.25
//! address rules — it is the logical origin of the encapsulated packet.

use aprs_core::Error;

use crate::decoded::ThirdParty;

/// Decode a `}` third-party packet from the full information field.
pub fn decode(info: &[u8]) -> Result<ThirdParty<'_>, Error> {
    if info.first() != Some(&b'}') {
        return Err(Error::UnknownDataType(info[0]));
    }

    let body = &info[1..];

    // The `:` separates the TNC-2 header from the original information field.
    let colon = body
        .iter()
        .position(|&b| b == b':')
        .ok_or(Error::Truncated)?;
    let header = &body[..colon];
    let inner_info = &body[colon + 1..];

    // Parse `src>dst,path` from the header. The source and destination are
    // printable-ASCII strings and are retained verbatim (no AX.25 validation).
    let gt = header
        .iter()
        .position(|&b| b == b'>')
        .ok_or(Error::Truncated)?;
    let source = &header[..gt];
    if source.is_empty() {
        return Err(Error::Truncated);
    }
    let rest = &header[gt + 1..];
    let (destination, path) = match rest.iter().position(|&b| b == b',') {
        Some(comma) => (&rest[..comma], &rest[comma + 1..]),
        None => (rest, &[] as &[u8]),
    };

    Ok(ThirdParty {
        source,
        destination,
        path,
        info: inner_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_spec_example() {
        // §17 example: an iGate encapsulates a message.
        let info = b"}WB4APR-14>APxxxx,TCPIP,G9RXG*::G3NRW    :Hi Ian{001";
        let tp = decode(info).unwrap();
        assert_eq!(tp.source, b"WB4APR-14");
        assert_eq!(tp.destination, b"APxxxx");
        assert_eq!(tp.path, b"TCPIP,G9RXG*");
        assert_eq!(tp.info, b":G3NRW    :Hi Ian{001");
    }

    #[test]
    fn decodes_without_path() {
        let info = b"}VE3VTY>APRS::hello";
        let tp = decode(info).unwrap();
        assert_eq!(tp.source, b"VE3VTY");
        assert_eq!(tp.destination, b"APRS");
        assert!(tp.path.is_empty());
        assert_eq!(tp.info, b":hello");
    }

    #[test]
    fn long_source_is_preserved() {
        // The inner source may exceed 6 characters (a non-AX.25 identifier).
        let info = b"}LONGCALL-12>APRS::pos";
        let tp = decode(info).unwrap();
        assert_eq!(tp.source, b"LONGCALL-12");
    }

    #[test]
    fn rejects_non_third_party() {
        assert!(decode(b"!4903.50N/07201.75W-").is_err());
    }

    #[test]
    fn decodes_mic_e_inner_packet() {
        // A Mic-E packet encapsulated in third-party: the inner destination
        // carries the latitude.
        let tp = decode(b"}N0CALL>S32UVT:'(_fn\"Oj/").unwrap();
        let decoded = crate::decode_third_party(&tp).unwrap();
        match decoded {
            crate::Decoded::Position(p) => {
                assert!((p.coordinates.latitude - 33.4273).abs() < 1e-3);
            }
            other => panic!("expected Mic-E position, got {other:?}"),
        }
    }
}
