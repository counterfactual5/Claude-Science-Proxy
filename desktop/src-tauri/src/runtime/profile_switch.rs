//! Profile switch transaction: scratch-verify candidate → start formal proxy → health → commit or rollback.

use serde_json::{json, Value};

use crate::runtime::i18n::{hint_payload, i18n_err};
use crate::runtime::operation::{OperationKind, OperationStage, OperationTrace};
use crate::runtime::profile::{nonactive_probe_verdict, probe_kind_for, ConnectionEdit};
use crate::runtime::provider::{
    assert_format_supported, is_native_adapter, proxy_args_for,
    reject_openai_custom_anthropic_base, relay_missing_profile_models, should_scratch_candidate,
};
use crate::runtime::platter::{validate_platter_entries, proxy_args_for_platter};
use crate::runtime::proxy_lifecycle::{start_proxy_for_platter, start_proxy_for_profiles};
use crate::runtime::system::asset_root;
use crate::runtime::transaction::{
    decide_switch, rollback_status_key, skip_scratch_verify, SwitchOutcome,
};
use crate::{config, lifecycle, lock, proc, scratch, SharedAppState};

fn switch_ctx(is_edit: bool) -> &'static str {
    if is_edit {
        "Edit"
    } else {
        "Switch"
    }
}

fn merge_hint(mut base: Value, extra: Value) -> Value {
    if let (Some(a), Some(b)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    base
}

/// Validate a non-active candidate without touching config, AppState, or the active proxy.
pub(crate) fn scratch_validate_candidate(
    app: &tauri::AppHandle,
    candidate: &config::Profile,
) -> Result<bool, String> {
    let launch = proxy_args_for(candidate);
    let trace = OperationTrace::start(
        OperationKind::ValidateConnection,
        format!(
            "profile_id={} template_id={} adapter={}",
            candidate.id, candidate.template_id, launch.adapter
        ),
    );
    if !should_scratch_candidate(&launch.adapter, &launch.key, &launch.base_url) {
        trace.finish("skipped reason=missing_key_or_base");
        return Ok(false);
    }
    let root = asset_root(app).ok_or_else(|| i18n_err("errProxyScriptMissing", json!({})))?;
    let py = proc::find_exe("python3").ok_or_else(|| i18n_err("errPythonMissing", json!({})))?;
    let script = root.join("proxy/core/csp_proxy.py");
    let res = scratch::scratch_probe(
        &py,
        &script,
        &scratch::ScratchTarget {
            provider: &launch.adapter,
            key_env: launch.key_env,
            base_url: &launch.base_url,
            key: &launch.key,
            model: Some(&launch.model),
            relay_thinking: launch.thinking_policy,
        },
        probe_kind_for(&launch.adapter, &launch.model),
        Some(&trace),
    );
    let outcome = scratch::classify(res.status);
    trace.finish(format!("outcome={outcome:?}"));
    nonactive_probe_verdict(&outcome)
}

/// Switch the active profile transactionally: scratch validate, start real proxy, then commit.
///
/// Callers must hold the command serializer lock.
pub(crate) fn set_active_profile_txn(
    app: &tauri::AppHandle,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    id: &str,
    skip_verify: bool,
    conn_edit: Option<&ConnectionEdit>,
) -> Result<Value, String> {
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
    let mut candidate = cfg
        .profile_by_id(id)
        .cloned()
        .ok_or_else(|| i18n_err("errProfileNotFound", json!({ "id": id })))?;
    if let Some(edit) = conn_edit {
        edit.apply(&mut candidate);
    }
    let is_edit = conn_edit.is_some();
    let ctx = switch_ctx(is_edit);
    assert_format_supported(&candidate)?;
    let launch = proxy_args_for(&candidate);
    reject_openai_custom_anthropic_base(&launch.adapter, &candidate.base_url)?;
    if launch.key.is_empty() {
        return Err(i18n_err(
            "errMissingApiKey",
            json!({ "name": candidate.name }),
        ));
    }
    let native = is_native_adapter(&launch.adapter);
    if !native && launch.base_url.is_empty() {
        return Err(i18n_err("errMissingBaseUrl", json!({})));
    }
    if relay_missing_profile_models(&launch.adapter, &candidate) {
        return Err(i18n_err("errMissingModel", json!({})));
    }

    let trace = OperationTrace::start(
        if is_edit {
            OperationKind::UpdateActiveConnection
        } else {
            OperationKind::ActivateProfile
        },
        format!(
            "profile_id={} template_id={} adapter={} skip_verify={}",
            candidate.id, candidate.template_id, launch.adapter, skip_verify
        ),
    );

    let scratch_ok = if skip_scratch_verify(native, skip_verify) {
        trace.stage(OperationStage::ScratchUpstreamProbe, "skipped_by_user");
        true
    } else {
        let root = asset_root(app).ok_or_else(|| i18n_err("errProxyScriptMissing", json!({})))?;
        let py =
            proc::find_exe("python3").ok_or_else(|| i18n_err("errPythonMissing", json!({})))?;
        let script = root.join("proxy/core/csp_proxy.py");
        let res = scratch::scratch_probe(
            &py,
            &script,
            &scratch::ScratchTarget {
                provider: &launch.adapter,
                key_env: launch.key_env,
                base_url: &launch.base_url,
                key: &launch.key,
                model: Some(&launch.model),
                relay_thinking: launch.thinking_policy,
            },
            probe_kind_for(&launch.adapter, &launch.model),
            Some(&trace),
        );
        let outcome = scratch::classify(res.status);
        trace.stage(
            OperationStage::ScratchUpstreamProbe,
            format!("outcome={outcome:?}"),
        );
        match outcome {
            scratch::ProbeOutcome::Ok => true,
            scratch::ProbeOutcome::Auth(code) => {
                trace.finish(format!("rejected status={code}"));
                return Ok(merge_hint(
                    hint_payload(&format!("switchUpstreamAuth{ctx}"), json!({ "code": code })),
                    json!({ "committed": false }),
                ));
            }
            scratch::ProbeOutcome::ModelError(code) => {
                trace.finish(format!("model_error status={code}"));
                return Ok(merge_hint(
                    hint_payload(
                        &format!("switchUpstreamModel{ctx}"),
                        json!({ "code": code }),
                    ),
                    json!({ "committed": false }),
                ));
            }
            scratch::ProbeOutcome::Ambiguous(_)
            | scratch::ProbeOutcome::NoResponse
            | scratch::ProbeOutcome::Unsupported(_) => {
                trace.finish("ambiguous can_skip=true");
                return Ok(merge_hint(
                    hint_payload(&format!("switchUpstreamAmbiguous{ctx}"), json!({})),
                    json!({ "committed": false, "can_skip": true }),
                ));
            }
        }
    };

    lifecycle.bump_generation();
    let profiles_after: Vec<config::Profile> = vec![candidate.clone()];
    let real_healthy = scratch_ok
        && start_proxy_for_profiles(app, state, lifecycle, &profiles_after, Some(&trace)).is_ok();

    match decide_switch(scratch_ok, real_healthy) {
        SwitchOutcome::Commit => {
            if is_edit {
                config::write_rolling_backup(&dir).ok();
            }
            if let Err(e) = config::update(&dir, |c| {
                c.set_exclusive_active(id);
                if let Some(edit) = conn_edit {
                    if let Some(p) = c.profile_by_id_mut(id) {
                        edit.apply(p);
                    }
                }
            }) {
                trace.stage(OperationStage::Rollback, "reason=config_write_failed");
                let restored =
                    restore_proxy_for_active_profile(app, state, lifecycle, &cfg, Some(&trace));
                trace.finish(format!("error=config_write_failed restored={restored}"));
                return Err(i18n_err(
                    "errConfigWriteFailed",
                    json!({
                        "error": e.to_string(),
                        "rollback_key": rollback_status_key(restored),
                    }),
                ));
            }
            trace.stage(OperationStage::Commit, "ok");
            trace.finish("committed=true");
            Ok(json!({
                "committed": true,
                "active_id": id,
            }))
        }
        SwitchOutcome::RollbackToOld => {
            trace.stage(OperationStage::Rollback, "reason=proxy_unhealthy");
            let restored =
                restore_proxy_for_active_profile(app, state, lifecycle, &cfg, Some(&trace));
            trace.finish(format!("rollback restored={restored}"));
            Err(i18n_err(
                &format!("switchProxyRollback{ctx}"),
                json!({ "rollback_key": rollback_status_key(restored) }),
            ))
        }
        SwitchOutcome::AbortBeforeStart => {
            trace.finish("aborted_before_start");
            Err(i18n_err(&format!("switchAbortBeforeStart{ctx}"), json!({})))
        }
    }
}

fn restore_proxy_for_active_profile(
    app: &tauri::AppHandle,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    cfg: &config::Config,
    trace: Option<&OperationTrace>,
) -> bool {
    if cfg.is_platter_active() && !cfg.model_platter.entries.is_empty() {
        lifecycle.bump_generation();
        return start_proxy_for_platter(app, state, lifecycle, cfg, trace).is_ok();
    }
    let Some(p) = cfg.active_profile() else {
        return true;
    };
    lifecycle.bump_generation();
    start_proxy_for_profiles(app, state, lifecycle, std::slice::from_ref(p), trace).is_ok()
}

/// If platter is active and a formal proxy is running, bump generation and restart
/// with the current on-disk platter (registry + credentials). Returns whether a
/// reload was attempted and succeeded.
pub(crate) fn reload_active_platter_proxy(
    app: &tauri::AppHandle,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
) -> Result<bool, String> {
    let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
    if !cfg.is_platter_active() || cfg.model_platter.entries.is_empty() {
        return Ok(false);
    }
    let proxy_running = lock(state).proxy.is_some();
    if !proxy_running {
        return Ok(false);
    }
    validate_platter_entries(&cfg, &cfg.model_platter.entries)?;
    lifecycle.bump_generation();
    start_proxy_for_platter(app, state, lifecycle, &cfg, None)?;
    Ok(true)
}

/// Activate the multi-provider model platter (transactional proxy switch).
pub(crate) fn set_active_platter_txn(
    app: &tauri::AppHandle,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    skip_verify: bool,
) -> Result<Value, String> {
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
    validate_platter_entries(&cfg, &cfg.model_platter.entries)?;
    let launch = proxy_args_for_platter(&cfg)?;
    let first_profile = cfg
        .profile_by_id(&cfg.model_platter.entries[0].profile_id)
        .cloned()
        .ok_or_else(|| i18n_err("errPlatterEmpty", json!({})))?;
    // Scratch must use the first entry's real adapter (openai-custom / relay / deepseek),
    // not the host process adapter which is always relay for platter.
    let probe_launch = proxy_args_for(&first_profile);
    let probe_model = cfg.model_platter.entries[0].model.clone();

    let trace = OperationTrace::start(
        OperationKind::ActivateProfile,
        format!(
            "platter models={} host={} probe_adapter={} skip_verify={}",
            cfg.model_platter.entries.len(),
            launch.adapter,
            probe_launch.adapter,
            skip_verify
        ),
    );

    let native = is_native_adapter(&probe_launch.adapter);
    let scratch_ok = if skip_scratch_verify(native, skip_verify) {
        trace.stage(OperationStage::ScratchUpstreamProbe, "skipped_by_user");
        true
    } else {
        let root = asset_root(app).ok_or_else(|| i18n_err("errProxyScriptMissing", json!({})))?;
        let py =
            proc::find_exe("python3").ok_or_else(|| i18n_err("errPythonMissing", json!({})))?;
        let script = root.join("proxy/core/csp_proxy.py");
        let res = scratch::scratch_probe(
            &py,
            &script,
            &scratch::ScratchTarget {
                provider: &probe_launch.adapter,
                key_env: probe_launch.key_env,
                base_url: &probe_launch.base_url,
                key: &probe_launch.key,
                model: Some(&probe_model),
                relay_thinking: probe_launch.thinking_policy,
            },
            probe_kind_for(&probe_launch.adapter, &probe_model),
            Some(&trace),
        );
        let outcome = scratch::classify(res.status);
        trace.stage(
            OperationStage::ScratchUpstreamProbe,
            format!("outcome={outcome:?}"),
        );
        match outcome {
            scratch::ProbeOutcome::Ok => true,
            scratch::ProbeOutcome::Auth(code) => {
                trace.finish(format!("rejected status={code}"));
                return Ok(merge_hint(
                    hint_payload("switchUpstreamAuthSwitch", json!({ "code": code })),
                    json!({ "committed": false }),
                ));
            }
            scratch::ProbeOutcome::ModelError(code) => {
                trace.finish(format!("model_error status={code}"));
                return Ok(merge_hint(
                    hint_payload("switchUpstreamModelSwitch", json!({ "code": code })),
                    json!({ "committed": false }),
                ));
            }
            scratch::ProbeOutcome::Ambiguous(_)
            | scratch::ProbeOutcome::NoResponse
            | scratch::ProbeOutcome::Unsupported(_) => {
                trace.finish("ambiguous can_skip=true");
                return Ok(merge_hint(
                    hint_payload("switchUpstreamAmbiguousSwitch", json!({})),
                    json!({ "committed": false, "can_skip": true }),
                ));
            }
        }
    };

    lifecycle.bump_generation();
    let real_healthy =
        scratch_ok && start_proxy_for_platter(app, state, lifecycle, &cfg, Some(&trace)).is_ok();

    match decide_switch(scratch_ok, real_healthy) {
        SwitchOutcome::Commit => {
            if let Err(e) = config::update(&dir, |c| {
                c.set_active_platter();
            }) {
                trace.stage(OperationStage::Rollback, "reason=config_write_failed");
                let restored =
                    restore_proxy_for_active_profile(app, state, lifecycle, &cfg, Some(&trace));
                trace.finish(format!("error=config_write_failed restored={restored}"));
                return Err(i18n_err(
                    "errConfigWriteFailed",
                    json!({
                        "error": e.to_string(),
                        "rollback_key": rollback_status_key(restored),
                    }),
                ));
            }
            trace.stage(OperationStage::Commit, "ok");
            trace.finish("committed=true");
            Ok(json!({
                "committed": true,
                "active_id": config::PLATTER_ACTIVE_ID,
            }))
        }
        SwitchOutcome::RollbackToOld => {
            trace.stage(OperationStage::Rollback, "reason=proxy_unhealthy");
            let restored =
                restore_proxy_for_active_profile(app, state, lifecycle, &cfg, Some(&trace));
            trace.finish(format!("rollback restored={restored}"));
            Err(i18n_err(
                "switchProxyRollbackSwitch",
                json!({ "rollback_key": rollback_status_key(restored) }),
            ))
        }
        SwitchOutcome::AbortBeforeStart => {
            trace.finish("aborted_before_start");
            Err(i18n_err("switchAbortBeforeStartSwitch", json!({})))
        }
    }
}
