//! Deploy remote (SSE / Streamable HTTP) MCP servers into Science's org DB.
//!
//! Confirmed against Science `0.1.17-dev`: custom remotes live in
//! `orgs/<org_uuid>/operon-cli.db` table `custom_mcp_servers` (not
//! `local-mcp.json`, which is stdio-only). Required columns:
//! `user_id`, `name`, `url`, `transport` (`sse`|`streamable_http`), optional
//! `description`, `headers_helper`. Assignments go in `mcp_agent_assignments`
//! (agent `OPERON`). Under CSP virtual login the org DB `user_id` is
//! `local-dev`.
//!
//! `headers_helper` is a shell command that prints a JSON object of string
//! header values. Science's create API encrypts it with USER_SECRET; writing
//! plaintext via SQL still works — Science decrypts on read and falls back to
//! the raw string when it is not ciphertext-shaped.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::model::{McpServer, McpTransport};
use crate::oauth_forge::real_ancestor;

/// Science sandbox user id observed in org `operon-cli.db` under CSP virtual login.
pub(crate) const SCIENCE_USER_ID: &str = "local-dev";
/// Default agent that custom MCP servers attach to.
pub(crate) const SCIENCE_AGENT: &str = "OPERON";

const REMOTE_STATE_FILE: &str = "csp-remote-mcp.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RemoteDeployState {
    /// Connector names last written by CSP (so disable/remove can delete rows).
    names: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RemoteMcpDeployReport {
    pub deployed: Vec<String>,
    pub removed: Vec<String>,
    pub changed: bool,
    /// Human-readable skip/error note (non-fatal for sandbox launch).
    pub note: String,
}

/// Sync enabled remote MCP servers into the org DB. Stdio servers are ignored.
///
/// `data_dir` is the sandbox Science root (`$SANDBOX_HOME/.claude-science`).
/// `org_uuid` selects `orgs/<org>/operon-cli.db`. When the DB is missing
/// (Science never launched), this is a no-op with a note — stdio deploy still
/// proceeds separately.
pub(crate) fn deploy_remote_mcp(
    enabled: &[McpServer],
    data_dir: &Path,
    sandbox_root: &Path,
    real_science_dir: &Path,
    org_uuid: Option<&str>,
) -> Result<RemoteMcpDeployReport, String> {
    let resolved = real_ancestor(data_dir);
    let real_root = real_ancestor(real_science_dir);
    let root = real_ancestor(sandbox_root);
    if resolved.starts_with(&real_root) {
        return Err(format!(
            "refuse: sandbox mcp dir resolves inside real Science dir ({})",
            resolved.display()
        ));
    }
    if !resolved.starts_with(&root) {
        return Err(format!(
            "refuse: sandbox mcp dir resolves outside sandbox root ({} not under {})",
            resolved.display(),
            root.display()
        ));
    }

    let remotes: Vec<&McpServer> = enabled.iter().filter(|s| s.transport.is_remote()).collect();

    let state_path = data_dir.join("mcp").join(REMOTE_STATE_FILE);
    let prev = load_state(&state_path);

    let Some(org) = org_uuid.filter(|o| !o.is_empty()) else {
        return Ok(RemoteMcpDeployReport {
            note: "remote deploy skipped: org uuid unavailable".into(),
            ..Default::default()
        });
    };

    let db_path = data_dir.join("orgs").join(org).join("operon-cli.db");
    if !db_path.is_file() {
        // First-launch race: Science creates the org DB on first boot. CSP will
        // sync on the next Start / redeploy after the DB exists.
        if remotes.is_empty() && prev.names.is_empty() {
            return Ok(RemoteMcpDeployReport::default());
        }
        return Ok(RemoteMcpDeployReport {
            note: format!(
                "remote deploy deferred: org DB missing at {} (start Science once)",
                db_path.display()
            ),
            ..Default::default()
        });
    }

    let mut report = RemoteMcpDeployReport::default();
    let conn = Connection::open(&db_path).map_err(|e| format!("open org operon-cli.db: {e}"))?;
    ensure_tables(&conn)?;

    let desired: BTreeSet<String> = remotes.iter().map(|s| s.name.clone()).collect();
    let previous: BTreeSet<String> = prev.names.iter().cloned().collect();

    for name in previous.difference(&desired) {
        if delete_by_name(&conn, name)? {
            report.removed.push(name.clone());
            report.changed = true;
        }
    }

    for server in &remotes {
        if upsert_remote(&conn, server)? {
            report.changed = true;
        }
        report.deployed.push(server.name.clone());
    }

    let new_state = RemoteDeployState {
        names: report.deployed.clone(),
    };
    if new_state.names != prev.names || report.changed {
        save_state(&state_path, &new_state)?;
        report.changed = true;
    }

    Ok(report)
}

fn ensure_tables(conn: &Connection) -> Result<(), String> {
    // Refuse to write if Science schema is absent (wrong DB / older build).
    let has: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='custom_mcp_servers'",
            [],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| format!("probe custom_mcp_servers: {e}"))?
        .unwrap_or(false);
    if !has {
        return Err(
            "org operon-cli.db has no custom_mcp_servers table — Science build too old?".into(),
        );
    }
    Ok(())
}

