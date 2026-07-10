use crate::runtime::i18n::i18n_err;
use crate::runtime::model_sort;
use crate::{config, templates};
use serde_json::json;

/// Non-cryptographic key fingerprint (SipHash) for detecting config changes. Never logged or persisted.
pub(crate) fn key_fingerprint(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Adapter → expected key env var name (python proxy `PROVIDERS[...]["key_env"]`).
pub(crate) fn key_env_for_adapter(adapter: &str) -> &'static str {
    match adapter {
        "deepseek" => "DEEPSEEK_API_KEY",
        "qwen" => "DASHSCOPE_API_KEY",
        "openai-custom" | "openai-responses" => "CSP_OPENAI_KEY",
        _ => "CSP_RELAY_KEY", // relay / fallback
    }
}

/// Derive all proxy launch parameters from one profile (pure fn, testable).
pub(crate) struct ProxyLaunch {
    pub(crate) adapter: String,
    pub(crate) base_url: String,
    pub(crate) model: String,
    pub(crate) key: String,
    pub(crate) key_env: &'static str,
    pub(crate) thinking_policy: &'static str,
    pub(crate) model_registry_json: Option<String>,
}

pub(crate) fn adapter_for_profile(p: &config::Profile) -> &'static str {
    if p.template_id == "custom" {
        match p.api_format.as_str() {
            "openai_chat" => "openai-custom",
            "openai_responses" => "openai-responses",
            _ => templates::adapter_for(&p.template_id),
        }
    } else {
        templates::adapter_for(&p.template_id)
    }
}

pub(crate) fn proxy_args_for(p: &config::Profile) -> ProxyLaunch {
    let adapter = adapter_for_profile(p).to_string();
    let key_env = key_env_for_adapter(&adapter);
    let registry = build_model_registry_json(p);
    let model = p.effective_default_model();
    ProxyLaunch {
        adapter,
        base_url: p.base_url.clone(),
        model,
        key: p.api_key.clone(),
        key_env,
        thinking_policy: templates::thinking_policy_for(&p.template_id),
        model_registry_json: registry,
    }
}

/// Proxy launch args for the currently active profile (single active only).
pub(crate) fn proxy_args_for_active_profiles(
    profiles: &[config::Profile],
) -> Result<ProxyLaunch, String> {
    profiles
        .first()
        .map(proxy_args_for)
        .ok_or_else(|| i18n_err("noActiveProfile", json!({})))
}

pub(crate) fn proxy_fingerprint(p: &config::Profile, launch: &ProxyLaunch) -> u64 {
    proxy_fingerprint_with_runtime(
        p,
        launch,
        gateway_kind_for_adapter(&launch.adapter),
        current_shim_mode_for_adapter(&launch.adapter),
    )
}

pub(crate) fn proxy_fingerprint_with_runtime(
    p: &config::Profile,
    launch: &ProxyLaunch,
    gateway_kind: &str,
    shim_mode: &str,
) -> u64 {
    key_fingerprint(&format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        p.template_id,
        p.api_format,
        launch.adapter,
        launch.base_url,
        launch.model,
        launch.thinking_policy,
        launch.key,
        gateway_kind,
        shim_mode,
        launch.model_registry_json.as_deref().unwrap_or("")
    ))
}

/// Supported formats: anthropic / openai_chat / openai_responses; others may exist in schema but activation rejects them (track 2: Rust proxy).
pub(crate) fn assert_format_supported(p: &config::Profile) -> Result<(), String> {
    match p.api_format.as_str() {
        "anthropic" | "openai_chat" | "openai_responses" => Ok(()),
        other => Err(i18n_err(
            "errApiFormatUnsupported",
            json!({ "format": other }),
        )),
    }
}

fn looks_like_anthropic_endpoint(base_url: &str) -> bool {
    let u = base_url.trim().trim_end_matches('/').to_ascii_lowercase();
    u.contains("/anthropic")
}

pub(crate) fn reject_openai_custom_anthropic_base(
    adapter: &str,
    base_url: &str,
) -> Result<(), String> {
    if is_openai_adapter(adapter) && looks_like_anthropic_endpoint(base_url) {
        Err(i18n_err("errAnthropicBaseUrlHint", json!({})))
    } else {
        Ok(())
    }
}

