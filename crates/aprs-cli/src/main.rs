//! `aprs-cli` — decode APRS packets and audit captured corpora.
//!
//! Subcommands:
//! - `decode` — decode a single TNC2 packet, or a stream from stdin.
//! - `audit` — replay a directory of captured frames through the decoders and
//!   report decode statistics.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use aprs_core::tnc2;
use aprs_core::DataType;
use aprs_decoders::{decode_packet, Decoded, PositionEncoding};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aprs-cli", version, about = "APRS decode and corpus tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decode a single TNC2 packet, or a stream from stdin.
    Decode {
        /// A single TNC2 packet to decode (reads stdin when omitted).
        packet: Option<String>,
    },
    /// Replay a corpus of captured TNC2 frames and report decode statistics.
    Audit {
        /// Directory (or single file) containing captured `.tnc2` frames.
        path: PathBuf,
        /// Maximum number of example lines to show per category.
        #[arg(long, default_value_t = 5)]
        examples: usize,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Decode { packet } => decode_cmd(packet),
        Command::Audit { path, examples } => audit_cmd(&path, examples),
    }
}

// --- decode subcommand -----------------------------------------------------

fn decode_cmd(packet: Option<String>) {
    if let Some(packet) = packet {
        decode_line(packet.as_bytes());
        return;
    }

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let line = trim_ascii(&buf);
                if !line.is_empty() {
                    decode_line(line);
                }
            }
            Err(e) => {
                eprintln!("error reading stdin: {e}");
                break;
            }
        }
    }
}

fn decode_line(line: &[u8]) {
    let frame = match tnc2::parse(line) {
        Ok(f) => f,
        Err(e) => {
            println!("<parse error> {e}: {}", String::from_utf8_lossy(line));
            return;
        }
    };

    print!("{}>{}: ", frame.source, frame.destination);
    match decode_packet(&frame.to_packet()) {
        Ok(decoded) => println!("{}", format_decoded(decoded)),
        Err(e) => println!("<decode error> {e}"),
    }
}

fn format_decoded(decoded: Decoded<'_>) -> String {
    match decoded {
        Decoded::Position(p) => {
            let enc = match p.encoding {
                PositionEncoding::Uncompressed => "uncompressed",
                PositionEncoding::Compressed => "compressed",
                PositionEncoding::MicE => "Mic-E",
            };
            let mut s = format!(
                "position [{enc}] {:.5},{:.5} symbol={}",
                p.coordinates.latitude, p.coordinates.longitude, p.symbol_code as char
            );
            if let Some(c) = p.course {
                s.push_str(&format!(" course={c}°"));
            }
            if let Some(sp) = p.speed_knots {
                s.push_str(&format!(" speed={sp}kt"));
            }
            if let Some(alt) = p.altitude_feet {
                s.push_str(&format!(" alt={alt}ft"));
            }
            if !p.comment.is_empty() {
                s.push_str(&format!(" comment={}", String::from_utf8_lossy(p.comment)));
            }
            s
        }
        Decoded::Message(m) => {
            let mut s = format!(
                "message to {}: {}",
                String::from_utf8_lossy(m.addressee.trim_ascii_end()),
                String::from_utf8_lossy(m.text)
            );
            if let Some(id) = m.id {
                s.push_str(&format!(" [id={}]", String::from_utf8_lossy(id)));
            }
            s
        }
        Decoded::Object(o) => {
            let kind = if o.is_item { "item" } else { "object" };
            let name = String::from_utf8_lossy(o.name.trim_ascii_end());
            if let Some(p) = o.position {
                format!(
                    "{kind} {} ({}): {:.5},{:.5}",
                    name,
                    if o.live { "live" } else { "killed" },
                    p.coordinates.latitude,
                    p.coordinates.longitude
                )
            } else {
                format!(
                    "{kind} {} ({})",
                    name,
                    if o.live { "live" } else { "killed" }
                )
            }
        }
        Decoded::Status(s) => format!("status: {}", String::from_utf8_lossy(s)),
        Decoded::Telemetry(t) => {
            let mut s = format!(
                "telemetry seq={} analog={:?} digital={:08b}",
                t.sequence, t.analog, t.digital
            );
            if !t.comment.is_empty() {
                s.push_str(&format!(" comment={}", String::from_utf8_lossy(t.comment)));
            }
            s
        }
        Decoded::Weather(w) => {
            let mut parts = Vec::new();
            if let Some(d) = w.wind_direction {
                parts.push(format!("wind={d}°"));
            }
            if let Some(s) = w.wind_speed_mph {
                parts.push(format!("{s}mph"));
            }
            if let Some(t) = w.temperature_f {
                parts.push(format!("{t}°F"));
            }
            if let Some(h) = w.humidity {
                parts.push(format!("{h}%"));
            }
            if let Some(p) = w.pressure_mbar_tenths {
                parts.push(format!("{:.1}mbar", p as f64 / 10.0));
            }
            format!("weather {}", parts.join(" "))
        }
        Decoded::Unsupported(dt) => format!("unsupported type ({dt:?})"),
    }
}

// --- audit subcommand ------------------------------------------------------

#[derive(Default)]
struct Stats {
    total: usize,
    parse_errors: usize,
    decode_errors: usize,

    position_uncompressed: usize,
    position_compressed: usize,
    position_mice: usize,
    message: usize,
    object: usize,
    status: usize,
    telemetry: usize,
    weather: usize,

    unsupported: HashMap<DataType, usize>,

    parse_error_examples: Vec<String>,
    decode_error_examples: Vec<String>,
    unsupported_examples: Vec<(DataType, String)>,
}

