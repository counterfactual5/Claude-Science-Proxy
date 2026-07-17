//! Tauri commands for the local MCP Manager (frontend-facing).

use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_manager::model::{
    DiscoveredMcpServer, McpInspection, McpServerSummary, McpTransport,
};
use crate::mcp_manager::store::{McpServerInput, McpStore};
use crate::runtime::sandbox_session::{redeploy_sandbox_mcp, stop_running_sandbox_for_redeploy};
use crate::{config, run_blocking, SharedAppState};
use tauri::State;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerInputDto {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `stdio` | `sse` | `streamable_http` (also accepts `http` → streamable_http).
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

impl From<McpServerInputDto> for McpServerInput {
    fn from(d: McpServerInputDto) -> Self {
        let transport = d
            .transport
            .as_deref()
            .and_then(McpTransport::parse)
            .unwrap_or(McpTransport::Stdio);
        McpServerInput {
            name: d.name,
            description: d.description,
            transport,
            command: d.command,
            args: d.args,
            env: d.env,
            url: d.url,
            headers: d.headers,
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

#[tauri::command]
pub async fn open_mcp_inventory_json() -> Result<String, String> {
    run_blocking(|| {
        let dir = config::default_dir().join("mcp");
        fs::create_dir_all(&dir).map_err(|e| format!("create mcp store dir: {e}"))?;
        let path = dir.join("inventory.json");
        if !path.exists() {
            fs::write(&path, b"{\n  \"servers\": {}\n}\n")
                .map_err(|e| format!("write mcp inventory: {e}"))?;
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
            }
        }
        crate::runtime::system::open_path_in_default_app(&path)?;
        Ok(path.display().to_string())
    })
    .await
}

/// Ensure `~/.csp/network-allowlist.json` exists and open it in the default editor.
///
/// Built-in web-search provider hosts are always merged on Start; this file is
/// for *extra* Science sandbox egress domains (hostnames only).
#[tauri::command]
pub async fn open_network_allowlist_json() -> Result<String, String> {
    run_blocking(|| {
        let path = crate::mcp_manager::network_allowlist::ensure_user_file()?;
        crate::runtime::system::open_path_in_default_app(&path)?;
        Ok(path.display().to_string())
    })
    .await
}

/// A JSON config file that may hold local stdio MCP definitions, plus the object
/// key the servers live under. Three shapes are seen in the wild:
/// - `mcpServers` — Cursor, Claude Desktop, Claude Code, Devin Desktop / Windsurf,
///   Google Antigravity / Gemini, Continue, and the domestic tools Qoder /
///   通义灵码 (Alibaba), Trae / TRAE SOLO (ByteDance), and CodeBuddy (Tencent).
/// - `servers` — VS Code / Insiders.
/// - `context_servers` — Zed.
///
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
        label: "Claude Desktop (Linux)",
        key: "mcpServers",
    },
    // Devin Desktop (the June 2026 rebrand of Windsurf, formerly Codeium). The
    // global MCP config moved up out of the `windsurf/` subfolder; keep the
    // legacy path too for older installs.
    McpSource {
        rel_path: ".codeium/mcp_config.json",
        label: "Devin Desktop",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".codeium/windsurf/mcp_config.json",
        label: "Windsurf",
        key: "mcpServers",
    },
    // VS Code / Insiders — user-scope `mcp.json` (`servers` key, not mcpServers).
    McpSource {
        rel_path: "Library/Application Support/Code/User/mcp.json",
        label: "VS Code",
        key: "servers",
    },
    McpSource {
        rel_path: "Library/Application Support/Code - Insiders/User/mcp.json",
        label: "VS Code Insiders",
        key: "servers",
    },
    // Zed uses `context_servers` instead of `mcpServers`.
    McpSource {
        rel_path: ".config/zed/settings.json",
        label: "Zed",
        key: "context_servers",
    },
    // Continue — global/home config (workspace copies live under `<repo>/.continue/`).
    McpSource {
        rel_path: ".continue/mcpServers/mcp.json",
        label: "Continue",
        key: "mcpServers",
    },
    // Kimi Code — user MCP config (default: ~/.kimi-code/mcp.json).
    McpSource {
        rel_path: ".kimi-code/mcp.json",
        label: "Kimi Code",
        key: "mcpServers",
    },
    // MiniMax CLI / agent tools — default external-tool config (default:
    // ~/.minimax/mcp.json). Some clients use `servers`, others `mcpServers`.
    McpSource {
        rel_path: ".minimax/mcp.json",
        label: "MiniMax",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".minimax/mcp.json",
        label: "MiniMax",
        key: "servers",
    },
    // Google Antigravity / Gemini CLI — both use mcpServers.
    McpSource {
        rel_path: ".gemini/antigravity/mcp_config.json",
        label: "Antigravity",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".gemini/config/mcp_config.json",
        label: "Gemini",
        key: "mcpServers",
    },
    // --- Domestic (China) agents / IDEs. All confirmed to use the `mcpServers`
    // key with stdio `{ command, args, env }` and/or remote `{ url, type|transport,
    // headers }` shapes. ---
    // Alibaba Qoder / Tongyi Lingma family — CLI / IDE user scope uses
    // `~/.qoder/settings.json`; IDE app data uses
    // `<app>/SharedClientCache/mcp.json`.
    McpSource {
        rel_path: ".qoder/settings.json",
        label: "Qoder",
        key: "mcpServers",
    },
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
    // Tencent CodeBuddy — CLI under `~/.codebuddy/`; IDE app data mirrors Trae's
    // `<app>/User/mcp.json` layout when present.
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
    McpSource {
        rel_path: "Library/Application Support/CodeBuddy/User/mcp.json",
        label: "CodeBuddy",
        key: "mcpServers",
    },
    McpSource {
        rel_path: "Library/Application Support/CodeBuddy CN/User/mcp.json",
        label: "CodeBuddy CN",
        key: "mcpServers",
    },
    // --- Verified additional roots (docs-checked 2026-07). Only paths that are
    // real *user/home*-scope MCP files are listed; project-scope files like
    // `.trae/mcp.json`, `.kiro/settings/mcp.json` (workspace), `.cursor/mcp.json`
    // (project) live in repos, not HOME, so they are intentionally excluded. ---
    // OpenClaw — `mcp.servers` nested under ~/.openclaw/openclaw.json (JSON5).
    McpSource {
        rel_path: ".openclaw/openclaw.json",
        label: "OpenClaw",
        key: "mcp.servers",
    },
    // AWS Kiro — user scope (kiro.dev/docs/mcp/configuration): ~/.kiro/settings/mcp.json.
    McpSource {
        rel_path: ".kiro/settings/mcp.json",
        label: "Kiro",
        key: "mcpServers",
    },
    // Tencent CodeBuddy — legacy single-file user config (docs: ~/.codebuddy.json).
    McpSource {
        rel_path: ".codebuddy.json",
        label: "CodeBuddy",
        key: "mcpServers",
    },
    // Tencent WorkBuddy (desktop / VS Code extension). Distinct from CodeBuddy:
    // user MCP is ~/.workbuddy/mcp.json (and recommended ~/.workbuddy/.mcp.json).
    McpSource {
        rel_path: ".workbuddy/.mcp.json",
        label: "WorkBuddy",
        key: "mcpServers",
    },
    McpSource {
        rel_path: ".workbuddy/mcp.json",
        label: "WorkBuddy",
        key: "mcpServers",
    },
    // Factory Droid (factory.ai / VS Code + CLI) — user MCP: ~/.factory/mcp.json.
    McpSource {
        rel_path: ".factory/mcp.json",
        label: "Factory",
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewDiscoveredMcpInput {
    pub source_path: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredMcpPreview {
    pub name: String,
    pub source_path: String,
    pub content: String,
    pub truncated: bool,
    pub char_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenDiscoveredMcpSourceInput {
    pub source_path: String,
}

const MAX_MCP_PREVIEW_CHARS: usize = 200_000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpActionResult {
    pub server: McpServerSummary,
    /// Sandbox was stopped so the frontend can restart with MCP changes applied.
    pub needs_restart: bool,
}

fn mcp_redeploy_maybe_restart(
    app: &tauri::AppHandle,
    state: &SharedAppState,
) -> Result<bool, String> {
    if redeploy_sandbox_mcp() {
        stop_running_sandbox_for_redeploy(app, state)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn import_discovered_mcp_server(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: ImportDiscoveredMcpServerInput,
) -> Result<McpActionResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let server = read_source_server(Path::new(&input.source_path), &input.name)?
            .ok_or_else(|| format!("Discovered MCP not found: {}", input.name))?;
        let store = McpStore::open()?;
        let summary = store.create(server)?;
        let needs_restart = if summary.enabled {
            mcp_redeploy_maybe_restart(&app, &state)?
        } else {
            false
        };
        Ok(McpActionResult {
            server: summary,
            needs_restart,
        })
    })
    .await
}

#[tauri::command]
pub async fn preview_discovered_mcp(
    input: PreviewDiscoveredMcpInput,
) -> Result<DiscoveredMcpPreview, String> {
    run_blocking(move || preview_discovered_mcp_server(&input.source_path, &input.name)).await
}

#[tauri::command]
pub async fn open_discovered_mcp_source(
    input: OpenDiscoveredMcpSourceInput,
) -> Result<String, String> {
    run_blocking(move || {
        let path = PathBuf::from(&input.source_path);
        if !path.is_file() {
            return Err(format!("source config not found: {}", input.source_path));
        }
        crate::runtime::system::reveal_path_in_finder(&path)?;
        Ok(path.display().to_string())
    })
    .await
}

#[tauri::command]
pub async fn inspect_mcp_server(input: McpServerInputDto) -> Result<McpInspection, String> {
    run_blocking(move || Ok(McpStore::inspect(&input.into()))).await
}

#[tauri::command]
pub async fn create_mcp_server(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: McpServerInputDto,
) -> Result<McpActionResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let store = McpStore::open()?;
        let summary = store.create(input.into())?;
        let needs_restart = if summary.enabled {
            mcp_redeploy_maybe_restart(&app, &state)?
        } else {
            false
        };
        Ok(McpActionResult {
            server: summary,
            needs_restart,
        })
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
pub async fn update_mcp_server(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: UpdateMcpServerInput,
) -> Result<McpActionResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let store = McpStore::open()?;
        let summary = store.update(&input.server_id, input.server.into())?;
        // Always redeploy: enabled edits must apply; disabled edits clear any
        // stale sandbox entry left from a prior enabled deploy.
        let needs_restart = mcp_redeploy_maybe_restart(&app, &state)?;
        Ok(McpActionResult {
            server: summary,
            needs_restart,
        })
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
pub async fn set_mcp_server_enabled(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: SetMcpEnabledInput,
) -> Result<McpActionResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let store = McpStore::open()?;
        let summary = store.set_enabled(&input.server_id, input.enabled)?;
        let needs_restart = mcp_redeploy_maybe_restart(&app, &state)?;
        Ok(McpActionResult {
            server: summary,
            needs_restart,
        })
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
            continue; // malformed → skip
        };
        if !seen.insert((source_path.clone(), input.name.clone())) {
            continue;
        }
        out.push(discovered_from_input(
            input,
            label,
            &source_path,
            existing_names,
        ));
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
    // Prefer nested OpenClaw-style `mcp.servers`, then flat keys.
    for key in ["mcp.servers"]
        .into_iter()
        .chain(SERVER_KEYS.iter().copied())
    {
        if let Some(servers) = server_map(&root, key) {
            if let Some(value) = servers.get(server_name) {
                return Ok(parse_server(server_name, value));
            }
        }
    }
    Ok(None)
}

fn find_server_value_in_root<'a>(root: &'a Value, server_name: &str) -> Option<&'a Value> {
    for key in ["mcp.servers"]
        .into_iter()
        .chain(SERVER_KEYS.iter().copied())
    {
        if let Some(servers) = server_map(root, key) {
            if let Some(value) = servers.get(server_name) {
                return Some(value);
            }
        }
    }
    None
}

