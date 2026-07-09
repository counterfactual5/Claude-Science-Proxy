use std::path::Path;

use serde_json::json;

use crate::runtime::i18n::i18n_err;
use crate::runtime::model_sort;
use crate::runtime::provider::{
    adapter_for_profile, assert_format_supported, is_native_adapter, is_openai_adapter,
    reject_openai_custom_anthropic_base,
};
use crate::{config, scratch, templates};

/// Whether a model id belongs in the Science selector main list (claude-{opus|sonnet|haiku}-<digits…>).
/// Used only for fetch-models sort order, not auth.
pub(crate) fn is_main_list_model(id: &str) -> bool {
    for fam in ["claude-opus-", "claude-sonnet-", "claude-haiku-"] {
        if let Some(rest) = id.strip_prefix(fam) {
            return rest
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);
        }
    }
    false
}

fn build_capabilities(
    adapter: &str,
    api_format: &str,
    model_required: bool,
    thinking_policy: &str,
) -> serde_json::Value {
    let model_discovery = if is_native_adapter(adapter) {
        "builtin_static"
    } else if is_openai_adapter(adapter) || matches!(api_format, "openai_chat" | "openai_responses")
    {
        "openai_models_or_manual"
    } else {
        "anthropic_models_or_manual"
    };
    let tools_hint = match api_format {
        "openai_chat" | "openai_responses" => "translated",
        "anthropic" if is_native_adapter(adapter) => "native",
        "anthropic" => "passthrough",
        _ => "unknown",
    };
    json!({
        "base_url_required": !is_native_adapter(adapter),
        "model_required": model_required,
        "model_discovery": model_discovery,
        "supports_thinking_policy": !thinking_policy.is_empty(),
        "thinking_policy": thinking_policy,
        "supports_tools_hint": tools_hint,
    })
}

pub(crate) fn template_capabilities(t: &templates::Template) -> serde_json::Value {
    build_capabilities(
        t.adapter,
        t.api_format,
        t.requires_model_override,
        t.thinking_policy,
    )
}

pub(crate) fn profile_capabilities(p: &config::Profile) -> serde_json::Value {
    match templates::by_id(&p.template_id) {
        Some(t) => {
            let api_format = if p.api_format.trim().is_empty() {
                t.api_format
            } else {
                &p.api_format
            };
            build_capabilities(
                t.adapter,
                api_format,
                t.requires_model_override,
                t.thinking_policy,
            )
        }
        None => {
            let api_format = if p.api_format.trim().is_empty() {
                "anthropic"
            } else {
                &p.api_format
            };
            build_capabilities("relay", api_format, true, "")
        }
    }
}

/// Build get_config payload: profile keys are masked only; full keys never leave the backend.
pub(crate) fn build_get_config(dir: &Path) -> Result<serde_json::Value, String> {
    let cfg = config::load_from(dir).map_err(|e| e.to_string())?;
    // One-shot migration notice: clear after read so get_config does not repeat it.
    let notice = cfg.pending_notice.clone();
    if notice.is_some() {
        config::update(dir, |c| c.pending_notice = None).map_err(|e| e.to_string())?;
    }
    let profiles: Vec<serde_json::Value> = cfg
        .profiles
        .iter()
        .map(|p| {
            let key_masked = config::mask(&p.api_key);
            json!({
                "id": p.id, "name": p.name, "template_id": p.template_id, "category": p.category,
                "api_format": p.api_format, "base_url": p.base_url, "model": p.model,
                "active_models": p.active_models, "default_model": p.default_model,
                "key": key_masked.clone(), "has_key": !p.api_key.is_empty(), "key_masked": key_masked,
                "capabilities": profile_capabilities(p), "icon": p.icon, "icon_color": p.icon_color,
                "website_url": p.website_url, "sort_index": p.sort_index, "notes": p.notes,
            })
        })
        .collect();
    Ok(json!({
        "schema_version": cfg.schema_version,
        "active_id": cfg.active_id,
        "active_ids": cfg.active_ids,
        "profiles": profiles,
        "templates": build_list_templates(), "proxy_port": cfg.proxy_port,
        "sandbox_port": cfg.sandbox_port, "pending_notice": notice,
    }))
}

