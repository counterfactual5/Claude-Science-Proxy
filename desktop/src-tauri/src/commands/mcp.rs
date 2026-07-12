//! Tauri commands for the local MCP Manager (frontend-facing).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::mcp_manager::model::{DiscoveredMcpServer, McpInspection, McpServerSummary};
use crate::mcp_manager::store::{McpServerInput, McpStore};
use crate::run_blocking;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerInputDto {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl From<McpServerInputDto> for McpServerInput {
    fn from(d: McpServerInputDto) -> Self {
        McpServerInput {
            name: d.name,
            description: d.description,
            command: d.command,
            args: d.args,
            env: d.env,
        }
    }
}

#[tauri::command]
pub async fn list_mcp_servers() -> Result<Vec<McpServerSummary>, String> {
    run_blocking(|| {
        let store = McpStore::open()?;
        store.list()
    })
    .await
}

/// A JSON config file that may hold local stdio MCP definitions, plus the object
/// key the servers live under. Three shapes are seen in the wild:
/// - `mcpServers` — Cursor, Claude Desktop, Claude Code, Devin Desktop (Windsurf),
///   and the domestic tools Qoder / 通义灵码 (Alibaba), Trae / TRAE SOLO
///   (ByteDance), and CodeBuddy (Tencent).
/// - `servers` — VS Code.
/// - `context_servers` — Zed.
/// Codex CLI is handled separately (TOML `[mcp_servers.*]`), not via this list.
/// All use the same per-server object `{ command, args, env, description? }`.
struct McpSource {
    /// Path relative to `$HOME`.
    rel_path: &'static str,
    /// Human-readable client label.
    label: &'static str,
    /// Top-level object key the server map lives under.
    key: &'static str,
}

const MCP_SOURCES: &[McpSource] = &[
    McpSource {
        rel_path: ".cursor/mcp.json",
        label: "Cursor",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/Claude/claude_desktop_config.json",
        label: "Claude Desktop",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".claude.json",
        label: "Claude Code",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".config/claude/claude_desktop_config.json",
        label: "Claude",
        key: "mcpServers",
    },
    // Devin Desktop (the June 2026 rebrand of Windsurf, formerly Codeium). The
    // global MCP config moved up out of the `windsurf/` subfolder.
    McpSource {
        rel_path: ".codeium/mcp_config.json",
        label: "Devin Desktop",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".vscode/mcp.json",
        label: "VS Code",
        key: "servers",
    },
    // Zed uses `context_servers` instead of `mcpServers`.
    McpSource {
        rel_path: ".config/zed/settings.json",
        label: "Zed",
        key: "context_servers",
    },
    // --- Domestic (China) agents / IDEs. All confirmed to use the `mcpServers`
    // key with the same `{ command, args, env }` stdio shape. Remote/SSE entries
    // (`type`/`url`, no `command`) are ignored by discover_source as usual. ---
    // Alibaba Qoder / Tongyi Lingma family — global MCP under
    // `<app>/SharedClientCache/mcp.json`.
    McpSource {
        rel_path: "Library/Application Support/Qoder/SharedClientCache/mcp.json",
        label: "Qoder",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/QoderCN/SharedClientCache/mcp.json",
        label: "Qoder CN",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/QoderWork/SharedClientCache/mcp.json",
        label: "QoderWork",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/QoderWork CN/SharedClientCache/mcp.json",
        label: "QoderWork CN",
        key: "mcpServers",
    },
    // ByteDance Trae family — global MCP under `<app>/User/mcp.json`.
    McpSource {
        rel_path: "Library/Application Support/Trae/User/mcp.json",
        label: "Trae",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/Trae CN/User/mcp.json",
        label: "Trae CN",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/TRAE SOLO/User/mcp.json",
        label: "TRAE SOLO",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/TRAE SOLO CN/User/mcp.json",
        label: "TRAE SOLO CN",
        key: "mcpServers",
    },
    // Tencent CodeBuddy — user-scope MCP under `~/.codebuddy/`. `.mcp.json` is the
    // current recommended path; `mcp.json` is the deprecated fallback.
    McpSource {
        rel_path: ".codebuddy/.mcp.json",
        label: "CodeBuddy",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".codebuddy/mcp.json",
        label: "CodeBuddy",
        key: "mcpServers",
    },
];

#[tauri::command]
pub async fn discover_mcp_servers() -> Result<Vec<DiscoveredMcpServer>, String> {
    run_blocking(|| {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is not set".to_string())?;
        let store = McpStore::open()?;
        let existing: std::collections::BTreeSet<String> =
            store.list()?.into_iter().map(|s| s.name).collect();

        let mut found: Vec<DiscoveredMcpServer> = Vec::new();
        // De-dup by (source path, name): the same file could be reachable twice, but
        // identical names across different clients are kept (they are distinct rows).
        let mut seen: std::collections::BTreeSet<(String, String)> =
            std::collections::BTreeSet::new();
        for source in MCP_SOURCES {
            let path = home.join(source.rel_path);
            discover_source(
                &path,
                source.label,
                source.key,
                &existing,
                &mut seen,
                &mut found,
            )?;
        }
        // Codex CLI stores MCP under `[mcp_servers.*]` in TOML, not JSON.
        let codex_config = home.join(".codex/config.toml");
        discover_codex_toml(&codex_config, "Codex", &existing, &mut seen, &mut found);
        found.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.source_label.cmp(&b.source_label))
        });
        Ok(found)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportDiscoveredMcpServerInput {
    pub source_path: String,
    pub name: String,
}

#[tauri::command]
pub async fn import_discovered_mcp_server(
    input: ImportDiscoveredMcpServerInput,
) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let server = read_source_server(Path::new(&input.source_path), &input.name)?
            .ok_or_else(|| format!("Discovered MCP not found: {}", input.name))?;
        let store = McpStore::open()?;
        store.create(server)
    })
    .await
}

