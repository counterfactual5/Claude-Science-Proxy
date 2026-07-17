//! Local MCP Manager — custom stdio + remote HTTP/SSE connectors for the
//! isolated Science sandbox.
//!
//! - **stdio** definitions deploy to `<data-dir>/mcp/local-mcp.json` with
//!   sandbox read grants in `<data-dir>/config.toml`.
//! - **sse / streamable_http** definitions deploy to the org
//!   `operon-cli.db` `custom_mcp_servers` table (Science's actual remote schema).
//! Inventory: `~/.csp/mcp/inventory.json`.

pub mod builtin;
pub mod deploy;
pub mod model;
pub mod network_allowlist;
pub mod remote_deploy;
pub mod store;

#[allow(unused_imports)]
pub(crate) use deploy::deploy_enabled_mcp;
#[allow(unused_imports)]
pub use model::{McpServer, McpServerId, McpServerSummary};
#[allow(unused_imports)]
pub(crate) use remote_deploy::deploy_remote_mcp;
#[allow(unused_imports)]
pub use store::McpStore;

/// Seed the built-in `web-search` connector into the inventory once (first
/// run). Enabled by default and no-key usable; users can later disable/remove
/// it or add optional API keys via the MCP tab. After seeding, refresh the
/// built-in description so app upgrades propagate to already-seeded users.
/// Never fails startup — any error is swallowed (worst case: the connector
/// simply isn't seeded).
pub fn seed_builtin_connectors() {
    let sbx_home = crate::runtime::science::sandbox_home();
    // Refresh the bundled Python server onto disk as soon as the desktop app
    // opens — not only on Start. Otherwise a rebuilt CSP binary can sit in the
    // Dock while Science keeps the previous script (and a later Start from an
    // *old* still-running process can even rewrite Wikipedia back into
    // GENERAL). Start still restarts the sandbox when bytes change.
    let mcp_dir = sbx_home.join(".claude-science").join("mcp");
    let _ = std::fs::create_dir_all(&mcp_dir);
    let _ = builtin::write_web_search_server(&mcp_dir);

    let python = builtin::resolve_sandbox_python(&sbx_home)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_string());
    let script = builtin::web_search_script_path(&sbx_home)
        .to_string_lossy()
        .into_owned();
    let server = builtin::build_web_search_server(python, script);
    if let Ok(store) = McpStore::open() {
        let _ = store.seed_once(builtin::WEB_SEARCH_SEED_SENTINEL, server);
        // Self-heal description on every launch (no-op if the user removed it).
        let _ = store.refresh_builtin(
            builtin::BUILTIN_WEB_SEARCH_NAME,
            builtin::BUILTIN_WEB_SEARCH_DESCRIPTION,
        );
    }
}