/// Template registry for the frontend UI (single source; frontend does not duplicate constants).
pub(crate) fn build_list_templates() -> Vec<serde_json::Value> {
    templates::all()
        .iter()
        .map(|t| {
            json!({
                "id": t.id, "name": t.name, "category": t.category, "api_format": t.api_format,
                "adapter": t.adapter, "base_url": t.base_url, "base_url_editable": t.base_url_editable,
                "requires_model_override": t.requires_model_override,
                "builtin_models": t.builtin_models, "icon": t.icon, "icon_color": t.icon_color,
                "website_url": t.website_url, "capabilities": template_capabilities(t),
            })
        })
        .collect()
}

pub(crate) fn create_profile_inner(
    dir: &Path,
    template_id: &str,
    name: &str,
    key: Option<&str>,
    base_url_override: Option<&str>,
    model: Option<&str>,
) -> Result<String, String> {
    let tpl = templates::by_id(template_id)
        .ok_or_else(|| i18n_err("errUnknownTemplate", json!({ "id": template_id })))?;
    let id = config::new_id();
    let base_url = base_url_override
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| tpl.base_url.to_string());
    let p = config::Profile {
        id: id.clone(),
        name: name.to_string(),
        template_id: template_id.to_string(),
        category: tpl.category.to_string(),
        api_format: tpl.api_format.to_string(),
        base_url,
        api_key: key.unwrap_or("").to_string(),
        model: model.unwrap_or("").to_string(),
        active_models: Vec::new(),
        default_model: String::new(),
        website_url: Some(tpl.website_url.to_string()),
        icon: Some(tpl.icon.to_string()),
        icon_color: Some(tpl.icon_color.to_string()),
        sort_index: Some(config::now_ms()),
        created_at: Some(config::now_ms()),
        notes: None,
    };
    assert_format_supported(&p)?; // reject unsupported format on custom template
    let adapter = adapter_for_profile(&p);
    reject_openai_custom_anthropic_base(adapter, &p.base_url)?;
    // Model optional at create time; profile_switch validates before activation.
    let mut p = p;
    p.sync_model_fields();
    config::update(dir, |c| c.profiles.push(p)).map_err(|e| e.to_string())?;
    Ok(id)
}

