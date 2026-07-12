//! Local MCP Manager — custom stdio connectors for the isolated Science sandbox.
//!
//! First phase: local **stdio** MCP servers only (`command` + `args` + `env`).
//! Definitions live in `~/.csp/mcp/inventory.json`; enabled ones are written to
//! the sandbox's `<data-dir>/mcp/local-mcp.json` before launch, with sandbox read
//! grants merged into `<data-dir>/config.toml`. No remote HTTP/SSE, no marketplace.

pub mod deploy;
pub mod model;
pub mod store;

#[allow(unused_imports)]
pub(crate) use deploy::deploy_enabled_mcp;
#[allow(unused_imports)]
pub use model::{McpServer, McpServerId, McpServerSummary};
#[allow(unused_imports)]
pub use store::McpStore;
