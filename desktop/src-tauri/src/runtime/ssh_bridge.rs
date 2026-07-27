//! SSH bridge for sandboxed Claude Science.
//!
//! Two layers (mirrors CSSwitch 0.8.2 learnings):
//! 1. Sandbox `~/.ssh/config` → absolute `Include` of the real user config so
//!    system OpenSSH can resolve Host / IdentityFile without copying keys.
//! 2. Science `config.toml` `ssh_hosts` → concrete Host aliases enumerated from
//!    the real config (Include-expanded). Science's own preflight skips
//!    cross-HOME `Include`, so the allowlist must list aliases explicitly.
//!
//! Fail-soft: any error is logged; sandbox launch continues.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::Value;

const MAX_ALIAS_COUNT: usize = 256;
const MAX_CONFIG_FILES: usize = 128;
const MAX_INCLUDE_DEPTH: usize = 16;
const MAX_CONFIG_FILE_BYTES: u64 = 256 * 1024;
const MAX_CONFIG_TOTAL_BYTES: u64 = 1024 * 1024;
const STATE_FILE: &str = "csp-ssh-bridge.v1.json";

#[derive(Default)]
struct ParseState {
    visited: HashSet<PathBuf>,
    aliases: Vec<String>,
    alias_set: BTreeSet<String>,
    total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct BridgeState {
    schema_version: u32,
    original_ssh_hosts: Option<Vec<String>>,
    effective_ssh_hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_effective_ssh_hosts: Option<Vec<String>>,
    managed_hosts: Vec<String>,
}

/// Public entry: Include bridge + Science `ssh_hosts` allowlist. Fail-soft.
///
/// Returns `true` when sandbox SSH-facing files changed (caller may need to
/// restart Science so it reloads `config.toml`).
pub(crate) fn ensure_ssh_bridge(sbx_home: &Path) -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return false;
    };
    let real_config = home.join(".ssh").join("config");
    if !real_config.is_file() {
        return false;
    }

    let mut changed = false;
    match write_include_stub(sbx_home, &real_config) {
        Ok(true) => changed = true,
        Ok(false) => {}
        Err(e) => eprintln!("SSH Include bridge skipped (non-fatal): {e}"),
    }

    match prepare_science_ssh_hosts(sbx_home, &home) {
        Ok((hosts, hosts_changed)) => {
            changed |= hosts_changed;
            if !hosts.is_empty() {
                eprintln!(
                    "SSH bridge: {} Host alias(es) in Science ssh_hosts (changed={})",
                    hosts.len(),
                    hosts_changed
                );
            } else {
                eprintln!(
                    "SSH bridge: no concrete Host aliases in ~/.ssh/config \
                     (wildcards-only configs cannot feed Science preflight)"
                );
            }
        }
        Err(e) => eprintln!("SSH ssh_hosts bridge skipped (non-fatal): {e}"),
    }
    changed
}

/// Build an OpenSSH `Include` line for the real user's `~/.ssh/config`.
pub(crate) fn ssh_include_line_for(real_config: &Path) -> Option<String> {
    if !real_config.is_file() {
        return None;
    }
    let canonical = real_config.canonicalize().ok()?;
    if !canonical.is_file() {
        return None;
    }
    let path_str = canonical.to_string_lossy();
    if path_str.contains('\n') || path_str.contains('\r') || path_str.contains('\0') {
        return None;
    }
    let escaped = path_str.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!("Include \"{escaped}\"\n"))
}

fn write_include_stub(sbx_home: &Path, real_config: &Path) -> Result<bool, String> {
    let Some(include_line) = ssh_include_line_for(real_config) else {
        return Ok(false);
    };
    let sbx_ssh_dir = sbx_home.join(".ssh");
    let sbx_ssh_config = sbx_ssh_dir.join("config");
    fs::create_dir_all(&sbx_ssh_dir).map_err(|e| format!("mkdir .ssh: {e}"))?;
    if sbx_ssh_config.is_file() {
        if let Ok(existing) = fs::read_to_string(&sbx_ssh_config) {
            if existing == include_line {
                let _ = fs::set_permissions(&sbx_ssh_config, fs::Permissions::from_mode(0o600));
                let _ = fs::set_permissions(&sbx_ssh_dir, fs::Permissions::from_mode(0o700));
                return Ok(false);
            }
        }
    }
    fs::write(&sbx_ssh_config, include_line).map_err(|e| format!("write config: {e}"))?;
    let _ = fs::set_permissions(&sbx_ssh_config, fs::Permissions::from_mode(0o600));
    let _ = fs::set_permissions(&sbx_ssh_dir, fs::Permissions::from_mode(0o700));
    Ok(true)
}

