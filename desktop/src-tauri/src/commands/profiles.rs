use serde_json::json;
use tauri::State;

use crate::runtime::i18n::i18n_err;
use crate::runtime::profile::{
    build_get_config, create_profile_inner, delete_profile_inner, save_model_platter_inner,
    update_profile_connection_inner, update_profile_metadata_inner, ConnectionEdit,
};
use crate::runtime::profile_switch::{
    reload_active_platter_proxy, scratch_validate_candidate, set_active_platter_txn,
    set_active_profile_txn,
};
use crate::runtime::provider::{
    adapter_for_profile, reject_openai_custom_anthropic_base, relay_missing_base_url,
    relay_missing_profile_models,
};
use crate::{config, lock, run_blocking, SharedAppState, SharedLifecycle};

#[tauri::command]
pub(crate) fn get_config() -> Result<serde_json::Value, String> {
    build_get_config(&config::default_dir())
}

// Profile CRUD commands (thin wrappers over `*_inner`, all serialized via lifecycle).
#[tauri::command]
pub(crate) fn create_profile(
    lifecycle: State<'_, SharedLifecycle>,
    template_id: String,
    name: String,
    key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    lifecycle.with_serialized(|| {
        create_profile_inner(
            &config::default_dir(),
            &template_id,
            &name,
            key.as_deref(),
            base_url.as_deref(),
            model.as_deref(),
        )
    })
}

#[tauri::command]
pub(crate) fn update_profile_metadata(
    lifecycle: State<'_, SharedLifecycle>,
    id: String,
    name: String,
    notes: Option<String>,
) -> Result<(), String> {
    lifecycle.with_serialized(|| {
        update_profile_metadata_inner(&config::default_dir(), &id, &name, notes.as_deref())
    })
}

