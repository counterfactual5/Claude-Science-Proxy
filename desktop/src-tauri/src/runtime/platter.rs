//! Multi-provider model platter: registry JSON, per-profile credentials, proxy launch.

use std::collections::BTreeSet;

use serde_json::json;

use crate::config::{self, Config, PlatterEntry, MAX_PLATTER_MODELS};
use crate::runtime::i18n::i18n_err;
use crate::runtime::provider::{
    adapter_for_profile, is_native_adapter, proxy_fingerprint, ProxyLaunch,
};
use crate::templates;

const FAST_HINTS: &[&str] = &["haiku", "flash", "mini", "lite", "air", "fast", "turbo"];

/// Any saved provider with a key can join the platter (relay / DeepSeek / OpenAI-custom).
pub(crate) fn platter_profile_supported(p: &config::Profile) -> bool {
    !p.api_key.trim().is_empty()
}

fn platter_needs_base_url(p: &config::Profile) -> bool {
    !is_native_adapter(adapter_for_profile(p))
}

pub(crate) fn validate_platter_entries(cfg: &Config, entries: &[PlatterEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Err(i18n_err("errPlatterEmpty", json!({})));
    }
    if entries.len() > MAX_PLATTER_MODELS {
        return Err(i18n_err("errPlatterTooMany", json!({ "max": MAX_PLATTER_MODELS })));
    }
    for e in entries {
        let model = e.model.trim();
        if model.is_empty() {
            return Err(i18n_err("errPlatterMissingModel", json!({})));
        }
        let p = cfg
            .profile_by_id(&e.profile_id)
            .ok_or_else(|| i18n_err("errProfileNotFound", json!({ "id": e.profile_id })))?;
        if p.api_key.trim().is_empty() {
            return Err(i18n_err("errMissingApiKey", json!({ "name": p.name })));
        }
        if platter_needs_base_url(p) && p.base_url.trim().is_empty() {
            return Err(i18n_err("errMissingBaseUrl", json!({})));
        }
        if !platter_profile_supported(p) {
            return Err(i18n_err(
                "errPlatterAdapterUnsupported",
                json!({ "name": p.name }),
            ));
        }
    }
    Ok(())
}

pub(crate) fn infer_fast_model(entries: &[PlatterEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    if entries.len() == 1 {
        return entries[0].model.clone();
    }
    for e in entries {
        let lower = e.model.to_lowercase();
        if FAST_HINTS.iter().any(|h| lower.contains(h)) {
            return e.model.clone();
        }
    }
    entries
        .get(1)
        .map(|e| e.model.clone())
        .unwrap_or_else(|| entries[0].model.clone())
}

pub(crate) fn build_platter_registry_json(cfg: &Config) -> Result<String, String> {
    let entries = &cfg.model_platter.entries;
    validate_platter_entries(cfg, entries)?;
    let default_model = entries[0].model.clone();
    let fast_model = infer_fast_model(entries);
    let platter_entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let name = cfg
                .profile_by_id(&e.profile_id)
                .map(|p| p.name.as_str())
                .unwrap_or("");
            json!({
                "profile_id": e.profile_id,
                "model": e.model,
                "display_prefix": name,
            })
        })
        .collect();
    Ok(json!({
        "platter": true,
        "entries": platter_entries,
        "default_model": default_model,
        "fast_model": fast_model,
    })
    .to_string())
}

pub(crate) fn build_profile_credentials_json(cfg: &Config) -> Result<String, String> {
    let entries = &cfg.model_platter.entries;
    validate_platter_entries(cfg, entries)?;
    let mut seen = BTreeSet::new();
    let mut creds = serde_json::Map::new();
    for e in entries {
        if !seen.insert(e.profile_id.clone()) {
            continue;
        }
        let p = cfg.profile_by_id(&e.profile_id).unwrap();
        creds.insert(
            e.profile_id.clone(),
            json!({
                "adapter": adapter_for_profile(p),
                "key": p.api_key,
                "base_url": p.base_url,
                "thinking_policy": templates::thinking_policy_for(&p.template_id),
            }),
        );
    }
    Ok(serde_json::Value::Object(creds).to_string())
}