#[tauri::command]
pub async fn inspect_mcp_server(input: McpServerInputDto) -> Result<McpInspection, String> {
    run_blocking(move || Ok(McpStore::inspect(&input.into()))).await
}

#[tauri::command]
pub async fn create_mcp_server(input: McpServerInputDto) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.create(input.into())
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateMcpServerInput {
    pub server_id: String,
    pub server: McpServerInputDto,
}

#[tauri::command]
pub async fn update_mcp_server(input: UpdateMcpServerInput) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.update(&input.server_id, input.server.into())
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetMcpEnabledInput {
    pub server_id: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_mcp_server_enabled(input: SetMcpEnabledInput) -> Result<McpServerSummary, String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.set_enabled(&input.server_id, input.enabled)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveMcpServerInput {
    pub server_id: String,
}

#[tauri::command]
pub async fn remove_mcp_server(input: RemoveMcpServerInput) -> Result<(), String> {
    run_blocking(move || {
        let store = McpStore::open()?;
        store.remove(&input.server_id)
    })
    .await
}

/// Keys under which a config file might store its stdio MCP map. We probe the
/// source's declared key first, then the alternates, so a client that migrates
/// its schema still surfaces.
const SERVER_KEYS: &[&str] = &["mcpServers", "context_servers", "servers"];

fn discover_source(
    path: &Path,
    label: &str,
    primary_key: &str,
    existing_names: &std::collections::BTreeSet<String>,
    seen: &mut std::collections::BTreeSet<(String, String)>,
    out: &mut Vec<DiscoveredMcpServer>,
) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    // A malformed file from one client must not sink the whole scan.
    let root = match read_jsonc(path) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(()),
        Err(_) => return Ok(()),
    };
    let Some(servers) = server_map(&root, primary_key) else {
        return Ok(());
    };
    let source_path = path.to_string_lossy().to_string();
    for (name, value) in servers {
        let Some(input) = parse_server(name, value) else {
            continue; // remote (no command) or malformed → skip
        };
        if !seen.insert((source_path.clone(), input.name.clone())) {
            continue;
        }
        out.push(DiscoveredMcpServer {
            env_keys: input.env.keys().cloned().collect(),
            description: input.description.clone(),
            command: input.command.clone(),
            args: input.args.clone(),
            already_imported: existing_names.contains(&input.name),
            name: input.name,
            source_label: label.to_string(),
            source_path: source_path.clone(),
        });
    }
    Ok(())
}

