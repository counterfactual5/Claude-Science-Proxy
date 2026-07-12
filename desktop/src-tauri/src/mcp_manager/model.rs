//! Local MCP (stdio connector) data models.
//!
//! First-phase scope: local **stdio** MCP servers only. Confirmed against a live
//! Claude Science sandbox — user stdio connectors are read from
//! `<data-dir>/mcp/local-mcp.json` with the shape
//! `{ "servers": [ { name, command, args, env, description? } ] }` and surface as
//! `source: "local-stdio"`. There is **no `cwd`** in Science's local schema, so we
//! do not model one. No remote HTTP/SSE, no marketplace catalog in this phase.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unique identifier for an MCP server. Format: `mcp_<32 hex chars>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpServerId(String);

impl McpServerId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        // Local-only uniqueness via epoch nanos; not cryptographic. A process-wide
        // atomic salt guards against two ids minted within the same nanosecond.
        static SALT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let salt = SALT.fetch_add(1, Ordering::Relaxed);
        let mixed = nanos.wrapping_mul(1_000_003).wrapping_add(salt);
        Self(format!("mcp_{:016x}{:016x}", nanos, mixed))
    }

    #[allow(dead_code)]
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.starts_with("mcp_") && s.len() == 36 && s[4..].chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(s.to_string()))
        } else {
            Err(format!("Invalid McpServerId format: {}", s))
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for McpServerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for McpServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A local stdio MCP server entry stored in the CSP inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: McpServerId,
    /// Connector name. Also the `local-mcp.json` server key after sanitization.
    pub name: String,
    /// Optional human-readable description (surfaced by Science).
    #[serde(default)]
    pub description: String,
    /// Executable: `node`, `npx`, `npm`, `python`, `python3`, `pip`, `pip3`, or an
    /// absolute path (Science's managed-runtime command whitelist).
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment overrides. Stored in plaintext (local, 0600 inventory); only ever
    /// returned to the UI masked.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub enabled: bool,
    /// ISO 8601 timestamps.
    pub created_at: String,
    pub updated_at: String,
}

impl McpServer {
    /// Build a UI-facing summary with env values masked (keys preserved).
    pub fn to_summary(&self) -> McpServerSummary {
        let env: BTreeMap<String, String> = self
            .env
            .iter()
            .map(|(k, v)| (k.clone(), crate::config::mask(v)))
            .collect();
        McpServerSummary {
            id: self.id.to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env,
            enabled: self.enabled,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

/// UI-facing summary. `env` values are masked; keys are shown verbatim.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// A local stdio MCP server discovered from another client config.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMcpServer {
    /// Connector name in the source client.
    pub name: String,
    /// Optional human-readable description.
    pub description: String,
    /// Executable command.
    pub command: String,
    /// Stdio command arguments.
    pub args: Vec<String>,
    /// Environment variable names only. Values stay backend-only until import.
    pub env_keys: Vec<String>,
    /// Human-readable origin label (e.g. `Zed ~/.config/zed/settings.json`).
    pub source_label: String,
    /// Absolute source config path.
    pub source_path: String,
    /// True when a CSP server with the same name already exists.
    pub already_imported: bool,
}

/// Result of validating a proposed stdio MCP server (before create/update).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInspection {
    pub valid: bool,
    /// Whether `command` is on the managed-runtime whitelist or an absolute path.
    pub command_ok: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_format_is_correct() {
        let id = McpServerId::new();
        let s = id.to_string();
        assert!(s.starts_with("mcp_"));
        assert_eq!(s.len(), 36);
        assert!(s[4..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn id_parses_valid_and_rejects_invalid() {
        let id = McpServerId::new();
        assert_eq!(McpServerId::parse(&id.to_string()).unwrap(), id);
        assert!(McpServerId::parse("invalid").is_err());
        assert!(McpServerId::parse("mcp_short").is_err());
    }

    #[test]
    fn ids_are_unique_under_tight_loop() {
        let a = McpServerId::new();
        let b = McpServerId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn summary_masks_env_values() {
        let mut env = BTreeMap::new();
        env.insert("API_TOKEN".to_string(), "sk-secret-tail1234".to_string());
        let srv = McpServer {
            id: McpServerId::new(),
            name: "demo".into(),
            description: String::new(),
            command: "python3".into(),
            args: vec![],
            env,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let sum = srv.to_summary();
        let masked = sum.env.get("API_TOKEN").unwrap();
        assert!(!masked.contains("secret"));
        assert!(masked.starts_with("••••"));
    }
}
