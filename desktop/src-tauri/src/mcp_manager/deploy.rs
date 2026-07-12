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
//! Egress: Science injects `HTTPS_PROXY=http://localhost:<operon-port>` into
//! every MCP child, and the child's OS-level sandbox network policy only
//! permits outbound connections to that one loopback address — any other
//! local port (including one CSP might run itself) is denied with `EPERM`.
//! Operon's proxy supports a real CONNECT tunnel, but many bundled Node HTTP
//! clients (axios via `follow-redirects`, used by e.g.
//! `@notionhq/notion-mcp-server`) never issue one for HTTPS targets — they
//! relay the request in absolute-form instead, which Operon then forwards as
//! plain HTTP onto the origin's port 443 (`400 The plain HTTP request was
//! sent to HTTPS port`). We fix this client-side: `mcp_http_tunnel_shim.cjs`
//! is written into `<data-dir>/mcp/` and loaded into Node via
//! `--require`. Live probe showed Science **strips `NODE_OPTIONS` from
//! `local-mcp.json` env** (token env is kept; `NODE_OPTIONS` is absent on
//! the running MCP process), so we wrap the connector with `/bin/bash` that
//! re-exports `NODE_OPTIONS` immediately before `exec` — Operon's proxy env
//! is left untouched. The shim then turns absolute-form requests into a
//! real CONNECT+TLS tunnel to the already-permitted Operon address.
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
const SHIM_FILE: &str = "csp-http-tunnel-shim.cjs";
const SHIM_SOURCE: &str = include_str!("mcp_http_tunnel_shim.cjs");

/// Summary of a deployment pass (launch-log observability).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct McpDeployReport {
    /// Server names written to `local-mcp.json`.
    pub deployed: Vec<String>,
    /// Directories granted read access via `user_read_paths`.
    pub granted_paths: usize,
    /// Whether a stale `local-mcp.json` was removed (no enabled servers).
    pub cleared: bool,
    /// Whether anything on disk actually changed this pass. Lets a caller decide
    /// whether a running sandbox needs restarting for Science to re-read config.
    pub changed: bool,
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
    command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
}

/// Idempotently write the bundled Node shim (see `mcp_http_tunnel_shim.cjs`)
/// into `<data_dir>/mcp/` and return its path, or `None` on write failure
/// (never fatal — deployment continues without the shim).
fn write_shim(mcp_dir: &Path) -> Option<std::path::PathBuf> {
    let path = mcp_dir.join(SHIM_FILE);
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() != Some(SHIM_SOURCE) && fs::write(&path, SHIM_SOURCE).is_err() {
        return None;
    }
    Some(path)
}

