//! Local MCP server store — persists stdio connector definitions in
//! `~/.csp/mcp/inventory.json` (0600). Mirrors `skill_manager::store` in spirit:
//! a dependency-light JSON inventory that the UI lists and the deployer reads.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::model::{McpInspection, McpServer, McpServerId, McpServerSummary};

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

/// Fields accepted from the UI to create or update a server. `env` values are
/// stored verbatim; only ever returned masked.
#[derive(Clone, Debug, Default)]
pub struct McpServerInput {
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
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
        } else if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            errors.push(
                "Name may only contain letters, digits, '-', '_' and '.'".to_string(),
            );
        }

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

        if input.env.keys().any(|k| k.trim().is_empty()) {
            errors.push("Environment variable names cannot be empty".to_string());
        }

        McpInspection {
            valid: errors.is_empty(),
            command_ok,
            warnings,
            errors,
        }
    }

    /// Create a new server. Rejects duplicate names and invalid input.
    pub fn create(&self, input: McpServerInput) -> Result<McpServer, String> {
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
        let server = McpServer {
            id: id.clone(),
            name,
            description: input.description.trim().to_string(),
            command: input.command.trim().to_string(),
            args: input.args,
            env: input.env,
            enabled: true,
            created_at: now.clone(),
            updated_at: now,
        };
        inv.servers.insert(id.to_string(), server.clone());
        self.save_inventory(&inv)?;
        Ok(server)
    }

    /// Update an existing server's definition (name/command/args/env/description).
    pub fn update(&self, id: &str, input: McpServerInput) -> Result<McpServer, String> {
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
        server.name = name;
        server.description = input.description.trim().to_string();
        server.command = input.command.trim().to_string();
        server.args = input.args;
        server.env = input.env;
        server.updated_at = current_iso8601();
        let updated = server.clone();
        self.save_inventory(&inv)?;
        Ok(updated)
    }

    /// Toggle enabled state.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<McpServer, String> {
        let mut inv = self.load_inventory()?;
        let server = inv
            .servers
            .get_mut(id)
            .ok_or_else(|| format!("MCP server not found: {id}"))?;
        server.enabled = enabled;
        server.updated_at = current_iso8601();
        let updated = server.clone();
        self.save_inventory(&inv)?;
        Ok(updated)
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
}

/// Current UTC time as RFC 3339 / ISO 8601. Reuses the skill store's helper.
fn current_iso8601() -> String {
    crate::skill_manager::store::current_iso8601()
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
            command: command.to_string(),
            args: vec![],
            env: BTreeMap::new(),
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
}
