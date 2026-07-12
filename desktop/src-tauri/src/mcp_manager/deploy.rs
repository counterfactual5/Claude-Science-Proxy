//! Deploy enabled local stdio MCP servers into the isolated Science sandbox.
//!
//! Confirmed against a live sandbox: Claude Science reads user stdio connectors
//! from `<data-dir>/mcp/local-mcp.json` (shape
//! `{ "servers": [ { name, command, args, env, description? } ] }`) and they
//! surface as `source: "local-stdio"`. The restricted MCP child sandbox can only
//! read paths granted via `<data-dir>/config.toml` `[sandbox] user_read_paths`,
//! so we also grant read access to the parent directory of every absolute path a
//! server references (its command and any absolute-path args).
//!
//! Iron rules (mirror `skill_manager::deploy` / `oauth_forge`):
//! - Only ever write under the sandbox root; never the real `~/.claude-science`.
//! - CSP owns `local-mcp.json` and the `[sandbox].user_read_paths` key only;
//!   all other `config.toml` keys are preserved on read-modify-write.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Serialize;

use super::model::McpServer;
use crate::oauth_forge::real_ancestor;

const LOCAL_MCP_FILE: &str = "local-mcp.json";

/// Summary of a deployment pass (launch-log observability).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct McpDeployReport {
    /// Server names written to `local-mcp.json`.
    pub deployed: Vec<String>,
    /// Directories granted read access via `user_read_paths`.
    pub granted_paths: usize,
    /// Whether a stale `local-mcp.json` was removed (no enabled servers).
    pub cleared: bool,
}

#[derive(Serialize)]
struct LocalMcpFile<'a> {
    servers: Vec<LocalMcpEntry<'a>>,
}

#[derive(Serialize)]
struct LocalMcpEntry<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    command: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    args: &'a [String],
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    env: &'a BTreeMap<String, String>,
}

/// Deploy `enabled` stdio MCP servers into `<data_dir>/mcp/local-mcp.json` and
/// grant sandbox read access to referenced absolute paths.
///
/// `data_dir` is the sandbox Science data-dir; `sandbox_root` is `$SANDBOX_HOME`;
/// `real_science_dir` is the real `~/.claude-science` used only for the guard.
pub(crate) fn deploy_enabled_mcp(
    enabled: &[McpServer],
    data_dir: &Path,
    sandbox_root: &Path,
    real_science_dir: &Path,
) -> Result<McpDeployReport, String> {
    // —— Iron-rule guards: resolve to nearest real ancestor, then reject the real
    // Science tree and anything outside the sandbox root. ——
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

    let mcp_dir = data_dir.join("mcp");
    let mcp_json = mcp_dir.join(LOCAL_MCP_FILE);
    let config_toml = data_dir.join("config.toml");
    let mut report = McpDeployReport::default();

    if enabled.is_empty() {
        // Clear stale config so disabled/removed servers don't linger.
        if mcp_json.exists() {
            fs::remove_file(&mcp_json).map_err(|e| format!("remove stale local-mcp.json: {e}"))?;
            report.cleared = true;
        }
        set_user_read_paths(&config_toml, &[])?;
        return Ok(report);
    }

    fs::create_dir_all(&mcp_dir).map_err(|e| format!("create mcp dir: {e}"))?;

    let file = LocalMcpFile {
        servers: enabled
            .iter()
            .map(|s| LocalMcpEntry {
                name: &s.name,
                description: &s.description,
                command: &s.command,
                args: &s.args,
                env: &s.env,
            })
            .collect(),
    };
    let json =
        serde_json::to_vec_pretty(&file).map_err(|e| format!("serialize local-mcp.json: {e}"))?;
    let tmp = mcp_json.with_extension("json.tmp");
    fs::write(&tmp, &json).map_err(|e| format!("write local-mcp.json tmp: {e}"))?;
    fs::rename(&tmp, &mcp_json).map_err(|e| format!("rename local-mcp.json: {e}"))?;
    report.deployed = enabled.iter().map(|s| s.name.clone()).collect();

    // Grant read access to the parent dir of every absolute path referenced.
    let read_paths = collect_read_paths(enabled);
    report.granted_paths = read_paths.len();
    set_user_read_paths(&config_toml, &read_paths)?;

    Ok(report)
}

/// Gather parent directories of absolute paths referenced by servers (command +
/// args). These are the paths Science's MCP child sandbox must be allowed to read.
fn collect_read_paths(servers: &[McpServer]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for s in servers {
        for candidate in std::iter::once(&s.command).chain(s.args.iter()) {
            let p = Path::new(candidate);
            if !p.is_absolute() {
                continue;
            }
            // Grant the containing directory (least privilege: not the whole tree).
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    set.insert(parent.to_string_lossy().to_string());
                }
            }
        }
    }
    set.into_iter().collect()
}