impl Stats {
    fn decoded(&self) -> usize {
        self.position_uncompressed
            + self.position_compressed
            + self.position_mice
            + self.message
            + self.object
            + self.status
            + self.telemetry
            + self.weather
    }

    fn unsupported_total(&self) -> usize {
        self.unsupported.values().sum()
    }

    fn report(&self, max_examples: usize) {
        if self.total == 0 {
            println!("no packets found");
            return;
        }
        let pct = |n: usize| (n as f64 / self.total as f64) * 100.0;

        println!("total packets:     {}", self.total);
        println!(
            "  decoded:         {} ({:.1}%)",
            self.decoded(),
            pct(self.decoded())
        );
        println!(
            "  unsupported:     {} ({:.1}%)",
            self.unsupported_total(),
            pct(self.unsupported_total())
        );
        println!(
            "  parse errors:    {} ({:.1}%)",
            self.parse_errors,
            pct(self.parse_errors)
        );
        println!(
            "  decode errors:   {} ({:.1}%)",
            self.decode_errors,
            pct(self.decode_errors)
        );

        println!("\ndecoded by type:");
        println!("  position (uncompressed): {}", self.position_uncompressed);
        println!("  position (compressed):   {}", self.position_compressed);
        println!("  position (Mic-E):        {}", self.position_mice);
        println!("  message:                 {}", self.message);
        println!("  object/item:             {}", self.object);
        println!("  status:                  {}", self.status);
        println!("  telemetry:               {}", self.telemetry);
        println!("  weather:                 {}", self.weather);

        if !self.unsupported.is_empty() {
            println!("\nunsupported by type:");
            let mut entries: Vec<(DataType, usize)> =
                self.unsupported.iter().map(|(k, v)| (*k, *v)).collect();
            entries.sort_by(|a, b| {
                b.1.cmp(&a.1)
                    .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
            });
            for (dt, count) in entries {
                println!("  {dt:?}: {count}");
            }
        }

        print_examples(
            "\ndecode error examples",
            &self.decode_error_examples,
            max_examples,
        );
        print_examples(
            "\nparse error examples",
            &self.parse_error_examples,
            max_examples,
        );
        print_examples(
            "\nunsupported examples",
            &self
                .unsupported_examples
                .iter()
                .map(|(_, s)| s.clone())
                .collect::<Vec<_>>(),
            max_examples,
        );
    }
}

fn print_examples(header: &str, examples: &[String], max_examples: usize) {
    if examples.is_empty() {
        return;
    }
    println!("{header} ({}):", examples.len());
    for ex in examples.iter().take(max_examples) {
        println!("  {ex}");
    }
}

fn audit_cmd(path: &Path, max_examples: usize) {
    let mut files = Vec::new();
    collect_files(path, &mut files);

    if files.is_empty() {
        eprintln!("no files found under {}", path.display());
        std::process::exit(1);
    }

    let mut stats = Stats::default();
    for file in &files {
        audit_file(file, &mut stats, max_examples);
    }

    println!("audited {} file(s)", files.len());
    stats.report(max_examples);
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path.to_path_buf());
        return;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            collect_files(&entry.path(), files);
        }
    }
}

fn audit_file(path: &Path, stats: &mut Stats, max_examples: usize) {
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipping {}: {e}", path.display());
            return;
        }
    };

    for raw_line in content.split(|&b| b == b'\n') {
        let line = trim_ascii(raw_line);
        // Skip blank lines and server status/comment lines.
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        stats.total += 1;

        let frame = match tnc2::parse(line) {
            Ok(f) => f,
            Err(e) => {
                stats.parse_errors += 1;
                if stats.parse_error_examples.len() < max_examples {
                    stats
                        .parse_error_examples
                        .push(format!("{}  [{e}]", String::from_utf8_lossy(line)));
                }
                continue;
            }
        };

        match decode_packet(&frame.to_packet()) {
            Ok(Decoded::Position(p)) => match p.encoding {
                PositionEncoding::Uncompressed => stats.position_uncompressed += 1,
                PositionEncoding::Compressed => stats.position_compressed += 1,
                PositionEncoding::MicE => stats.position_mice += 1,
            },
            Ok(Decoded::Message(_)) => stats.message += 1,
            Ok(Decoded::Object(_)) => stats.object += 1,
            Ok(Decoded::Status(_)) => stats.status += 1,
            Ok(Decoded::Telemetry(_)) => stats.telemetry += 1,
            Ok(Decoded::Weather(_)) => stats.weather += 1,
            Ok(Decoded::Unsupported(dt)) => {
                *stats.unsupported.entry(dt).or_insert(0) += 1;
                if stats.unsupported_examples.len() < max_examples {
                    stats
                        .unsupported_examples
                        .push((dt, String::from_utf8_lossy(line).into_owned()));
                }
            }
            Err(e) => {
                stats.decode_errors += 1;
                if stats.decode_error_examples.len() < max_examples {
                    stats
                        .decode_error_examples
                        .push(format!("{}  [{e}]", String::from_utf8_lossy(line)));
                }
            }
        }
    }
}

/// Trim leading and trailing ASCII whitespace from a byte slice.
fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while let Some(&b) = bytes.first() {
        if b.is_ascii_whitespace() {
            bytes = &bytes[1..];
        } else {
            break;
        }
    }
    while let Some(&b) = bytes.last() {
        if b.is_ascii_whitespace() {
            bytes = &bytes[..bytes.len() - 1];
        } else {
            break;
        }
    }
    bytes
}
