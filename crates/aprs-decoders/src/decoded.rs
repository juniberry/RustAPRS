//! Shared decoded result types.
//!
//! All decoders borrow variable-length text (comments, messages, etc.) from the
//! input buffer rather than allocating, keeping the crate `no_std` and
//! allocation-free.

use aprs_core::DataType;

/// A geographic coordinate in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    /// Latitude in degrees, `-90.0..=90.0`.
    pub latitude: f64,
    /// Longitude in degrees, `-180.0..=180.0`.
    pub longitude: f64,
}

/// A Maidenhead grid locator (grid square).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Maidenhead {
    /// Uppercase grid-square characters (4 or 6), space-padded on the right.
    pub grid: [u8; 6],
    /// Number of significant characters (4 or 6).
    pub precision: u8,
    /// Center of the grid square in decimal degrees.
    pub coordinates: Coordinates,
}

/// APRS symbol table identifier.
///
/// The two standard tables are `/` (primary) and `\` (alternate), but the
/// APRS 1.2 symbol set and various gateways also use application-specific
/// table characters (e.g. `D` for D-STAR, `M` for DMR, `Y` for YSF/C4FM).
/// The raw byte is preserved so any table character round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolTable(pub u8);

impl SymbolTable {
    pub const fn from_byte(b: u8) -> SymbolTable {
        SymbolTable(b)
    }

    /// True for the primary (`/`) table.
    pub const fn is_primary(self) -> bool {
        self.0 == b'/'
    }

    /// True for the alternate (`\`) table.
    pub const fn is_alternate(self) -> bool {
        self.0 == b'\\'
    }
}

/// A timestamp embedded in position/weather/object packets (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timestamp {
    /// Day/Hour/Minute (`DDHHMMz` for UTC or `DDHHMM/` for local).
    DHM {
        day: u8,
        hour: u8,
        minute: u8,
        /// True for UTC (`z`), false for local time (`/`).
        utc: bool,
    },
    /// Hour/Minute/Second (`HHMMSSh`, UTC).
    HMS { hour: u8, minute: u8, second: u8 },
    /// Month/Day/Hour/Minute (8 digits, no suffix; positionless weather only).
    MDHM {
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
    },
}

/// How a position was encoded on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    /// Uncompressed `DDMM.MM` degrees/minutes.
    Uncompressed,
    /// Base-91 compressed.
    Compressed,
    /// Mic-E.
    MicE,
}

/// Power/Height/Gain/Directivity information from a `PHGphgd` data extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phg {
    /// Transmitter power in watts (`p` squared).
    pub power_watts: u16,
    /// Effective antenna height above average terrain, in feet (`10 * 2^h`).
    pub height_feet: u32,
    /// Antenna gain in dBi (`g`).
    pub gain_dbi: u8,
    /// Directivity in degrees (`d` × 45).
    pub directivity_deg: u16,
}

/// NMEA source of a compressed position's data (§9 Compression Type byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmeaSource {
    Other,
    Gll,
    Gga,
    Rmc,
}

/// Origin of a compressed position (§9 Compression Type byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionOrigin {
    /// Compressed by unspecified means.
    Compressed,
    /// TNC BText beacon.
    TncBText,
    /// Software (DOS/Mac/Win/+SA).
    Software,
    /// Reserved.
    Tbd,
    /// KPC3 TNC.
    Kpc3,
    /// Pico.
    Pico,
    /// Other tracker.
    OtherTracker,
    /// Digipeater conversion.
    Digipeater,
}

/// Decoded compression-type (`T`) byte of a compressed position (§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionType {
    /// True for a current GPS fix, false for an old (last) fix.
    pub gps_fix_current: bool,
    /// The NMEA sentence type the data originated from.
    pub nmea_source: NmeaSource,
    /// What performed the compression.
    pub origin: CompressionOrigin,
}

/// A decoded position report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position<'a> {
    pub coordinates: Coordinates,
    pub encoding: PositionEncoding,
    pub symbol_table: SymbolTable,
    /// The symbol code byte (see the APRS symbol tables).
    pub symbol_code: u8,
    /// Free-form comment following the position (may be empty).
    pub comment: &'a [u8],
    pub timestamp: Option<Timestamp>,
    /// Number of low-order digits blanked for position ambiguity (0 = exact).
    pub ambiguity: u8,
    /// True when the originating station supports APRS messaging.
    pub messaging: bool,
    /// Raw device identifier suffix (e.g. `|3`, `_0`, `=`) stripped from the
    /// Mic-E status text, when a known suffix was recognized.
    pub device: Option<&'a [u8]>,
    /// Course over ground in degrees, when available (Mic-E / compressed / CSE).
    pub course: Option<u16>,
    /// Speed in knots, when available.
    pub speed_knots: Option<u16>,
    /// Altitude in feet, when available.
    pub altitude_feet: Option<i32>,
    /// Power/height/gain/directivity, when a `PHG` extension is present.
    pub phg: Option<Phg>,
    /// Pre-calculated radio range in miles, when an `RNG` extension is present.
    pub radio_range_miles: Option<u16>,
    /// Decoded compression-type byte, for compressed positions with a meaningful
    /// `c` byte (None otherwise).
    pub compression_type: Option<CompressionType>,
    /// Maidenhead grid locator, when present (Mic-E status text).
    pub maidenhead: Option<Maidenhead>,
}