fn read_source_server(path: &Path, server_name: &str) -> Result<Option<McpServerInput>, String> {
    // Codex uses TOML; every other supported source is JSON(C).
    if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        return Ok(read_codex_toml_server(path, server_name));
    }
    let Some(root) = read_jsonc(path)? else {
        return Ok(None);
    };
    for key in SERVER_KEYS {
        if let Some(value) = root
            .get(*key)
            .and_then(Value::as_object)
            .and_then(|servers| servers.get(server_name))
        {
            return Ok(parse_server(server_name, value));
        }
    }
    Ok(None)
}

/// Locate the server map, trying the source's declared key first then the
/// known alternates.
fn server_map<'a>(
    root: &'a Value,
    primary_key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    if let Some(m) = root.get(primary_key).and_then(Value::as_object) {
        return Some(m);
    }
    SERVER_KEYS
        .iter()
        .find_map(|k| root.get(*k).and_then(Value::as_object))
}

/// Parse one server entry into an importable input. Returns `None` for remote
/// connectors (no `command`) or malformed entries.
fn parse_server(name: &str, value: &Value) -> Option<McpServerInput> {
    let obj = value.as_object()?;
    let command = obj.get("command")?.as_str()?.trim().to_string();
    if command.is_empty() {
        return None;
    }
    let args = match obj.get("args") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect(),
        Some(Value::String(s)) => split_shell_like(s),
        _ => Vec::new(),
    };
    let env = obj
        .get("env")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Some(McpServerInput {
        name: name.to_string(),
        description: obj
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        command,
        args,
        env,
    })
}

/// Discover Codex CLI stdio MCP servers from `~/.codex/config.toml`
/// (`[mcp_servers.<name>]` tables). Errors are swallowed so a malformed config
/// never sinks the whole scan.
fn discover_codex_toml(
    path: &Path,
    label: &str,
    existing_names: &std::collections::BTreeSet<String>,
    seen: &mut std::collections::BTreeSet<(String, String)>,
    out: &mut Vec<DiscoveredMcpServer>,
) {
    if !path.is_file() {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let Ok(root) = text.parse::<toml::Table>() else {
        return;
    };
    let Some(servers) = root.get("mcp_servers").and_then(toml::Value::as_table) else {
        return;
    };
    let source_path = path.to_string_lossy().to_string();
    for (name, value) in servers {
        let Some(input) = parse_codex_toml_server(name, value) else {
            continue; // remote (url only) or malformed → skip
        };
        if !seen.insert((source_path.clone(), input.name.clone())) {
            continue;
        }
        out.push(DiscoveredMcpServer {
            env_keys: input.env.keys().cloned().collect(),
            description: input.description.clone(),
            command: input.command.clone(),
            args: input.args.clone(),
            already_imported: existing_names.contains(&input.name),
            name: input.name,
            source_label: label.to_string(),
            source_path: source_path.clone(),
        });
    }
}

fn read_codex_toml_server(path: &Path, server_name: &str) -> Option<McpServerInput> {
    let text = fs::read_to_string(path).ok()?;
    let root = text.parse::<toml::Table>().ok()?;
    let value = root
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get(server_name))?;
    parse_codex_toml_server(server_name, value)
}

/// Parse one `[mcp_servers.<name>]` table. Returns `None` for remote servers
/// (no `command`, e.g. `url`-based) or malformed entries.
fn parse_codex_toml_server(name: &str, value: &toml::Value) -> Option<McpServerInput> {
    let table = value.as_table()?;
    let command = table.get("command")?.as_str()?.trim().to_string();
    if command.is_empty() {
        return None;
    }
    let args = table
        .get("args")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    let env = table
        .get("env")
        .and_then(toml::Value::as_table)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Some(McpServerInput {
        name: name.to_string(),
        description: table
            .get("description")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string(),
        command,
        args,
        env,
    })
}

