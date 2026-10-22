# RustAPRS

A modular APRS (Automatic Packet Reporting System) protocol engine in Rust,
targeting embedded systems (STM32-class MCUs) with host-side decode testing
against APRS-IS bulk data.

## Workspace layout

| Crate | Purpose | `no_std` |
|-------|---------|----------|
| `aprs-core` | Data model, data-type detection, base-91 utilities, errors | ✅ |
| `aprs-decoders` | Payload decoders (position, Mic-E, weather, telemetry, messages, objects, status) | ✅ |
| `aprs-ax25` | AX.25 frame layer (FCS/CRC, addressing) — no KISS/RF | ✅ |
| `aprs-is` | APRS-IS client (async, std) | ❌ (planned) |
| `aprs-capture` | CLI corpus collector from APRS-IS | ❌ (planned) |
| `aprs-cli` | Human-friendly decode CLI | ❌ (planned) |

## Design goals

- **Decode-first**: pure `&[u8] -> Result<_, Error>` functions, no allocation,
  no I/O. Encoders will follow once decode is verified against real traffic.
- **Zero-dependency core**: `aprs-core`, `aprs-decoders`, and `aprs-ax25`
  depend only on the Rust standard library's `core`, so they compile for any
  `no_std` target.
- **Corpus-driven testing**: golden files of real TNC2 monitor-format lines
  (as delivered by APRS-IS) drive unit tests and regression suites.

## Status

- [x] `aprs-core` — data model + detection
- [x] `aprs-decoders` — position / Mic-E / messages / objects / status / telemetry / weather
- [x] `aprs-ax25` — FCS + addressing
- [x] `aprs-is` — async APRS-IS client
- [x] `aprs-capture` — corpus collector
- [x] `aprs-cli` — decode CLI
- [ ] encoders

## Usage

Decode a packet or a stream of TNC2 lines:

```sh
cargo run -p aprs-cli -- decode "N0CALL-9>APRS,WIDE1-1,WIDE2-1:!4903.50N/07201.75W-Test"
# ...or pipe a corpus file:
cargo run -p aprs-cli -- decode < corpus/sample.tnc2
```

Audit a captured corpus and report decode statistics:

```sh
cargo run -p aprs-cli -- audit corpus/raw --examples 10
```

Collect bulk traffic from APRS-IS (requires a valid callsign/passcode):

```sh
cargo run -p aprs-capture -- --callsign N0CALL --center 43.65 -79.38 --radius 500 --max-packets 100000
```

`--center LAT LON` + `--radius KM` build a radial range filter (`r/lat/lon/dist`),
sent as part of the login line; use `--filter '...'` for an arbitrary APRS-IS
filter string instead.

## License

MIT
