//! Tauri commands for Skill Manager (frontend-facing).

use std::path::PathBuf;

use crate::run_blocking;
use crate::skill_manager::model::{InspectionResult, Skill, SkillSummary};
use crate::skill_manager::store::SkillStore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCommandError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCommandResult<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SkillCommandError>,
}

impl<T: Serialize> SkillCommandResult<T> {
    fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn failure(err: SkillCommandError) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(err),
        }
    }
}

fn map_err(e: String) -> SkillCommandError {
    SkillCommandError {
        code: "skill_error".into(),
        message: e,
    }
}

#[tauri::command]
pub async fn list_skills() -> Result<SkillCommandResult<Vec<SkillSummary>>, String> {
    run_blocking(
        || -> Result<SkillCommandResult<Vec<SkillSummary>>, String> {
            let store = SkillStore::open()?;
            match store.list() {
                Ok(list) => Ok(SkillCommandResult::success(list)),
                Err(e) => Ok(SkillCommandResult::failure(map_err(e))),
            }
        },
    )
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
) -> Result<SkillCommandResult<InspectionResult>, String> {
    let source = PathBuf::from(input.source_path);
    run_blocking(
        move || -> Result<SkillCommandResult<InspectionResult>, String> {
            match SkillStore::inspect_source(&source) {
                Ok(result) => Ok(SkillCommandResult::success(result)),
                Err(e) => Ok(SkillCommandResult::failure(map_err(e))),
            }
        },
    )
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportSkillInput {
    pub source_path: String,
}

#[tauri::command]
pub async fn import_skill(input: ImportSkillInput) -> Result<SkillCommandResult<Skill>, String> {
    let source = PathBuf::from(input.source_path);
    run_blocking(move || -> Result<SkillCommandResult<Skill>, String> {
        let store = SkillStore::open()?;
        match store.import(&source) {
            Ok(skill) => Ok(SkillCommandResult::success(skill)),
            Err(e) => Ok(SkillCommandResult::failure(map_err(e))),
        }
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
    input: SetSkillEnabledInput,
) -> Result<SkillCommandResult<Skill>, String> {
    run_blocking(move || -> Result<SkillCommandResult<Skill>, String> {
        let store = SkillStore::open()?;
        match store.set_enabled(&input.skill_id, input.enabled) {
            Ok(skill) => Ok(SkillCommandResult::success(skill)),
            Err(e) => Ok(SkillCommandResult::failure(map_err(e))),
        }
    })
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveSkillInput {
    pub skill_id: String,
}

#[tauri::command]
pub async fn remove_skill(input: RemoveSkillInput) -> Result<SkillCommandResult<()>, String> {
    run_blocking(move || -> Result<SkillCommandResult<()>, String> {
        let store = SkillStore::open()?;
        match store.remove(&input.skill_id) {
            Ok(()) => Ok(SkillCommandResult::success(())),
            Err(e) => Ok(SkillCommandResult::failure(map_err(e))),
        }
    })
    .await
}