fn read_jsonc(path: &Path) -> Result<Option<Value>, String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read MCP source {}: {e}", path.display())),
    };
    let json = strip_jsonc(&text);
    serde_json::from_str(&json)
        .map(Some)
        .map_err(|e| format!("parse MCP source {}: {e}", path.display()))
}

fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for nc in chars.by_ref() {
                if nc == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for nc in chars.by_ref() {
                if prev == '*' && nc == '/' {
                    break;
                }
                prev = nc;
            }
            continue;
        }
        out.push(c);
    }
    remove_trailing_commas(&out)
}

fn remove_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let mut look = chars.clone();
            while matches!(look.peek(), Some(ch) if ch.is_whitespace()) {
                look.next();
            }
            if matches!(look.peek(), Some('}' | ']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn split_shell_like(s: &str) -> Vec<String> {
    // Good enough for UI-entered args such as `--transport stdio`; quoted paths can
    // still be edited manually after import.
    s.split_whitespace().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strip_jsonc_preserves_urls_and_strips_comments() {
        let src = r#"{
            // line comment
            "url": "https://example.com/x", /* block */
            "a": 1,
        }"#;
        let v: Value = serde_json::from_str(&strip_jsonc(src)).unwrap();
        assert_eq!(v["url"], "https://example.com/x");
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parse_server_reads_mcp_servers_shape() {
        let v = json!({
            "command": "/usr/bin/notion",
            "args": ["--transport", "stdio"],
            "env": { "NOTION_TOKEN": "secret" },
            "description": "notion"
        });
        let input = parse_server("notion", &v).unwrap();
        assert_eq!(input.command, "/usr/bin/notion");
        assert_eq!(input.args, vec!["--transport", "stdio"]);
        assert_eq!(
            input.env.get("NOTION_TOKEN").map(String::as_str),
            Some("secret")
        );
    }

    #[test]
    fn parse_server_skips_remote_without_command() {
        let v = json!({ "url": "https://mcp.example.com/sse", "type": "sse" });
        assert!(parse_server("remote", &v).is_none());
    }

    #[test]
    fn parse_server_accepts_string_args() {
        let v = json!({ "command": "python3", "args": "server.py --port 9" });
        let input = parse_server("x", &v).unwrap();
        assert_eq!(input.args, vec!["server.py", "--port", "9"]);
    }

    #[test]
    fn parse_codex_toml_server_reads_stdio() {
        let toml_src = r#"
[mcp_servers.fs]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/data"]
[mcp_servers.fs.env]
API_KEY = "secret"
"#;
        let root = toml_src.parse::<toml::Table>().unwrap();
        let v = root
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .and_then(|t| t.get("fs"))
            .unwrap();
        let input = parse_codex_toml_server("fs", v).unwrap();
        assert_eq!(input.command, "npx");
        assert_eq!(input.args.len(), 3);
        assert_eq!(input.env.get("API_KEY").map(String::as_str), Some("secret"));
    }

    #[test]
    fn parse_codex_toml_server_skips_remote() {
        let toml_src = r#"
[mcp_servers.figma]
url = "https://mcp.figma.com/mcp"
bearer_token_env_var = "FIGMA_TOKEN"
"#;
        let root = toml_src.parse::<toml::Table>().unwrap();
        let v = root
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .and_then(|t| t.get("figma"))
            .unwrap();
        assert!(parse_codex_toml_server("figma", v).is_none());
    }

    #[test]
    fn server_map_prefers_primary_then_alternates() {
        let ctx = json!({ "context_servers": { "a": { "command": "x" } } });
        assert!(server_map(&ctx, "context_servers").is_some());
        // Wrong primary key still finds the alternate.
        assert!(server_map(&ctx, "mcpServers").is_some());
    }
}
