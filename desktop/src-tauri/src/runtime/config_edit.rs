//! CSP.json 导出/导入：供高级用户批量编辑 profile 与模型列表（不含明文 key 导出）。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::runtime::provider::{
    adapter_for_profile, assert_format_supported, reject_openai_custom_anthropic_base,
    relay_missing_base_url, relay_missing_profile_models,
};
use crate::{config, templates};

const CSP_EDIT_SCHEMA: &str = "csp-edit-v1";

#[derive(Debug, Serialize, Deserialize)]
struct CspEditDoc {
    #[serde(default)]
    schema: String,
    profiles: Vec<CspEditProfile>,
    #[serde(default)]
    active_ids: Vec<String>,
    #[serde(default)]
    proxy_port: Option<u16>,
    #[serde(default)]
    sandbox_port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CspEditProfile {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default = "default_template")]
    template_id: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    active_models: Vec<String>,
    #[serde(default)]
    default_model: String,
    /// 仅导入时：非空则更新 key；导出时不包含此字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

fn default_template() -> String {
    "custom".to_string()
}

fn normalize_models(models: &[String], default_model: &str) -> (Vec<String>, String) {
    let mut out: Vec<String> = models
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    out.sort();
    out.dedup();
    let default = if !default_model.trim().is_empty() {
        default_model.trim().to_string()
    } else {
        out.first().cloned().unwrap_or_default()
    };
    if !default.is_empty() && !out.contains(&default) {
        out.insert(0, default.clone());
    }
    (out, default)
}

fn export_profile(p: &config::Profile) -> CspEditProfile {
    let models = p.effective_models();
    CspEditProfile {
        id: p.id.clone(),
        name: p.name.clone(),
        template_id: p.template_id.clone(),
        base_url: p.base_url.clone(),
        active_models: models.clone(),
        default_model: p.effective_default_model(),
        api_key: None,
        notes: p.notes.clone(),
    }
}

fn apply_edit_to_profile(existing: &config::Profile, edit: &CspEditProfile) -> Result<config::Profile, String> {
    let tpl = templates::by_id(&edit.template_id)
        .ok_or_else(|| format!("未知 template_id：{}", edit.template_id))?;
    let (active_models, default_model) =
        normalize_models(&edit.active_models, &edit.default_model);
    let mut p = existing.clone();
    p.name = edit.name.trim().to_string();
    if p.name.is_empty() {
        return Err("profile 名称不能为空。".into());
    }
    p.template_id = edit.template_id.clone();
    p.category = tpl.category.to_string();
    p.api_format = tpl.api_format.to_string();
    p.base_url = edit.base_url.trim().to_string();
    if p.base_url.is_empty() && tpl.base_url_editable {
        p.base_url = tpl.base_url.to_string();
    }
    if !tpl.base_url_editable && !tpl.base_url.is_empty() {
        p.base_url = tpl.base_url.to_string();
    }
    p.active_models = active_models;
    p.default_model = default_model.clone();
    p.model = default_model;
    p.notes = edit.notes.clone();
    if let Some(key) = edit.api_key.as_ref() {
        let k = key.trim();
        if !k.is_empty() {
            p.api_key = k.to_string();
        }
    }
    p.website_url = Some(tpl.website_url.to_string());
    p.icon = Some(tpl.icon.to_string());
    p.icon_color = Some(tpl.icon_color.to_string());
    p.sync_model_fields();
    assert_format_supported(&p)?;
    let adapter = adapter_for_profile(&p);
    reject_openai_custom_anthropic_base(adapter, &p.base_url)?;
    if relay_missing_base_url(adapter, &p.base_url) {
        return Err(format!("「{}」须填写 base_url。", p.name));
    }
    if relay_missing_profile_models(adapter, &p) {
        return Err(format!("「{}」须至少配置一个模型（active_models）。", p.name));
    }
    Ok(p)
}

fn new_profile_from_edit(edit: &CspEditProfile) -> Result<config::Profile, String> {
    let id = if edit.id.trim().is_empty() {
        config::new_id()
    } else {
        edit.id.trim().to_string()
    };
    let stub = config::Profile {
        id,
        api_key: edit.api_key.clone().unwrap_or_default(),
        ..Default::default()
    };
    apply_edit_to_profile(&stub, edit)
}

