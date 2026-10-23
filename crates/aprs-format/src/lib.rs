//! Human-readable formatting for decoded APRS packets.
//!
//! This is a host-side presentation concern (it allocates `String`s), kept out
//! of the `no_std` core/decoder crates so those remain usable on embedded
//! targets. Used by the `aprs-cli` and `aprs-capture` tools.

use aprs_decoders::{decode_third_party, Decoded, PositionEncoding};

/// Render a [`Decoded`] value as a single-line human-readable description.
pub fn format_decoded(decoded: Decoded<'_>) -> String {
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
            if let Some(phg) = p.phg {
                s.push_str(&format!(
                    " phg={}W/{}ft/{}dBi/{}°",
                    phg.power_watts, phg.height_feet, phg.gain_dbi, phg.directivity_deg
                ));
            }
            if let Some(r) = p.radio_range_miles {
                s.push_str(&format!(" range={r}mi"));
            }
            if p.messaging {
                s.push_str(" messaging");
            }
            if let Some(dev) = p.device {
                s.push_str(&format!(" device={}", device_name(dev)));
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
        Decoded::Status(s) => {
            let mut out = String::from("status");
            if let Some(m) = s.locator {
                out.push_str(&format!(
                    " [grid {}]",
                    String::from_utf8_lossy(&m.grid[..m.precision as usize])
                ));
            }
            if !s.text.is_empty() {
                out.push_str(&format!(": {}", String::from_utf8_lossy(s.text)));
            }
            out
        }
        Decoded::Maidenhead(m) => format!(
            "maidenhead {} ({:.5},{:.5})",
            String::from_utf8_lossy(&m.grid[..m.precision as usize]),
            m.coordinates.latitude,
            m.coordinates.longitude
        ),
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
        Decoded::ThirdParty(tp) => {
            let inner = match decode_third_party(&tp) {
                Ok(d) => format_decoded(d),
                Err(e) => format!("<decode error: {e}>"),
            };
            format!(
                "third-party origin={} -> {}",
                String::from_utf8_lossy(tp.source),
                inner
            )
        }
        Decoded::Unsupported(dt) => format!("unsupported type ({dt:?})"),
    }
}

/// Map a Mic-E device identifier suffix to a human-readable name.
fn device_name(device: &[u8]) -> String {
    match device {
        b"|3" => "Byonics TinyTrack 3".to_string(),
        b"|4" => "Byonics TinyTrack 4".to_string(),
        b"_(" => "Yaesu FT2D".to_string(),
        b"_0" => "Yaesu FT3D".to_string(),
        b"_3" => "Yaesu FT5D".to_string(),
        b"=" => "Kenwood TH-D72/TM-D710".to_string(),
        b"^" => "Kenwood TH-D74".to_string(),
        b"&" => "Kenwood TH-D75".to_string(),
        other => format!("?{}", String::from_utf8_lossy(other)),
    }
}
