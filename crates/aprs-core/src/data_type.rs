//! APRS data-type identification.

/// The broad category of an APRS information-field payload, determined by its
/// leading data-type identifier byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    /// Position report without timestamp, no APRS messaging (`!` or `=`),
    /// and compressed position reports (`!`/`/` with base-91 body).
    Position,
    /// Position report with timestamp (`/` or `@`).
    PositionWithTimestamp,
    /// Current Mic-E position report (backtick `` ` ``).
    MicECurrent,
    /// Old Mic-E position report (apostrophe `'`).
    MicEOld,
    /// APRS message (`:`).
    Message,
    /// Object (`;`).
    Object,
    /// Item (`)`).
    Item,
    /// Status report (`>`).
    Status,
    /// Telemetry data (`T`) or telemetry metadata (`:` addressed to the station).
    Telemetry,
    /// Positionless weather report (`_`).
    Weather,
    /// Raw weather-station data (`*`, `#`, `$`): Peet Bros U-II or Ultimeter
    /// 2000 formats. Not recommended and not decoded.
    RawWeather,
    /// Station capability (`<`).
    Capability,
    /// Query (`?`).
    Query,
    /// Third-party traffic (`}`).
    ThirdParty,
    /// Maidenhead grid locator beacon (`[`).
    GridLocator,
    /// User-defined packet (`{`).
    UserDefined,
    /// Invalid data or test (`~`).
    Invalid,
}

impl DataType {
    /// Classify the first byte of an APRS information field.
    pub const fn from_byte(b: u8) -> DataType {
        match b {
            b'!' | b'=' => DataType::Position,
            b'/' | b'@' => DataType::PositionWithTimestamp,
            b'`' => DataType::MicECurrent,
            b'\'' => DataType::MicEOld,
            b':' => DataType::Message,
            b';' => DataType::Object,
            b')' => DataType::Item,
            b'>' => DataType::Status,
            b'T' => DataType::Telemetry,
            b'_' => DataType::Weather,
            b'*' | b'#' | b'$' => DataType::RawWeather,
            b'<' => DataType::Capability,
            b'?' => DataType::Query,
            b'}' => DataType::ThirdParty,
            b'[' => DataType::GridLocator,
            b'{' => DataType::UserDefined,
            _ => DataType::Invalid,
        }
    }

    /// True if this type carries a position (lat/lon).
    pub const fn is_position(self) -> bool {
        matches!(
            self,
            DataType::Position
                | DataType::PositionWithTimestamp
                | DataType::MicECurrent
                | DataType::MicEOld
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_common_types() {
        assert_eq!(DataType::from_byte(b'!'), DataType::Position);
        assert_eq!(DataType::from_byte(b'='), DataType::Position);
        assert_eq!(DataType::from_byte(b'/'), DataType::PositionWithTimestamp);
        assert_eq!(DataType::from_byte(b'@'), DataType::PositionWithTimestamp);
        assert_eq!(DataType::from_byte(b'`'), DataType::MicECurrent);
        assert_eq!(DataType::from_byte(b':'), DataType::Message);
        assert_eq!(DataType::from_byte(b';'), DataType::Object);
        assert_eq!(DataType::from_byte(b'>'), DataType::Status);
        assert_eq!(DataType::from_byte(b'T'), DataType::Telemetry);
        assert_eq!(DataType::from_byte(b'*'), DataType::RawWeather);
        assert_eq!(DataType::from_byte(b'#'), DataType::RawWeather);
        assert_eq!(DataType::from_byte(b'$'), DataType::RawWeather);
        assert_eq!(DataType::from_byte(b'_'), DataType::Weather);
        assert_eq!(DataType::from_byte(b'Z'), DataType::Invalid);
    }

    #[test]
    fn unknown_is_invalid() {
        assert_eq!(DataType::from_byte(0x00), DataType::Invalid);
        assert_eq!(DataType::from_byte(0xFF), DataType::Invalid);
    }
}
