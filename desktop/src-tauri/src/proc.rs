//! Pure std helpers for the process manager: liveness probes, dependency resolution, one-shot secret generation, upstream reachability.
//! No third-party deps for easy unit tests; stateful child orchestration lives in lib.rs (holds Child handles).

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// HTTP liveness probe against local loopback proxy: `GET /<secret>/health`; status line containing 200 counts as healthy.
/// When the proxy uses path-secret auth, secret must be included or the probe gets 403.
pub fn http_health(port: u16, secret: Option<&str>, timeout_ms: u64) -> bool {
    let addr = match ("127.0.0.1", port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
    {
        Some(a) => a,
        None => return false,
    };
    let dur = Duration::from_millis(timeout_ms);
    let mut stream = match TcpStream::connect_timeout(&addr, dur) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(dur));
    let _ = stream.set_write_timeout(Some(dur));
    let path = match secret {
        Some(s) if !s.is_empty() => format!("/{s}/health"),
        _ => "/health".to_string(),
    };
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    // Only need the status line; read cap as a safety bound.
    let mut chunk = [0u8; 1024];
    while buf.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let status_line = head.lines().next().unwrap_or("");
    // e.g. "HTTP/1.1 200 OK": strictly compare second token to 200, avoid contains() matching reason phrase.
    status_line.split_whitespace().nth(1) == Some("200")
}

/// POST JSON to local loopback proxy (`POST /<secret><path>`); returns HTTP status code;
/// None on connect failure / no response. Used to validate a stored key with a minimal request—
/// traffic goes through the proxy to upstream: 200=ok, 401/403=key rejected. Loopback is plaintext, no TLS.
/// timeout_ms should be generous (proxy forwards upstream); callers should pass ~15000.
pub fn http_post_status(
    port: u16,
    secret: Option<&str>,
    path_suffix: &str,
    body: &[u8],
    timeout_ms: u64,
) -> Option<u16> {
    let addr = ("127.0.0.1", port).to_socket_addrs().ok()?.next()?;
    let dur = Duration::from_millis(timeout_ms);
    let mut stream = TcpStream::connect_timeout(&addr, dur).ok()?;
    let _ = stream.set_read_timeout(Some(dur));
    let _ = stream.set_write_timeout(Some(dur));
    let path = match secret {
        Some(s) if !s.is_empty() => format!("/{s}{path_suffix}"),
        _ => path_suffix.to_string(),
    };
    let req = format!(
        "POST {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(req.as_bytes()).ok()?;
    stream.write_all(body).ok()?;
    // Only need status line (non-streaming; arrives with first header block); read cap as safety bound.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    while buf.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let status_line = head.lines().next().unwrap_or("");
    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
}

/// GET path on local loopback proxy (`GET /<secret><path>`); returns (status code, response body).
/// None on connect failure / no response. Used to pull relay `/v1/models` through the proxy—the proxy has TLS (urllib);
/// this side only hits loopback plaintext, no Rust TLS. timeout_ms should be generous (~15000).
pub fn http_get_body(
    port: u16,
    secret: Option<&str>,
    path_suffix: &str,
    timeout_ms: u64,
) -> Option<(u16, String)> {
    let addr = ("127.0.0.1", port).to_socket_addrs().ok()?.next()?;
    let dur = Duration::from_millis(timeout_ms);
    let mut stream = TcpStream::connect_timeout(&addr, dur).ok()?;
    let _ = stream.set_read_timeout(Some(dur));
    let _ = stream.set_write_timeout(Some(dur));
    let path = match secret {
        Some(s) if !s.is_empty() => format!("/{s}{path_suffix}"),
        _ => path_suffix.to_string(),
    };
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    // Read full response (status + headers + body). Cap at 1 MiB; model lists are usually tens of KB.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if buf.len() > 1_048_576 {
            break;
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())?;
    // Split headers and body at first blank line.
    let body = match text.split_once("\r\n\r\n") {
        Some((_, b)) => b.to_string(),
        None => String::new(),
    };
    Some((status, body))
}

/// Upstream host reachability (TCP connect only, no key check). Green=reachable, yellow=unreachable.
pub fn tcp_reachable(host: &str, port: u16, timeout_ms: u64) -> bool {
    let dur = Duration::from_millis(timeout_ms);
    match (host, port).to_socket_addrs() {
        Ok(addrs) => {
            for a in addrs {
                if TcpStream::connect_timeout(&a, dur).is_ok() {
                    return true;
                }
            }
            false
        }
        Err(_) => false,
    }
}