/// deepseek/qwen use fixed official endpoints (hardcoded in python); all others are relay and need base_url.
pub(crate) fn is_native_adapter(adapter: &str) -> bool {
    adapter == "deepseek" || adapter == "qwen"
}

pub(crate) fn is_openai_adapter(adapter: &str) -> bool {
    matches!(adapter, "openai-custom" | "openai-responses")
}

pub(crate) fn gateway_kind_for_adapter(_adapter: &str) -> &'static str {
    "python"
}

pub(crate) fn normalize_shim_mode(adapter: &str, raw: Option<&str>) -> &'static str {
    if adapter != "deepseek" {
        return "off";
    }
    match raw.unwrap_or("").trim() {
        "detect" => "detect",
        "rewrite" => "rewrite",
        _ => "off",
    }
}

pub(crate) fn current_shim_mode_for_adapter(adapter: &str) -> &'static str {
    normalize_shim_mode(adapter, std::env::var("CSP_TOOLUSE_SHIM").ok().as_deref())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UpstreamEndpoint {
    pub(crate) host: String,
    pub(crate) port: u16,
}

/// Upstream authority (host + port) for status lights to probe the real scheme/port.
pub(crate) fn upstream_endpoint(adapter: &str, base_url: &str) -> Option<UpstreamEndpoint> {
    match adapter {
        "deepseek" => Some(UpstreamEndpoint {
            host: "api.deepseek.com".to_string(),
            port: 443,
        }),
        "qwen" => Some(UpstreamEndpoint {
            host: "dashscope.aliyuncs.com".to_string(),
            port: 443,
        }),
        _ => parse_endpoint(base_url),
    }
}

/// Extract host + port from `http(s)://host[:port]/path`. Returns None if unparseable (no url crate).
pub(crate) fn parse_endpoint(url: &str) -> Option<UpstreamEndpoint> {
    let (rest, default_port) = url
        .strip_prefix("https://")
        .map(|r| (r, 443))
        .or_else(|| url.strip_prefix("http://").map(|r| (r, 80)))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let (host, port) = if let Some(after_open) = authority.strip_prefix('[') {
        let (host, rest) = after_open.split_once(']')?;
        let port = match rest.strip_prefix(':') {
            Some(raw) if !raw.is_empty() => raw.parse().ok()?,
            Some(_) => return None,
            None => default_port,
        };
        (host.to_string(), port)
    } else {
        let (host, port) = match authority.split_once(':') {
            Some((host, raw)) if !raw.is_empty() => (host, raw.parse().ok()?),
            Some(_) => return None,
            None => (authority, default_port),
        };
        (host.to_string(), port)
    };
    if host.is_empty() {
        None
    } else {
        Some(UpstreamEndpoint { host, port })
    }
}

/// Whether to run upstream scratch validation for a candidate connection.
pub(crate) fn should_scratch_candidate(adapter: &str, key: &str, base_url: &str) -> bool {
    if key.is_empty() {
        return false; // no key → nothing to validate; mark unvalidated honestly
    }
    if !is_native_adapter(adapter) && base_url.is_empty() {
        return false; // relay family without base_url → nothing to validate
    }
    true
}

/// Pre-save guard: non-native family with empty base_url is invalid.
pub(crate) fn relay_missing_base_url(adapter: &str, base_url: &str) -> bool {
    !is_native_adapter(adapter) && base_url.trim().is_empty()
}

/// Pre-save/activation guard: non-native family with no usable model list.
pub(crate) fn relay_missing_profile_models(adapter: &str, profile: &config::Profile) -> bool {
    !is_native_adapter(adapter) && profile.effective_models().is_empty()
}