/// Delete profile via lifecycle. If it was active or a platter member while
/// platter is active: clear/adjust active, bump generation, stop proxy.
#[tauri::command]
pub(crate) fn delete_profile(
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    id: String,
) -> Result<(), String> {
    lifecycle.with_serialized(|| {
        let dir = config::default_dir();
        let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
        let was_active = cfg.is_profile_active(&id);
        let in_active_platter = cfg.uses_platter_profile(&id);
        delete_profile_inner(&dir, &id)?;
        if was_active || in_active_platter {
            lifecycle.bump_generation();
            let mut st = lock(state.inner());
            st.stop_proxy();
        }
        Ok(())
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn update_profile_connection(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    id: String,
    base_url: Option<String>,
    api_format: Option<String>,
    model: Option<String>,
    active_models: Option<Vec<String>>,
    default_model: Option<String>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || {
        update_profile_connection_inner_cmd(
            app,
            state,
            lifecycle,
            id,
            base_url,
            api_format,
            model,
            active_models,
            default_model,
            key,
        )
    })
    .await
}

#[allow(clippy::too_many_arguments)]
fn update_profile_connection_inner_cmd(
    app: tauri::AppHandle,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    id: String,
    base_url: Option<String>,
    api_format: Option<String>,
    model: Option<String>,
    active_models: Option<Vec<String>>,
    default_model: Option<String>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    lifecycle.with_serialized(|| {
        let dir = config::default_dir();
        let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
        // Missing id → Err (never silent Ok).
        let mut candidate = cfg
            .profile_by_id(&id)
            .cloned()
            .ok_or_else(|| i18n_err("errProfileNotFound", serde_json::json!({ "id": id })))?;
        // Candidate connection after merge (None = keep existing); shared by active/non-active paths.
        let edit = ConnectionEdit::with_models(
            base_url.clone(),
            api_format.clone(),
            model.clone(),
            active_models.clone(),
            default_model.clone(),
            key.clone(),
        );
        edit.apply(&mut candidate);
        let adapter = adapter_for_profile(&candidate);
        reject_openai_custom_anthropic_base(adapter, &candidate.base_url)?;
        // Guard: relay/custom endpoints require non-empty base_url before persist.
        if relay_missing_base_url(adapter, &candidate.base_url) {
            return Err(i18n_err("errRelayMissingBaseUrl", serde_json::json!({})));
        }
        // Guard: relay/custom endpoints require at least one model.
        if relay_missing_profile_models(adapter, &candidate) {
            return Err(i18n_err("errRelayMissingModel", serde_json::json!({})));
        }
        if cfg.is_profile_active(&id) {
            // Active profile: validate-before-persist via switch transaction; disk unchanged on failure.
            let v =
                set_active_profile_txn(&app, &state, lifecycle.as_ref(), &id, false, Some(&edit))?;
            // Surface scratch/health failure as error; disk and proxy stay on previous connection.
            if v.get("committed").and_then(|b| b.as_bool()) == Some(false) {
                if let Some(key) = v.get("hint_key").and_then(|k| k.as_str()) {
                    return Err(serde_json::to_string(&serde_json::json!({
                        "i18n": key,
                        "vars": v.get("hint_vars").cloned().unwrap_or(serde_json::json!({})),
                    }))
                    .unwrap_or_else(|_| "errConnValidateFailed".to_string()));
                }
                return Err(i18n_err("errConnValidateFailed", serde_json::json!({})));
            }
            Ok(json!({ "validated": true }))
        } else if cfg.uses_platter_profile(&id) {
            // Platter member while platter is active: persist then reload proxy.
            // Auth/ModelError already Err from scratch; Ambiguous → save unverified + reload.
            let validated = scratch_validate_candidate(&app, &candidate)?;
            update_profile_connection_inner(
                &dir,
                &id,
                base_url.as_deref(),
                api_format.as_deref(),
                model.as_deref(),
                active_models.as_deref(),
                default_model.as_deref(),
                key.as_deref(),
            )?;
            let proxy_reloaded =
                reload_active_platter_proxy(&app, &state, lifecycle.as_ref())?;
            Ok(json!({ "validated": validated, "proxy_reloaded": proxy_reloaded }))
        } else {
            // Non-active: scratch upstream verify; explicit reject blocks, else best-effort save + validated flag.
            let validated = scratch_validate_candidate(&app, &candidate)?;
            update_profile_connection_inner(
                &dir,
                &id,
                base_url.as_deref(),
                api_format.as_deref(),
                model.as_deref(),
                active_models.as_deref(),
                default_model.as_deref(),
                key.as_deref(),
            )?;
            Ok(json!({ "validated": validated }))
        }
    })
}

/// Set active profile via [`set_active_profile_txn`] (serialized switch transaction).
#[tauri::command]
pub(crate) async fn set_active_profile(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    id: String,
    skip_verify: bool,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || set_active_profile_inner_cmd(app, state, lifecycle, id, skip_verify)).await
}

fn set_active_profile_inner_cmd(
    app: tauri::AppHandle,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    id: String,
    skip_verify: bool,
) -> Result<serde_json::Value, String> {
    lifecycle.with_serialized(|| {
        set_active_profile_txn(&app, &state, lifecycle.as_ref(), &id, skip_verify, None)
    })
}

#[derive(serde::Deserialize)]
pub(crate) struct PlatterEntryInput {
    pub profile_id: String,
    pub model: String,
}

#[tauri::command]
pub(crate) fn save_model_platter(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    entries: Vec<PlatterEntryInput>,
) -> Result<serde_json::Value, String> {
    lifecycle.with_serialized(|| {
        let mapped: Vec<config::PlatterEntry> = entries
            .into_iter()
            .map(|e| config::PlatterEntry {
                profile_id: e.profile_id,
                model: e.model,
            })
            .collect();
        let dir = config::default_dir();
        let was_platter = config::load_from(&dir)
            .map(|c| c.is_platter_active())
            .unwrap_or(false);
        save_model_platter_inner(&dir, mapped)?;
        let proxy_reloaded = if was_platter {
            reload_active_platter_proxy(&app, state.inner(), lifecycle.as_ref())?
        } else {
            false
        };
        Ok(json!({ "proxy_reloaded": proxy_reloaded }))
    })
}

#[tauri::command]
pub(crate) async fn set_active_platter(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    skip_verify: bool,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || {
        lifecycle.with_serialized(|| {
            set_active_platter_txn(&app, &state, lifecycle.as_ref(), skip_verify)
        })
    })
    .await
}

#[tauri::command]
pub(crate) fn open_csp_json() -> Result<String, String> {
    let dir = config::default_dir();
    config::ensure_dir(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(config::CONFIG_BASENAME);
    if !path.exists() {
        let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
        config::save_to(&dir, &cfg).map_err(|e| e.to_string())?;
    }
    crate::runtime::system::open_path_in_default_app(&path)?;
    Ok(path.display().to_string())
}