/// 导出可编辑 JSON（不含 api_key）。
pub(crate) fn export_csp_edit_json(dir: &Path) -> Result<String, String> {
    let cfg = config::load_from(dir).map_err(|e| e.to_string())?;
    let doc = CspEditDoc {
        schema: CSP_EDIT_SCHEMA.to_string(),
        profiles: cfg.profiles.iter().map(export_profile).collect(),
        active_ids: cfg.active_ids.clone(),
        proxy_port: Some(cfg.proxy_port),
        sandbox_port: Some(cfg.sandbox_port),
    };
    serde_json::to_string_pretty(&doc).map_err(|e| format!("序列化 CSP.json 失败：{e}"))
}

/// 自 CSP.json 合并写回 config（保留未出现在 JSON 中的 profile；JSON 中无 id 的项新建）。
pub(crate) fn import_csp_edit_json(dir: &Path, raw: &str) -> Result<(), String> {
    let doc: CspEditDoc =
        serde_json::from_str(raw).map_err(|e| format!("JSON 解析失败：{e}"))?;
    if !doc.schema.is_empty() && doc.schema != CSP_EDIT_SCHEMA {
        return Err(format!("不支持的 schema：{}（期望 {CSP_EDIT_SCHEMA}）", doc.schema));
    }
    if doc.profiles.is_empty() {
        return Err("profiles 不能为空。".into());
    }
    let mut next: Vec<config::Profile> = Vec::new();
    for edit in &doc.profiles {
        let p = if edit.id.trim().is_empty() {
            new_profile_from_edit(edit)?
        } else {
            let cfg = config::load_from(dir).map_err(|e| e.to_string())?;
            let existing = cfg
                .profile_by_id(edit.id.trim())
                .cloned()
                .unwrap_or_else(|| config::Profile {
                    id: edit.id.trim().to_string(),
                    ..Default::default()
                });
            apply_edit_to_profile(&existing, edit)?
        };
        if next.iter().any(|x| x.id == p.id) {
            return Err(format!("重复的 profile id：{}", p.id));
        }
        next.push(p);
    }
    let id_set: std::collections::HashSet<_> = next.iter().map(|p| p.id.as_str()).collect();
    let active_ids: Vec<String> = if doc.active_ids.is_empty() {
        Vec::new()
    } else {
        doc.active_ids
            .into_iter()
            .filter(|id| id_set.contains(id.as_str()))
            .collect()
    };
    let proxy_port = doc.proxy_port.unwrap_or_else(config::default_proxy_port);
    let sandbox_port = doc.sandbox_port.unwrap_or_else(config::default_sandbox_port);
    config::validate_runtime_ports(proxy_port, sandbox_port).map_err(|e| e.to_string())?;
    config::update(dir, |c| {
        c.profiles = next;
        c.active_ids = active_ids;
        c.sync_active_id();
        c.proxy_port = proxy_port;
        c.sandbox_port = sandbox_port;
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    fn tmpdir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("csp-edit-{}", config::new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample_profile() -> Profile {
        Profile {
            id: "p1".into(),
            name: "GLM".into(),
            template_id: "custom-openai".into(),
            category: "custom".into(),
            api_format: "openai_chat".into(),
            base_url: "https://open.bigmodel.cn/api/coding/paas/v4".into(),
            api_key: "secret-key".into(),
            active_models: vec!["glm-5".into(), "glm-4".into()],
            default_model: "glm-5".into(),
            model: "glm-5".into(),
            ..Default::default()
        }
    }

    #[test]
    fn export_omits_api_key() {
        let d = tmpdir();
        let mut cfg = config::Config::default();
        cfg.profiles.push(sample_profile());
        config::save_to(&d, &cfg).unwrap();
        let raw = export_csp_edit_json(&d).unwrap();
        assert!(!raw.contains("secret-key"));
        assert!(raw.contains("glm-5"));
    }

    #[test]
    fn import_updates_models_and_preserves_key() {
        let d = tmpdir();
        let mut cfg = config::Config::default();
        cfg.profiles.push(sample_profile());
        cfg.active_ids = vec!["p1".into()];
        config::save_to(&d, &cfg).unwrap();
        let edited = r#"{
          "schema": "csp-edit-v1",
          "profiles": [{
            "id": "p1",
            "name": "GLM Coding",
            "template_id": "custom-openai",
            "base_url": "https://open.bigmodel.cn/api/coding/paas/v4",
            "active_models": ["glm-5.2", "glm-4.7"],
            "default_model": "glm-5.2"
          }],
          "active_ids": ["p1"]
        }"#;
        import_csp_edit_json(&d, edited).unwrap();
        let got = config::load_from(&d).unwrap();
        let p = got.profile_by_id("p1").unwrap();
        assert_eq!(p.api_key, "secret-key");
        assert_eq!(p.active_models, vec!["glm-4.7", "glm-5.2"]);
        assert_eq!(p.default_model, "glm-5.2");
    }
}
