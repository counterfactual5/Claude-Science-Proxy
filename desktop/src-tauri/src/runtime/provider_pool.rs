//! Provider pool：多 profile 同时生效时的注册表合并与 pool 启动参数。

use crate::config;
use crate::runtime::model_sort;
use crate::runtime::provider::{
    adapter_for_profile, is_native_adapter, proxy_args_for,
    ProxyLaunch,
};
use crate::templates;

/// 单条 profile 的虚拟模型注册表切片（供 Python 合并分配 shell）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistrySlice {
    pub profile_id: String,
    pub display_name: String,
    pub models: Vec<String>,
    pub default_model: String,
    pub fast_model: String,
}

/// 从 profile 提取可进合并注册表的模型列表。native 用模板 builtin；relay/openai 用 effective_models。
pub(crate) fn registry_slice_for(p: &config::Profile) -> Option<RegistrySlice> {
    let adapter = adapter_for_profile(p);
    let models = registry_models_for(p, &adapter)?;
    if models.is_empty() {
        return None;
    }
    let default_model = p.effective_default_model();
    let fast_model = models
        .last()
        .cloned()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| default_model.clone());
    Some(RegistrySlice {
        profile_id: p.id.clone(),
        display_name: p.name.clone(),
        models,
        default_model,
        fast_model,
    })
}