fn preview_discovered_mcp_server(
    source_path: &str,
    server_name: &str,
) -> Result<DiscoveredMcpPreview, String> {
    let path = PathBuf::from(source_path);
    if !path.is_file() {
        return Err(format!("source config not found: {source_path}"));
    }

    let content = if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("read MCP source {}: {e}", path.display()))?;
        let root = text
            .parse::<toml::Table>()
            .map_err(|e| format!("parse MCP source {}: {e}", path.display()))?;
        let value = root
            .get("mcp_servers")
            .and_then(toml::Value::as_table)
            .and_then(|servers| servers.get(server_name))
            .ok_or_else(|| format!("server '{server_name}' not found in {}", path.display()))?;
        toml::to_string_pretty(value).map_err(|e| format!("format MCP preview: {e}"))?
    } else {
        let root = read_jsonc(&path)?.ok_or_else(|| format!("empty MCP source: {}", path.display()))?;
        let value = find_server_value_in_root(&root, server_name).ok_or_else(|| {
            format!(
                "server '{server_name}' not found in {}",
                path.display()
            )
        })?;
        serde_json::to_string_pretty(value).map_err(|e| format!("format MCP preview: {e}"))?
    };

    let char_count = content.chars().count();
    let (content, truncated) = if char_count > MAX_MCP_PREVIEW_CHARS {
        (
            content.chars().take(MAX_MCP_PREVIEW_CHARS).collect(),
            true,
        )
    } else {
        (content, false)
    };

    Ok(DiscoveredMcpPreview {
        name: server_name.to_string(),
        source_path: source_path.to_string(),
        content,
        truncated,
        char_count,
    })
}