fn is_concrete_alias(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.starts_with('-')
        && !value.starts_with('!')
        && !value.bytes().any(|b| b.is_ascii_control())
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'.' | b'_' | b':' | b'@' | b'%' | b'+' | b'-')
        })
        && !value.bytes().any(|b| matches!(b, b'*' | b'?' | b'['))
}

fn ssh_words(line: &str) -> Result<Vec<String>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(Vec::new());
    }
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                while let Some(c) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    if c == '\\' {
                        if let Some(n) = chars.next() {
                            cur.push(n);
                        }
                    } else {
                        cur.push(c);
                    }
                }
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    Ok(words)
}

/// Expand a single Include pattern without the `glob` crate.
/// Supports `~`, absolute paths, paths relative to `~/.ssh`, and a trailing `*`
/// filename glob in one directory (e.g. `conf.d/*.conf`).
fn expand_include(pattern: &str, home: &Path) -> Result<Vec<PathBuf>, String> {
    if pattern.as_bytes().iter().any(|b| b.is_ascii_control()) {
        return Err("SSH Include contains control characters".into());
    }
    let expanded = if pattern == "~" {
        home.to_path_buf()
    } else if let Some(rest) = pattern.strip_prefix("~/") {
        home.join(rest)
    } else if pattern.contains("%d") {
        let home_text = home
            .to_str()
            .ok_or("HOME path is not valid UTF-8 for SSH Include")?;
        PathBuf::from(pattern.replace("%d", home_text))
    } else if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        home.join(".ssh").join(pattern)
    };
    let expanded_str = expanded.to_string_lossy();
    if expanded_str.contains('%') {
        return Err("SSH Include uses an unsupported token".into());
    }

    // Trailing filename glob only: dir/*.ext
    if let Some(parent) = expanded.parent() {
        let name = expanded
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if name.contains('*') || name.contains('?') || name.contains('[') {
            if name.matches('*').count() == 1
                && !name.contains('?')
                && !name.contains('[')
                && parent.is_dir()
            {
                let (prefix, suffix) = name.split_once('*').unwrap();
                let mut paths = Vec::new();
                for ent in fs::read_dir(parent).map_err(|e| format!("Include glob: {e}"))? {
                    let ent = ent.map_err(|e| format!("Include glob: {e}"))?;
                    let fname = ent.file_name();
                    let fname = fname.to_string_lossy();
                    if fname.starts_with(prefix) && fname.ends_with(suffix) {
                        let p = ent.path();
                        if p.is_file() {
                            paths.push(p);
                        }
                    }
                }
                paths.sort();
                return Ok(paths);
            }
            return Err("SSH Include glob is too complex to expand safely".into());
        }
    }
    Ok(vec![expanded])
}