/// Build virtual model registry JSON for the formal proxy (relay / openai family).
/// Display-name sanitization for Science's V2_ filter happens in Python
/// (`proxy/model_registry.py::science_safe_display_name`), not here.
pub(crate) fn build_model_registry_json(p: &config::Profile) -> Option<String> {
    let adapter = adapter_for_profile(p);
    if is_native_adapter(adapter) {
        return None;
    }
    let mut models = p.effective_models();
    if models.is_empty() {
        return None;
    }
    model_sort::sort_model_ids(&mut models);
    let default_model = models[0].clone();
    let fast_model = models
        .last()
        .cloned()
        .unwrap_or_else(|| default_model.clone());
    let payload = serde_json::json!({
        "models": models,
        "default_model": default_model,
        "fast_model": fast_model,
        "profile_id": p.id,
    });
    Some(payload.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        adapter_for_profile, assert_format_supported, build_model_registry_json,
        gateway_kind_for_adapter, key_env_for_adapter, key_fingerprint, normalize_shim_mode,
        parse_endpoint, proxy_args_for, proxy_args_for_active_profiles, proxy_fingerprint,
        proxy_fingerprint_with_runtime, reject_openai_custom_anthropic_base,
        relay_missing_base_url, relay_missing_profile_models, should_scratch_candidate,
        upstream_endpoint,
    };
    use crate::config::Profile;

    #[test]
    fn proxy_args_for_active_profiles_uses_first_profile() {
        let p1 = Profile {
            template_id: "glm".into(),
            api_format: "anthropic".into(),
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_key: "gk".into(),
            model: "glm-5".into(),
            ..Default::default()
        };
        let p2 = Profile {
            id: "b".into(),
            template_id: "kimi".into(),
            ..p1.clone()
        };
        let single = proxy_args_for_active_profiles(std::slice::from_ref(&p1)).unwrap();
        assert_eq!(single.adapter, "relay");
        let first = proxy_args_for_active_profiles(&[p1, p2]).unwrap();
        assert_eq!(first.adapter, "relay");
        assert_eq!(first.model, "glm-5");
    }

    #[test]
    fn proxy_args_derive_adapter_and_key_env() {
        let ds = Profile {
            template_id: "deepseek".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_key: "sk-ds".into(),
            ..Default::default()
        };
        let a = proxy_args_for(&ds);
        assert_eq!(a.adapter, "deepseek");
        assert_eq!(a.key_env, "DEEPSEEK_API_KEY");

        let glm = Profile {
            template_id: "glm".into(),
            api_format: "anthropic".into(),
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_key: "gk".into(),
            model: "glm-5".into(),
            ..Default::default()
        };
        let b = proxy_args_for(&glm);
        assert_eq!(b.adapter, "relay");
        assert_eq!(b.key_env, "CSP_RELAY_KEY");
        assert_eq!(b.base_url, "https://open.bigmodel.cn/api/anthropic");
        assert_eq!(b.model, "glm-5");

        let custom_openai = Profile {
            template_id: "custom-openai".into(),
            api_format: "openai_chat".into(),
            base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
            api_key: "ok".into(),
            model: "glm-4.5".into(),
            ..Default::default()
        };
        let c = proxy_args_for(&custom_openai);
        assert_eq!(c.adapter, "openai-custom");
        assert_eq!(c.key_env, "CSP_OPENAI_KEY");
        assert_eq!(c.base_url, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(c.model, "glm-4.5");

        let custom_responses = Profile {
            template_id: "custom-openai-responses".into(),
            api_format: "openai_responses".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: "ok".into(),
            model: "gpt-5.2".into(),
            ..Default::default()
        };
        let d = proxy_args_for(&custom_responses);
        assert_eq!(d.adapter, "openai-responses");
        assert_eq!(d.key_env, "CSP_OPENAI_KEY");
        assert_eq!(d.base_url, "https://api.openai.com/v1");
        assert_eq!(d.model, "gpt-5.2");

        let custom_profile_openai = Profile {
            template_id: "custom".into(),
            api_format: "openai_chat".into(),
            base_url: "https://api.example.com/v1".into(),
            api_key: "ok".into(),
            model: "gpt-5.2".into(),
            ..Default::default()
        };
        let e = proxy_args_for(&custom_profile_openai);
        assert_eq!(adapter_for_profile(&custom_profile_openai), "openai-custom");
        assert_eq!(e.adapter, "openai-custom");
        assert_eq!(e.key_env, "CSP_OPENAI_KEY");

        let custom_profile_responses = Profile {
            api_format: "openai_responses".into(),
            ..custom_profile_openai
        };
        let f = proxy_args_for(&custom_profile_responses);
        assert_eq!(
            adapter_for_profile(&custom_profile_responses),
            "openai-responses"
        );
        assert_eq!(f.adapter, "openai-responses");
        assert_eq!(f.key_env, "CSP_OPENAI_KEY");

        let non_custom_openai_format = Profile {
            template_id: "glm".into(),
            api_format: "openai_chat".into(),
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_key: "ok".into(),
            model: "glm-5".into(),
            ..Default::default()
        };
        assert_eq!(adapter_for_profile(&non_custom_openai_format), "relay");
    }

    #[test]
    fn unsupported_api_format_is_rejected() {
        let p = Profile {
            template_id: "custom".into(),
            api_format: "gemini_native".into(),
            base_url: "https://x/y".into(),
            api_key: "k".into(),
            ..Default::default()
        };
        assert!(assert_format_supported(&p).is_err());
        let ok = Profile {
            api_format: "anthropic".into(),
            ..p.clone()
        };
        assert!(assert_format_supported(&ok).is_ok());
        let ok2 = Profile {
            api_format: "openai_chat".into(),
            ..p.clone()
        };
        assert!(assert_format_supported(&ok2).is_ok());
        let ok3 = Profile {
            api_format: "openai_responses".into(),
            ..ok2
        };
        assert!(assert_format_supported(&ok3).is_ok());
    }

    #[test]
    fn custom_openai_rejects_anthropic_base_url() {
        let err = reject_openai_custom_anthropic_base(
            "openai-custom",
            "https://api.moonshot.cn/anthropic",
        )
        .unwrap_err();
        assert!(err.contains("errAnthropicBaseUrlHint"));
        assert!(
            reject_openai_custom_anthropic_base("openai-custom", "https://api.moonshot.cn/v1",)
                .is_ok()
        );
        assert!(reject_openai_custom_anthropic_base(
            "openai-responses",
            "https://api.moonshot.cn/anthropic",
        )
        .is_err());
        assert!(
            reject_openai_custom_anthropic_base("relay", "https://api.moonshot.cn/anthropic",)
                .is_ok()
        );
    }

    #[test]
    fn key_env_for_adapter_maps_adapters() {
        assert_eq!(key_env_for_adapter("deepseek"), "DEEPSEEK_API_KEY");
        assert_eq!(key_env_for_adapter("qwen"), "DASHSCOPE_API_KEY");
        assert_eq!(key_env_for_adapter("openai-custom"), "CSP_OPENAI_KEY");
        assert_eq!(key_env_for_adapter("openai-responses"), "CSP_OPENAI_KEY");
        assert_eq!(key_env_for_adapter("relay"), "CSP_RELAY_KEY");
        assert_eq!(key_env_for_adapter("anything-else"), "CSP_RELAY_KEY");
    }

    #[test]
    fn proxy_fingerprint_includes_protocol_semantics() {
        let mut p = Profile {
            template_id: "kimi".into(),
            api_format: "anthropic".into(),
            base_url: "https://same.example/anthropic".into(),
            api_key: "same-key".into(),
            model: "same-model".into(),
            ..Default::default()
        };
        let kimi_launch = proxy_args_for(&p);
        let kimi_fp = proxy_fingerprint(&p, &kimi_launch);

        p.template_id = "custom".into();
        let custom_launch = proxy_args_for(&p);
        let custom_fp = proxy_fingerprint(&p, &custom_launch);
        assert_ne!(
            kimi_fp, custom_fp,
            "same adapter/base/model/key but different template semantics must force proxy restart"
        );
    }

    #[test]
    fn proxy_fingerprint_includes_gateway_and_shim_identity() {
        let p = Profile {
            template_id: "deepseek".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.deepseek.com/anthropic".into(),
            api_key: "same-key".into(),
            model: "same-model".into(),
            ..Default::default()
        };
        let launch = proxy_args_for(&p);
        let python_off = proxy_fingerprint_with_runtime(&p, &launch, "python", "off");
        let rust_off = proxy_fingerprint_with_runtime(&p, &launch, "rust", "off");
        let python_detect = proxy_fingerprint_with_runtime(&p, &launch, "python", "detect");
        assert_ne!(
            python_off, rust_off,
            "gateway change must prevent mistaken reuse"
        );
        assert_ne!(
            python_off, python_detect,
            "shim change must prevent mistaken reuse"
        );
    }

    #[test]
    fn parse_endpoint_preserves_scheme_default_and_explicit_ports() {
        assert_eq!(
            parse_endpoint("https://relay.example.com/api"),
            Some(super::UpstreamEndpoint {
                host: "relay.example.com".to_string(),
                port: 443,
            })
        );
        assert_eq!(
            parse_endpoint("http://127.0.0.1:11434/v1"),
            Some(super::UpstreamEndpoint {
                host: "127.0.0.1".to_string(),
                port: 11434,
            })
        );
        assert_eq!(
            parse_endpoint("http://localhost/v1"),
            Some(super::UpstreamEndpoint {
                host: "localhost".to_string(),
                port: 80,
            })
        );
        assert_eq!(parse_endpoint("https://relay.example.com:"), None);
    }

    #[test]
    fn upstream_endpoint_by_adapter() {
        assert_eq!(
            upstream_endpoint("openai-custom", "http://127.0.0.1:11434/v1"),
            Some(super::UpstreamEndpoint {
                host: "127.0.0.1".to_string(),
                port: 11434,
            })
        );
        assert_eq!(upstream_endpoint("", ""), None);
    }

    #[test]
    fn runtime_identity_contract_defaults_to_python_and_deepseek_only_shim() {
        assert_eq!(gateway_kind_for_adapter("deepseek"), "python");
        assert_eq!(gateway_kind_for_adapter("openai-custom"), "python");
        assert_eq!(normalize_shim_mode("deepseek", Some("detect")), "detect");
        assert_eq!(normalize_shim_mode("deepseek", Some("rewrite")), "rewrite");
        assert_eq!(normalize_shim_mode("deepseek", Some("bad")), "off");
        assert_eq!(normalize_shim_mode("relay", Some("rewrite")), "off");
        assert_eq!(normalize_shim_mode("qwen", Some("detect")), "off");
    }

    #[test]
    fn key_fingerprint_stable_and_distinct() {
        assert_eq!(key_fingerprint("sk-aaaa"), key_fingerprint("sk-aaaa"));
        assert_ne!(key_fingerprint("sk-aaaa"), key_fingerprint("sk-bbbb"));
        assert_ne!(key_fingerprint(""), key_fingerprint("x"));
    }

    #[test]
    fn native_candidate_is_upstream_validated_even_without_base_url() {
        // Non-active edit: native adapters validate even with empty base_url (hardcoded official endpoint).
        assert!(should_scratch_candidate("deepseek", "sk-x", ""));
        assert!(should_scratch_candidate("qwen", "sk-x", ""));
        // Relay still needs base_url; empty key skips validation.
        assert!(!should_scratch_candidate("relay", "sk-x", ""));
        assert!(should_scratch_candidate("relay", "sk-x", "https://r"));
        assert!(!should_scratch_candidate("deepseek", "", ""));
    }

    #[test]
    fn relay_empty_base_url_is_rejected_before_save() {
        // Relay/custom endpoint with empty (or whitespace-only) base_url → reject before persist.
        assert!(relay_missing_base_url("relay", ""));
        assert!(relay_missing_base_url("glm", "   "));
        assert!(relay_missing_base_url("custom", ""));
        // Relay with a URL is allowed.
        assert!(!relay_missing_base_url("relay", "https://r"));
        // Native uses hardcoded endpoints; empty base_url is fine → do not reject.
        assert!(!relay_missing_base_url("deepseek", ""));
        assert!(!relay_missing_base_url("qwen", ""));
    }

    #[test]
    fn relay_empty_model_is_rejected() {
        let empty = Profile::default();
        assert!(relay_missing_profile_models("relay", &empty));
        assert!(relay_missing_profile_models("glm", &empty));
        assert!(relay_missing_profile_models("custom", &empty));
        let with_model = Profile {
            model: "glm-5.2".into(),
            ..Default::default()
        };
        assert!(!relay_missing_profile_models("relay", &with_model));
        assert!(!relay_missing_profile_models("deepseek", &empty));
        assert!(!relay_missing_profile_models("qwen", &empty));
    }

    #[test]
    fn build_model_registry_json_uses_flagship_default() {
        let p = Profile {
            template_id: "custom-openai".into(),
            api_format: "openai_chat".into(),
            default_model: "glm-4.5".into(),
            model: "glm-4.5".into(),
            active_models: vec!["glm-4.5".into(), "glm-5.2".into(), "glm-4.7".into()],
            ..Default::default()
        };
        let json = build_model_registry_json(&p).expect("registry json");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["default_model"], "glm-5.2");
        assert_eq!(v["models"][0], "glm-5.2");
    }
}
