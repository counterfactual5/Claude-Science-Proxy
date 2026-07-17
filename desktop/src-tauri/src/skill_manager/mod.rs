//! Skill Manager module - simplified implementation for Claude Science Proxy.
//!
//! Provides local Skill discovery, import, and deployment for the isolated
//! Science sandbox. Skills are copied to managed storage and tracked separately
//! from real Claude credentials.

pub mod builtin;
pub mod deploy;
pub mod model;
pub mod native_pick;
pub mod science_sync;
pub mod source_resolve;
pub mod store;
pub mod workspace_ingress;

#[allow(unused_imports)]
pub(crate) use deploy::deploy_enabled_skills;
#[allow(unused_imports)]
pub use model::{Skill, SkillId, SkillSummary};
#[allow(unused_imports)]
pub use store::SkillStore;

/// Seed the built-in `csp-environment` standing-guidance Skill into the
/// inventory once (first run), enabled by default, then refresh its bundled
/// content so app upgrades propagate new guidance text. Migrates prior installs
/// of the legacy `csp-web-access` name while respecting sticky opt-out. The Skill
/// is the CSP local-environment handbook (web lanes, `/mnt/data`,
/// `save_artifacts`, CJK fonts, skill/env conventions, network allowlist).
/// Sentinel-guarded: a user who later disables or removes it is respected. Never
/// fails startup — any error is swallowed (worst case: the Skill simply isn't
/// seeded).
pub fn seed_builtin_skills() {
    if let Ok(store) = SkillStore::open() {
        let files = builtin::bundled_files();
        let _ = store.seed_or_migrate_environment_skill(
            builtin::ENVIRONMENT_SEED_SENTINEL,
            builtin::BUILTIN_ENVIRONMENT_NAME,
            builtin::BUILTIN_ENVIRONMENT_DESCRIPTION,
            &files,
            builtin::ENVIRONMENT_REQUIREMENTS,
            builtin::LEGACY_WEB_ACCESS_NAME,
            builtin::LEGACY_WEB_ACCESS_SEED_SENTINEL,
        );
    }
}
