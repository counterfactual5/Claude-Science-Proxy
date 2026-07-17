//! MCP server store — persists stdio + remote connector definitions in
//! `~/.csp/mcp/inventory.json` (0600). Mirrors `skill_manager::store` in spirit:
//! a dependency-light JSON inventory that the UI lists and the deployer reads.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::model::{McpInspection, McpServer, McpServerId, McpServerSummary, McpTransport};

const STORE_DIR: &str = "mcp";
const INVENTORY_FILE: &str = "inventory.json";
const MAX_SERVERS: usize = 64;

/// Executables Claude Science's managed runtime resolves for local stdio MCP.
/// Anything else must be an absolute path; otherwise we warn (not hard-fail),
/// since the whitelist may widen upstream.
const KNOWN_COMMANDS: &[&str] = &[
    "node", "npm", "npx", "python", "python3", "pip", "pip3", "uv", "uvx", "deno", "bun", "bunx",
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Inventory {
    servers: BTreeMap<String, McpServer>,
}

pub struct McpStore {
    root: PathBuf,
}

/// Fields accepted from the UI to create or update a server. `env` / `headers`
/// values are stored verbatim; only ever returned masked.
#[derive(Clone, Debug, Default)]
pub struct McpServerInput {
    pub name: String,
    pub description: String,
    pub transport: McpTransport,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

impl McpStore {
    /// Open the store at `~/.csp/mcp/`. Creates it if missing.
    pub fn open() -> Result<Self, String> {
        let root = crate::config::default_dir().join(STORE_DIR);
        fs::create_dir_all(&root).map_err(|e| format!("create mcp store dir: {e}"))?;
        Ok(Self { root })
    }

    fn inventory_path(&self) -> PathBuf {
        self.root.join(INVENTORY_FILE)
    }

    fn load_inventory(&self) -> Result<Inventory, String> {
        let path = self.inventory_path();
        if !path.exists() {
            return Ok(Inventory::default());
        }
        let data = fs::read(&path).map_err(|e| format!("read mcp inventory: {e}"))?;
        serde_json::from_slice(&data).map_err(|e| format!("parse mcp inventory: {e}"))
    }

    fn save_inventory(&self, inv: &Inventory) -> Result<(), String> {
        let path = self.inventory_path();
        let tmp = path.with_extension("json.tmp");
        let data =
            serde_json::to_vec_pretty(inv).map_err(|e| format!("serialize mcp inventory: {e}"))?;
        fs::write(&tmp, &data).map_err(|e| format!("write mcp inventory tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename mcp inventory: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// List all servers as UI summaries (env masked).
    pub fn list(&self) -> Result<Vec<McpServerSummary>, String> {
        let inv = self.load_inventory()?;
        Ok(inv.servers.values().map(McpServer::to_summary).collect())
    }

    /// Validate a proposed server without persisting it.
    pub fn inspect(input: &McpServerInput) -> McpInspection {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        let name = input.name.trim();
        if name.is_empty() {
            errors.push("Name is required".to_string());
        } else if input.transport.is_remote() {
            // Science `custom_mcp_servers` name rules (ox_): lowercase, digits,
            // hyphens; 1–64 chars; no leading/trailing hyphen; no anthropic/claude.
            if name.len() > 64 {
                errors.push("Remote MCP name must be at most 64 characters".to_string());
            }
            if !name
                .chars()
                .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-'))
            {
                errors.push(
                    "Remote MCP name may only contain lowercase letters, digits, and '-'"
                        .to_string(),
                );
            }
            if name.starts_with('-') || name.ends_with('-') {
                errors.push("Remote MCP name may not start or end with '-'".to_string());
            }
            if name.contains("anthropic") || name.contains("claude") {
                errors.push("Remote MCP name may not contain 'anthropic' or 'claude'".to_string());
            }
        } else if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            errors.push("Name may only contain letters, digits, '-', '_' and '.'".to_string());
        }

        let command_ok = if input.transport.is_remote() {
            inspect_remote(input, &mut warnings, &mut errors);
            true
        } else {
            inspect_stdio(input, &mut warnings, &mut errors)
        };

        McpInspection {
            valid: errors.is_empty(),
            command_ok,
            warnings,
            errors,
        }
    }

    /// Create a new server. Rejects duplicate names and invalid input.
    /// Returns a masked summary (env values never leave the store in clear).
    pub fn create(&self, input: McpServerInput) -> Result<McpServerSummary, String> {
        let inspection = Self::inspect(&input);
        if !inspection.valid {
            return Err(inspection.errors.join("; "));
        }
        let mut inv = self.load_inventory()?;
        if inv.servers.len() >= MAX_SERVERS {
            return Err(format!("Too many MCP servers (max {MAX_SERVERS})"));
        }
        let name = input.name.trim().to_string();
        if inv.servers.values().any(|s| s.name == name) {
            return Err(format!("An MCP server named '{name}' already exists"));
        }

        let now = current_iso8601();
        let id = McpServerId::new();
        // On create there is no prior value, so empty env/header values are stored as-is.
        let mut server = McpServer {
            id: id.clone(),
            name,
            description: input.description.trim().to_string(),
            transport: input.transport.clone(),
            command: input.command.trim().to_string(),
            args: input.args,
            env: input.env,
            url: input.url.trim().to_string(),
            headers: input.headers,
            enabled: true,
            builtin: false,
            created_at: now.clone(),
            updated_at: now,
        };
        if server.transport.is_remote() {
            server.command.clear();
            server.args.clear();
            server.env.clear();
        } else {
            server.url.clear();
            server.headers.clear();
        }
        let summary = server.to_summary();
        inv.servers.insert(id.to_string(), server);
        self.save_inventory(&inv)?;
        Ok(summary)
    }

    /// Update an existing server's definition.
    ///
    /// Env/header merge semantics (so the UI never has to round-trip masked secrets):
    /// - a key present with a **non-empty** value → updated;
    /// - a key present with an **empty** value → keep the previously stored value
    ///   (or empty if it never existed);
    /// - a key **absent** from the input → deleted.
    pub fn update(&self, id: &str, input: McpServerInput) -> Result<McpServerSummary, String> {
        let inspection = Self::inspect(&input);
        if !inspection.valid {
            return Err(inspection.errors.join("; "));
        }
        let mut inv = self.load_inventory()?;
        let name = input.name.trim().to_string();
        // Name uniqueness against *other* servers.
        if inv
            .servers
            .iter()
            .any(|(sid, s)| sid != id && s.name == name)
        {
            return Err(format!("An MCP server named '{name}' already exists"));
        }
        let server = inv
            .servers
            .get_mut(id)
            .ok_or_else(|| format!("MCP server not found: {id}"))?;
        let merged_env = merge_env(&server.env, &input.env);
        let merged_headers = merge_env(&server.headers, &input.headers);
        server.name = name;
        server.description = input.description.trim().to_string();
        server.transport = input.transport.clone();
        server.command = input.command.trim().to_string();
        server.args = input.args;
        server.env = merged_env;
        server.url = input.url.trim().to_string();
        server.headers = merged_headers;
        // Switching transport clears irrelevant fields so inventory stays tidy.
        if server.transport.is_remote() {
            server.command.clear();
            server.args.clear();
            server.env.clear();
        } else {
            server.url.clear();
            server.headers.clear();
        }
        server.updated_at = current_iso8601();
        let summary = server.to_summary();
        self.save_inventory(&inv)?;
        Ok(summary)
    }

    /// Toggle enabled state. Returns a masked summary.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<McpServerSummary, String> {
        let mut inv = self.load_inventory()?;
        let server = inv
            .servers
            .get_mut(id)
            .ok_or_else(|| format!("MCP server not found: {id}"))?;
        server.enabled = enabled;
        server.updated_at = current_iso8601();
        let summary = server.to_summary();
        self.save_inventory(&inv)?;
        Ok(summary)
    }

    /// Remove a server.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut inv = self.load_inventory()?;
        inv.servers
            .remove(id)
            .ok_or_else(|| format!("MCP server not found: {id}"))?;
        self.save_inventory(&inv)?;
        Ok(())
    }

    /// All enabled servers (for sandbox deployment).
    pub fn enabled_servers(&self) -> Result<Vec<McpServer>, String> {
        let inv = self.load_inventory()?;
        Ok(inv
            .servers
            .values()
            .filter(|s| s.enabled)
            .cloned()
            .collect())
    }

    /// Seed a CSP-managed connector into the inventory exactly once, guarded by
    /// a sentinel file so a user who later disables or removes it is never
    /// overridden on the next launch. No-op (returns `false`) if the sentinel
    /// exists or a server with the same name is already present.
    ///
    /// `sentinel` is a dotfile name under the store root (e.g.
    /// `.seeded-web-search`). Timestamps are stamped here so callers only need
    /// to supply the connector's identity and command.
    pub fn seed_once(&self, sentinel: &str, mut server: McpServer) -> Result<bool, String> {
        let marker = self.root.join(sentinel);
        if marker.exists() {
            return Ok(false);
        }
        let mut inv = self.load_inventory()?;
        let seeded = if inv.servers.values().any(|s| s.name == server.name) {
            false
        } else {
            let now = current_iso8601();
            server.created_at = now.clone();
            server.updated_at = now;
            inv.servers.insert(server.id.to_string(), server);
            self.save_inventory(&inv)?;
            true
        };
        // Stamp the sentinel regardless, so a name collision does not make us
        // retry (and resurrect) on every launch.
        let _ = fs::write(&marker, b"1\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&marker, fs::Permissions::from_mode(0o600));
        }
        Ok(seeded)
    }

    /// Refresh the description of an already-seeded built-in connector so app
    /// upgrades propagate new guidance text. Only touches a connector still
    /// present with `builtin == true` (so a user removal stays removed) and
    /// leaves command/args/env/enabled untouched. No-op (returns `false`) if
    /// the named built-in connector is absent.
    pub fn refresh_builtin(&self, name: &str, description: &str) -> Result<bool, String> {
        let mut inv = self.load_inventory()?;
        let Some(server) = inv
            .servers
            .values_mut()
            .find(|s| s.builtin && s.name == name)
        else {
            return Ok(false);
        };
        if server.description == description {
            return Ok(true);
        }
        server.description = description.to_string();
        server.updated_at = current_iso8601();
        self.save_inventory(&inv)?;
        Ok(true)
    }
}

/// Current UTC time as RFC 3339 / ISO 8601. Reuses the skill store's helper.
fn current_iso8601() -> String {
    crate::skill_manager::store::current_iso8601()
}

/// Merge submitted env against the previously stored env. See `update` docs for
/// the semantics (empty value keeps old, absent key deletes).
fn merge_env(
    old: &BTreeMap<String, String>,
    submitted: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (k, v) in submitted {
        if v.is_empty() {
            if let Some(prev) = old.get(k) {
                out.insert(k.clone(), prev.clone());
            } else {
                out.insert(k.clone(), String::new());
            }
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn inspect_stdio(
    input: &McpServerInput,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> bool {
    let command = input.command.trim();
    let command_ok = if command.is_empty() {
        errors.push("Command is required".to_string());
        false
    } else if command.starts_with('/') {
        if !PathBuf::from(command).exists() {
            warnings.push(format!("Absolute command not found on disk: {command}"));
        }
        true
    } else if KNOWN_COMMANDS.contains(&command) {
        true
    } else {
        warnings.push(format!(
            "'{command}' is not a known managed-runtime command; Science may reject it. \
             Use an absolute path or one of: {}",
            KNOWN_COMMANDS.join(", ")
        ));
        false
    };

    // Relative script-like args almost never work: the MCP child sandbox has no
    // configurable cwd and only absolute paths get `user_read_paths` grants.
    for arg in &input.args {
        if looks_like_relative_script(arg) {
            warnings.push(format!(
                "'{arg}' looks like a relative path; the sandbox has no working directory \
                 and only absolute paths are granted read access. Use an absolute path."
            ));
        }
    }

    if input.env.keys().any(|k| k.trim().is_empty()) {
        errors.push("Environment variable names cannot be empty".to_string());
    }
    command_ok
}

fn inspect_remote(input: &McpServerInput, warnings: &mut Vec<String>, errors: &mut Vec<String>) {
    let url = input.url.trim();
    if url.is_empty() {
        errors.push("URL is required for remote MCP".to_string());
    } else if !(url.starts_with("https://") || url.starts_with("http://")) {
        errors.push("URL must start with https:// or http://".to_string());
    } else {
        if url.starts_with("http://") {
            warnings.push(
                "Science marketplace plugins require https; custom remotes usually \
                 work best over https as well."
                    .to_string(),
            );
        }
        // Detect userinfo (https://user:pass@host/...) without a full URL parser.
        if let Some(after_scheme) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
        {
            let authority = after_scheme.split('/').next().unwrap_or("");
            if authority.contains('@') {
                warnings.push(
                    "Do not put secrets in the URL (userinfo). Use Headers instead \
                     — they are stored in the 0600 inventory and deployed via \
                     Science headers_helper."
                        .to_string(),
                );
            }
        }
    }

    if input.headers.keys().any(|k| k.trim().is_empty()) {
        errors.push("Header names cannot be empty".to_string());
    }
    if !input.headers.is_empty() {
        warnings.push(
            "Header values are stored in CSP's local 0600 inventory and deployed as a \
             Science headers_helper shell snippet that prints JSON. Prefer short-lived \
             tokens; do not paste secrets into the URL."
                .to_string(),
        );
    }
}

/// True when `arg` looks like a relative filesystem path to a script/resource,
/// which the MCP child sandbox cannot resolve (no cwd, no read grant).
fn looks_like_relative_script(arg: &str) -> bool {
    let a = arg.trim();
    if a.is_empty() || a.starts_with('/') || a.starts_with('-') {
        return false; // absolute, or a flag like `--port`
    }
    // A path separator, or a common script/module extension.
    if a.contains('/') {
        return true;
    }
    const EXTS: &[&str] = &[
        ".py", ".js", ".mjs", ".cjs", ".ts", ".rb", ".sh", ".jar", ".json",
    ];
    EXTS.iter().any(|e| a.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn rand_u64() -> u64 {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        nanos.wrapping_mul(1_000_003).wrapping_add(n)
    }

    fn temp_store() -> McpStore {
        let dir = env::temp_dir().join(format!("csp-mcp-test-{}", rand_u64()));
        fs::create_dir_all(&dir).unwrap();
        McpStore { root: dir }
    }

    fn input(name: &str, command: &str) -> McpServerInput {
        McpServerInput {
            name: name.to_string(),
            description: String::new(),
            transport: McpTransport::Stdio,
            command: command.to_string(),
            args: vec![],
            env: BTreeMap::new(),
            url: String::new(),
            headers: BTreeMap::new(),
        }
    }

    fn remote_input(name: &str, url: &str) -> McpServerInput {
        McpServerInput {
            name: name.to_string(),
            description: String::new(),
            transport: McpTransport::StreamableHttp,
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: url.to_string(),
            headers: BTreeMap::new(),
        }
    }

    #[test]
    fn empty_store_lists_nothing() {
        assert!(temp_store().list().unwrap().is_empty());
    }

    #[test]
    fn create_and_list_roundtrip() {
        let store = temp_store();
        let s = store.create(input("demo", "python3")).unwrap();
        assert_eq!(s.name, "demo");
        assert!(s.enabled);
        assert_eq!(store.list().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn duplicate_name_rejected() {
        let store = temp_store();
        store.create(input("dup", "node")).unwrap();
        assert!(store.create(input("dup", "npx")).is_err());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn inspect_flags_missing_fields() {
        let empty = McpServerInput::default();
        let r = McpStore::inspect(&empty);
        assert!(!r.valid);
        assert!(r.errors.iter().any(|e| e.contains("Name")));
        assert!(r.errors.iter().any(|e| e.contains("Command")));
    }

    #[test]
    fn inspect_unknown_command_warns_but_allows_when_named() {
        let r = McpStore::inspect(&input("x", "definitely-not-a-runtime"));
        assert!(r.valid); // name+command present → valid
        assert!(!r.command_ok); // but flagged as off-whitelist
        assert!(!r.warnings.is_empty());
    }

    #[test]
    fn inspect_absolute_command_is_ok() {
        let r = McpStore::inspect(&input("x", "/usr/bin/python3"));
        assert!(r.command_ok);
    }

    #[test]
    fn update_changes_fields() {
        let store = temp_store();
        let s = store.create(input("demo", "python3")).unwrap();
        let mut upd = input("demo2", "node");
        upd.args = vec!["server.js".into()];
        let out = store.update(&s.id.to_string(), upd).unwrap();
        assert_eq!(out.name, "demo2");
        assert_eq!(out.command, "node");
        assert_eq!(out.args, vec!["server.js".to_string()]);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn set_enabled_and_enabled_servers_filter() {
        let store = temp_store();
        let s = store.create(input("demo", "python3")).unwrap();
        store.set_enabled(&s.id.to_string(), false).unwrap();
        assert!(store.enabled_servers().unwrap().is_empty());
        store.set_enabled(&s.id.to_string(), true).unwrap();
        assert_eq!(store.enabled_servers().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn remove_deletes_entry() {
        let store = temp_store();
        let s = store.create(input("demo", "python3")).unwrap();
        store.remove(&s.id.to_string()).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(store.remove(&s.id.to_string()).is_err());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn env_stored_but_summary_masked() {
        let store = temp_store();
        let mut inp = input("demo", "python3");
        inp.env.insert("TOKEN".into(), "supersecretvalue".into());
        store.create(inp).unwrap();
        let sum = &store.list().unwrap()[0];
        assert_eq!(sum.env.get("TOKEN").unwrap(), "••••alue");
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn create_and_update_return_masked_summary() {
        let store = temp_store();
        let mut inp = input("demo", "python3");
        inp.env.insert("TOKEN".into(), "supersecretvalue".into());
        let created = store.create(inp).unwrap();
        assert_eq!(created.env.get("TOKEN").unwrap(), "••••alue");

        let mut upd = input("demo", "python3");
        upd.env.insert("TOKEN".into(), "".into()); // keep
        let updated = store.update(&created.id, upd).unwrap();
        assert_eq!(updated.env.get("TOKEN").unwrap(), "••••alue");
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn update_env_empty_value_keeps_old_secret() {
        let store = temp_store();
        let mut inp = input("demo", "python3");
        inp.env.insert("TOKEN".into(), "keepme-123456".into());
        let s = store.create(inp).unwrap();

        // Submit blank value for TOKEN → old secret preserved.
        let mut upd = input("demo", "python3");
        upd.env.insert("TOKEN".into(), "".into());
        store.update(&s.id, upd).unwrap();

        // Verify the stored (unmasked) value survived by re-masking a known tail.
        let sum = &store.list().unwrap()[0];
        assert_eq!(sum.env.get("TOKEN").unwrap(), "••••3456");
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn update_env_absent_key_is_deleted() {
        let store = temp_store();
        let mut inp = input("demo", "python3");
        inp.env.insert("A".into(), "aaaa1111".into());
        inp.env.insert("B".into(), "bbbb2222".into());
        let s = store.create(inp).unwrap();

        // Only resubmit A (blank keep); B omitted → deleted.
        let mut upd = input("demo", "python3");
        upd.env.insert("A".into(), "".into());
        store.update(&s.id, upd).unwrap();

        let sum = &store.list().unwrap()[0];
        assert!(sum.env.contains_key("A"));
        assert!(!sum.env.contains_key("B"));
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn update_env_new_value_overwrites() {
        let store = temp_store();
        let mut inp = input("demo", "python3");
        inp.env.insert("TOKEN".into(), "oldvalue-0000".into());
        let s = store.create(inp).unwrap();

        let mut upd = input("demo", "python3");
        upd.env.insert("TOKEN".into(), "brandnew-9999".into());
        store.update(&s.id, upd).unwrap();

        let sum = &store.list().unwrap()[0];
        assert_eq!(sum.env.get("TOKEN").unwrap(), "••••9999");
        let _ = fs::remove_dir_all(&store.root);
    }

    fn builtin_server(name: &str) -> McpServer {
        McpServer {
            id: McpServerId::new(),
            name: name.to_string(),
            description: "built-in".into(),
            transport: McpTransport::Stdio,
            command: "python3".into(),
            args: vec!["/x/server.py".into()],
            env: BTreeMap::new(),
            url: String::new(),
            headers: BTreeMap::new(),
            enabled: true,
            builtin: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn seed_once_seeds_then_is_guarded_by_sentinel() {
        let store = temp_store();
        // First seed inserts and enables the connector.
        let seeded = store
            .seed_once(".seeded-web-search", builtin_server("web-search"))
            .unwrap();
        assert!(seeded);
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].builtin);
        assert!(list[0].enabled);

        // Simulate the user removing it, then relaunch: sentinel present → no resurrection.
        store.remove(&list[0].id).unwrap();
        let seeded_again = store
            .seed_once(".seeded-web-search", builtin_server("web-search"))
            .unwrap();
        assert!(!seeded_again);
        assert!(store.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn seed_once_skips_when_name_collides() {
        let store = temp_store();
        store.create(input("web-search", "python3")).unwrap();
        // A user connector already owns the name → don't duplicate, but stamp sentinel.
        let seeded = store
            .seed_once(".seeded-web-search", builtin_server("web-search"))
            .unwrap();
        assert!(!seeded);
        assert_eq!(store.list().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn refresh_builtin_updates_description_and_preserves_enabled() {
        let store = temp_store();
        let mut server = builtin_server("web-search");
        server.description = "old description".into();
        server.enabled = false;
        store.seed_once(".seeded-web-search", server).unwrap();
        assert!(store
            .refresh_builtin("web-search", "new description")
            .unwrap());
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description, "new description");
        assert!(!list[0].enabled); // enabled state preserved
                                   // Absent name → false, no error.
        assert!(!store.refresh_builtin("nope", "x").unwrap());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn refresh_builtin_skips_non_builtin() {
        let store = temp_store();
        store.create(input("web-search", "python3")).unwrap();
        assert!(!store
            .refresh_builtin("web-search", "should not apply")
            .unwrap());
        assert_eq!(store.list().unwrap()[0].description, "");
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn inspect_warns_on_relative_script_arg() {
        let mut inp = input("demo", "python3");
        inp.args = vec!["server.py".into()];
        let r = McpStore::inspect(&inp);
        assert!(r.valid); // still valid, just warned
        assert!(r.warnings.iter().any(|w| w.contains("relative")));
    }

    #[test]
    fn inspect_no_warn_on_absolute_arg_or_flag() {
        let mut inp = input("demo", "python3");
        inp.args = vec!["/abs/server.py".into(), "--port".into(), "8080".into()];
        let r = McpStore::inspect(&inp);
        assert!(!r.warnings.iter().any(|w| w.contains("relative")));
    }

    #[test]
    fn create_remote_http_roundtrip() {
        let store = temp_store();
        let mut inp = remote_input("remote-demo", "https://mcp.example.com/mcp");
        inp.headers
            .insert("Authorization".into(), "Bearer secret-abcd".into());
        let s = store.create(inp).unwrap();
        assert_eq!(s.transport, McpTransport::StreamableHttp);
        assert_eq!(s.url, "https://mcp.example.com/mcp");
        assert!(s.command.is_empty());
        assert_eq!(s.headers.get("Authorization").unwrap(), "••••abcd");
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn inspect_remote_requires_url_and_strict_name() {
        let mut bad = remote_input("Bad_Name", "");
        let r = McpStore::inspect(&bad);
        assert!(!r.valid);
        assert!(r.errors.iter().any(|e| e.contains("URL")));
        assert!(r.errors.iter().any(|e| e.contains("lowercase")));

        bad = remote_input("ok-name", "https://mcp.example.com/sse");
        bad.transport = McpTransport::Sse;
        let r = McpStore::inspect(&bad);
        assert!(r.valid);
    }

    #[test]
    fn update_stdio_to_remote_clears_command() {
        let store = temp_store();
        let s = store.create(input("flip", "python3")).unwrap();
        let out = store
            .update(&s.id, remote_input("flip", "https://mcp.example.com/mcp"))
            .unwrap();
        assert_eq!(out.transport, McpTransport::StreamableHttp);
        assert!(out.command.is_empty());
        assert_eq!(out.url, "https://mcp.example.com/mcp");
        let _ = fs::remove_dir_all(&store.root);
    }
}