/// Locate the server map, trying the source's declared key first then the
/// known alternates. Dotted keys (e.g. `mcp.servers`) walk nested objects.
fn server_map<'a>(
    root: &'a Value,
    primary_key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    if let Some(m) = lookup_object_path(root, primary_key) {
        return Some(m);
    }
    SERVER_KEYS
        .iter()
        .find_map(|k| lookup_object_path(root, k))
}

fn lookup_object_path<'a>(
    root: &'a Value,
    path: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    cur.as_object()
}

fn discovered_from_input(
    input: McpServerInput,
    label: &str,
    source_path: &str,
    existing_names: &std::collections::BTreeSet<String>,
) -> DiscoveredMcpServer {
    DiscoveredMcpServer {
        env_keys: input.env.keys().cloned().collect(),
        header_keys: input.headers.keys().cloned().collect(),
        description: input.description.clone(),
        transport: input.transport.clone(),
        command: input.command.clone(),
        args: input.args.clone(),
        url: input.url.clone(),
        already_imported: existing_names.contains(&input.name),
        name: input.name,
        source_label: label.to_string(),
        source_path: source_path.to_string(),
    }
}

/// Parse one server entry into an importable input. Supports stdio
/// (`command` + `args` + `env`) and remote (`url` + optional `type`/`transport`
/// + `headers`). Returns `None` when neither shape is present.
fn parse_server(name: &str, value: &Value) -> Option<McpServerInput> {
    let obj = value.as_object()?;
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Prefer explicit remote markers, then fall back to command (stdio).
    // Windsurf/Cascade uses `serverUrl` instead of `url` for remote servers.
    let url = obj
        .get("url")
        .or_else(|| obj.get("serverUrl"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let type_hint = obj
        .get("type")
        .or_else(|| obj.get("transport"))
        .and_then(Value::as_str)
        .and_then(McpTransport::parse);

    if let Some(url) = url {
        // Remote entry (Cursor / Claude Desktop / VS Code / …).
        let transport = match type_hint {
            Some(McpTransport::Stdio) => McpTransport::StreamableHttp, // url wins
            Some(t) => t,
            None => McpTransport::StreamableHttp,
        };
        let headers = obj
            .get("headers")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        return Some(McpServerInput {
            name: sanitize_discovered_name(name, true),
            description,
            transport,
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url,
            headers,
        });
    }

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
        name: sanitize_discovered_name(name, false),
        description,
        transport: McpTransport::Stdio,
        command,
        args,
        env,
        url: String::new(),
        headers: BTreeMap::new(),
    })
}

