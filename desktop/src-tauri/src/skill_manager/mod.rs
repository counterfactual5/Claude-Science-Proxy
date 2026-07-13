//! Skill Manager module - simplified implementation for Claude Science Proxy.
//!
//! Provides local Skill discovery, import, and deployment for the isolated
//! Science sandbox. Skills are copied to managed storage and tracked separately
//! from real Claude credentials.

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
