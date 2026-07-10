use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::runtime::i18n::i18n_err;
use crate::runtime::provider::{adapter_for_profile, build_model_registry_json};
use crate::{config, templates};

const STATIC_CATALOG_JSON: &str = include_str!("../../../../catalog/capabilities.v1.json");

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CatalogRule {
    pub(crate) id: String,
    pub(crate) scope: String,
    #[serde(rename = "match")]
    pub(crate) match_fields: BTreeMap<String, Value>,
    pub(crate) status: String,
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) evidence: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapabilityCatalog {
    pub(crate) schema_version: u32,
    pub(crate) providers: Vec<CatalogRule>,
    pub(crate) tool_rules: Vec<CatalogRule>,
    pub(crate) mcp_servers: Vec<CatalogRule>,
    pub(crate) skills: Vec<CatalogRule>,
    pub(crate) science_versions: Vec<CatalogRule>,
    pub(crate) transport_rules: Vec<CatalogRule>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CatalogContext {
    provider: String,
    api_format: String,
    base_url: String,
    model: String,
    thinking_policy: String,
    shim_mode: String,
    uses_virtual_registry: bool,
}

impl CapabilityCatalog {
    fn all_rules(&self) -> Vec<&CatalogRule> {
        self.providers
            .iter()
            .chain(self.tool_rules.iter())
            .chain(self.mcp_servers.iter())
            .chain(self.skills.iter())
            .chain(self.science_versions.iter())
            .chain(self.transport_rules.iter())
            .collect()
    }

    fn rule_by_id(&self, id: &str) -> Option<&CatalogRule> {
        self.all_rules().into_iter().find(|rule| rule.id == id)
    }
}

pub(crate) fn load_static_catalog() -> Result<CapabilityCatalog, String> {
    let catalog: CapabilityCatalog = serde_json::from_str(STATIC_CATALOG_JSON)
        .map_err(|e| i18n_err("errCatalogParseFailed", json!({ "detail": e.to_string() })))?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

pub(crate) fn validate_catalog(catalog: &CapabilityCatalog) -> Result<(), String> {
    if catalog.schema_version != 1 {
        return Err(i18n_err(
            "errCatalogSchemaUnsupported",
            json!({ "version": catalog.schema_version }),
        ));
    }
    let scopes = [
        "provider",
        "model",
        "tool",
        "mcp",
        "skill",
        "science_version",
        "transport",
    ];
    let statuses = ["supported", "limited", "unsupported", "unknown"];
    let actions = [
        "none",
        "normalize",
        "drop",
        "disable",
        "degrade",
        "diagnose",
        "document",
    ];
    let mut ids = BTreeSet::new();
    for rule in catalog.all_rules() {
        if rule.id.trim().is_empty() {
            return Err(i18n_err("errCatalogRuleIdEmpty", json!({})));
        }
        if !ids.insert(rule.id.clone()) {
            return Err(i18n_err(
                "errCatalogRuleIdDuplicate",
                json!({ "id": rule.id }),
            ));
        }
        if !scopes.contains(&rule.scope.as_str()) {
            return Err(i18n_err(
                "errCatalogRuleScopeInvalid",
                json!({ "scope": rule.scope, "id": rule.id }),
            ));
        }
        if !statuses.contains(&rule.status.as_str()) {
            return Err(i18n_err(
                "errCatalogRuleStatusInvalid",
                json!({ "status": rule.status, "id": rule.id }),
            ));
        }
        if !actions.contains(&rule.action.as_str()) {
            return Err(i18n_err(
                "errCatalogRuleActionInvalid",
                json!({ "action": rule.action, "id": rule.id }),
            ));
        }
        if rule.reason.trim().is_empty() {
            return Err(i18n_err(
                "errCatalogRuleReasonEmpty",
                json!({ "id": rule.id }),
            ));
        }
        if rule.evidence.is_empty() {
            return Err(i18n_err(
                "errCatalogRuleEvidenceEmpty",
                json!({ "id": rule.id }),
            ));
        }
    }
    Ok(())
}

fn profile_api_format(p: &config::Profile) -> String {
    if !p.api_format.trim().is_empty() {
        return p.api_format.clone();
    }
    templates::by_id(&p.template_id)
        .map(|t| t.api_format.to_string())
        .unwrap_or_else(|| "anthropic".to_string())
}

pub(crate) fn context_for_profile(p: &config::Profile, shim_mode: &str) -> CatalogContext {
    CatalogContext {
        provider: adapter_for_profile(p).to_string(),
        api_format: profile_api_format(p),
        base_url: p.base_url.clone(),
        model: p.model.clone(),
        thinking_policy: templates::thinking_policy_for(&p.template_id).to_string(),
        shim_mode: shim_mode.to_string(),
        uses_virtual_registry: build_model_registry_json(p).is_some(),
    }
}

fn text_match(fields: &BTreeMap<String, Value>, key: &str, actual: &str) -> bool {
    match fields.get(key).and_then(Value::as_str) {
        Some(expected) => expected == actual,
        None => true,
    }
}

fn contains_match(fields: &BTreeMap<String, Value>, key: &str, actual: &str) -> bool {
    match fields.get(key).and_then(Value::as_str) {
        Some(needle) => actual
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase()),
        None => true,
    }
}

fn condition_match(fields: &BTreeMap<String, Value>, ctx: &CatalogContext) -> bool {
    match fields.get("condition").and_then(Value::as_str) {
        Some("virtual_model_registry active") => ctx.uses_virtual_registry,
        Some("relay_force_fallback") => {
            !ctx.uses_virtual_registry
                && !ctx.model.trim().is_empty()
                && matches!(
                    ctx.provider.as_str(),
                    "relay" | "openai-custom" | "openai-responses"
                )
        }
        Some("relay_force_model present") => {
            ctx.provider == "relay" && !ctx.model.trim().is_empty()
        }
        Some(_) | None => true,
    }
}

fn rule_is_profile_scoped(rule: &CatalogRule) -> bool {
    rule.match_fields.keys().all(|key| {
        matches!(
            key.as_str(),
            "provider"
                | "api_format"
                | "thinking_policy"
                | "shim_mode"
                | "base_url_contains"
                | "model_contains"
                | "condition"
        )
    })
}

fn rule_matches_context(rule: &CatalogRule, ctx: &CatalogContext) -> bool {
    if !rule_is_profile_scoped(rule) {
        return false;
    }
    let fields = &rule.match_fields;
    text_match(fields, "provider", &ctx.provider)
        && text_match(fields, "api_format", &ctx.api_format)
        && text_match(fields, "thinking_policy", &ctx.thinking_policy)
        && text_match(fields, "shim_mode", &ctx.shim_mode)
        && contains_match(fields, "base_url_contains", &ctx.base_url)
        && contains_match(fields, "model_contains", &ctx.model)
        && condition_match(fields, ctx)
}

fn rule_summary(rule: &CatalogRule) -> Value {
    json!({
        "id": rule.id,
        "scope": rule.scope,
        "status": rule.status,
        "action": rule.action,
        "reason": rule.reason,
    })
}

fn boundary_rule_ids() -> &'static [&'static str] {
    &[
        "mcp.hosted-anthropic.hcls-boundary",
        "mcp.streamable-http.external-bio",
        "mcp.directory-connectors.virtual-login",
        "skill.remote-official.virtual-login-boundary",
        "science.version.0_1_15_dev.route-diff",
        "science.auth.refresh-hardcoded-0_1_15",
        "science.auth.virtual-oauth-scope-boundary",
        "transport.connect.anthropic-fastfail-401",
        "transport.connect.non-anthropic-direct-tunnel",
        "transport.http-proxy.not-set-by-default",
        "transport.upstream-proxy.planned-for-http-mcp",
    ]
}