fn upsert_remote(conn: &Connection, server: &McpServer) -> Result<bool, String> {
    let transport = match server.transport {
        McpTransport::Sse => "sse",
        McpTransport::StreamableHttp => "streamable_http",
        McpTransport::Stdio => return Ok(false),
    };
    let helper = headers_helper_command(&server.headers);
    let desc = if server.description.is_empty() {
        None
    } else {
        Some(server.description.as_str())
    };

    let existing: Option<(String, String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT id, url, transport, description, headers_helper
             FROM custom_mcp_servers WHERE user_id = ?1 AND name = ?2",
            params![SCIENCE_USER_ID, server.name],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("select custom mcp '{}': {e}", server.name))?;

    let now_ms = now_millis();
    let changed = if let Some((id, url, tr, old_desc, old_helper)) = existing {
        let same = url == server.url
            && tr == transport
            && old_desc.as_deref() == desc
            && old_helper.as_deref() == helper.as_deref();
        if same {
            ensure_assignment(conn, &id)?;
            return Ok(false);
        }
        conn.execute(
            "UPDATE custom_mcp_servers SET url = ?1, transport = ?2, description = ?3,
             headers_helper = ?4, source = 'custom', updated_at = ?5
             WHERE id = ?6",
            params![server.url, transport, desc, helper, now_ms, id],
        )
        .map_err(|e| format!("update custom mcp '{}': {e}", server.name))?;
        ensure_assignment(conn, &id)?;
        true
    } else {
        let id = new_uuid();
        conn.execute(
            "INSERT INTO custom_mcp_servers
             (id, user_id, name, description, url, transport, source, headers_helper,
              created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'custom', ?7, ?8, ?8)",
            params![
                id,
                SCIENCE_USER_ID,
                server.name,
                desc,
                server.url,
                transport,
                helper,
                now_ms
            ],
        )
        .map_err(|e| format!("insert custom mcp '{}': {e}", server.name))?;
        ensure_assignment(conn, &id)?;
        true
    };
    Ok(changed)
}

fn ensure_assignment(conn: &Connection, server_id: &str) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM mcp_agent_assignments
             WHERE mcp_server_id = ?1 AND agent_name = ?2 AND user_id = ?3",
            params![server_id, SCIENCE_AGENT, SCIENCE_USER_ID],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| format!("probe mcp assignment: {e}"))?
        .unwrap_or(false);
    if exists {
        return Ok(());
    }
    let id = new_uuid();
    let now_ms = now_millis();
    // excluded_tools is JSON text in the live schema.
    conn.execute(
        "INSERT INTO mcp_agent_assignments
         (id, mcp_server_id, agent_name, user_id, excluded_tools, created_at)
         VALUES (?1, ?2, ?3, ?4, '[]', ?5)",
        params![id, server_id, SCIENCE_AGENT, SCIENCE_USER_ID, now_ms],
    )
    .map_err(|e| format!("insert mcp assignment: {e}"))?;
    Ok(())
}

fn delete_by_name(conn: &Connection, name: &str) -> Result<bool, String> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM custom_mcp_servers WHERE user_id = ?1 AND name = ?2",
            params![SCIENCE_USER_ID, name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("lookup custom mcp '{name}' for delete: {e}"))?;
    let Some(id) = id else {
        return Ok(false);
    };
    // Assignments cascade in Science schema; delete explicitly for older DBs.
    let _ = conn.execute(
        "DELETE FROM mcp_agent_assignments WHERE mcp_server_id = ?1",
        params![id],
    );
    let n = conn
        .execute("DELETE FROM custom_mcp_servers WHERE id = ?1", params![id])
        .map_err(|e| format!("delete custom mcp '{name}': {e}"))?;
    Ok(n > 0)
}

/// Build a shell one-liner that prints `{"Header":"value",...}` for Science's
/// headers_helper runner. Empty map → `None` (NULL in DB).
pub(crate) fn headers_helper_command(
    headers: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    if headers.is_empty() {
        return None;
    }
    let json = serde_json::to_string(headers).ok()?;
    // Base64 avoids shell quoting hazards for arbitrary header values.
    use base64::{engine::general_purpose::STANDARD, Engine};
    let b64 = STANDARD.encode(json.as_bytes());
    Some(format!(
        "python3 -c \"import base64,sys; sys.stdout.buffer.write(base64.b64decode('{b64}'))\""
    ))
}

