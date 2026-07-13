//! Tauri commands for Skill Manager (frontend-facing).

use std::path::PathBuf;

use crate::run_blocking;
use crate::runtime::sandbox_session::redeploy_sandbox_skills;
use crate::runtime::science::{sandbox_running_ours, stop_sandbox};
use crate::skill_manager::model::{DiscoveredSkill, InspectionResult, Skill, SkillSummary};
use crate::skill_manager::store::SkillStore;
use crate::skill_manager::workspace_ingress::{
    self, AdoptWorkspaceSkillsResult, WorkspaceSkillCandidate,
};
use crate::{config, lock, SharedAppState};
use serde::Deserialize;
use tauri::State;

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
    ".trae/skills",        // ByteDance Trae (international)
    ".trae-cn/skills",     // ByteDance Trae CN
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenSkillInput {
    pub skill_id: String,
}

/// Reveal a Skill's managed folder in Finder.
#[tauri::command]
pub async fn open_skill_folder(input: OpenSkillInput) -> Result<String, String> {
    run_blocking(move || {
        let store = SkillStore::open()?;
        let skill = store
            .get(&input.skill_id)?
            .ok_or_else(|| "skill not found".to_string())?;
        crate::runtime::system::open_path_in_default_app(&skill.store_path)?;
        Ok(skill.store_path.display().to_string())
    })
    .await
}

/// Open a Skill's `SKILL.md` in the default editor (falls back to the folder).
#[tauri::command]
pub async fn open_skill_file(input: OpenSkillInput) -> Result<String, String> {
    run_blocking(move || {
        let store = SkillStore::open()?;
        let skill = store
            .get(&input.skill_id)?
            .ok_or_else(|| "skill not found".to_string())?;
        let md = skill.store_path.join("SKILL.md");
        let target = if md.is_file() { md } else { skill.store_path.clone() };
        crate::runtime::system::open_path_in_default_app(&target)?;
        Ok(target.display().to_string())
    })
    .await
}

#[tauri::command]
pub async fn discover_workspace_skills() -> Result<Vec<WorkspaceSkillCandidate>, String> {
    run_blocking(workspace_ingress::discover_workspace_skills).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdoptWorkspaceSkillsInput {
    pub keys: Vec<String>,
}

#[tauri::command]
pub async fn adopt_workspace_skills(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: AdoptWorkspaceSkillsInput,
) -> Result<AdoptWorkspaceSkillsResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut result = workspace_ingress::adopt_workspace_skills(&input.keys)?;
        if !result.adopted.is_empty() && redeploy_sandbox_skills() {
            let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
            if sandbox_running_ours(cfg.sandbox_port) {
                let mut st = lock(&state);
                let mut child = st.sandbox.take();
                let mut url = st.sandbox_url.take();
                stop_sandbox(&app, &mut child, &mut url)?;
                st.sandbox = child;
                st.sandbox_url = url;
                result.needs_restart = true;
            }
        }
        Ok(result)
    })
    .await
}