struct DeployCmd {
    command: String,
    args: Vec<String>,
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// True when the file starts with `#!/usr/bin/env node` (optional `-S` and flags).
fn shebang_wants_env_node(path: &Path) -> bool {
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    use std::io::Read;
    let mut buf = [0u8; 128];
    let n = f.read(&mut buf).unwrap_or(0);
    if n < 12 {
        return false;
    }
    let line = String::from_utf8_lossy(&buf[..n]);
    let first = line.lines().next().unwrap_or("");
    first == "#!/usr/bin/env node"
        || first.starts_with("#!/usr/bin/env -S node")
        || first.starts_with("#!/usr/bin/env node ")
}

/// Locate a `node` binary for a script installed via npm-style layouts.
fn find_node_for_script(command: &Path) -> Option<std::path::PathBuf> {
    if let Some(parent) = command.parent() {
        let sibling = parent.join("node");
        if is_executable(&sibling) {
            return Some(sibling);
        }
    }
    let real = fs::canonicalize(command).ok()?;
    let mut cur = real.parent();
    while let Some(dir) = cur {
        let same_dir = dir.join("node");
        if is_executable(&same_dir) {
            return Some(same_dir);
        }
        if let Some(parent) = dir.parent() {
            let bin_node = parent.join("bin").join("node");
            if is_executable(&bin_node) {
                return Some(bin_node);
            }
        }
        cur = dir.parent();
    }
    None
}

/// Rewrite `#!/usr/bin/env node` shims to an absolute `node <script>` invocation.
///
/// Science's MCP child sandbox does not inherit the host PATH, so `env node`
/// fails with `No such file or directory` even when `user_read_paths` grants
/// the script and its package tree (confirmed with global npm shims such as
/// `notion-mcp-server`).
fn resolve_node_shebang(command: &str, args: &[String]) -> (String, Vec<String>) {
    let cmd_path = Path::new(command);
    if !cmd_path.is_absolute() {
        return (command.to_string(), args.to_vec());
    }
    let script = if shebang_wants_env_node(cmd_path) {
        cmd_path.to_path_buf()
    } else if let Ok(real) = fs::canonicalize(cmd_path) {
        if shebang_wants_env_node(&real) {
            real
        } else {
            return (command.to_string(), args.to_vec());
        }
    } else {
        return (command.to_string(), args.to_vec());
    };
    let Some(node) = find_node_for_script(cmd_path) else {
        return (command.to_string(), args.to_vec());
    };
    let node = fs::canonicalize(&node).unwrap_or(node);
    let script = fs::canonicalize(&script).unwrap_or(script);
    let mut out_args = vec![script.to_string_lossy().into_owned()];
    out_args.extend(args.iter().cloned());
    (node.to_string_lossy().into_owned(), out_args)
}

/// Wrap the connector so Node loads the HTTP-tunnel shim.
///
/// Science strips `NODE_OPTIONS` from `local-mcp.json` env (confirmed live:
/// `NOTION_TOKEN` reaches the MCP child, `NODE_OPTIONS` does not). Putting
/// `--require` only in env therefore never loads the shim. Instead wrap with
/// bash that re-exports `NODE_OPTIONS` right before `exec`, preserving any
/// user-set `NODE_OPTIONS` from the connector env (passed as `$1`). Non-Node
/// commands ignore `NODE_OPTIONS`; Operon proxy env is left alone.
fn deploy_command(server: &McpServer, shim_path: Option<&Path>) -> DeployCmd {
    let (real_command, real_args) = resolve_node_shebang(&server.command, &server.args);
    let Some(shim) = shim_path else {
        return DeployCmd {
            command: real_command,
            args: real_args,
        };
    };
    // bash -c '…' name USER_NODE_OPTIONS REAL_CMD REAL_ARGS…
    // $1 = optional user NODE_OPTIONS; after shift, "$@" is the real connector.
    // Prepend our --require; append any user flags so theirs still apply.
    let script = format!(
        "export NODE_OPTIONS=\"--require {shim}${{1:+ $1}}\"; shift; exec \"$@\"",
        shim = shim.display()
    );
    let user_node_options = server
        .env
        .get("NODE_OPTIONS")
        .cloned()
        .unwrap_or_default();
    let mut bash_args = vec![
        "-c".to_string(),
        script,
        "csp-mcp-shim".to_string(),
        user_node_options,
        real_command,
    ];
    bash_args.extend(real_args);
    DeployCmd {
        command: "/bin/bash".to_string(),
        args: bash_args,
    }
}

/// Keep the user's connector env as-is. Do **not** put `NODE_OPTIONS` here —
/// Science strips that key from `local-mcp.json` env. Shim loading is done by
/// [`deploy_command`]'s bash preamble instead.
fn effective_env(server: &McpServer) -> BTreeMap<String, String> {
    let mut env = server.env.clone();
    // Avoid a misleading entry that Science would drop anyway; the bash
    // wrapper already merges any user NODE_OPTIONS into the real process env.
    env.remove("NODE_OPTIONS");
    env
}

/// Deploy `enabled` stdio MCP servers into `<data_dir>/mcp/local-mcp.json` and
/// grant sandbox read access to referenced absolute paths.
///
/// `data_dir` is the sandbox Science data-dir; `sandbox_root` is `$SANDBOX_HOME`;
/// `real_science_dir` is the real `~/.claude-science` used only for the guard.
/// Also writes the Node HTTP-tunnel shim (see module docs) into
/// `<data_dir>/mcp/` and wraps each connector so bash re-exports `NODE_OPTIONS`
/// before exec (Science strips that key from `local-mcp.json` env).
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
            report.changed = true;
        }
        let paths_changed = set_user_read_paths(&config_toml, &[])?;
        report.changed = report.changed || paths_changed;
        return Ok(report);
    }

    fs::create_dir_all(&mcp_dir).map_err(|e| format!("create mcp dir: {e}"))?;
    let shim_path = write_shim(&mcp_dir);

    let file = LocalMcpFile {
        servers: enabled
            .iter()
            .map(|s| {
                let cmd = deploy_command(s, shim_path.as_deref());
                LocalMcpEntry {
                    name: &s.name,
                    description: &s.description,
                    command: cmd.command,
                    args: cmd.args,
                    env: effective_env(s),
                }
            })
            .collect(),
    };
    let json =
        serde_json::to_vec_pretty(&file).map_err(|e| format!("serialize local-mcp.json: {e}"))?;
    // Idempotent write: only touch disk when the bytes actually differ, so a
    // reopen of an unchanged config does not look like a change.
    let current = fs::read(&mcp_json).ok();
    if current.as_deref() != Some(json.as_slice()) {
        let tmp = mcp_json.with_extension("json.tmp");
        fs::write(&tmp, &json).map_err(|e| format!("write local-mcp.json tmp: {e}"))?;
        // `env` may hold plaintext secrets, so lock the file down (0600) to match
        // the CSP inventory before it becomes visible under its final name.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &mcp_json).map_err(|e| format!("rename local-mcp.json: {e}"))?;
        report.changed = true;
    } else {
        // Even when content is unchanged, make sure perms are tight (e.g. a file
        // written by an older build with a laxer umask).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&mcp_json, fs::Permissions::from_mode(0o600));
        }
    }
    report.deployed = enabled.iter().map(|s| s.name.clone()).collect();

    // Grant read access for every absolute path referenced (least privilege),
    // plus the mcp dir itself so the sandboxed child can `--require` the shim.
    let mut read_paths = collect_read_paths(enabled);
    if shim_path.is_some() {
        let mcp_dir_str = mcp_dir.to_string_lossy().to_string();
        if !read_paths.iter().any(|p| p == &mcp_dir_str) {
            read_paths.push(mcp_dir_str);
            read_paths.sort();
        }
    }
    report.granted_paths = read_paths.len();
    let paths_changed = set_user_read_paths(&config_toml, &read_paths)?;
    report.changed = report.changed || paths_changed;

    Ok(report)
}