/// Parse one `[mcp_servers.<name>]` table. Supports stdio and remote
/// (`url` / `bearer_token_env_var`).
fn parse_codex_toml_server(name: &str, value: &toml::Value) -> Option<McpServerInput> {
    let table = value.as_table()?;
    let description = table
        .get("description")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();

    if let Some(url) = table
        .get("url")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let transport = table
            .get("transport")
            .or_else(|| table.get("type"))
            .and_then(toml::Value::as_str)
            .and_then(McpTransport::parse)
            .unwrap_or(McpTransport::StreamableHttp);
        let mut headers = BTreeMap::new();
        // Codex stores the bearer token env *name*; we cannot resolve the value
        // here — surface the header key so the user can fill it after import.
        if table.get("bearer_token_env_var").is_some() {
            headers.insert("Authorization".to_string(), String::new());
        }
        if let Some(hdrs) = table.get("http_headers").and_then(toml::Value::as_table) {
            for (k, v) in hdrs {
                if let Some(s) = v.as_str() {
                    headers.insert(k.clone(), s.to_string());
                }
            }
        }
        return Some(McpServerInput {
            name: sanitize_discovered_name(name, true),
            description,
            transport: if transport.is_remote() {
                transport
            } else {
                McpTransport::StreamableHttp
            },
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            url: url.to_string(),
            headers,
        });
    }

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
        name: sanitize_discovered_name(name, false),
        description,
        transport: McpTransport::Stdio,
        command,
        args,
        env,
        url: String::new(),
        headers: BTreeMap::new(),
    })
}