fn platter_launch_base_url(p: &config::Profile) -> String {
    let base = p.base_url.trim();
    if !base.is_empty() {
        return base.to_string();
    }
    // Native DeepSeek has a fixed upstream; process still starts as relay host.
    if is_native_adapter(adapter_for_profile(p)) {
        return "https://api.deepseek.com/anthropic".to_string();
    }
    String::new()
}

pub(crate) fn proxy_args_for_platter(cfg: &Config) -> Result<ProxyLaunch, String> {
    let registry = build_platter_registry_json(cfg)?;
    let credentials = build_profile_credentials_json(cfg)?;
    let first = cfg
        .profile_by_id(&cfg.model_platter.entries[0].profile_id)
        .ok_or_else(|| i18n_err("errPlatterEmpty", json!({})))?;
    // Host process is always relay + registry; per-request routing picks adapter/key.
    Ok(ProxyLaunch {
        adapter: "relay".to_string(),
        base_url: platter_launch_base_url(first),
        model: cfg.model_platter.entries[0].model.clone(),
        key: first.api_key.clone(),
        key_env: "CSP_RELAY_KEY",
        thinking_policy: "",
        model_registry_json: Some(registry),
        profile_credentials_json: Some(credentials),
    })
}

pub(crate) fn platter_fingerprint(cfg: &Config, launch: &ProxyLaunch) -> u64 {
    let p = cfg
        .profile_by_id(&cfg.model_platter.entries[0].profile_id)
        .cloned()
        .unwrap_or_default();
    proxy_fingerprint(&p, launch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    fn relay_profile(id: &str, name: &str, model: &str) -> Profile {
        Profile {
            id: id.into(),
            name: name.into(),
            template_id: "glm".into(),
            api_format: "anthropic".into(),
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_key: "gk-test".into(),
            model: model.into(),
            active_models: vec![model.into()],
            default_model: model.into(),
            ..Default::default()
        }
    }

    #[test]
    fn infer_fast_model_prefers_hint() {
        let entries = vec![
            PlatterEntry {
                profile_id: "a".into(),
                model: "glm-5.2".into(),
            },
            PlatterEntry {
                profile_id: "b".into(),
                model: "glm-4.5-air".into(),
            },
        ];
        assert_eq!(infer_fast_model(&entries), "glm-4.5-air");
    }

    #[test]
    fn build_platter_registry_preserves_order() {
        let mut cfg = Config::default();
        cfg.profiles = vec![
            relay_profile("p1", "GLM", "glm-5.2"),
            relay_profile("p2", "Kimi", "kimi-k2"),
        ];
        cfg.model_platter.entries = vec![
            PlatterEntry {
                profile_id: "p1".into(),
                model: "glm-5.2".into(),
            },
            PlatterEntry {
                profile_id: "p2".into(),
                model: "kimi-k2".into(),
            },
        ];
        let raw = build_platter_registry_json(&cfg).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["platter"], true);
        assert_eq!(v["default_model"], "glm-5.2");
        assert_eq!(v["entries"][0]["model"], "glm-5.2");
        assert_eq!(v["entries"][1]["model"], "kimi-k2");
    }

    #[test]
    fn deepseek_and_openai_profiles_accepted_for_platter() {
        let mut cfg = Config::default();
        cfg.profiles = vec![
            Profile {
                id: "ds".into(),
                template_id: "deepseek".into(),
                api_format: "anthropic".into(),
                base_url: "https://api.deepseek.com/anthropic".into(),
                api_key: "sk-ds".into(),
                model: "deepseek-v4-pro".into(),
                ..Default::default()
            },
            Profile {
                id: "oa".into(),
                template_id: "custom-openai".into(),
                api_format: "openai_chat".into(),
                base_url: "https://api.example.com/v1".into(),
                api_key: "sk-oa".into(),
                model: "gpt-5.2".into(),
                ..Default::default()
            },
        ];
        let entries = vec![
            PlatterEntry {
                profile_id: "ds".into(),
                model: "deepseek-v4-pro".into(),
            },
            PlatterEntry {
                profile_id: "oa".into(),
                model: "gpt-5.2".into(),
            },
        ];
        assert!(validate_platter_entries(&cfg, &entries).is_ok());
    }
}