/// Gather the least-privilege set of read paths for absolute paths referenced by
/// servers (command + args). Granularity:
/// - an existing **directory** → grant the directory itself;
/// - an existing **file** → grant its parent directory;
/// - a **non-existent** path → assume a file and grant its parent directory.
/// - a **symlink** → also grant the canonical target; for global Node shims this
///   includes the package root under `node_modules` so the script can load its
///   adjacent modules.
///
/// This avoids over-granting (e.g. an arg of `/Users/me/data` no longer opens up
/// all of `/Users/me`).
fn collect_read_paths(servers: &[McpServer]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for s in servers {
        let (command, args) = resolve_node_shebang(&s.command, &s.args);
        collect_read_paths_for(&command, &args, &mut set);
    }
    set.into_iter().collect()
}

fn collect_read_paths_for(command: &str, args: &[String], set: &mut BTreeSet<String>) {
    for candidate in std::iter::once(command).chain(args.iter().map(String::as_str)) {
        let p = Path::new(candidate);
        if !p.is_absolute() {
            continue;
        }
        let grant = if p.is_dir() {
            Some(p.to_path_buf())
        } else {
            // File (or missing → assume file): grant the containing directory.
            p.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.to_path_buf())
        };
        if let Some(dir) = grant {
            set.insert(dir.to_string_lossy().to_string());
        }
        if let Ok(real) = fs::canonicalize(p) {
            add_grant_for_path(&real, set);
            if let Some(pkg_root) = node_package_root(&real) {
                set.insert(pkg_root.to_string_lossy().to_string());
            }
        }
    }
}

fn add_grant_for_path(p: &Path, set: &mut BTreeSet<String>) {
    let grant = if p.is_dir() {
        Some(p.to_path_buf())
    } else {
        p.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.to_path_buf())
    };
    if let Some(dir) = grant {
        set.insert(dir.to_string_lossy().to_string());
    }
}

/// Return the package root for paths inside `node_modules/<package>/...`,
/// including scoped packages such as `node_modules/@notionhq/notion-mcp-server`.
fn node_package_root(p: &Path) -> Option<std::path::PathBuf> {
    let comps: Vec<_> = p.components().collect();
    for (idx, comp) in comps.iter().enumerate() {
        if comp.as_os_str() != "node_modules" {
            continue;
        }
        let name_idx = idx + 1;
        let first = comps.get(name_idx)?.as_os_str().to_string_lossy();
        let root_end = if first.starts_with('@') {
            name_idx + 2
        } else {
            name_idx + 1
        };
        let mut root = std::path::PathBuf::new();
        for c in comps.iter().take(root_end) {
            root.push(c.as_os_str());
        }
        return Some(root);
    }
    None
}

