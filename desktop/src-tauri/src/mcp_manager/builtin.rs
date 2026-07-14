//! Built-in `web-search` MCP connector.
//!
//! CSP bundles a small multi-provider web-search + page-fetch MCP server
//! (`web_search_server.py`) so Claude Science can do real web search/fetch even
//! though Anthropic's hosted `web_search` tool is unavailable under CSP's
//! virtual login.
//!
//! **Important:** bare `web_search` / `web_fetch` are Anthropic *native server
//! tools*, not local MCP entry points. Under CSP virtual login they are
//! stripped from OPERON's toolset, so top-level calls fail with
//! `Tool 'web_search' not found on agent 'OPERON'`. Local MCP tools are **not**
//! top-level model tools — they are only callable via the `repl` tool as
//! `host.mcp("web-search", "<method>", ...)`. Names in `tools/list`
//! (`web_search`, `search_literature`, `csp_web_search`, `fetch_url`,
//! `web_fetch`) are **method names for `host.mcp` only**; re-advertising them
//! cannot intercept bare native calls. Standing guidance is injected by the
//! proxy (`inject_csp_web_access_guidance` in `anthropic_compat.py`).
//!
//! The server is:
//!
//! - **Free and no-key out of the box** — it falls back across DuckDuckGo and
//!   Wikipedia (plus arXiv/Crossref for papers) with no configuration.
//! - **Upgradeable** — advanced users can drop a Brave / Serper / Tavily API
//!   key into the connector's `env` (via the MCP tab) and the server prefers
//!   those higher-quality providers automatically (OpenClaw-style fallback).
//!
//! It is written in Python because the sandbox already ships a Python runtime
//! and, unlike the Node/axios stacks, Python's `requests`/`urllib` honour the
//! injected `HTTPS_PROXY` and issue a real `CONNECT` tunnel — so it reaches the
//! internet through Science's operon proxy without needing CSP's Node shim.
//!
//! The script itself is bundled via `include_str!` and written into the sandbox
//! `mcp/` dir at deploy time (mirroring `mcp_http_tunnel_shim.cjs`); the
//! connector's interpreter and script path are (re-)resolved on every deploy so
//! the entry self-heals even if the sandbox's Python layout changes. A rewrite
//! of the embedded script marks deploy as changed so a running sandbox is
//! restarted (MCP children keep the old script in memory otherwise).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{McpServer, McpServerId};

/// Connector name (also the `local-mcp.json` server key).
pub const BUILTIN_WEB_SEARCH_NAME: &str = "web-search";
/// Sentinel under the store root that records the one-time seed.
pub const WEB_SEARCH_SEED_SENTINEL: &str = ".seeded-web-search";
/// File name the bundled server is written to inside the sandbox `mcp/` dir.
pub const WEB_SEARCH_SCRIPT_FILE: &str = "csp-web-search-server.py";

/// The bundled Python server, embedded at compile time.
const WEB_SEARCH_SOURCE: &str = include_str!("web_search_server.py");

/// Description surfaced to Science (the model reads this to decide when to call
/// the tools). English on purpose — it is the tool description, not chrome.
pub const BUILTIN_WEB_SEARCH_DESCRIPTION: &str = "Local CSP web + literature search (two host.mcp lanes). Under CSP virtual login Anthropic-hosted web_search/web_fetch are unavailable. GENERAL: host.mcp(\"web-search\", \"web_search\"|\"csp_web_search\", query=...) — auto: Brave/Serper/Tavily (if keyed) → duckduckgo_ia. LITERATURE: host.mcp(\"web-search\", \"search_literature\", query=...) — auto: wikipedia → Crossref → arXiv → PubMed. Then fetch_url/web_fetch. hits = data[\"results\"] (dict, not a bare list). CSP pre-grants search hosts on Start; extend via ~/.csp/network-allowlist.json.";

/// Optional API-key env vars seeded (empty) so the MCP tab surfaces them as
/// editable fields; empty values are treated as "unset" by the server.
const OPTIONAL_KEY_ENV: &[&str] = &["BRAVE_SEARCH_API_KEY", "SERPER_API_KEY", "TAVILY_API_KEY"];

