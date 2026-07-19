//! Tauri commands for Skill Manager (frontend-facing).

use std::path::PathBuf;

use crate::run_blocking;
use crate::runtime::sandbox_session::{redeploy_sandbox_skills, stop_running_sandbox_for_redeploy};
use crate::skill_manager::model::{DiscoveredSkill, InspectionResult, Skill, SkillSummary};
use crate::skill_manager::source_resolve::{self, inspect_resolved_source};
use crate::skill_manager::science_sync::{self, ScienceSkillSyncCandidate, SyncScienceSkillsResult};
use crate::skill_manager::store::SkillStore;
use crate::skill_manager::workspace_ingress::{
    self, AdoptWorkspaceSkillsResult, WorkspaceSkillCandidate, WorkspaceSkillPreview,
};
use crate::SharedAppState;
use serde::{Deserialize, Serialize};
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
/// Each entry is scanned for immediate subfolders containing a `SKILL.md`
/// (same shape everywhere — no recursive HOME walk).
const DISCOVERY_ROOTS: &[&str] = &[
    ".agents/skills",
    ".codex/skills",
    ".claude/skills",
    ".cursor/skills",
    ".cursor/skills-cursor", // Cursor-managed / synced Skills
    // Kimi Code — user level skills (default: ~/.kimi-code/skills).
    ".kimi-code/skills",
    // MiniMax CLI / agents — default skills root (default: ~/.minimax/skills).
    ".minimax/skills",
    // Google Antigravity / Gemini — each product variant has its own global
    // Skills dir (Antigravity 2.0 hub / IDE / CLI + shared), all SKILL.md layout:
    ".gemini/config/skills",          // Antigravity 2.0 — seen by all variants
    ".gemini/antigravity/skills",     // Antigravity IDE
    ".gemini/antigravity-cli/skills", // Antigravity CLI
    ".gemini/skills",                 // shared (CLI + IDE)
    // Windsurf Cascade — user/global Skills root (workspace uses .windsurf/skills/).
    ".windsurf/skills",
    // OpenClaw (ex-Moltbot / Clawdbot) — ClawHub-installed Skills, plus the
    // default agent workspace root (docs: workspace `<ws>/skills` has highest
    // precedence; default workspace is ~/.openclaw/workspace).
    ".openclaw/skills",
    ".openclaw/workspace/skills",
    // Tencent QClaw (小龙虾) — OpenClaw wrapper with its own state dir ~/.qclaw.
    // User skills + ClawHub-installed skills + default workspace skills.
    ".qclaw/skills",
    ".qclaw/skillhub-skills",
    ".qclaw/workspace/skills",
    // AWS Kiro — empty on fresh installs; same SKILL.md folder layout when used.
    ".kiro/skills",
    // Domestic (China) agents / IDEs that also use the `SKILL.md` folder layout.
    ".trae/skills",      // ByteDance Trae (international)
    ".trae-cn/skills",   // ByteDance Trae CN
    ".codebuddy/skills", // Tencent CodeBuddy
    // Tencent WorkBuddy (desktop / VS Code extension) — user skills live under
    // ~/.workbuddy/skills (distinct from CodeBuddy's ~/.codebuddy/skills).
    ".workbuddy/skills",
    // Factory Droid (factory.ai) — personal skills: ~/.factory/skills.
    ".factory/skills",
    // OpenCode — global Agent Skills (agentskills.io spec): ~/.config/opencode/skills.
    ".config/opencode/skills",
    // Qwen Code — personal skills: ~/.qwen/skills (iFlow mirrors the same layout).
    ".qwen/skills",
    ".iflow/skills",
    // Alibaba Qoder / Qoder CN (通义灵码 series) — personal skills: ~/.qoder/skills.
    ".qoder/skills",
    // Alibaba QoderWork (desktop work agent) — all skills live in ~/.qoderwork/skills.
    ".qoderwork/skills",
    // Note: Cline / Kimi / Zed / Warp resolve to the shared ~/.agents/skills root above.
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
    pub source: String,
}

