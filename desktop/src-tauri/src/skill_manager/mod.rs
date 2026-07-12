//! Skill Manager module - simplified implementation for Claude Science Proxy.
//!
//! Provides local Skill discovery, import, and deployment for the isolated
//! Science sandbox. Skills are copied to managed storage and tracked separately
//! from real Claude credentials.

pub mod model;
pub mod store;

#[allow(unused_imports)]
pub use model::{Skill, SkillId, SkillSummary};
#[allow(unused_imports)]
pub use store::SkillStore;
