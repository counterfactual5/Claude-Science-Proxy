//! 中转站预设表：Rust 后端单一来源（spec §4.1）。前端 get_relay_presets 取一次铺 UI，
//! 不在前端复制常量。base_url 只读（自定义走 relay-custom）；requires_model_override 决定
//! 是否允许「留空透传」（不认 claude-* 的家 = true，必须选模型）。

/// 一个中转站预设。所有字段静态常量（非密钥），可整表回显给前端。
#[derive(Clone)]
pub struct RelayPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub base_url_editable: bool,
    pub requires_model_override: bool,
    pub builtin_models: &'static [&'static str],
}

pub fn all() -> &'static [RelayPreset] {
    PRESETS
}

/// 按 id 查预设。
pub fn by_id(id: &str) -> Option<&'static RelayPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// 按 base_url 匹配内置预设（归一化去尾 `/` 精确比对），返回预设 id。
/// 空 base_url（relay-custom）不参与匹配 → None。供遗留 provider=relay 迁移用。
pub fn match_base_url(url: &str) -> Option<&'static str> {
    let norm = url.trim().trim_end_matches('/');
    if norm.is_empty() {
        return None;
    }
    PRESETS
        .iter()
        .find(|p| !p.base_url.is_empty() && p.base_url.trim_end_matches('/') == norm)
        .map(|p| p.id)
}

static PRESETS: &[RelayPreset] = &[
    RelayPreset {
        id: "relay-xiaomi",
        name: "小米 MiMo",
        base_url: "https://api.xiaomimimo.com/anthropic",
        base_url_editable: false,
        requires_model_override: true, // 无 /v1/models + 不认 claude-*，必须选模型
        builtin_models: &["mimo-v2.5-pro"],
    },
    RelayPreset {
        id: "relay-glm",
        name: "智谱 GLM",
        base_url: "https://open.bigmodel.cn/api/anthropic",
        base_url_editable: false,
        requires_model_override: false, // 有 /v1/models + 认 claude-*（含裸名），可透传
        builtin_models: &["glm-4.6", "glm-5", "glm-4.5-air"],
    },
    RelayPreset {
        id: "relay-siliconflow",
        name: "硅基流动",
        base_url: "https://api.siliconflow.cn",
        base_url_editable: false,
        requires_model_override: true, // 有 models 但不认 claude-*，必须选模型
        builtin_models: &["deepseek-ai/DeepSeek-V3", "zai-org/GLM-5.2"],
    },
    RelayPreset {
        id: "relay-openrouter",
        name: "OpenRouter",
        base_url: "https://openrouter.ai/api",
        base_url_editable: false,
        requires_model_override: false, // 有 /v1/models + 认裸/前缀 claude-*，可透传
        builtin_models: &[
            "anthropic/claude-sonnet-5",
            "anthropic/claude-opus-4.8-fast",
        ],
    },
    RelayPreset {
        id: "relay-custom",
        name: "自定义",
        base_url: "",
        base_url_editable: true,
        requires_model_override: true, // 未实测兼容前默认要求选模型（高级用户前端可显式解锁）
        builtin_models: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_five_presets_with_expected_ids() {
        let ids: Vec<&str> = all().iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![
                "relay-xiaomi",
                "relay-glm",
                "relay-siliconflow",
                "relay-openrouter",
                "relay-custom"
            ]
        );
    }

    #[test]
    fn requires_model_override_matches_spec() {
        // 不认 claude-* / 无 models 的家必须选模型；GLM/OpenRouter 可透传；custom 默认 true。
        assert!(by_id("relay-xiaomi").unwrap().requires_model_override);
        assert!(by_id("relay-siliconflow").unwrap().requires_model_override);
        assert!(!by_id("relay-glm").unwrap().requires_model_override);
        assert!(!by_id("relay-openrouter").unwrap().requires_model_override);
        assert!(by_id("relay-custom").unwrap().requires_model_override);
    }

    #[test]
    fn custom_has_empty_editable_base_url() {
        let c = by_id("relay-custom").unwrap();
        assert_eq!(c.base_url, "");
        assert!(c.base_url_editable);
    }

    #[test]
    fn match_base_url_finds_preset_and_ignores_trailing_slash() {
        assert_eq!(
            match_base_url("https://open.bigmodel.cn/api/anthropic/"),
            Some("relay-glm")
        );
        assert_eq!(
            match_base_url("https://api.xiaomimimo.com/anthropic"),
            Some("relay-xiaomi")
        );
        assert_eq!(match_base_url("https://unknown.example/relay"), None);
        assert_eq!(match_base_url(""), None);
    }

    #[test]
    fn builtin_models_present_for_known_presets() {
        assert!(by_id("relay-xiaomi")
            .unwrap()
            .builtin_models
            .contains(&"mimo-v2.5-pro"));
        assert!(by_id("relay-custom").unwrap().builtin_models.is_empty());
    }
}