/// Soft-sanitize a discovered name. Remote Science rules are strict (lowercase
/// + hyphens); we lowercase and map invalid chars to `-` so import can succeed
/// with a warning-free inspect when possible. Stdio keeps the original name
/// when it already matches CSP's looser charset.
fn sanitize_discovered_name(name: &str, remote: bool) -> String {
    let trimmed = name.trim();
    if !remote {
        return trimmed.to_string();
    }
    let mut out: String = trimmed
        .chars()
        .map(|c| {
            let l = c.to_ascii_lowercase();
            if matches!(l, 'a'..='z' | '0'..='9' | '-') {
                l
            } else {
                '-'
            }
        })
        .collect();
    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "imported-mcp".to_string()
    } else {
        out.truncate(64);
        out
    }
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
            continue; // malformed → skip
        };
        if !seen.insert((source_path.clone(), input.name.clone())) {
            continue;
        }
        out.push(discovered_from_input(
            input,
            label,
            &source_path,
            existing_names,
        ));
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
    fn server_map_reads_nested_mcp_servers() {
        let root = json!({
            "mcp": {
                "servers": {
                    "docs": { "command": "npx", "args": ["-y", "x"] }
                }
            }
        });
        let map = server_map(&root, "mcp.servers").unwrap();
        assert!(map.contains_key("docs"));
        assert!(server_map(&root, "mcpServers").is_none());
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
        let input = parse_server("remote", &v).unwrap();
        assert_eq!(input.transport, McpTransport::Sse);
        assert_eq!(input.url, "https://mcp.example.com/sse");
        assert!(input.command.is_empty());
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
    fn parse_codex_toml_server_reads_remote() {
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
        let input = parse_codex_toml_server("figma", v).unwrap();
        assert_eq!(input.transport, McpTransport::StreamableHttp);
        assert_eq!(input.url, "https://mcp.figma.com/mcp");
        assert!(input.headers.contains_key("Authorization"));
    }

    #[test]
    fn parse_server_http_type_maps_to_streamable() {
        let v = json!({
            "type": "http",
            "url": "https://example.com/mcp",
            "headers": { "Authorization": "Bearer x" }
        });
        let input = parse_server("My Server", &v).unwrap();
        assert_eq!(input.transport, McpTransport::StreamableHttp);
        assert_eq!(input.name, "my-server"); // sanitized
        assert_eq!(
            input.headers.get("Authorization").map(String::as_str),
            Some("Bearer x")
        );
    }

    #[test]
    fn server_map_prefers_primary_then_alternates() {
        let ctx = json!({ "context_servers": { "a": { "command": "x" } } });
        assert!(server_map(&ctx, "context_servers").is_some());
        // Wrong primary key still finds the alternate.
        assert!(server_map(&ctx, "mcpServers").is_some());
    }

    #[test]
    fn find_server_value_in_root_finds_mcp_servers_key() {
        let root = json!({
            "mcpServers": {
                "notion": { "command": "notion-mcp", "args": [] }
            }
        });
        let v = find_server_value_in_root(&root, "notion").unwrap();
        assert_eq!(v["command"], "notion-mcp");
    }
}