pub(crate) fn diagnostics_for_profile(
    profile: Option<&config::Profile>,
    shim_mode: &str,
) -> serde_json::Value {
    let catalog = match load_static_catalog() {
        Ok(catalog) => catalog,
        Err(e) => {
            return json!({
                "schema_version": null,
                "status": "error",
                "error": e,
                "active_rules": [],
                "boundary_rules": [],
            });
        }
    };

    let active_rules: Vec<Value> = match profile {
        Some(p) => {
            let ctx = context_for_profile(p, shim_mode);
            catalog
                .providers
                .iter()
                .chain(catalog.tool_rules.iter())
                .filter(|rule| rule_matches_context(rule, &ctx))
                .map(rule_summary)
                .collect()
        }
        None => Vec::new(),
    };
    let boundary_rules: Vec<Value> = boundary_rule_ids()
        .iter()
        .filter_map(|id| catalog.rule_by_id(id))
        .map(rule_summary)
        .collect();

    json!({
        "schema_version": catalog.schema_version,
        "status": "loaded",
        "active_rules": active_rules,
        "boundary_rules": boundary_rules,
    })
}

#[cfg(test)]
mod tests {
    use super::{context_for_profile, diagnostics_for_profile, load_static_catalog};
    use crate::config::Profile;

    fn ids(v: &serde_json::Value, key: &str) -> Vec<String> {
        v[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn static_catalog_loads_and_validates() {
        let catalog = load_static_catalog().unwrap();
        assert_eq!(catalog.schema_version, 1);
        assert!(catalog
            .providers
            .iter()
            .any(|r| r.id == "provider.virtual-model-registry"));
        assert!(catalog
            .providers
            .iter()
            .any(|r| r.id == "provider.relay.force-model-shell"));
    }

    #[test]
    fn profile_context_derives_template_adapter_and_policy() {
        let p = Profile {
            template_id: "kimi".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.moonshot.cn/anthropic".into(),
            model: "kimi-k2.7-code".into(),
            ..Default::default()
        };
        let ctx = context_for_profile(&p, "off");
        assert_eq!(ctx.provider, "relay");
        assert_eq!(ctx.thinking_policy, "enabled");
    }

    #[test]
    fn diagnostics_surface_profile_rules_and_boundaries() {
        let p = Profile {
            template_id: "kimi".into(),
            api_format: "anthropic".into(),
            base_url: "https://api.moonshot.cn/anthropic".into(),
            model: "kimi-k2.7-code".into(),
            ..Default::default()
        };
        let v = diagnostics_for_profile(Some(&p), "off");
        let active = ids(&v, "active_rules");
        assert!(active.contains(&"provider.virtual-model-registry".to_string()));
        assert!(!active.contains(&"provider.relay.force-model-shell".to_string()));
        assert!(active.contains(&"provider.kimi.relay-thinking-enabled".to_string()));
        assert!(!active.contains(&"tool.kimi.web_search.server-tool-filter".to_string()));
        assert!(!active.contains(&"tool.relay.input-schema-normalize".to_string()));

        let boundaries = ids(&v, "boundary_rules");
        assert!(boundaries.contains(&"mcp.hosted-anthropic.hcls-boundary".to_string()));
        assert!(boundaries.contains(&"science.auth.refresh-hardcoded-0_1_15".to_string()));
        assert!(boundaries.contains(&"transport.upstream-proxy.planned-for-http-mcp".to_string()));
    }

    #[test]
    fn diagnostics_surface_dashscope_responses_rules() {
        let p = Profile {
            template_id: "custom-openai-responses".into(),
            api_format: "openai_responses".into(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            model: "qwen-max".into(),
            ..Default::default()
        };
        let v = diagnostics_for_profile(Some(&p), "off");
        let active = ids(&v, "active_rules");
        assert!(!active.contains(&"provider.dashscope.responses-tools-cap".to_string()));
        assert!(!active.contains(&"tool.dashscope.responses.web_search-drop".to_string()));
    }
}