fn registry_models_for(p: &config::Profile, adapter: &str) -> Option<Vec<String>> {
    let mut models = if is_native_adapter(adapter) {
        if let Some(t) = templates::by_id(&p.template_id) {
            let builtin: Vec<String> = t.builtin_models.iter().map(|s| (*s).to_string()).collect();
            if !builtin.is_empty() {
                builtin
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        let m = p.effective_models();
        if m.is_empty() {
            return None;
        }
        m
    };
    model_sort::sort_model_ids(&mut models);
    Some(models)
}

/// 合并所有 active profile 的注册表切片为 Python 可消费的 JSON。
/// 格式：`{"merge":true,"profiles":[{models,default_model,fast_model,profile_id,display_name},...]}`
pub(crate) fn build_merged_model_registry_json(profiles: &[&config::Profile]) -> Option<String> {
    let slices: Vec<RegistrySlice> = profiles.iter().filter_map(|p| registry_slice_for(p)).collect();
    if slices.is_empty() {
        return None;
    }
    if slices.len() == 1 {
        let s = &slices[0];
        let payload = serde_json::json!({
            "models": s.models,
            "default_model": s.default_model,
            "fast_model": s.fast_model,
            "profile_id": s.profile_id,
            "display_name": s.display_name,
        });
        return Some(payload.to_string());
    }
    let profiles_json: Vec<serde_json::Value> = slices
        .iter()
        .map(|s| {
            serde_json::json!({
                "models": s.models,
                "default_model": s.default_model,
                "fast_model": s.fast_model,
                "profile_id": s.profile_id,
                "display_name": s.display_name,
            })
        })
        .collect();
    Some(
        serde_json::json!({
            "merge": true,
            "profiles": profiles_json,
        })
        .to_string(),
    )
}

/// Provider pool 启动描述：每条 active profile 的上游连接参数（key 仅经子进程 env 传递）。
pub(crate) fn build_provider_pool_json(profiles: &[&config::Profile]) -> Result<String, String> {
    if profiles.len() < 2 {
        return Err("provider pool 至少需要 2 条 active profile。".into());
    }
    let entries: Vec<serde_json::Value> = profiles
        .iter()
        .map(|p| {
            let adapter = adapter_for_profile(p);
            serde_json::json!({
                "profile_id": p.id,
                "adapter": adapter,
                "api_format": p.api_format,
                "base_url": p.base_url,
                "key": p.api_key,
                "thinking_policy": templates::thinking_policy_for(&p.template_id),
                "default_model": p.effective_default_model(),
            })
        })
        .collect();
    Ok(serde_json::json!({ "profiles": entries }).to_string())
}

/// 多 profile pool 启动参数。
pub(crate) fn proxy_args_for_pool(profiles: &[config::Profile]) -> Result<ProxyLaunch, String> {
    if profiles.len() < 2 {
        return Err("proxy_args_for_pool 需要至少 2 条 profile。".into());
    }
    let refs: Vec<&config::Profile> = profiles.iter().collect();
    let registry = build_merged_model_registry_json(&refs);
    let pool = build_provider_pool_json(&refs)?;
    let default_model = profiles
        .first()
        .map(|p| p.effective_default_model())
        .unwrap_or_default();
    Ok(ProxyLaunch {
        adapter: "pool".to_string(),
        base_url: String::new(),
        model: default_model,
        key: pool.clone(),
        key_env: "CSSWITCH_POOL_MARKER",
        thinking_policy: "",
        model_registry_json: registry,
        provider_pool_json: Some(pool),
    })
}

/// 单 profile 沿用既有逻辑；多 profile 走 pool。
pub(crate) fn proxy_args_for_active_profiles(
    profiles: &[config::Profile],
) -> Result<ProxyLaunch, String> {
    match profiles.len() {
        0 => Err("无 active profile。".into()),
        1 => Ok(proxy_args_for(&profiles[0])),
        _ => proxy_args_for_pool(profiles),
    }
}

/// pool 指纹：合并注册表 + pool JSON + 各 profile 连接字段。
pub(crate) fn proxy_fingerprint_pool(profiles: &[config::Profile], launch: &ProxyLaunch) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut parts = Vec::new();
    for p in profiles {
        let a = proxy_args_for(p);
        parts.push(format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            p.id, p.template_id, a.adapter, a.base_url, a.model, a.key
        ));
    }
    parts.push(launch.model_registry_json.clone().unwrap_or_default());
    parts.push(launch.provider_pool_json.clone().unwrap_or_default());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.join("\n---\n").hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;

    fn relay_profile(id: &str, name: &str, models: &[&str]) -> Profile {
        Profile {
            id: id.into(),
            name: name.into(),
            template_id: "glm".into(),
            api_format: "anthropic".into(),
            base_url: "https://open.bigmodel.cn/api/anthropic".into(),
            api_key: "gk".into(),
            active_models: models.iter().map(|m| (*m).to_string()).collect(),
            default_model: models.first().map(|m| (*m).to_string()).unwrap_or_default(),
            ..Default::default()
        }
    }

    #[test]
    fn merge_registry_combines_two_profiles() {
        let p1 = relay_profile("a", "GLM", &["glm-5.2", "glm-4.7"]);
        let p2 = relay_profile("b", "Kimi", &["kimi-k2"]);
        let json = build_merged_model_registry_json(&[&p1, &p2]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["merge"], true);
        assert_eq!(v["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(v["profiles"][0]["profile_id"], "a");
        assert_eq!(v["profiles"][1]["profile_id"], "b");
    }

    #[test]
    fn single_profile_registry_uses_flat_payload() {
        let p = relay_profile("a", "GLM", &["glm-5.2"]);
        let json = build_merged_model_registry_json(&[&p]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("merge").is_none());
        assert_eq!(v["profile_id"], "a");
        assert_eq!(v["models"][0], "glm-5.2");
    }

    #[test]
    fn build_model_registry_json_matches_slice_for_single_relay() {
        let p = relay_profile("a", "GLM", &["glm-5.2", "glm-4.7"]);
        let merged = build_merged_model_registry_json(&[&p]).unwrap();
        let single = build_merged_model_registry_json(&[&p]).unwrap();
        let m: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let s: serde_json::Value = serde_json::from_str(&single).unwrap();
        assert_eq!(m["models"], s["models"]);
        assert_eq!(m["profile_id"], s["profile_id"]);
    }

    #[test]
    fn provider_pool_json_requires_two_profiles() {
        let p = relay_profile("a", "GLM", &["glm-5.2"]);
        assert!(build_provider_pool_json(&[&p]).is_err());
        let p2 = relay_profile("b", "Kimi", &["kimi-k2"]);
        let json = build_provider_pool_json(&[&p, &p2]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["profiles"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn proxy_args_for_active_profiles_routes_by_count() {
        let p1 = relay_profile("a", "GLM", &["glm-5.2"]);
        let p2 = relay_profile("b", "Kimi", &["kimi-k2"]);
        let single = proxy_args_for_active_profiles(&[p1.clone()]).unwrap();
        assert_eq!(single.adapter, "relay");
        assert!(single.provider_pool_json.is_none());
        let pool = proxy_args_for_active_profiles(&[p1, p2]).unwrap();
        assert_eq!(pool.adapter, "pool");
        assert!(pool.provider_pool_json.is_some());
    }
}