/// True if `ip` is in a range Science Operon treats as private/reserved for CONNECT
/// SSRF protection — including common VPN/Clash **Fake-IP** pools (`198.18.0.0/15`,
/// ULA `fd00::/8` such as `fdfe:dcba:9876::`).
pub fn ip_is_operon_blocked_range(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            // RFC1918 + loopback + link-local + CGNAT + Fake-IP benchmarking range
            o[0] == 10
                || o[0] == 127
                || (o[0] == 169 && o[1] == 254)
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 198 && (18..=19).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique-local fd00::/8 (covers Clash-style fdfe:dcba:… Fake-IP)
                || (s[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10
                || (s[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Resolve `host:0` and return the first address that looks like Fake-IP / private.
/// `None` = resolved to public addresses only, or DNS failed (unknown — not a Fake-IP hit).
pub fn resolve_fake_ip_sample(host: &str) -> Option<String> {
    let addrs = (host, 0u16).to_socket_addrs().ok()?;
    for a in addrs {
        if ip_is_operon_blocked_range(a.ip()) {
            return Some(a.ip().to_string());
        }
    }
    None
}

/// Hosts Science MCP egress commonly needs. If system DNS returns Fake-IP for these,
/// Operon logs `denied … (private/reserved range)` and CONNECT returns 403 — even when
/// CSP proxy + Science HTTP health are green and network allowlist grants are present.
const EGRESS_DNS_PROBE_HOSTS: &[&str] = &["api.duckduckgo.com", "arxiv.org"];

/// Probe whether host DNS currently returns Fake-IP/private ranges that break Operon egress.
pub fn egress_dns_probe() -> EgressDnsProbe {
    for host in EGRESS_DNS_PROBE_HOSTS {
        if let Some(sample) = resolve_fake_ip_sample(host) {
            return EgressDnsProbe {
                ok: false,
                fake_ip: true,
                host: (*host).to_string(),
                sample,
            };
        }
    }
    EgressDnsProbe {
        ok: true,
        fake_ip: false,
        host: String::new(),
        sample: String::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EgressDnsProbe {
    pub ok: bool,
    pub fake_ip: bool,
    pub host: String,
    pub sample: String,
}

/// Find executable on PATH (simple which) with macOS GUI minimal-PATH fallback.
///
/// GUI processes launched from Finder / .app get minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`),
/// **excluding** Homebrew (`/usr/local/bin`, `/opt/homebrew/bin`), nvm / volta / asdf, etc.—
/// so `which("node")` misses common installs (`python3` works because `/usr/bin/python3` is in system PATH).
/// When PATH misses, scan [`common_bin_dirs`] (fix #2). Returns None if not found.
pub fn which(name: &str) -> Option<PathBuf> {
    // Absolute/relative path: check directly.
    let p = PathBuf::from(name);
    if p.is_absolute() {
        return if is_exec(&p) { Some(p) } else { None };
    }
    // 1) Normal PATH (enough when launched from terminal). Missing PATH does not early-return; fall through.
    if let Some(path) = std::env::var_os("PATH") {
        if let Some(hit) = find_in_dirs(name, std::env::split_paths(&path)) {
            return Some(hit);
        }
    }
    // 2) GUI/.app minimal PATH fallback: scan common install dirs.
    find_in_dirs(name, common_bin_dirs())
}

/// Find executable in given directory sequence (first hit wins).
fn find_in_dirs(name: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    for dir in dirs {
        let cand = dir.join(name);
        if is_exec(&cand) {
            return Some(cand);
        }
    }
    None
}

/// Common macOS install dirs for node/python (excluding `/usr/bin` etc. already in minimal PATH):
/// Homebrew (Apple Silicon / Intel), MacPorts, volta, asdf, `~/.local/bin`,
/// and nvm `~/.nvm/versions/node/<ver>/bin` (directory enumeration).
fn common_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"), // Homebrew (Apple Silicon)
        PathBuf::from("/usr/local/bin"),    // Homebrew (Intel) / manual install
        PathBuf::from("/opt/local/bin"),    // MacPorts
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".volta/bin"));
        dirs.push(home.join(".asdf/shims"));
        dirs.push(home.join(".local/bin"));
        // nvm: version dirs are dynamic; enumerate ~/.nvm/versions/node/*/bin.
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            for e in entries.flatten() {
                dirs.push(e.path().join("bin"));
            }
        }
    }
    dirs
}

/// Last resort when [`which`] fails: resolve the user's **real PATH** via login shell.
///
/// GUI/.app from Finder has minimal PATH; version managers (fnm / nvm / asdf in `.zshrc`) are not covered by
/// [`common_bin_dirs`] static list. Runs `zsh -lic 'command -v <name>'` (login + interactive, sources user rc).
/// Separate thread + `recv_timeout` so a broken rc cannot hang the caller.
pub fn which_via_login_shell(name: &str) -> Option<PathBuf> {
    // name comes from this codebase ("node"/"python3"); still whitelist to block shell injection.
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
    {
        return None;
    }
    let arg = format!("command -v {name} 2>/dev/null");
    // spawn + poll + timeout kill: broken rc cannot leak thread/process (fix P3).
    let mut child = Command::new("zsh")
        .args(["-lic", &arg])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // 3s is enough for command -v; kill and bail if still running.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    // rc may print noise to stdout: take last line that is an absolute executable path.
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines().rev() {
        let p = PathBuf::from(line.trim());
        if p.is_absolute() && is_exec(&p) {
            return Some(p);
        }
    }
    None
}

/// Locate executable (with login-shell fallback): [`which`] (PATH + common dirs) then
/// [`which_via_login_shell`] for real user PATH. Used for node / python3 to cover
/// "GUI minimal PATH + version manager" installs reported as missing (fix #2).
pub fn find_exe(name: &str) -> Option<PathBuf> {
    which(name).or_else(|| which_via_login_shell(name))
}

fn is_exec(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(md) => md.is_file() && (md.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

/// Generate one-shot path-secret: 16 bytes from /dev/urandom, hex-encoded to 32 chars.
/// Fail-closed: Err if urandom unavailable; never fall back to guessable weak secret (prefer proxy start failure).
pub fn gen_secret() -> std::io::Result<String> {
    use std::fs::File;
    let mut b = [0u8; 16];
    let mut f = File::open("/dev/urandom")?;
    f.read_exact(&mut b)?;
    Ok(hex(&b))
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        s.push(H[(byte >> 4) as usize] as char);
        s.push(H[(byte & 0xf) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_false_when_nothing_listening() {
        // High port almost certainly not listening.
        assert!(!http_health(59999, None, 300));
    }

    #[test]
    fn get_body_none_when_nothing_listening() {
        // Nothing listening → cannot connect → None (same fail-closed semantics as http_health).
        assert!(http_get_body(59998, Some("secret"), "/v1/models", 300).is_none());
    }

    #[test]
    fn operon_blocked_ranges_cover_fake_ip_pools() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        assert!(ip_is_operon_blocked_range(IpAddr::V4(Ipv4Addr::new(
            198, 18, 0, 141
        ))));
        assert!(ip_is_operon_blocked_range(IpAddr::V4(Ipv4Addr::new(
            198, 19, 1, 1
        ))));
        assert!(ip_is_operon_blocked_range(IpAddr::V4(Ipv4Addr::new(
            10, 0, 0, 1
        ))));
        assert!(!ip_is_operon_blocked_range(IpAddr::V4(Ipv4Addr::new(
            1, 1, 1, 1
        ))));
        // fdfe:dcba:9876::8c
        let fake_v6: Ipv6Addr = "fdfe:dcba:9876::8c".parse().unwrap();
        assert!(ip_is_operon_blocked_range(IpAddr::V6(fake_v6)));
        let public_v6: Ipv6Addr = "2606:4700:4700::1111".parse().unwrap();
        assert!(!ip_is_operon_blocked_range(IpAddr::V6(public_v6)));
    }

    #[test]
    fn which_finds_sh() {
        let sh = which("sh");
        assert!(sh.is_some(), "sh should be found on PATH");
        assert!(sh.unwrap().is_absolute());
    }

    #[test]
    fn which_absent_returns_none() {
        assert!(which("definitely-not-a-real-binary-xyzzy").is_none());
    }

    #[test]
    fn find_in_dirs_locates_exec() {
        // /bin/sh almost certainly exists and is executable.
        let hit = find_in_dirs("sh", vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
        assert!(hit.is_some(), "should find sh under /usr/bin or /bin");
        assert!(is_exec(&hit.unwrap()));
    }

    #[test]
    fn find_in_dirs_none_when_absent() {
        assert!(find_in_dirs("definitely-not-xyzzy", vec![PathBuf::from("/bin")]).is_none());
    }

    #[test]
    fn login_shell_resolves_sh_when_zsh_present() {
        // Skip when zsh missing (some CI images lack it).
        if which("zsh").is_none() {
            return;
        }
        let p = which_via_login_shell("sh");
        assert!(p.is_some(), "login shell should resolve sh");
        let p = p.unwrap();
        assert!(p.is_absolute() && is_exec(&p));
    }

    #[test]
    fn login_shell_rejects_bad_names_without_spawning() {
        // Whitelist: names with shell metacharacters rejected (injection guard); empty name too.
        assert!(which_via_login_shell("node; rm -rf /").is_none());
        assert!(which_via_login_shell("$(whoami)").is_none());
        assert!(which_via_login_shell("").is_none());
    }

    #[test]
    fn find_exe_finds_sh() {
        assert!(find_exe("sh").is_some());
    }

    #[test]
    fn common_bin_dirs_covers_homebrew_and_home_managers() {
        let dirs = common_bin_dirs();
        // Both Homebrew prefixes + MacPorts must be present.
        assert!(dirs
            .iter()
            .any(|d| d == &PathBuf::from("/opt/homebrew/bin")));
        assert!(dirs.iter().any(|d| d == &PathBuf::from("/usr/local/bin")));
        assert!(dirs.iter().any(|d| d == &PathBuf::from("/opt/local/bin")));
        // When HOME is set, should include version-manager dirs (volta).
        if std::env::var_os("HOME").is_some() {
            assert!(
                dirs.iter()
                    .any(|d| d.to_string_lossy().contains(".volta/bin")),
                "HOME should include .volta/bin fallback dir"
            );
        }
    }

    #[test]
    fn gen_secret_is_32_hex_and_varies() {
        let a = gen_secret().unwrap();
        let b = gen_secret().unwrap();
        assert_eq!(a.len(), 32, "urandom path should yield 32 hex chars");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "two generations should differ");
    }

    #[test]
    fn hex_encodes_known_bytes() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
    }
}
