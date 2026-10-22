//! `aprs-capture` — collect bulk APRS traffic from APRS-IS into a corpus.
//!
//! Connects to an APRS-IS server, logs in, optionally applies a filter, and
//! writes deduplicated TNC2 frames to sharded text files for decoder testing.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aprs_is::{passcode, Client};
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "aprs-capture",
    version,
    about = "Collect APRS-IS traffic into a corpus"
)]
struct Args {
    /// APRS-IS server (host or host:port).
    #[arg(long, default_value = "rotate.aprs2.net:14580")]
    server: String,

    /// Callsign to log in with (uppercased automatically).
    #[arg(long)]
    callsign: String,

    /// APRS-IS passcode; computed from the callsign when omitted.
    #[arg(long)]
    passcode: Option<u16>,

    /// APRS-IS filter string (overrides `--center`/`--radius`, e.g. `m/100`).
    #[arg(long)]
    filter: Option<String>,

    /// Center point (LAT LON in decimal degrees) for a radial range filter.
    #[arg(long, num_args = 2, value_names = ["LAT", "LON"], allow_hyphen_values = true)]
    center: Option<Vec<f64>>,

    /// Radius in km for the radial filter around `--center`.
    #[arg(long, default_value_t = 500.0)]
    radius: f64,

    /// Directory to write corpus files into.
    #[arg(long, default_value = "corpus/raw")]
    out_dir: PathBuf,

    /// Maximum number of packets to collect (0 = unlimited).
    #[arg(long, default_value_t = 0)]
    max_packets: u64,

    /// Packets per output file.
    #[arg(long, default_value_t = 10_000)]
    shard_size: u64,

    /// Log every line received from the server (including `#` status lines).
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let callsign = args.callsign.to_ascii_uppercase();
    let pass = args.passcode.unwrap_or_else(|| passcode(&callsign));

    // Build the filter: an explicit raw filter if given, otherwise a radial
    // filter around `--center`. Radial (`r/`) filters carry their own center,
    // so no position beacon is required. The filter is sent as part of the
    // login line.
    let filter = args.filter.clone().or_else(|| {
        args.center
            .as_ref()
            .map(|c| radial_filter(c[0], c[1], args.radius))
    });

    eprintln!("connecting to {} as {}...", args.server, callsign);
    let mut client = match Client::connect(
        &args.server,
        &callsign,
        pass,
        "RustAPRS",
        "0.1.0",
        filter.as_deref(),
        args.debug,
    )
    .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            eprintln!("login rejected (unverified callsign or bad passcode)");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("connection error: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("logged in.");

    match &filter {
        Some(f) => eprintln!("applied filter: {f}"),
        None => eprintln!("warning: no filter set; the filtered port will send no traffic"),
    }

    std::fs::create_dir_all(&args.out_dir).expect("create output directory");
    eprintln!("waiting for packets...");

    let mut seen: HashSet<u64> = HashSet::new();
    let mut collected: u64 = 0;
    let mut shard_index: u64 = 0;
    let mut file = open_shard(&args.out_dir, shard_index);

    while args.max_packets == 0 || collected < args.max_packets {
        let line = match client.next_line().await {
            Ok(Some(l)) => l,
            Ok(None) => {
                eprintln!("server closed the connection");
                break;
            }
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        };

        let frame = line;

        // Deduplicate by hash of the full line.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        frame.hash(&mut hasher);
        if !seen.insert(hasher.finish()) {
            continue;
        }

        file.write_all(&frame).expect("write frame");
        file.write_all(b"\n").expect("write newline");
        collected += 1;

        if collected == 1 {
            eprintln!("first packet: {}", String::from_utf8_lossy(&frame));
        }
        if collected.is_multiple_of(args.shard_size) {
            shard_index += 1;
            file = open_shard(&args.out_dir, shard_index);
        }
        if collected.is_multiple_of(100) {
            eprintln!("collected {collected} packets");
        }
    }

    eprintln!(
        "done: collected {collected} unique packets into {}",
        args.out_dir.display()
    );
}

/// Build an APRS-IS radial range filter string from a center point and radius.
///
/// The `r/lat/lon/dist` filter carries its own center, so no position beacon
/// is required.
fn radial_filter(lat: f64, lon: f64, radius: f64) -> String {
    format!("r/{lat}/{lon}/{radius}")
}

fn open_shard(dir: &Path, index: u64) -> std::fs::File {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = dir.join(format!("capture-{stamp}-{index:04}.tnc2"));
    std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_filter_format() {
        assert_eq!(radial_filter(43.65, -79.38, 500.0), "r/43.65/-79.38/500");
        assert_eq!(radial_filter(-33.45, -70.71, 200.0), "r/-33.45/-70.71/200");
    }
}
