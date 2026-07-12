//! Skill data models.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a Skill. Format: `sk_<32 hex chars>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SkillId(String);

impl SkillId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        // Local-only uniqueness via epoch nanos; not cryptographic. A process-wide
        // atomic salt guards against two ids minted within the same nanosecond
        // (e.g. importing several Skills in a tight loop).
        static SALT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let salt = SALT.fetch_add(1, Ordering::Relaxed);
        let mixed = nanos.wrapping_mul(1_000_003).wrapping_add(salt);
        Self(format!("sk_{:016x}{:016x}", nanos, mixed))
    }

    #[allow(dead_code)]
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.starts_with("sk_") && s.len() == 35 && s[3..].chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(s.to_string()))
        } else {
            Err(format!("Invalid SkillId format: {}", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SkillId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A Skill entry stored in the local Skill store.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Unique identifier
    pub id: SkillId,
    /// Human-readable name (e.g. "Data Analysis")
    pub name: String,
    /// Optional description from SKILL.md
    #[serde(default)]
    pub description: String,
    /// Path to the imported Skill in managed storage
    pub store_path: PathBuf,
    /// Original source path (read-only reference, not necessarily existing)
    pub source_path: PathBuf,
    /// Whether the Skill is currently enabled
    pub enabled: bool,
    /// Size in bytes
    pub size_bytes: u64,
    /// ISO 8601 timestamp of import
    pub imported_at: String,
    /// Detected requirements (e.g. "python", "network", "mcp")
    #[serde(default)]
    pub requirements: Vec<String>,
}

/// A summary of a Skill for UI listing.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub imported_at: String,
    pub requirements: Vec<String>,
}

impl From<&Skill> for SkillSummary {
    fn from(s: &Skill) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name.clone(),
            description: s.description.clone(),
            enabled: s.enabled,
            size_bytes: s.size_bytes,
            imported_at: s.imported_at.clone(),
            requirements: s.requirements.clone(),
        }
    }
}

/// Result of a Skill source inspection (before import).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectionResult {
    pub valid: bool,
    pub name: String,
    pub description: String,
    pub file_count: u32,
    pub total_size_bytes: u64,
    pub requirements: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_id_format_is_correct() {
        let id = SkillId::new();
        let s = id.to_string();
        assert!(s.starts_with("sk_"));
        assert_eq!(s.len(), 35);
    }

    #[test]
    fn skill_id_parses_valid() {
        let id = SkillId::new();
        let parsed = SkillId::parse(&id.to_string()).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn skill_id_rejects_invalid() {
        assert!(SkillId::parse("invalid").is_err());
        assert!(SkillId::parse("sk_tooshort").is_err());
        assert!(SkillId::parse("sk_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX").is_err());
    }
}
