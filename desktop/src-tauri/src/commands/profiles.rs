use serde_json::json;
use tauri::State;

use crate::runtime::i18n::i18n_err;
use crate::runtime::profile::{
    build_get_config, create_profile_inner, delete_profile_inner, update_profile_connection_inner,
    update_profile_metadata_inner, ConnectionEdit,
};
use crate::runtime::profile_switch::{scratch_validate_candidate, set_active_profile_txn};
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

/// 删 profile：经串行器；删的是【生效】profile → active 置空（inner 内）+ bump + 停代理。
#[tauri::command]
pub(crate) fn delete_profile(
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    id: String,
) -> Result<(), String> {
    lifecycle.with_serialized(|| {
        let dir = config::default_dir();
        let was_active = config::load_from(&dir)
            .map(|c| c.is_profile_active(&id))
            .unwrap_or(false);
        delete_profile_inner(&dir, &id)?;
        if was_active {
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
        // 未命中 id → Err（不静默 Ok）。
        let mut candidate = cfg
            .profile_by_id(&id)
            .cloned()
            .ok_or_else(|| i18n_err("errProfileNotFound", serde_json::json!({ "id": id })))?;
        // 生效【后】的候选连接（None=不改则沿用旧值），active/非 active 共用一份。
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
        // 保存前守卫（修 P2）：relay/自定义端点清空 base_url → 不可用连接（激活必失败）。
        // 校验生效后的 base_url，空则拒绝落盘、绝不谎报「已保存」；native 走硬编码端点，空无妨。
        if relay_missing_base_url(adapter, &candidate.base_url) {
            return Err(i18n_err("errRelayMissingBaseUrl", serde_json::json!({})));
        }
        // 保存前守卫（修 #9 P1-a）：relay/自定义端点空 model → 无 force → 退回 passthrough（显示 claude）。
        if relay_missing_profile_models(adapter, &candidate) {
            return Err(i18n_err("errRelayMissingModel", serde_json::json!({})));
        }
        if cfg.is_profile_active(&id) {
            // active（有正在服务的代理）：validate-before-persist —— 新连接作【内存候选】喂进
            // 切换事务（校验→起正式→健康），探活健康【才】连同落盘；失败则磁盘连接零改动、
            // 仍跑旧连接（杜绝「盘新运行旧」，修 P1-4）。
            let v =
                set_active_profile_txn(&app, &state, lifecycle.as_ref(), &id, false, Some(&edit))?;
            // 连接编辑：committed:false（scratch 分类失败）也如实作为错误上抛（磁盘未改、代理仍跑旧的）。
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
            // active：已连同起正式代理探活并落盘，视为已校验。
            Ok(json!({ "validated": true }))
        } else {
            // 非 active：无正在服务的代理。先对候选做上游 scratch 校验（仅明确拒绝才拦，其余
            // best-effort 落盘并如实标记「未校验」，修 P2-d：贴合设计「校验候选后提交」+ 如实报告），
            // 再落盘（inner 内含格式门 + 覆盖前留底）。
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

/// 一键切生效 profile：经串行器走 [`set_active_profile_txn`] 切换事务。
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
