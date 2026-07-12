//! Tauri commands for Skill Manager (frontend-facing).

use std::path::PathBuf;

use crate::run_blocking;
use crate::skill_manager::model::{DiscoveredSkill, InspectionResult, Skill, SkillSummary};
use crate::skill_manager::store::SkillStore;
use serde::Deserialize;

#[tauri::command]
pub async fn list_skills() -> Result<Vec<SkillSummary>, String> {
    run_blocking(|| {
        let store = SkillStore::open()?;
        store.list()
    })
    .await
}

/// Known roots (relative to `$HOME`) where local agent Skills commonly live.
/// The sandbox's own `~/.claude-science/skills` is deliberately excluded — those
/// are Science's bundled skills and importing them would be meaningless/unsafe.
/// Each entry is scanned for immediate subfolders containing a `SKILL.md`.
const DISCOVERY_ROOTS: &[&str] = &[
    ".agents/skills",
    ".codex/skills",
    ".claude/skills",
    ".cursor/skills",
    // Domestic (China) agents / IDEs that also use the `SKILL.md` folder layout.
    ".trae/skills",        // ByteDance Trae
    ".codebuddy/skills",   // Tencent CodeBuddy
];

#[tauri::command]
pub async fn discover_skills() -> Result<Vec<DiscoveredSkill>, String> {
    run_blocking(|| {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is not set".to_string())?;
        let roots: Vec<(PathBuf, String)> = DISCOVERY_ROOTS
            .iter()
            .map(|rel| (home.join(rel), format!("~/{rel}")))
            .collect();
        let store = SkillStore::open()?;
        store.discover(&roots)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectSkillSourceInput {
    pub source_path: String,
}

#[tauri::command]
pub async fn inspect_skill_source(
    input: InspectSkillSourceInput,
) -> Result<InspectionResult, String> {
    let source = PathBuf::from(input.source_path);
    run_blocking(move || SkillStore::inspect_source(&source)).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportSkillInput {
    pub source_path: String,
}

#[tauri::command]
pub async fn import_skill(input: ImportSkillInput) -> Result<Skill, String> {
    let source = PathBuf::from(input.source_path);
    run_blocking(move || {
        let store = SkillStore::open()?;
        store.import(&source)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetSkillEnabledInput {
    pub skill_id: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_skill_enabled(input: SetSkillEnabledInput) -> Result<Skill, String> {
    run_blocking(move || {
        let store = SkillStore::open()?;
        store.set_enabled(&input.skill_id, input.enabled)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveSkillInput {
    pub skill_id: String,
}

#[tauri::command]
pub async fn remove_skill(input: RemoveSkillInput) -> Result<(), String> {
    run_blocking(move || {
        let store = SkillStore::open()?;
        store.remove(&input.skill_id)
    })
    .await
}