/// Read-modify-write `config.toml`, owning only `[sandbox].user_read_paths`.
///
/// All other keys/tables are preserved. An empty `paths` removes the key (and the
/// `[sandbox]` table if it becomes empty), and deletes `config.toml` entirely if
/// nothing else remains — so a clean sandbox never keeps stale grants.
///
/// Returns `true` when the file on disk actually changed (idempotent otherwise).
fn set_user_read_paths(config_toml: &Path, paths: &[String]) -> Result<bool, String> {
    use toml::Value;

    let root: toml::Table = if config_toml.exists() {
        fs::read_to_string(config_toml)
            .map_err(|e| format!("read config.toml: {e}"))?
            .parse::<toml::Table>()
            .map_err(|e| format!("parse config.toml: {e}"))?
    } else {
        toml::Table::new()
    };
    // Compare against the parsed (semantic) form, not raw bytes: a TOML round-trip
    // reflows formatting/comments, so byte comparison would falsely report a
    // change on every reopen and trigger needless sandbox restarts.
    let before = root.clone();
    let mut root = root;

    if paths.is_empty() {
        // Strip our key; drop [sandbox] if it becomes empty.
        if let Some(Value::Table(sandbox)) = root.get_mut("sandbox") {
            sandbox.remove("user_read_paths");
            if sandbox.is_empty() {
                root.remove("sandbox");
            }
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

    if root == before {
        // No semantic change: leave the file (and any comments) untouched.
        return Ok(false);
    }

    if root.is_empty() {
        // Our key was the only content: remove the file so nothing lingers.
        if config_toml.exists() {
            fs::remove_file(config_toml).map_err(|e| format!("remove empty config.toml: {e}"))?;
        }
        return Ok(true);
    }

    let serialized =
        toml::to_string_pretty(&root).map_err(|e| format!("serialize config.toml: {e}"))?;
    let tmp = config_toml.with_extension("toml.tmp");
    fs::write(&tmp, serialized.as_bytes()).map_err(|e| format!("write config.toml tmp: {e}"))?;
    fs::rename(&tmp, config_toml).map_err(|e| format!("rename config.toml: {e}"))?;
    Ok(true)
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
        // Command is wrapped with bash so NODE_OPTIONS survives Science's env strip.
        assert_eq!(v["servers"][0]["command"], "/bin/bash");
        assert_eq!(v["servers"][0]["args"][0], "-c");
        assert_eq!(v["servers"][0]["args"][2], "csp-mcp-shim");
        assert_eq!(v["servers"][0]["args"][4], "python3");
        assert_eq!(v["servers"][0]["args"][5], "/opt/x/server.py");
        // empty description/env omitted (shim is argv-side, not env)
        assert!(v["servers"][0].get("description").is_none());
        assert!(v["servers"][0].get("env").is_none());
    }

    #[test]
    fn wraps_with_bash_to_export_node_options_shim() {
        let f = fixture();
        let mut s = server("demo", "node", vec!["/opt/x/server.js".into()]);
        s.env.insert("NOTION_TOKEN".into(), "ntn_secret".into());
        deploy_enabled_mcp(&[s], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let json = fs::read_to_string(f.data_dir.join("mcp").join(LOCAL_MCP_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Science strips NODE_OPTIONS from env — load shim via bash export instead.
        assert_eq!(v["servers"][0]["command"], "/bin/bash");
        let args = v["servers"][0]["args"].as_array().unwrap();
        assert_eq!(args[0], "-c");
        assert!(args[1].as_str().unwrap().contains("--require"));
        assert!(args[1].as_str().unwrap().contains(SHIM_FILE));
        assert_eq!(args[2], "csp-mcp-shim");
        assert_eq!(args[3], ""); // no user NODE_OPTIONS
        assert_eq!(args[4], "node");
        assert_eq!(args[5], "/opt/x/server.js");
        assert_eq!(v["servers"][0]["env"]["NOTION_TOKEN"], "ntn_secret");
        assert!(v["servers"][0]["env"].get("NODE_OPTIONS").is_none());

        let shim_path = f.data_dir.join("mcp").join(SHIM_FILE);
        assert!(shim_path.exists(), "shim file must be written to disk");
        let toml = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        assert!(
            toml.contains(&f.data_dir.join("mcp").to_string_lossy().to_string()),
            "mcp dir itself must be readable so the sandboxed child can require the shim"
        );
    }

    #[test]
    fn preserves_existing_node_options_via_bash_arg() {
        let f = fixture();
        let mut s = server("demo", "node", vec!["/opt/x/server.js".into()]);
        s.env
            .insert("NODE_OPTIONS".into(), "--max-old-space-size=256".into());
        deploy_enabled_mcp(&[s], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let json = fs::read_to_string(f.data_dir.join("mcp").join(LOCAL_MCP_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // User NODE_OPTIONS is passed as $1 to bash (not left in env JSON).
        assert_eq!(v["servers"][0]["args"][3], "--max-old-space-size=256");
        assert!(v["servers"][0]["env"].get("NODE_OPTIONS").is_none());
    }

    #[test]
    fn grants_read_path_for_absolute_arg() {
        let f = fixture();
        let servers = vec![server(
            "demo",
            "python3",
            vec!["/opt/tools/mcp/server.py".into()],
        )];
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
    fn second_identical_deploy_reports_no_change() {
        let f = fixture();
        let servers = vec![server("demo", "python3", vec!["/opt/x/server.py".into()])];
        let r1 = deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(r1.changed, "first deploy writes files");
        let r2 = deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(!r2.changed, "identical redeploy is a no-op");
    }

    #[cfg(unix)]
    #[test]
    fn local_mcp_json_is_locked_down() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture();
        let mut s = server("demo", "python3", vec!["/opt/x/server.py".into()]);
        s.env.insert("TOKEN".into(), "supersecret".into());
        deploy_enabled_mcp(&[s], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        let json = f.data_dir.join("mcp").join(LOCAL_MCP_FILE);
        let mode = fs::metadata(&json).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "secret-bearing local-mcp.json must be 0600");
    }

    #[test]
    fn empty_deploy_leaves_unrelated_config_untouched_and_unchanged() {
        let f = fixture();
        // Sandbox has an unrelated config with a comment CSP doesn't own.
        let original = "# keep me\n[verification]\nenabled = false\n";
        fs::write(f.data_dir.join("config.toml"), original).unwrap();

        // No enabled servers, twice: must be a no-op and preserve the file verbatim.
        let r1 = deploy_enabled_mcp(&[], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(!r1.changed, "no MCP + unrelated config → no change");
        let after = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        assert_eq!(after, original, "file (and comment) left byte-for-byte");
    }

    #[test]
    fn repeated_reopen_with_config_reports_no_change() {
        let f = fixture();
        fs::write(
            f.data_dir.join("config.toml"),
            "[verification]\nenabled = true\n",
        )
        .unwrap();
        let servers = vec![server("demo", "python3", vec!["/opt/x/server.py".into()])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        // Second identical pass over an existing config must not report a change.
        let r = deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(!r.changed);
    }

    #[test]
    fn existing_directory_arg_grants_itself_not_parent() {
        let f = fixture();
        // Real directory referenced directly as an arg.
        let data_root = env::temp_dir().join(format!("csp-mcp-datadir-{}", uniq()));
        let data_sub = data_root.join("payload");
        fs::create_dir_all(&data_sub).unwrap();
        let servers = vec![server(
            "demo",
            "python3",
            vec![data_sub.to_string_lossy().into()],
        )];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let toml = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        // Grants the dir itself, NOT the broader parent.
        assert!(toml.contains(&data_sub.to_string_lossy().to_string()));
        assert!(!toml.contains(&format!("\"{}\"", data_root.to_string_lossy())));
        let _ = fs::remove_dir_all(&data_root);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_node_command_grants_real_package_root() {
        use std::os::unix::fs::symlink;

        let f = fixture();
        let tool_root = env::temp_dir().join(format!("csp-mcp-node-{}", uniq()));
        let bin = tool_root.join("bin");
        let pkg = tool_root.join("lib/node_modules/@notionhq/notion-mcp-server");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(pkg.join("bin")).unwrap();
        fs::write(pkg.join("package.json"), "{}").unwrap();
        fs::write(pkg.join("bin/cli.mjs"), "#!/usr/bin/env node\n").unwrap();
        fs::write(bin.join("node"), b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(bin.join("node"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        symlink(
            pkg.join("bin/cli.mjs"),
            bin.join("notion-mcp-server"),
        )
        .unwrap();

        let shim_cmd = bin.join("notion-mcp-server").to_string_lossy().to_string();
        let servers = vec![server("notion", &shim_cmd, vec![])];
        deploy_enabled_mcp(&servers, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        let json = fs::read_to_string(f.data_dir.join("mcp").join(LOCAL_MCP_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let args = v["servers"][0]["args"].as_array().unwrap();
        let node_arg = args[4].as_str().unwrap();
        let script_arg = args[5].as_str().unwrap();
        assert!(
            node_arg.ends_with("/bin/node"),
            "env-node shim must be rewritten to absolute node, got {node_arg}"
        );
        assert!(
            script_arg.contains("cli.mjs"),
            "script path must be canonical, got {script_arg}"
        );

        let toml = fs::read_to_string(f.data_dir.join("config.toml")).unwrap();
        assert!(
            toml.contains(&bin.to_string_lossy().to_string()),
            "the symlink directory itself remains granted"
        );
        assert!(
            toml.contains(&pkg.to_string_lossy().to_string()),
            "global Node shims also need the real package root granted"
        );
        let _ = fs::remove_dir_all(&tool_root);
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