fn load_state(path: &Path) -> RemoteDeployState {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_state(path: &Path, state: &RemoteDeployState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create mcp dir: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    let data =
        serde_json::to_vec_pretty(state).map_err(|e| format!("serialize remote state: {e}"))?;
    fs::write(&tmp, &data).map_err(|e| format!("write remote state tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename remote state: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_uuid() -> String {
    // Match oauth_forge uuid_v4 style without adding a uuid crate dep.
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SALT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u128)
        .unwrap_or(0);
    let salt = SALT.fetch_add(1, Ordering::Relaxed) as u128;
    let a = (nanos ^ (salt << 48)) as u32;
    let b = ((nanos >> 32) as u16) & 0xffff;
    let c = (0x4000 | (((nanos >> 48) as u16) & 0x0fff)) as u16; // version 4
    let d = (0x8000 | ((salt as u16) & 0x3fff)) as u16; // variant
    let e = (nanos.wrapping_mul(0x9e37_79b9) ^ salt) as u64;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        a,
        b,
        c,
        d,
        e & 0xffff_ffff_ffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_manager::model::McpServerId;
    use std::collections::BTreeMap;
    use std::env;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn uniq() -> u64 {
        static C: AtomicU64 = AtomicU64::new(0);
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        n.wrapping_mul(1_000_003)
            .wrapping_add(C.fetch_add(1, Ordering::Relaxed))
    }

    fn remote(name: &str, url: &str) -> McpServer {
        McpServer {
            id: McpServerId::new(),
            name: name.to_string(),
            description: "d".into(),
            transport: McpTransport::StreamableHttp,
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: url.to_string(),
            headers: BTreeMap::new(),
            enabled: true,
            builtin: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn fixture_db() -> (PathBuf, PathBuf, PathBuf, String) {
        let base = env::temp_dir().join(format!("csp-remote-mcp-{}", uniq()));
        let sandbox_root = base.join("sandbox/home");
        let data_dir = sandbox_root.join(".claude-science");
        let real_dir = base.join("real/.claude-science");
        let org = "11111111-2222-4333-8444-555555555555";
        let org_dir = data_dir.join("orgs").join(org);
        fs::create_dir_all(&org_dir).unwrap();
        fs::create_dir_all(&real_dir).unwrap();
        let db = org_dir.join("operon-cli.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE custom_mcp_servers (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                url TEXT NOT NULL,
                transport TEXT NOT NULL,
                oauth_server_url TEXT,
                client_id TEXT,
                scopes TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                source TEXT NOT NULL DEFAULT 'custom',
                headers_helper TEXT,
                resource_identifier TEXT
            );
            CREATE TABLE mcp_agent_assignments (
                id TEXT PRIMARY KEY,
                mcp_server_id TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                user_id TEXT NOT NULL,
                created_at INTEGER,
                excluded_tools TEXT NOT NULL DEFAULT '[]'
            );",
        )
        .unwrap();
        (sandbox_root, data_dir, real_dir, org.to_string())
    }

    #[test]
    fn headers_helper_emits_python_json() {
        let mut h = BTreeMap::new();
        h.insert("Authorization".into(), "Bearer x".into());
        let cmd = headers_helper_command(&h).unwrap();
        assert!(cmd.contains("python3 -c"));
        assert!(cmd.contains("base64.b64decode"));
        // Round-trip: decode the embedded base64 payload.
        let start = cmd.find("b64decode('").unwrap() + "b64decode('".len();
        let end = cmd[start..].find('\'').unwrap() + start;
        let b64 = &cmd[start..end];
        use base64::{engine::general_purpose::STANDARD, Engine};
        let json = String::from_utf8(STANDARD.decode(b64).unwrap()).unwrap();
        assert!(json.contains("Authorization"));
        assert!(headers_helper_command(&BTreeMap::new()).is_none());
    }

    #[test]
    fn upsert_and_remove_remote() {
        let (sandbox_root, data_dir, real_dir, org) = fixture_db();
        let servers = vec![remote("demo-remote", "https://mcp.example.com/mcp")];
        let r1 =
            deploy_remote_mcp(&servers, &data_dir, &sandbox_root, &real_dir, Some(&org)).unwrap();
        assert!(r1.changed);
        assert_eq!(r1.deployed, vec!["demo-remote".to_string()]);

        let db = data_dir.join("orgs").join(&org).join("operon-cli.db");
        let conn = Connection::open(&db).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM custom_mcp_servers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let a: i64 = conn
            .query_row("SELECT COUNT(*) FROM mcp_agent_assignments", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(a, 1);

        let r2 =
            deploy_remote_mcp(&servers, &data_dir, &sandbox_root, &real_dir, Some(&org)).unwrap();
        assert!(!r2.changed, "identical redeploy is a no-op");

        let r3 = deploy_remote_mcp(&[], &data_dir, &sandbox_root, &real_dir, Some(&org)).unwrap();
        assert!(r3.changed);
        assert_eq!(r3.removed, vec!["demo-remote".to_string()]);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM custom_mcp_servers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);

        let _ = fs::remove_dir_all(sandbox_root.parent().unwrap());
    }

    #[test]
    fn missing_db_defers_without_error() {
        let base = env::temp_dir().join(format!("csp-remote-mcp-miss-{}", uniq()));
        let sandbox_root = base.join("sandbox/home");
        let data_dir = sandbox_root.join(".claude-science");
        let real_dir = base.join("real/.claude-science");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&real_dir).unwrap();
        let servers = vec![remote("x", "https://example.com/mcp")];
        let r = deploy_remote_mcp(
            &servers,
            &data_dir,
            &sandbox_root,
            &real_dir,
            Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"),
        )
        .unwrap();
        assert!(r.note.contains("deferred") || r.note.contains("missing"));
        let _ = fs::remove_dir_all(&base);
    }
}
