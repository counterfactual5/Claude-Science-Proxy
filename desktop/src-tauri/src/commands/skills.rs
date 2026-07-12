//! Tauri commands for Skill Manager (frontend-facing).

use std::path::PathBuf;

use crate::run_blocking;
use crate::skill_manager::model::{InspectionResult, Skill, SkillSummary};
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