pub(crate) fn update_profile_metadata_inner(
    dir: &Path,
    id: &str,
    name: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    // Missing id → Err (never silent Ok).
    if config::load_from(dir)
        .map_err(|e| e.to_string())?
        .profile_by_id(id)
        .is_none()
    {
        return Err(i18n_err("errProfileNotFound", json!({ "id": id })));
    }
    config::update(dir, |c| {
        if let Some(p) = c.profile_by_id_mut(id) {
            p.name = name.to_string();
            p.notes = notes.map(str::to_string);
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn delete_profile_inner(dir: &Path, id: &str) -> Result<(), String> {
    config::update(dir, |c| {
        c.profiles.retain(|p| p.id != id);
        c.deactivate_profile(id);
    })
    .map_err(|e| e.to_string())?;
    config::drop_rolling_backup(dir);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_profile_connection_inner(
    dir: &Path,
    id: &str,
    base_url: Option<&str>,
    api_format: Option<&str>,
    model: Option<&str>,
    active_models: Option<&[String]>,
    default_model: Option<&str>,
    key: Option<&str>,
) -> Result<(), String> {
    if let Some(fmt) = api_format {
        let probe = config::Profile {
            api_format: fmt.to_string(),
            ..Default::default()
        };
        assert_format_supported(&probe)?;
    }
    // Missing id → Err (never silent Ok).
    if config::load_from(dir)
        .map_err(|e| e.to_string())?
        .profile_by_id(id)
        .is_none()
    {
        return Err(i18n_err("errProfileNotFound", json!({ "id": id })));
    }
    config::write_rolling_backup(dir).ok(); // rolling backup before overwrite
    config::update(dir, |c| {
        if let Some(p) = c.profile_by_id_mut(id) {
            if let Some(u) = base_url {
                p.base_url = u.to_string();
            }
            if let Some(f) = api_format {
                p.api_format = f.to_string();
            }
            if let Some(m) = model {
                p.model = m.to_string();
            }
            if let Some(models) = active_models {
                let mut sorted: Vec<String> = models
                    .iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                model_sort::sort_model_ids(&mut sorted);
                p.active_models = sorted;
            }
            if let Some(d) = default_model {
                p.default_model = d.to_string();
            }
            p.sync_model_fields();
            if let Some(k) = key {
                if !k.is_empty() {
                    p.api_key = k.to_string(); // empty key = leave existing
                }
            }
        }
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Non-active connection edit upstream probe verdict (pure fn):
/// - `Ok(true)` upstream accepted (200), validated;
/// - `Ok(false)` inconclusive (429/5xx/no response), save best-effort as unvalidated;
/// - `Err(hint)` upstream rejected (401/403/400/404/422), do not persist.
///
/// "Save with honest label": network blips do not block save, but we never claim validated falsely.
pub(crate) fn nonactive_probe_verdict(outcome: &scratch::ProbeOutcome) -> Result<bool, String> {
    match outcome {
        scratch::ProbeOutcome::Ok => Ok(true),
        scratch::ProbeOutcome::Auth(code) => Err(i18n_err(
            "errUpstreamAuthConnNotSaved",
            json!({ "code": code }),
        )),
        scratch::ProbeOutcome::ModelError(code) => Err(i18n_err(
            "errUpstreamModelRejected",
            json!({ "code": code }),
        )),
        // Inconclusive (405/429/5xx/no response): persist as unvalidated; re-probe on activation.
        // Unsupported(405) grouped here: save uses Message probe; 405 is rare (bad endpoint/url).
        scratch::ProbeOutcome::Ambiguous(_)
        | scratch::ProbeOutcome::NoResponse
        | scratch::ProbeOutcome::Unsupported(_) => Ok(false),
    }
}

/// In-memory candidate for active connection edit (validate-before-persist). Unchanged fields are None.
/// Probe applies this onto a cloned profile; on success the same [`ConnectionEdit::apply`] persists
/// with active_id to avoid "disk new, runtime old" from persist-before-validate.
#[derive(Default)]
pub(crate) struct ConnectionEdit {
    base_url: Option<String>,
    api_format: Option<String>,
    model: Option<String>,
    active_models: Option<Vec<String>>,
    default_model: Option<String>,
    key: Option<String>,
}

impl ConnectionEdit {
    pub(crate) fn with_models(
        base_url: Option<String>,
        api_format: Option<String>,
        model: Option<String>,
        active_models: Option<Vec<String>>,
        default_model: Option<String>,
        key: Option<String>,
    ) -> Self {
        Self {
            base_url,
            api_format,
            model,
            active_models,
            default_model,
            key,
        }
    }

    /// Apply non-empty edit values to target profile (shared by memory candidate and persist).
    /// Same semantics as `update_profile_connection_inner`: None = unchanged; empty key = keep stored key.
    pub(crate) fn apply(&self, p: &mut config::Profile) {
        if let Some(u) = &self.base_url {
            p.base_url = u.clone();
        }
        if let Some(f) = &self.api_format {
            p.api_format = f.clone();
        }
        if let Some(m) = &self.model {
            p.model = m.clone();
        }
        if let Some(models) = &self.active_models {
            p.active_models = models.clone();
        }
        if let Some(d) = &self.default_model {
            p.default_model = d.clone();
        }
        if let Some(k) = &self.key {
            if !k.is_empty() {
                p.api_key = k.clone();
            }
        }
        p.sync_model_fields();
    }
}

/// Merge live probe results (id + capabilities) with builtin, dedupe by id, sort (tools → version → main-list ids).
pub(crate) fn merge_and_sort_models(
    live: Vec<(String, Option<bool>)>,
    builtin: &[&str],
) -> Vec<serde_json::Value> {
    let mut seen = std::collections::BTreeSet::new();
    let mut merged: Vec<(String, Option<bool>)> = Vec::new();
    for (id, st) in live {
        if seen.insert(id.clone()) {
            merged.push((id, st));
        }
    }
    for b in builtin {
        if seen.insert(b.to_string()) {
            merged.push((b.to_string(), None));
        }
    }
    fn cap_rank(st: &Option<bool>) -> u8 {
        match st {
            Some(true) => 0,
            None => 1,
            Some(false) => 2,
        }
    }
    merged.sort_by(|(id_a, st_a), (id_b, st_b)| {
        cap_rank(st_a)
            .cmp(&cap_rank(st_b))
            .then_with(|| model_sort::compare_models_desc(id_a, id_b))
            .then_with(|| {
                let main_a = if is_main_list_model(id_a) { 0u8 } else { 1 };
                let main_b = if is_main_list_model(id_b) { 0u8 } else { 1 };
                main_a.cmp(&main_b)
            })
    });
    merged
        .into_iter()
        .map(|(id, st)| json!({ "id": id, "supports_tools": st }))
        .collect()
}

/// Choose probe kind: native adapters use Message (static /v1/models cannot catch bad keys);
/// relay with empty model uses Models; relay with model uses Message for that model.
pub(crate) fn probe_kind_for(adapter: &str, model: &str) -> scratch::ProbeKind {
    if is_native_adapter(adapter) {
        return scratch::ProbeKind::Message; // native /v1/models is static; Message hits upstream for key auth.
    }
    probe_kind_for_model(model)
}

/// With model → validate that model (POST /v1/messages); empty → validate endpoint+auth (GET /v1/models).
pub(crate) fn probe_kind_for_model(model: &str) -> scratch::ProbeKind {
    if model.trim().is_empty() {
        scratch::ProbeKind::Models
    } else {
        scratch::ProbeKind::Message
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_get_config, build_list_templates, create_profile_inner, delete_profile_inner,
        is_main_list_model, merge_and_sort_models, nonactive_probe_verdict, probe_kind_for,
        probe_kind_for_model, profile_capabilities, template_capabilities,
        update_profile_connection_inner, update_profile_metadata_inner, ConnectionEdit,
    };
    use crate::config;

    /// 每个测试用独立临时 `.csswitch` 目录（进程 id + 线程 id + 随机后缀），互不干扰。
    fn tmpdir_profile() -> std::path::PathBuf {
        let base =
            std::env::temp_dir().join(format!("csswitch-profile-test-{}", std::process::id()));
        let d = base.join(format!(
            "{:?}-{}",
            std::thread::current().id(),
            config::new_id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join(".csswitch")
    }

    // ---------- P2-d: 非 active「如实标记后保存」裁决（明确拒绝才拦；200=已校验；含糊/无响应=落盘但未校验） ----------
    #[test]
    fn nonactive_probe_verdict_maps_outcomes() {
        use crate::scratch::ProbeOutcome;
        assert!(
            nonactive_probe_verdict(&ProbeOutcome::Auth(401))
                .unwrap_err()
                .contains("errUpstreamAuthConnNotSaved"),
            "401 明确鉴权失败 → 拦下不落盘"
        );
        assert!(
            nonactive_probe_verdict(&ProbeOutcome::ModelError(404))
                .unwrap_err()
                .contains("errUpstreamModelRejected"),
            "404 模型不被接受 → 拦下不落盘"
        );
        assert_eq!(
            nonactive_probe_verdict(&ProbeOutcome::Ok),
            Ok(true),
            "200 → 落盘且【已校验】"
        );
        assert_eq!(
            nonactive_probe_verdict(&ProbeOutcome::Ambiguous(Some(429))),
            Ok(false),
            "含糊(429) → best-effort 落盘但【未校验】"
        );
        assert_eq!(
            nonactive_probe_verdict(&ProbeOutcome::NoResponse),
            Ok(false),
            "无响应 → best-effort 落盘但【未校验】"
        );
    }

    // ---------- MP-2 fix [1]: 连接编辑 validate-before-persist 的字段应用逻辑（内存/落盘共用） ----------
    #[test]
    fn connection_edit_apply_only_changes_provided_fields() {
        use crate::config::Profile;
        let mut p = Profile {
            base_url: "old-url".into(),
            api_format: "anthropic".into(),
            model: "old-model".into(),
            api_key: "old-key".into(),
            ..Default::default()
        };
        let edit = ConnectionEdit::with_models(
            Some("new-url".into()),
            None,
            Some("new-model".into()),
            None,
            None,
            Some(String::new()),
        );
        edit.apply(&mut p);
        assert_eq!(p.base_url, "new-url");
        assert_eq!(p.api_format, "anthropic", "None 字段不改");
        assert_eq!(p.model, "new-model");
        assert_eq!(p.api_key, "old-key", "空 key 不覆盖已存 key");

        // 非空 key 覆盖；其余 None 不动。
        let edit2 =
            ConnectionEdit::with_models(None, None, None, None, None, Some("new-key".into()));
        edit2.apply(&mut p);
        assert_eq!(p.api_key, "new-key", "非空 key 覆盖");
        assert_eq!(p.base_url, "new-url", "None 字段不改");
        assert_eq!(p.model, "new-model", "None 字段不改");
    }

    // ---------- B4: profile CRUD *_inner ----------
    #[test]
    fn create_profile_from_template_prefills() {
        let d = tmpdir_profile();
        let id =
            create_profile_inner(&d, "glm", "我的 GLM", Some("gk"), None, Some("glm-5.2")).unwrap();
        let cfg = config::load_from(&d).unwrap();
        let p = cfg.profile_by_id(&id).unwrap();
        assert_eq!(p.template_id, "glm");
        assert_eq!(p.name, "我的 GLM");
        assert_eq!(p.api_format, "anthropic");
        assert_eq!(p.base_url, "https://open.bigmodel.cn/api/anthropic");
        assert_eq!(p.api_key, "gk");
        assert_eq!(cfg.active_id, "", "新建不自动生效");
    }

    #[test]
    fn create_relay_without_model_is_allowed() {
        let d = tmpdir_profile();
        let id = create_profile_inner(&d, "glm", "GLM", Some("gk"), None, None).unwrap();
        let cfg = config::load_from(&d).unwrap();
        let p = cfg.profile_by_id(&id).unwrap();
        assert!(p.effective_models().is_empty());
        assert!(create_profile_inner(&d, "deepseek", "DS", Some("gk"), None, None).is_ok());
    }

    #[test]
    fn update_metadata_does_not_touch_key() {
        let d = tmpdir_profile();
        let id =
            create_profile_inner(&d, "glm", "GLM", Some("secret9"), None, Some("glm-5.2")).unwrap();
        update_profile_metadata_inner(&d, &id, "改名", Some("备注")).unwrap();
        let cfg = config::load_from(&d).unwrap();
        let p = cfg.profile_by_id(&id).unwrap();
        assert_eq!(p.name, "改名");
        assert_eq!(p.notes.as_deref(), Some("备注"));
        assert_eq!(p.api_key, "secret9", "元数据编辑不动 key");
    }

    #[test]
    fn delete_active_clears_active() {
        let d = tmpdir_profile();
        let id = create_profile_inner(&d, "glm", "GLM", Some("k"), None, Some("glm-5.2")).unwrap();
        config::update(&d, |c| c.active_id = id.clone()).unwrap();
        delete_profile_inner(&d, &id).unwrap();
        let cfg = config::load_from(&d).unwrap();
        assert!(cfg.profile_by_id(&id).is_none());
        assert_eq!(cfg.active_id, "", "删 active → 置空");
    }

    #[test]
    fn update_connection_rejects_unsupported_format() {
        let d = tmpdir_profile();
        let id =
            create_profile_inner(&d, "custom", "C", None, Some("https://x/y"), Some("m")).unwrap();
        let e = update_profile_connection_inner(
            &d,
            &id,
            Some("https://x/y"),
            Some("gemini_native"),
            None,
            None,
            None,
            None,
        );
        assert!(e.is_err());
    }

    // ---------- MP-2 Minor [4]: 未命中 id → Err（不静默 Ok） ----------
    #[test]
    fn update_metadata_unknown_id_errors() {
        let d = tmpdir_profile();
        create_profile_inner(&d, "glm", "GLM", Some("k"), None, Some("glm-5.2")).unwrap();
        let e = update_profile_metadata_inner(&d, "no-such-id", "改名", None);
        assert!(e.is_err(), "未命中 id 应报错，而非静默成功");
        assert!(e.unwrap_err().contains("errProfileNotFound"));
    }

    #[test]
    fn update_connection_unknown_id_errors() {
        let d = tmpdir_profile();
        create_profile_inner(&d, "glm", "GLM", Some("k"), None, Some("glm-5.2")).unwrap();
        let e = update_profile_connection_inner(
            &d,
            "no-such-id",
            Some("https://x/y"),
            None,
            None,
            None,
            None,
            None,
        );
        assert!(e.is_err(), "未命中 id 应报错，而非静默成功");
        assert!(e.unwrap_err().contains("errProfileNotFound"));
    }

    // ---------- B5: build_get_config / build_list_templates ----------
    #[test]
    fn get_config_masks_keys_and_lists_profiles() {
        let d = tmpdir_profile();
        let id = create_profile_inner(
            &d,
            "glm",
            "GLM",
            Some("sk-longsecret9999"),
            None,
            Some("glm-5.2"),
        )
        .unwrap();
        let v = build_get_config(&d).unwrap();
        assert_eq!(v["schema_version"], 4);
        let arr = v["profiles"].as_array().unwrap();
        let p = arr.iter().find(|p| p["id"] == id).unwrap();
        assert!(p["key"].as_str().unwrap().ends_with("9999"));
        assert!(
            !p["key"].as_str().unwrap().contains("longsecret"),
            "只回掩码"
        );
        assert!(
            p.get("api_key").is_none() || p["api_key"].is_null(),
            "全 key 不出后端"
        );
        assert_eq!(p["has_key"], true);
        assert_eq!(p["key_masked"], p["key"], "保留旧 key 字段并补新掩码字段");
        assert_eq!(p["capabilities"]["model_required"], true);
        assert_eq!(
            p["capabilities"]["model_discovery"],
            "anthropic_models_or_manual"
        );
    }

    #[test]
    fn get_config_returns_notes_so_rename_does_not_wipe_them() {
        // M1 回归：build_get_config 必须回传 notes，否则前端读到空、下次改名把备注静默清掉。
        let d = tmpdir_profile();
        let id = create_profile_inner(&d, "glm", "GLM", Some("k"), None, Some("glm-5.2")).unwrap();
        update_profile_metadata_inner(&d, &id, "GLM", Some("我的备注")).unwrap();
        let v = build_get_config(&d).unwrap();
        let p = v["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap();
        assert_eq!(p["notes"], "我的备注", "notes 必须随 get_config 回传");
    }

    #[test]
    fn list_templates_has_eleven() {
        let v = build_list_templates();
        assert_eq!(v.len(), 11);
        assert!(v.iter().any(|t| t["id"] == "custom"));
        assert!(v.iter().any(|t| t["id"] == "custom-openai"));
        assert!(v.iter().any(|t| t["id"] == "custom-openai-responses"));
        assert!(v.iter().any(|t| t["id"] == "kimi"));
        assert!(v.iter().any(|t| t["id"] == "minimax"));
        let qwen = v.iter().find(|t| t["id"] == "qwen").unwrap();
        assert_eq!(qwen["capabilities"]["model_discovery"], "builtin_static");
        assert_eq!(qwen["capabilities"]["supports_tools_hint"], "translated");
        let custom = v.iter().find(|t| t["id"] == "custom-openai").unwrap();
        assert_eq!(
            custom["capabilities"]["model_discovery"],
            "openai_models_or_manual"
        );
        assert_eq!(custom["capabilities"]["base_url_required"], true);
    }

    #[test]
    fn capabilities_are_derived_from_template_contract() {
        let ds = template_capabilities(crate::templates::by_id("deepseek").unwrap());
        assert_eq!(ds["base_url_required"], false);
        assert_eq!(ds["model_required"], false);
        assert_eq!(ds["model_discovery"], "builtin_static");

        let relay = template_capabilities(crate::templates::by_id("glm").unwrap());
        assert_eq!(relay["base_url_required"], true);
        assert_eq!(relay["model_required"], true);
        assert_eq!(relay["model_discovery"], "anthropic_models_or_manual");
        assert_eq!(relay["thinking_policy"], "adaptive");

        let p = config::Profile {
            template_id: "custom-openai-responses".into(),
            ..Default::default()
        };
        assert_eq!(
            profile_capabilities(&p)["model_discovery"],
            "openai_models_or_manual"
        );
    }

    #[test]
    fn profile_capabilities_follow_profile_api_format_when_present() {
        let p = config::Profile {
            template_id: "custom".into(),
            api_format: "openai_responses".into(),
            ..Default::default()
        };
        let caps = profile_capabilities(&p);
        assert_eq!(caps["model_discovery"], "openai_models_or_manual");
        assert_eq!(caps["supports_tools_hint"], "translated");
    }

    #[test]
    fn main_list_model_matches_family_plus_digit() {
        assert!(is_main_list_model("claude-opus-4-8"));
        assert!(is_main_list_model("claude-sonnet-5"));
        assert!(is_main_list_model("claude-haiku-4-5-20251001"));
        assert!(!is_main_list_model("claude-3-5-sonnet-20241022"));
        assert!(!is_main_list_model("claude-fable-5"));
        assert!(!is_main_list_model("gpt-4o"));
    }

    #[test]
    fn merge_and_sort_prefers_tools_then_version_desc() {
        let live = vec![
            ("glm-4.5".to_string(), None),
            ("glm-5.2".to_string(), None),
            ("glm-4.7".to_string(), None),
        ];
        let out = merge_and_sort_models(live, &[]);
        let ids: Vec<String> = out
            .iter()
            .map(|v| v.get("id").unwrap().as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, vec!["glm-5.2", "glm-4.7", "glm-4.5"]);
    }

    #[test]
    fn merge_and_sort_prefers_tools_then_dedupes_builtin() {
        let live = vec![
            ("m-notools".to_string(), Some(false)),
            ("m-tools".to_string(), Some(true)),
            ("m-unknown".to_string(), None),
        ];
        let out = merge_and_sort_models(live, &["m-tools", "m-builtin-only"]);
        let ids: Vec<String> = out
            .iter()
            .map(|v| v.get("id").unwrap().as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids[0], "m-tools");
        assert!(ids.contains(&"m-builtin-only".to_string()));
        assert_eq!(ids.iter().filter(|i| *i == "m-tools").count(), 1, "去重");
        assert_eq!(ids.last().unwrap(), "m-notools");
    }

    #[test]
    fn probe_kind_picks_message_when_model_set() {
        assert!(matches!(
            probe_kind_for_model("mimo-v2.5-pro"),
            crate::scratch::ProbeKind::Message
        ));
        assert!(matches!(
            probe_kind_for_model(""),
            crate::scratch::ProbeKind::Models
        ));
    }

    // ---------- 修真机 P1：native adapter 上游校验（GPT 验收报告 RM-06） ----------

    #[test]
    fn native_probe_uses_message_since_native_models_is_static() {
        // native 的 /v1/models 是静态列表、探不出坏 key，故一律用 Message（打上游 /v1/messages）。
        assert!(matches!(
            probe_kind_for("deepseek", ""),
            crate::scratch::ProbeKind::Message
        ));
        assert!(matches!(
            probe_kind_for("qwen", ""),
            crate::scratch::ProbeKind::Message
        ));
        // relay：空 model 用 Models（/v1/models 回源即验鉴权）；选了 model 用 Message 验该模型。
        assert!(matches!(
            probe_kind_for("relay", ""),
            crate::scratch::ProbeKind::Models
        ));
        assert!(matches!(
            probe_kind_for("relay", "m1"),
            crate::scratch::ProbeKind::Message
        ));
    }
}
