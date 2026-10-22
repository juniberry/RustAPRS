//! APRS message decoding (`:` data type).

use aprs_core::Error;

use crate::decoded::Message;

/// Decode a `:` message from the full information field.
pub fn decode(info: &[u8]) -> Result<Message<'_>, Error> {
    if info.first() != Some(&b':') {
        return Err(Error::UnknownDataType(info[0]));
    }
    // addressee (9) + ':' separator
    if info.len() < 11 {
        return Err(Error::Truncated);
    }

    let mut addressee = [b' '; 9];
    addressee.copy_from_slice(&info[1..10]);

    let mut text = &info[11..];

    // Optional `{id}` suffix (1–5 alphanumeric, closing brace optional).
    let mut id = None;
    if let Some(open) = text.iter().rposition(|&b| b == b'{') {
        let after = &text[open + 1..];
        if !after.is_empty() {
            let end = after.iter().position(|&b| b == b'}').unwrap_or(after.len());
            id = Some(&after[..end]);
            text = &text[..open];
        }
    }

    // Detect acknowledgement/rejection messages.
    let ack = if text == b"ack" || text == b"rej" {
        Some(text)
    } else {
        None
    };

    Ok(Message {
        addressee,
        text,
        id,
        ack,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_message() {
        let m = decode(b":BLN1     :test message{123").unwrap();
        assert_eq!(&m.addressee, b"BLN1     ");
        assert_eq!(m.text, b"test message");
        assert_eq!(m.id, Some(&b"123"[..]));
        assert!(m.ack.is_none());
    }

    #[test]
    fn ack_message() {
        let m = decode(b":N0CALL   :ack{12").unwrap();
        assert_eq!(m.ack, Some(&b"ack"[..]));
        assert_eq!(m.id, Some(&b"12"[..]));
    }

    #[test]
    fn short_is_truncated() {
        assert_eq!(decode(b":AB").unwrap_err(), Error::Truncated);
    }
}