/// Candidate Python interpreters inside the sandbox, most-preferred first. The
/// `claude-science-mcp` env is the one Science provisions for MCP servers and
/// ships `requests`; the `python` env is a plain fallback.
const SANDBOX_PYTHON_CANDIDATES: &[&str] = &[
    ".claude-science/conda/envs/claude-science-mcp/bin/python3",
    ".claude-science/conda/envs/python/bin/python3",
];

/// Resolve a Python interpreter inside the sandbox by absolute path, or `None`
/// if the sandbox has not been provisioned yet (falls back to `python3`).
pub fn resolve_sandbox_python(sbx_home: &Path) -> Option<PathBuf> {
    for rel in SANDBOX_PYTHON_CANDIDATES {
        let p = sbx_home.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Absolute path the bundled server is deployed to inside the sandbox.
pub fn web_search_script_path(sbx_home: &Path) -> PathBuf {
    sbx_home
        .join(".claude-science")
        .join("mcp")
        .join(WEB_SEARCH_SCRIPT_FILE)
}

/// Idempotently write the bundled server into `<mcp_dir>`.
/// Returns `(path, rewritten)` where `rewritten` is true when disk bytes changed
/// (so callers can force a sandbox restart — a running MCP child keeps the old
/// script loaded). Returns `None` on write failure (non-fatal).
pub fn write_web_search_server(mcp_dir: &Path) -> Option<(PathBuf, bool)> {
    let path = mcp_dir.join(WEB_SEARCH_SCRIPT_FILE);
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() == Some(WEB_SEARCH_SOURCE) {
        return Some((path, false));
    }
    if fs::write(&path, WEB_SEARCH_SOURCE).is_err() {
        return None;
    }
    Some((path, true))
}

/// Build the built-in connector definition for seeding. `python` is the
/// interpreter command (absolute path when resolved, else `python3`), and
/// `script` is the absolute deploy path of the bundled server.
pub fn build_web_search_server(python: String, script: String) -> McpServer {
    let env: BTreeMap<String, String> = OPTIONAL_KEY_ENV
        .iter()
        .map(|k| ((*k).to_string(), String::new()))
        .collect();
    McpServer {
        id: McpServerId::new(),
        name: BUILTIN_WEB_SEARCH_NAME.to_string(),
        description: BUILTIN_WEB_SEARCH_DESCRIPTION.to_string(),
        command: python,
        args: vec![script],
        env,
        enabled: true,
        builtin: true,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
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

    #[test]
    fn bundled_source_is_a_python_stdio_server() {
        assert!(WEB_SEARCH_SOURCE.contains("def main()"));
        assert!(WEB_SEARCH_SOURCE.contains("tools/call"));
        // Multi-provider registry must be present: the no-key scholarly
        // defaults plus the general-web and key-based providers.
        assert!(WEB_SEARCH_SOURCE.contains("\"crossref\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"arxiv\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"pubmed\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"duckduckgo\""));
        assert!(WEB_SEARCH_SOURCE.contains("BRAVE_SEARCH_API_KEY"));
        // Under CSP virtual login bare web_search is unavailable as a top-level
        // tool; tools/list names are host.mcp method names only.
        assert!(WEB_SEARCH_SOURCE.contains("\"web_search\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"search_literature\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"csp_web_search\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"fetch_url\""));
        assert!(WEB_SEARCH_SOURCE.contains("\"web_fetch\""));
        // Confirm they appear in the advertised TOOLS list, not only dispatch.
        let tools_idx = WEB_SEARCH_SOURCE
            .find("TOOLS = [")
            .expect("TOOLS list present");
        let tools_block = &WEB_SEARCH_SOURCE[tools_idx..];
        assert!(tools_block.contains("\"name\": \"web_search\""));
        assert!(tools_block.contains("\"name\": \"web_fetch\""));
        assert!(tools_block.contains("\"name\": \"search_literature\""));
        assert!(tools_block.contains("\"name\": \"csp_web_search\""));
        assert!(tools_block.contains("\"name\": \"fetch_url\""));
        // Docstring must NOT claim tools/list intercepts bare native calls.
        assert!(!WEB_SEARCH_SOURCE.contains("model-native calls resolve"));
        assert!(WEB_SEARCH_SOURCE.contains("host.mcp"));
        // Return shape must be documented so models don't iterate the wrapper dict.
        assert!(WEB_SEARCH_SOURCE.contains("RETURN SHAPE"));
        // Python source escapes quotes: data[\"results\"]
        assert!(WEB_SEARCH_SOURCE.contains(r#"data[\"results\"]"#));
        assert!(WEB_SEARCH_SOURCE.contains("SERVER_VERSION = \"1.5.0\""));
        assert!(WEB_SEARCH_SOURCE.contains("GENERAL_FREE_FALLBACKS"));
        assert!(WEB_SEARCH_SOURCE.contains("LITERATURE_FREE_FALLBACKS"));
        assert!(WEB_SEARCH_SOURCE.contains("do_general_web_search"));
        assert!(WEB_SEARCH_SOURCE.contains("do_literature_search"));
        // literature auto must not start with duckduckgo_ia
        let lit = WEB_SEARCH_SOURCE
            .split("LITERATURE_FREE_FALLBACKS = [")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .unwrap_or("");
        assert!(lit.contains("wikipedia"));
        assert!(lit.contains("crossref"));
        assert!(!lit.contains("duckduckgo_ia"));
        let gen = WEB_SEARCH_SOURCE
            .split("GENERAL_FREE_FALLBACKS = [")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .unwrap_or("");
        assert!(gen.contains("duckduckgo_ia"));
        assert!(!gen.contains("crossref"));
    }

    #[test]
    fn build_web_search_server_is_builtin_and_keyed() {
        let s = build_web_search_server("python3".into(), "/x/server.py".into());
        assert_eq!(s.name, BUILTIN_WEB_SEARCH_NAME);
        assert!(s.builtin);
        assert!(s.enabled);
        assert_eq!(s.command, "python3");
        assert_eq!(s.args, vec!["/x/server.py".to_string()]);
        for k in OPTIONAL_KEY_ENV {
            assert_eq!(s.env.get(*k).map(String::as_str), Some(""));
        }
    }

    #[test]
    fn write_web_search_server_is_idempotent() {
        let dir = env::temp_dir().join(format!("csp-ws-{}", uniq()));
        fs::create_dir_all(&dir).unwrap();
        let (p1, rewritten1) = write_web_search_server(&dir).unwrap();
        assert!(p1.is_file());
        assert!(rewritten1, "first write must report rewritten");
        let contents = fs::read_to_string(&p1).unwrap();
        assert_eq!(contents, WEB_SEARCH_SOURCE);
        // Second write is a no-op (same bytes) and still returns the path.
        let (p2, rewritten2) = write_web_search_server(&dir).unwrap();
        assert_eq!(p1, p2);
        assert!(!rewritten2, "identical content must not report rewritten");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_web_search_server_reports_rewrite_when_stale() {
        let dir = env::temp_dir().join(format!("csp-ws-stale-{}", uniq()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(WEB_SEARCH_SCRIPT_FILE);
        fs::write(&path, b"# stale bundled script\n").unwrap();
        let (p, rewritten) = write_web_search_server(&dir).unwrap();
        assert_eq!(p, path);
        assert!(rewritten);
        assert_eq!(fs::read_to_string(&p).unwrap(), WEB_SEARCH_SOURCE);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_sandbox_python_finds_mcp_env_first() {
        let home = env::temp_dir().join(format!("csp-sbx-{}", uniq()));
        // No env yet → None.
        assert!(resolve_sandbox_python(&home).is_none());
        // Provision the preferred (claude-science-mcp) interpreter.
        let bin = home.join(".claude-science/conda/envs/claude-science-mcp/bin");
        fs::create_dir_all(&bin).unwrap();
        let py = bin.join("python3");
        fs::write(&py, b"#!/bin/sh\n").unwrap();
        assert_eq!(resolve_sandbox_python(&home).as_deref(), Some(py.as_path()));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn script_path_is_under_sandbox_mcp_dir() {
        let home = Path::new("/tmp/sbx");
        let p = web_search_script_path(home);
        assert!(p.ends_with(".claude-science/mcp/csp-web-search-server.py"));
    }
}