fn parse_config(path: &Path, home: &Path, depth: usize, state: &mut ParseState) -> Result<(), String> {
    if depth > MAX_INCLUDE_DEPTH {
        return Err("SSH Include depth exceeds safety limit".into());
    }
    let canonical = fs::canonicalize(path).map_err(|e| format!("canonicalize SSH config: {e}"))?;
    if !state.visited.insert(canonical.clone()) {
        return Ok(());
    }
    if state.visited.len() > MAX_CONFIG_FILES {
        return Err("SSH Include file count exceeds safety limit".into());
    }
    let metadata = fs::metadata(&canonical).map_err(|e| format!("stat SSH config: {e}"))?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG_FILE_BYTES {
        return Err("SSH config is not a regular file or is too large".into());
    }
    state.total_bytes = state
        .total_bytes
        .checked_add(metadata.len())
        .ok_or("SSH config total size overflow")?;
    if state.total_bytes > MAX_CONFIG_TOTAL_BYTES {
        return Err("SSH config total size exceeds safety limit".into());
    }
    let text = fs::read_to_string(&canonical).map_err(|e| format!("read SSH config: {e}"))?;
    for line in text.lines() {
        let words = ssh_words(line)?;
        let Some(first) = words.first() else {
            continue;
        };
        let mut keyword = first.clone();
        let mut values: Vec<String> = words.iter().skip(1).cloned().collect();
        if let Some(index) = keyword.find('=') {
            let inline = keyword[index + 1..].to_string();
            keyword.truncate(index);
            if !inline.is_empty() {
                values.insert(0, inline);
            }
        } else if values.first().is_some_and(|v| v == "=") {
            values.remove(0);
        } else if let Some(inline) = values.first().and_then(|v| v.strip_prefix('=').map(str::to_string)) {
            values[0] = inline;
            if values[0].is_empty() {
                values.remove(0);
            }
        }
        let keyword_l = keyword.to_ascii_lowercase();
        if keyword_l == "host" {
            for alias in values.iter().filter(|a| is_concrete_alias(a)) {
                if state.alias_set.insert(alias.clone()) {
                    state.aliases.push(alias.clone());
                    if state.aliases.len() > MAX_ALIAS_COUNT {
                        return Err("too many concrete SSH Host aliases".into());
                    }
                }
            }
        } else if keyword_l == "include" {
            for pattern in &values {
                for inc in expand_include(pattern, home)? {
                    if inc.is_file() {
                        parse_config(&inc, home, depth + 1, state)?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn system_ssh_hosts_for_home(home: &Path) -> Result<Vec<String>, String> {
    let mut state = ParseState::default();
    let config = home.join(".ssh").join("config");
    if !config.is_file() {
        return Ok(Vec::new());
    }
    parse_config(&config, home, 0, &mut state)?;
    Ok(state.aliases)
}

fn merge_unique(original: Option<&[String]>, managed: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for host in original.into_iter().flatten().chain(managed.iter()) {
        if seen.insert(host.clone()) {
            merged.push(host.clone());
        }
    }
    merged
}

fn read_ssh_hosts(root: &toml::Table) -> Result<Option<Vec<String>>, String> {
    let Some(item) = root.get("ssh_hosts") else {
        return Ok(None);
    };
    let arr = item
        .as_array()
        .ok_or("config.toml ssh_hosts is not a string array")?;
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let s = v
            .as_str()
            .ok_or("config.toml ssh_hosts contains a non-string")?;
        out.push(s.to_string());
    }
    Ok(Some(out))
}

fn set_ssh_hosts_in_table(root: &mut toml::Table, hosts: Option<&[String]>) {
    match hosts {
        Some(hosts) if !hosts.is_empty() => {
            let arr: Vec<Value> = hosts.iter().map(|h| Value::String(h.clone())).collect();
            root.insert("ssh_hosts".to_string(), Value::Array(arr));
        }
        _ => {
            root.remove("ssh_hosts");
        }
    }
}

fn read_state(path: &Path) -> Result<Option<BridgeState>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| format!("read SSH bridge state: {e}"))?;
    let state: BridgeState =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse SSH bridge state: {e}"))?;
    if state.schema_version != 1 {
        return Err("unsupported SSH bridge state schema".into());
    }
    Ok(Some(state))
}

fn write_state(path: &Path, state: &BridgeState) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| format!("serialize state: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &bytes).map_err(|e| format!("write state tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename state: {e}"))?;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    Ok(())
}

fn current_matches_owned(current: Option<&[String]>, state: &BridgeState) -> bool {
    current == Some(state.effective_ssh_hosts.as_slice())
        || current == state.original_ssh_hosts.as_deref()
        || state
            .previous_effective_ssh_hosts
            .as_deref()
            .is_some_and(|prev| current == Some(prev))
}

fn prepare_science_ssh_hosts(
    sbx_home: &Path,
    system_home: &Path,
) -> Result<(Vec<String>, bool), String> {
    let managed = system_ssh_hosts_for_home(system_home)?;
    if managed.is_empty() {
        return Ok((managed, false));
    }
    let data_dir = sbx_home.join(".claude-science");
    fs::create_dir_all(&data_dir).map_err(|e| format!("mkdir science data dir: {e}"))?;
    let config_path = data_dir.join("config.toml");
    let state_path = data_dir.join(STATE_FILE);

    let mut root: toml::Table = if config_path.is_file() {
        fs::read_to_string(&config_path)
            .map_err(|e| format!("read config.toml: {e}"))?
            .parse::<toml::Table>()
            .map_err(|e| format!("parse config.toml: {e}"))?
    } else {
        toml::Table::new()
    };
    let before = root.clone();
    let current = read_ssh_hosts(&root)?;
    let prior = read_state(&state_path)?;

    let original = match &prior {
        Some(state) if current_matches_owned(current.as_deref(), state) => {
            state.original_ssh_hosts.clone()
        }
        Some(_) => {
            return Err(
                "Science ssh_hosts was modified outside CSP; leaving it untouched".into(),
            );
        }
        None => current.clone(),
    };

    let effective = merge_unique(original.as_deref(), &managed);
    let transitional = BridgeState {
        schema_version: 1,
        original_ssh_hosts: original.clone(),
        effective_ssh_hosts: effective.clone(),
        previous_effective_ssh_hosts: current.clone(),
        managed_hosts: managed.clone(),
    };
    write_state(&state_path, &transitional)?;

    set_ssh_hosts_in_table(&mut root, Some(&effective));
    let mut changed = false;
    if root != before {
        let serialized =
            toml::to_string_pretty(&root).map_err(|e| format!("serialize config.toml: {e}"))?;
        let tmp = config_path.with_extension("toml.tmp");
        fs::write(&tmp, serialized.as_bytes()).map_err(|e| format!("write config.toml tmp: {e}"))?;
        fs::rename(&tmp, &config_path).map_err(|e| format!("rename config.toml: {e}"))?;
        let _ = fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600));
        changed = true;
    }

    let committed = BridgeState {
        previous_effective_ssh_hosts: None,
        ..transitional
    };
    write_state(&state_path, &committed)?;
    Ok((managed, changed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn tmpdir(tag: &str) -> PathBuf {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!(
            "csp-ssh-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn ssh_include_quotes_and_canonicalizes_path() {
        let d = tmpdir("include");
        let ssh = d.join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        // Path with a space — unquoted Include would break OpenSSH parsing.
        let cfg = ssh.join("my config");
        fs::write(&cfg, "Host *\n").unwrap();
        let line = ssh_include_line_for(&cfg).expect("should bridge real file");
        assert!(line.starts_with("Include \""), "must quote path: {line}");
        assert!(line.contains("my config"), "{line}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ssh_include_skips_missing_file() {
        let d = tmpdir("missing");
        let missing = d.join(".ssh").join("config");
        assert!(ssh_include_line_for(&missing).is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn parses_concrete_hosts_from_includes() {
        let home = tmpdir("parse");
        let ssh = home.join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        let conf_d = ssh.join("conf.d");
        fs::create_dir_all(&conf_d).unwrap();
        fs::write(
            ssh.join("config"),
            "Host direct-a \"direct-b\" * !blocked -bad\n  HostName secret.example\nInclude conf.d/*.conf\n",
        )
        .unwrap();
        fs::write(conf_d.join("extra.conf"), "Host included-a\nHost included-a\n").unwrap();
        let aliases = system_ssh_hosts_for_home(&home).unwrap();
        assert_eq!(aliases, vec!["direct-a", "direct-b", "included-a"]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn rejects_wildcard_only_as_empty_managed_list() {
        let home = tmpdir("wild");
        let ssh = home.join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        fs::write(ssh.join("config"), "Host * ?foo [abc] !blocked\n").unwrap();
        let aliases = system_ssh_hosts_for_home(&home).unwrap();
        assert!(aliases.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn prepare_writes_ssh_hosts_and_include_stub() {
        let system = tmpdir("sys");
        let sbx = tmpdir("sbx");
        let ssh = system.join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        fs::write(ssh.join("config"), "Host managed-a managed-b\n").unwrap();

        // Point HOME at system for this test via absolute Include write path.
        write_include_stub(&sbx, &ssh.join("config")).unwrap();
        assert!(sbx.join(".ssh/config").is_file());
        let include = fs::read_to_string(sbx.join(".ssh/config")).unwrap();
        assert!(include.starts_with("Include \""));

        // Write science data dir + hosts using system_home override.
        let (managed, changed) = prepare_science_ssh_hosts(&sbx, &system).unwrap();
        assert!(changed);
        assert_eq!(managed, vec!["managed-a", "managed-b"]);
        let toml_text = fs::read_to_string(sbx.join(".claude-science/config.toml")).unwrap();
        let table: toml::Table = toml_text.parse().unwrap();
        let hosts = read_ssh_hosts(&table).unwrap().unwrap();
        assert_eq!(hosts, vec!["managed-a", "managed-b"]);
        assert!(sbx.join(".claude-science").join(STATE_FILE).is_file());

        // Idempotent second pass.
        let (managed2, changed2) = prepare_science_ssh_hosts(&sbx, &system).unwrap();
        assert_eq!(managed2, managed);
        assert!(!changed2);

        let _ = fs::remove_dir_all(&system);
        let _ = fs::remove_dir_all(&sbx);
    }

    #[test]
    fn prepare_preserves_preexisting_user_ssh_hosts() {
        let system = tmpdir("sys2");
        let sbx = tmpdir("sbx2");
        fs::create_dir_all(system.join(".ssh")).unwrap();
        fs::write(system.join(".ssh/config"), "Host managed\n").unwrap();
        let data = sbx.join(".claude-science");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            data.join("config.toml"),
            "quiet_logs = true\nssh_hosts = [\"user-kept\"]\n",
        )
        .unwrap();

        prepare_science_ssh_hosts(&sbx, &system).unwrap();
        let table: toml::Table = fs::read_to_string(data.join("config.toml"))
            .unwrap()
            .parse()
            .unwrap();
        let hosts = read_ssh_hosts(&table).unwrap().unwrap();
        assert_eq!(hosts, vec!["user-kept", "managed"]);
        assert!(table.get("quiet_logs").is_some());
        let _ = fs::remove_dir_all(&system);
        let _ = fs::remove_dir_all(&sbx);
    }
}