#[tauri::command]
pub async fn inspect_skill_source(
    input: InspectSkillSourceInput,
) -> Result<InspectionResult, String> {
    let source = input.source;
    run_blocking(move || {
        let (mut inspection, resolved) = inspect_resolved_source(&source)?;
        inspection.import_path = resolved.import_root.display().to_string();
        inspection.logical_source = resolved.logical_source;
        Ok(inspection)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportSkillInput {
    /// Resolved directory from inspect (`importPath`); falls back to `source` when empty.
    pub import_path: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillActionResult {
    pub skill: Skill,
    /// Sandbox was stopped so the user can restart with the change applied.
    pub needs_restart: bool,
}

#[tauri::command]
pub async fn import_skill(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: ImportSkillInput,
) -> Result<SkillActionResult, String> {
    let import_path = input.import_path;
    let source = input.source;
    let state = state.inner().clone();
    run_blocking(move || {
        let store = SkillStore::open()?;
        let resolved = source_resolve::resolve_for_import(import_path.as_deref(), &source)?;
        let skill = store
            .import_with_logical_source(&resolved.import_root, Some(&resolved.logical_source))?;
        let mut needs_restart = false;
        // Imports land enabled; redeploy so a running sandbox can pick them up.
        if skill.enabled && redeploy_sandbox_skills() {
            needs_restart = stop_running_sandbox_for_redeploy(&app, &state)?;
        }
        Ok(SkillActionResult {
            skill,
            needs_restart,
        })
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSkillInput {
    /// Full `SKILL.md` content authored in the UI (front-matter + body). The
    /// name/description are parsed from its front-matter (single source of
    /// truth), so no separate name/description fields are needed here.
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillResult {
    pub skill: SkillSummary,
    /// Sandbox was stopped so the user can restart with the new Skill applied
    /// (mirrors the adopt flow's `needs_restart`).
    pub needs_restart: bool,
}

/// Author a brand-new Skill from scratch and persist it into `~/.csp/skills/`.
///
/// Reuses the shared import path (dedupe/inventory/copy), then follows the same
/// redeploy/restart behavior as [`adopt_workspace_skills`]: if enabled Skills on
/// disk changed and our sandbox is running, stop it and flag `needs_restart` so
/// the frontend can offer a clean restart.
#[tauri::command]
pub async fn create_skill(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: CreateSkillInput,
) -> Result<CreateSkillResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let store = SkillStore::open()?;
        let skill = store.create_from_content(&input.content)?;
        let mut needs_restart = false;
        if redeploy_sandbox_skills() {
            needs_restart = stop_running_sandbox_for_redeploy(&app, &state)?;
        }
        Ok(CreateSkillResult {
            skill: SkillSummary::from(&skill),
            needs_restart,
        })
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
pub async fn set_skill_enabled(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: SetSkillEnabledInput,
) -> Result<SkillActionResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let store = SkillStore::open()?;
        let skill = store.set_enabled(&input.skill_id, input.enabled)?;
        let mut needs_restart = false;
        if redeploy_sandbox_skills() {
            needs_restart = stop_running_sandbox_for_redeploy(&app, &state)?;
        }
        Ok(SkillActionResult {
            skill,
            needs_restart,
        })
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
        let target = if md.is_file() {
            md
        } else {
            skill.store_path.clone()
        };
        crate::runtime::system::open_path_in_default_app(&target)?;
        Ok(target.display().to_string())
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PickSkillSourceInput {
    pub title: Option<String>,
}

/// Native chooser for a Skill folder or `.zip` (one panel on macOS).
#[tauri::command]
pub async fn pick_skill_source(input: PickSkillSourceInput) -> Result<Option<String>, String> {
    let title = input
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Select Skill folder or zip".to_string());
    run_blocking(move || {
        let path = crate::skill_manager::native_pick::pick_skill_source(&title);
        Ok(path.map(|p| p.to_string_lossy().into_owned()))
    })
    .await
}

#[tauri::command]
pub async fn discover_workspace_skills() -> Result<Vec<WorkspaceSkillCandidate>, String> {
    run_blocking(workspace_ingress::discover_workspace_skills).await
}

/// Discover Science library drift + unpublished workspace drafts for the Sync page.
#[tauri::command]
pub async fn discover_science_skill_sync() -> Result<Vec<ScienceSkillSyncCandidate>, String> {
    run_blocking(science_sync::discover_science_skill_sync).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewWorkspaceSkillInput {
    pub key: String,
    pub file: Option<String>,
}

/// Read a Science workspace Skill draft for the adopt-page preview panel.
#[tauri::command]
pub async fn preview_workspace_skill(
    input: PreviewWorkspaceSkillInput,
) -> Result<WorkspaceSkillPreview, String> {
    run_blocking(move || {
        workspace_ingress::preview_workspace_skill(&input.key, input.file.as_deref())
    })
    .await
}

/// Preview a Science-library or workspace sync candidate.
#[tauri::command]
pub async fn preview_science_skill(
    input: PreviewWorkspaceSkillInput,
) -> Result<WorkspaceSkillPreview, String> {
    run_blocking(move || science_sync::preview_science_skill(&input.key, input.file.as_deref()))
        .await
}

/// Reveal a Science workspace Skill draft in Finder / the default app.
#[tauri::command]
pub async fn open_workspace_skill(input: PreviewWorkspaceSkillInput) -> Result<String, String> {
    run_blocking(move || workspace_ingress::open_workspace_skill(&input.key)).await
}

#[tauri::command]
pub async fn open_science_skill(input: PreviewWorkspaceSkillInput) -> Result<String, String> {
    run_blocking(move || science_sync::open_science_skill(&input.key)).await
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
            result.needs_restart = stop_running_sandbox_for_redeploy(&app, &state)?;
        }
        Ok(result)
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncScienceSkillsInput {
    pub keys: Vec<String>,
}

/// Harvest / import / adopt selected Science skill sync candidates into CSP store.
#[tauri::command]
pub async fn sync_science_skills(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    input: SyncScienceSkillsInput,
) -> Result<SyncScienceSkillsResult, String> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut result = science_sync::sync_science_skills(&input.keys)?;
        if !result.synced.is_empty() && redeploy_sandbox_skills() {
            result.needs_restart = stop_running_sandbox_for_redeploy(&app, &state)?;
        }
        Ok(result)
    })
    .await
}