/// A decoded APRS message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Message<'a> {
    /// Nine-character addressee, space padded.
    pub addressee: [u8; 9],
    /// Message text (after the separating `:`).
    pub text: &'a [u8],
    /// Optional `{id}` message identifier (1–5 alphanumeric chars).
    pub id: Option<&'a [u8]>,
    /// Set to `b"ack"`/`b"rej"` for acknowledgements/rejections.
    pub ack: Option<&'a [u8]>,
}

/// A decoded object or item report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Object<'a> {
    /// Nine-character name, space padded.
    pub name: [u8; 9],
    /// True for a live object (`*`), false for a killed object (`_`).
    pub live: bool,
    /// True when this is an item (`)`) rather than an object (`;`).
    pub is_item: bool,
    /// Position, when present.
    pub position: Option<Position<'a>>,
    /// Trailing comment, used for killed objects that carry no position.
    pub comment: &'a [u8],
}

/// A decoded telemetry report (`T#...`).
///
/// The analog channels are nominally 0–255 (relaxed to 0–999 in APRS 1.2),
/// but stations commonly send wider, decimal, and signed values; 1.2
/// encourages recognizing these, so they are stored as `f32`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Telemetry<'a> {
    /// Monotonically increasing sequence number.
    pub sequence: TelemetrySequence,
    /// Five analog channel values.
    pub analog: [f32; 5],
    /// Eight digital channel bits packed into the low byte.
    pub digital: u8,
    /// Optional trailing comment (may be empty).
    pub comment: &'a [u8],
}

/// The telemetry sequence number: a 3-digit value, or the literal `MIC` marker
/// used by the APRS Micro Interface Module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetrySequence {
    Number(u32),
    Mic,
}

impl core::fmt::Display for TelemetrySequence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TelemetrySequence::Number(n) => write!(f, "{n}"),
            TelemetrySequence::Mic => write!(f, "MIC"),
        }
    }
}

/// A decoded weather report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weather {
    pub timestamp: Option<Timestamp>,
    /// Wind direction in degrees (0–360, 0 = calm/unknown).
    pub wind_direction: Option<u16>,
    /// Sustained wind speed in mph.
    pub wind_speed_mph: Option<u16>,
    /// Wind gust in mph.
    pub gust_mph: Option<u16>,
    /// Temperature in degrees Fahrenheit.
    pub temperature_f: Option<i16>,
    /// Relative humidity in percent (0–100).
    pub humidity: Option<u8>,
    /// Barometric pressure in tenths of millibar.
    pub pressure_mbar_tenths: Option<u16>,
    /// Rain in hundredths of an inch since midnight.
    pub rain_hundredths_in: Option<u16>,
    /// Rain in hundredths of an inch in the last hour.
    pub rain_last_hour_hundredths_in: Option<u16>,
    /// Rain in hundredths of an inch in the last 24 hours.
    pub rain_24h_hundredths_in: Option<u16>,
}

/// A decoded third-party encapsulated packet (`}`).
///
/// Third-party encapsulation carries a complete APRS packet from another
/// network (e.g. APRS-IS) inside the information field of an RF frame. The
/// outer frame uses a valid AX.25 source (the gateway); the *logical* origin is
/// the inner source, which is a 1–9 character printable-ASCII string that need
/// not be a valid AX.25 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThirdParty<'a> {
    /// Original source address (the logical origin; a printable-ASCII string).
    pub source: &'a [u8],
    /// Original destination (device/system identifier, or Mic-E latitude).
    pub destination: &'a [u8],
    /// Original digipeater via path (raw, comma-separated).
    pub path: &'a [u8],
    /// The encapsulated original information field, to be decoded recursively.
    pub info: &'a [u8],
}

/// A decoded status report (`>`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status<'a> {
    /// Maidenhead grid locator, when the report begins with one.
    pub locator: Option<Maidenhead>,
    /// The remaining status text (after the `>` and any locator).
    pub text: &'a [u8],
}

/// The result of decoding a packet's information field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decoded<'a> {
    Position(Position<'a>),
    Message(Message<'a>),
    Object(Object<'a>),
    /// Status report (`>`).
    Status(Status<'a>),
    Telemetry(Telemetry<'a>),
    Weather(Weather),
    /// Third-party encapsulated traffic (`}`).
    ThirdParty(ThirdParty<'a>),
    /// Maidenhead grid locator beacon (`[`, obsolete).
    Maidenhead(Maidenhead),
    /// Recognized but not yet decoded by a dedicated decoder.
    Unsupported(DataType),
}
