//! Skill Manager module - simplified implementation for Claude Science Proxy.
//!
//! Provides local Skill discovery, import, and deployment for the isolated
//! Science sandbox. Skills are copied to managed storage and tracked separately
//! from real Claude credentials.

pub mod builtin;
pub mod deploy;
pub mod model;
pub mod store;
pub mod workspace_ingress;

#[allow(unused_imports)]
pub(crate) use deploy::deploy_enabled_skills;
#[allow(unused_imports)]
pub use model::{Skill, SkillId, SkillSummary};
#[allow(unused_imports)]
pub use store::SkillStore;

/// Seed the built-in `csp-web-access` standing-guidance Skill into the inventory
/// once (first run), enabled by default, then refresh its bundled content so app
/// upgrades propagate new guidance text. The Skill tells Claude Science to use
/// CSP's local `web-search` MCP for any web search and never the hosted
/// `web_search` tool. Sentinel-guarded: a user who later disables or removes it
/// is respected. Never fails startup — any error is swallowed (worst case: the
/// Skill simply isn't seeded).
pub fn seed_builtin_skills() {
    if let Ok(store) = SkillStore::open() {
        let files = builtin::bundled_files();
        let _ = store.seed_once(
            builtin::WEB_ACCESS_SEED_SENTINEL,
            builtin::BUILTIN_WEB_ACCESS_NAME,
            builtin::BUILTIN_WEB_ACCESS_DESCRIPTION,
            &files,
            builtin::WEB_ACCESS_REQUIREMENTS,
        );
        // Self-heal on every launch: keep the on-disk copy in sync with the
        // bundled guidance (no-op if the user removed it — sentinel stays).
        let _ = store.refresh_builtin(
            builtin::BUILTIN_WEB_ACCESS_NAME,
            builtin::BUILTIN_WEB_ACCESS_DESCRIPTION,
            &files,
        );
    }
}
