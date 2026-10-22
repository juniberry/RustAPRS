//! APRS-IS client and passcode generation.
//!
//! APRS-IS is the Internet backbone for APRS traffic. Clients open a TCP
//! connection to a server, send a login line, optionally a filter, and then
//! read a stream of TNC2 monitor-format frames.

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Default APRS-IS port carrying a filtered feed.
pub const DEFAULT_PORT: u16 = 14580;

/// Compute the APRS-IS passcode for a callsign.
///
/// This is a historical, non-cryptographic hash used by the APRS-IS login
/// protocol: start from `0x73E2`, XOR in successive pairs of (uppercased)
/// callsign bytes with the first byte of each pair shifted left by 8, then mask
/// to 15 bits.
pub fn passcode(callsign: &str) -> u16 {
    let bytes = callsign.to_ascii_uppercase().into_bytes();
    let mut hash: u16 = 0x73E2;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= (bytes[i] as u16) << 8;
        if i + 1 < bytes.len() {
            hash ^= bytes[i + 1] as u16;
        }
        i += 2;
    }
    hash & 0x7FFF
}

/// Build the login line sent to an APRS-IS server.
///
/// The filter, when provided, is appended to the login line as
/// `filter <filter>` rather than sent as a separate command.
pub fn login_command(
    callsign: &str,
    pass: u16,
    app: &str,
    version: &str,
    filter: Option<&str>,
) -> String {
    let mut cmd = format!("user {callsign} pass {pass} vers {app} {version}");
    if let Some(f) = filter {
        cmd.push_str(&format!(" filter {f}"));
    }
    cmd.push_str("\r\n");
    cmd
}

/// A connected, authenticated APRS-IS client.
pub struct Client {
    reader: BufReader<TcpStream>,
    debug: bool,
}

impl Client {
    /// Connect to an APRS-IS server and authenticate.
    ///
    /// `server` may be a `host:port` string or a hostname (default port is
    /// used when omitted). Returns `Ok(None)` if the server rejects the login.
    ///
    /// When `debug` is set, every line sent to and received from the server is
    /// logged to stderr as `[send]`/`[recv]`.
    pub async fn connect(
        server: &str,
        callsign: &str,
        pass: u16,
        app: &str,
        version: &str,
        filter: Option<&str>,
        debug: bool,
    ) -> io::Result<Option<Self>> {
        let addr = parse_addr(server)?;
        let stream = TcpStream::connect(addr).await?;
        let mut reader = BufReader::new(stream);

        let login = login_command(callsign, pass, app, version, filter);
        if debug {
            eprintln!("[send] {}", login.trim_end_matches(['\r', '\n']));
        }
        reader.get_mut().write_all(login.as_bytes()).await?;

        // The server responds with `# logresp <callsign> verified, server ...`
        // (or `unverified`). Read until we see it, or until the stream closes.
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf).await?;
            if n == 0 {
                return Ok(None); // connection closed before verification
            }
            trim_line_ending(&mut buf);
            if debug {
                eprintln!("[recv] {}", String::from_utf8_lossy(&buf));
            }
            if let Some(verified) = logresp_verified(&buf) {
                if !verified {
                    return Ok(None);
                }
                return Ok(Some(Client { reader, debug }));
            }
        }
    }

    /// Send a raw command (for example a `filter ...` command).
    pub async fn send(&mut self, command: &str) -> io::Result<()> {
        if self.debug {
            eprintln!("[send] {}", command.trim_end_matches(['\r', '\n']));
        }
        self.reader.get_mut().write_all(command.as_bytes()).await?;
        self.reader.get_mut().flush().await
    }

    /// Read the next line from the server as raw bytes (without the trailing
    /// CR/LF).
    ///
    /// Unlike [`Client::next_frame`], this returns *every* line, including
    /// server status/comment lines beginning with `#`. Blank lines are skipped.
    /// Returns `Ok(None)` when the server closes the connection.
    ///
    /// Lines are returned as raw bytes because APRS payloads are not guaranteed
    /// to be valid UTF-8 (Mic-E, compressed positions, raw weather bytes, etc.).
    pub async fn next_line(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let n = self.reader.read_until(b'\n', &mut buf).await?;
            if n == 0 {
                return Ok(None);
            }
            trim_line_ending(&mut buf);
            if buf.is_empty() {
                continue;
            }
            if self.debug {
                eprintln!("[recv] {}", String::from_utf8_lossy(&buf));
            }
            return Ok(Some(buf));
        }
    }

    /// Read the next TNC2 monitor-format frame as raw bytes (without the
    /// trailing newline).
    ///
    /// Returns `Ok(None)` when the server closes the connection. Server status
    /// and log messages (lines beginning with `#`) are skipped.
    pub async fn next_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        loop {
            match self.next_line().await? {
                Some(line) if !line.starts_with(b"#") => return Ok(Some(line)),
                Some(_) => continue,
                None => return Ok(None),
            }
        }
    }
}

/// Parse a `host[:port]` string into a socket address, resolving the hostname.
fn parse_addr(server: &str) -> io::Result<SocketAddr> {
    use std::net::ToSocketAddrs;

    if let Ok(addr) = server.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let (host, port) = match server.rsplit_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid port"))?,
        ),
        None => (server, DEFAULT_PORT),
    };
    let mut addrs = (host, port).to_socket_addrs()?;
    addrs
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "could not resolve host"))
}

/// Determine whether a `# logresp` line reports a verified login.
///
/// Returns `None` if `line` is not a `logresp` line, `Some(true)` for
/// `verified`, and `Some(false)` for `unverified`. The status token may be
/// followed by a comma (e.g. `verified, server T2XXX`).
fn logresp_verified(line: &[u8]) -> Option<bool> {
    let rest = line.strip_prefix(b"# logresp ")?;
    let status = rest.split(|&b| b == b' ').nth(1)?;
    let status = status.strip_suffix(b",").unwrap_or(status);
    Some(status.eq_ignore_ascii_case(b"verified"))
}

/// Strip a trailing `\r` and/or `\n` from a line buffer.
fn trim_line_ending(buf: &mut Vec<u8>) {
    while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
        buf.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passcode_known_value() {
        // Manually computed for the two-character callsign "AB".
        assert_eq!(passcode("AB"), 0x32A0);
        // Case-insensitive.
        assert_eq!(passcode("ab"), passcode("AB"));
    }

    #[test]
    fn login_line_format() {
        assert_eq!(
            login_command("N0CALL", 12345, "RustAPRS", "0.1.0", None),
            "user N0CALL pass 12345 vers RustAPRS 0.1.0\r\n"
        );
        assert_eq!(
            login_command(
                "N0CALL",
                12345,
                "RustAPRS",
                "0.1.0",
                Some("r/43.65/-79.38/500")
            ),
            "user N0CALL pass 12345 vers RustAPRS 0.1.0 filter r/43.65/-79.38/500\r\n"
        );
    }

    #[test]
    fn parses_logresp_verified() {
        // Real-world format includes a comma after the status token.
        assert_eq!(
            logresp_verified(b"# logresp VE3VTY verified, server T2TORONTO"),
            Some(true)
        );
        assert_eq!(
            logresp_verified(b"# logresp VE3VTY unverified, server T2TORONTO"),
            Some(false)
        );
        assert_eq!(logresp_verified(b"# javAPRSSrvr 3.15"), None);
    }
}