/// Read-modify-write `config.toml`, owning only `[sandbox].user_read_paths`.
///
/// All other keys/tables are preserved. An empty `paths` removes the key (and the
/// `[sandbox]` table if it becomes empty), and deletes `config.toml` entirely if
/// nothing else remains — so a clean sandbox never keeps stale grants.
fn set_user_read_paths(config_toml: &Path, paths: &[String]) -> Result<(), String> {
    use toml::Value;

    let mut root: toml::Table = if config_toml.exists() {
        let text = fs::read_to_string(config_toml).map_err(|e| format!("read config.toml: {e}"))?;
        text.parse::<toml::Table>()
            .map_err(|e| format!("parse config.toml: {e}"))?
    } else {
        toml::Table::new()
    };

    if paths.is_empty() {
        // Strip our key; drop [sandbox] if empty; drop the file if empty.
        if let Some(Value::Table(sandbox)) = root.get_mut("sandbox") {
            sandbox.remove("user_read_paths");
            let empty = sandbox.is_empty();
            if empty {
                root.remove("sandbox");
            }
        }
        if root.is_empty() {
            if config_toml.exists() {
                fs::remove_file(config_toml)
                    .map_err(|e| format!("remove empty config.toml: {e}"))?;
            }
            return Ok(());
        }
    } else {
        let arr: Vec<Value> = paths.iter().map(|p| Value::String(p.clone())).collect();
        let sandbox = root
            .entry("sandbox".to_string())
            .or_insert_with(|| Value::Table(toml::Table::new()));
        match sandbox {
            Value::Table(t) => {
                t.insert("user_read_paths".to_string(), Value::Array(arr));
            }
            // `sandbox` existed as a non-table; refuse to clobber unexpected shape.
            _ => {
                return Err("config.toml [sandbox] is not a table".to_string());
            }
        }
    }

    let serialized =
        toml::to_string_pretty(&root).map_err(|e| format!("serialize config.toml: {e}"))?;
    let tmp = config_toml.with_extension("toml.tmp");
    fs::write(&tmp, serialized.as_bytes()).map_err(|e| format!("write config.toml tmp: {e}"))?;
    fs::rename(&tmp, config_toml).map_err(|e| format!("rename config.toml: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_manager::model::McpServerId;
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

    fn server(name: &str, command: &str, args: Vec<String>) -> McpServer {
        McpServer {
            id: McpServerId::new(),
            name: name.to_string(),
            description: String::new(),
            command: command.to_string(),
            args,
            env: BTreeMap::new(),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    struct Fixture {
        sandbox_root: PathBuf,
        data_dir: PathBuf,
        real_dir: PathBuf,
    }

    fn fixture() -> Fixture {
        let base = env::temp_dir().join(format!("csp-mcp-deploy-{}", uniq()));
        let sandbox_root = base.join("sandbox/home");
        let data_dir = sandbox_root.join(".claude-science");
        let real_dir = base.join("real/.claude-science");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&real_dir).unwrap();
        Fixture {
            sandbox_root,
            data_dir,
            real_dir,
        }
    }

    #[test]
    fn writes_local_mcp_json() {
        let f = fixture();
        let servers = vec![server("demo", "python3", vec!["/opt/x/server.py".into()])];
        let r = deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert_eq!(r.deployed, vec!["demo".to_string()]);

        let json = fs::read_to_string(f.data_dir.join("mcp").join(LOCAL_MCP_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"][0]["name"], "demo");
        assert_eq!(v["servers"][0]["command"], "python3");
        assert_eq!(v["servers"][0]["args"][0], "/opt/x/server.py");
        // empty description/env must be omitted
        assert!(v["servers"][0].get("description").is_none());
        assert!(v["servers"][0].get("env").is_none());
    }

    #[test]
    fn grants_read_path_for_absolute_arg() {
        let f = fixture();
        let servers = vec![server("demo", "python3", vec!["/opt/tools/mcp/server.py".into()])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let toml = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        assert!(toml.contains("[sandbox]"));
        assert!(toml.contains("user_read_paths"));
        assert!(toml.contains("/opt/tools/mcp"));
    }

    #[test]
    fn preserves_other_config_keys() {
        let f = fixture();
        // Pre-existing unrelated config the user set.
        fs::write(
            f.data_dir.join("config.toml"),
            "[verification]\nenabled = false\n",
        )
        .unwrap();
        let servers = vec![server("demo", "/usr/local/bin/mcp-server", vec![])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let text = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        let parsed: toml::Table = text.parse().unwrap();
        assert_eq!(parsed["verification"]["enabled"].as_bool(), Some(false));
        assert!(parsed["sandbox"]["user_read_paths"].is_array());
    }

    #[test]
    fn empty_enabled_clears_json_and_owns_key_only() {
        let f = fixture();
        // First deploy something, plus an unrelated key.
        fs::write(
            f.data_dir.join("config.toml"),
            "[verification]\nenabled = false\n",
        )
        .unwrap();
        let servers = vec![server("demo", "python3", vec!["/opt/x/s.py".into()])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(f.data_dir.join("mcp").join(LOCAL_MCP_FILE).exists());

        // Now disable all: json removed, our key gone, unrelated key preserved.
        let r = deploy_enabled_mcp(&[], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(r.cleared);
        assert!(!f.data_dir.join("mcp").join(LOCAL_MCP_FILE).exists());
        let text = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        let parsed: toml::Table = text.parse().unwrap();
        assert_eq!(parsed["verification"]["enabled"].as_bool(), Some(false));
        assert!(parsed.get("sandbox").is_none());
    }

    #[test]
    fn empty_enabled_removes_config_when_nothing_else() {
        let f = fixture();
        let servers = vec![server("demo", "python3", vec!["/opt/x/s.py".into()])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(f.data_dir.join("config.toml").exists());

        deploy_enabled_mcp(&[], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        // config.toml held only our key → removed entirely.
        assert!(!f.data_dir.join("config.toml").exists());
    }

    #[test]
    fn rejects_real_science_dir() {
        let f = fixture();
        let servers = vec![server("x", "python3", vec![])];
        let r = deploy_enabled_mcp(&servers, &f.real_dir, &f.sandbox_root, &f.real_dir);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("real Science dir"));
    }

    #[test]
    fn rejects_outside_sandbox() {
        let f = fixture();
        let outside = env::temp_dir().join(format!("csp-mcp-outside-{}", uniq()));
        fs::create_dir_all(&outside).unwrap();
        let servers = vec![server("x", "python3", vec![])];
        let r = deploy_enabled_mcp(&servers, &outside, &f.sandbox_root, &f.real_dir);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("outside sandbox root"));
    }
}
