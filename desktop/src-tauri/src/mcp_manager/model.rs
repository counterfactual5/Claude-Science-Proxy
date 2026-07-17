//! MCP connector data models (stdio + remote HTTP/SSE).
//!
//! Confirmed against Claude Science `0.1.17-dev`:
//! - **Local stdio** → `<data-dir>/mcp/local-mcp.json` shape
//!   `{ "servers": [ { name, command, args, env, description? } ] }`, source
//!   `local-stdio`. No `cwd`.
//! - **Remote custom** → org DB table `custom_mcp_servers` with
//!   `transport ∈ {sse, streamable_http}`, `url`, optional `headers_helper`
//!   (shell command printing a JSON object of string headers), source `custom`.
//!   Plugin marketplace also uses `type: http|sse` → mapped to
//!   `streamable_http|sse`, but CSP does not write marketplace plugins.
//!
//! Inventory stores both kinds; deploy routes stdio → `local-mcp.json` and
//! remote → the org `operon-cli.db`.

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

/// Connection transport. Matches Science's connector `transport` enum for the
/// values we can deploy (`stdio` via local-mcp.json; `sse` /
/// `streamable_http` via `custom_mcp_servers`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    #[default]
    Stdio,
    Sse,
    StreamableHttp,
}

impl McpTransport {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Sse => "sse",
            Self::StreamableHttp => "streamable_http",
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Sse | Self::StreamableHttp)
    }

    /// Parse Science / client transport labels (`http` → streamable_http).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stdio" | "local" | "local-stdio" => Some(Self::Stdio),
            "sse" => Some(Self::Sse),
            "streamable_http" | "streamable-http" | "http" => Some(Self::StreamableHttp),
            _ => None,
        }
    }
}

/// An MCP server entry stored in the CSP inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: McpServerId,
    /// Connector name. Also the `local-mcp.json` / `custom_mcp_servers` key.
    pub name: String,
    /// Optional human-readable description (surfaced by Science).
    #[serde(default)]
    pub description: String,
    /// Connection type. Absent in pre-1.8.0 inventory → deserializes as `stdio`.
    #[serde(default)]
    pub transport: McpTransport,
    /// Stdio executable (ignored for remote).
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Stdio environment overrides. Stored in plaintext (local, 0600 inventory);
    /// only ever returned to the UI masked.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Remote endpoint URL (ignored for stdio).
    #[serde(default)]
    pub url: String,
    /// Optional HTTP headers for remote transports. Stored like `env` (0600,
    /// UI-masked). Deployed as Science `headers_helper` (shell → JSON object),
    /// not as a static headers column (custom MCP table has no headers map).
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
    /// True for connectors CSP seeds and manages itself (e.g. the bundled
    /// `web-search` server). The command/args of a built-in are re-resolved at
    /// deploy time (interpreter + bundled script path) so they self-heal, and
    /// the UI labels them as built-in.
    #[serde(default)]
    pub builtin: bool,
    /// ISO 8601 timestamps.
    pub created_at: String,
    pub updated_at: String,
}

impl McpServer {
    /// Build a UI-facing summary with env/header values masked (keys preserved).
    pub fn to_summary(&self) -> McpServerSummary {
        let env: BTreeMap<String, String> = self
            .env
            .iter()
            .map(|(k, v)| (k.clone(), crate::config::mask(v)))
            .collect();
        let headers: BTreeMap<String, String> = self
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), crate::config::mask(v)))
            .collect();
        McpServerSummary {
            id: self.id.to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            transport: self.transport.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            env,
            url: self.url.clone(),
            headers,
            enabled: self.enabled,
            builtin: self.builtin,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

/// UI-facing summary. `env` / `headers` values are masked; keys are shown verbatim.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub transport: McpTransport,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
    #[serde(default)]
    pub builtin: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// An MCP server discovered from another client config.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMcpServer {
    /// Connector name in the source client.
    pub name: String,
    /// Optional human-readable description.
    pub description: String,
    #[serde(default)]
    pub transport: McpTransport,
    /// Executable command (stdio).
    #[serde(default)]
    pub command: String,
    /// Stdio command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variable names only. Values stay backend-only until import.
    #[serde(default)]
    pub env_keys: Vec<String>,
    /// Remote URL when transport is SSE / streamable HTTP.
    #[serde(default)]
    pub url: String,
    /// Header names only for remote entries.
    #[serde(default)]
    pub header_keys: Vec<String>,
    /// Human-readable origin label (e.g. `Zed ~/.config/zed/settings.json`).
    pub source_label: String,
    /// Absolute source config path.
    pub source_path: String,
    /// True when a CSP server with the same name already exists.
    pub already_imported: bool,
}

/// Result of validating a proposed MCP server (before create/update).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInspection {
    pub valid: bool,
    /// Whether `command` is on the managed-runtime whitelist or an absolute path.
    /// Always `true` for remote transports (no local command).
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
    fn summary_masks_env_and_header_values() {
        let mut env = BTreeMap::new();
        env.insert("API_TOKEN".to_string(), "sk-secret-tail1234".to_string());
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer secret-tail9999".to_string(),
        );
        let srv = McpServer {
            id: McpServerId::new(),
            name: "demo".into(),
            description: String::new(),
            transport: McpTransport::StreamableHttp,
            command: String::new(),
            args: vec![],
            env,
            url: "https://mcp.example.com/mcp".into(),
            headers,
            enabled: true,
            builtin: false,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let sum = srv.to_summary();
        let masked = sum.env.get("API_TOKEN").unwrap();
        assert!(!masked.contains("secret"));
        assert!(masked.starts_with("••••"));
        let h = sum.headers.get("Authorization").unwrap();
        assert!(!h.contains("secret"));
        assert!(h.starts_with("••••"));
    }

    #[test]
    fn legacy_inventory_without_transport_deserializes_as_stdio() {
        let json = r#"{
            "id": "mcp_0123456789abcdef0123456789abcdef",
            "name": "legacy",
            "description": "",
            "command": "python3",
            "args": [],
            "env": {},
            "enabled": true,
            "createdAt": "t0",
            "updatedAt": "t0"
        }"#;
        let srv: McpServer = serde_json::from_str(json).unwrap();
        assert_eq!(srv.transport, McpTransport::Stdio);
        assert!(srv.url.is_empty());
        assert!(srv.headers.is_empty());
    }

    #[test]
    fn transport_parse_maps_http_alias() {
        assert_eq!(
            McpTransport::parse("http"),
            Some(McpTransport::StreamableHttp)
        );
        assert_eq!(McpTransport::parse("SSE"), Some(McpTransport::Sse));
        assert_eq!(McpTransport::parse("stdio"), Some(McpTransport::Stdio));
        assert!(McpTransport::parse("websocket").is_none());
    }
}
