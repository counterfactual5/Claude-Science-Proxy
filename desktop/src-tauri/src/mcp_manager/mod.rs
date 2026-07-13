//! Local MCP Manager — custom stdio connectors for the isolated Science sandbox.
//!
//! First phase: local **stdio** MCP servers only (`command` + `args` + `env`).
//! Definitions live in `~/.csp/mcp/inventory.json`; enabled ones are written to
//! the sandbox's `<data-dir>/mcp/local-mcp.json` before launch, with sandbox read
//! grants merged into `<data-dir>/config.toml`. No remote HTTP/SSE, no marketplace.

pub mod builtin;
pub mod deploy;
pub mod model;
pub mod store;

#[allow(unused_imports)]
pub(crate) use deploy::deploy_enabled_mcp;
#[allow(unused_imports)]
pub use model::{McpServer, McpServerId, McpServerSummary};
#[allow(unused_imports)]
pub use store::McpStore;

/// Seed the built-in `web-search` connector into the inventory once (first
/// run). Enabled by default and no-key usable; users can later disable/remove
/// it or add optional API keys via the MCP tab. Never fails startup — any error
/// is swallowed (worst case: the connector simply isn't seeded).
pub fn seed_builtin_connectors() {
    let sbx_home = crate::runtime::science::sandbox_home();
    let python = builtin::resolve_sandbox_python(&sbx_home)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "python3".to_string());
    let script = builtin::web_search_script_path(&sbx_home)
        .to_string_lossy()
        .into_owned();
    let server = builtin::build_web_search_server(python, script);
    if let Ok(store) = McpStore::open() {
        let _ = store.seed_once(builtin::WEB_SEARCH_SEED_SENTINEL, server);
    }
}
