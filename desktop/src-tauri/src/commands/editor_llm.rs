//! Scan local editor LLM configs and import as CSP profiles.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use serde_json::json;

use crate::config::{self, MAX_PLATTER_MODELS};
use crate::runtime::editor_llm_sources::{
    self, EditorLlmSourceKind,
};
use crate::runtime::i18n::i18n_err;
use crate::runtime::profile::{create_profile_inner, update_profile_connection_inner};
use crate::templates;

pub(crate) use editor_llm_sources::norm_base_url;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredEditorLlm {
    pub id: String,
    pub name: String,
    pub source_label: String,
    pub source_path: String,
    pub api_url: String,
    pub models: Vec<String>,
    pub already_imported: bool,
    pub has_key: bool,
    pub needs_key: bool,
    /// Where the key was resolved from: `config` | `env` | `keychain` | omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_source: Option<String>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub(crate) fn infer_template_id(base_url: &str) -> &'static str {
    let norm = norm_base_url(base_url);
    if norm.is_empty() {
        return "custom-openai";
    }
    for t in templates::all() {
        let tb = norm_base_url(t.base_url);
        if !tb.is_empty() && tb == norm {
            return t.id;
        }
    }
    if norm.contains("/responses") && !norm.contains("/anthropic") {
        return "custom-openai-responses";
    }
    if norm.contains("/v1")
        || norm.contains("/paas/")
        || norm.contains("compatible-mode")
        || norm.contains("/coding/")
        || norm.contains("/api/v3")
    {
        return "custom-openai";
    }
    if norm.contains("deepseek.com") {
        return "deepseek";
    }
    if (norm.contains("open.bigmodel.cn") || norm.contains("api.z.ai")) && norm.contains("/anthropic")
    {
        return "glm";
    }
    if norm.contains("xiaomimimo.com") || norm.contains("token-plan") {
        return "xiaomi";
    }
    if norm.contains("moonshot.cn") || norm.contains("moonshot.ai") {
        return "kimi";
    }
    if norm.contains("minimaxi.com") || norm.contains("minimax.io") {
        return "minimax";
    }
    if norm.contains("openrouter.ai") {
        return "openrouter";
    }
    "custom-openai"
}

/// Choose a CSP template for an imported endpoint. Anthropic-format sources
/// (Claude Code custom ANTHROPIC_BASE_URL) must not fall back to an OpenAI
/// template when no host-specific match exists.
pub(crate) fn pick_template_id(base_url: &str, kind: &EditorLlmSourceKind) -> &'static str {
    let inferred = infer_template_id(base_url);
    if kind.is_anthropic() {
        let is_anthropic_tmpl = templates::by_id(inferred)
            .map(|t| t.api_format == "anthropic")
            .unwrap_or(false);
        if !is_anthropic_tmpl {
            return "custom";
        }
    }
    inferred
}

/// Zed env var: provider id → UPPER_SNAKE + `_API_KEY`.
pub(crate) fn provider_env_key_name(provider_id: &str) -> String {
    let mut out = String::new();
    let mut prev_us = true;
    for c in provider_id.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out.push_str("_API_KEY");
    out
}

fn lookup_env_key(provider_id: &str) -> Option<String> {
    let name = provider_env_key_name(provider_id);
    std::env::var(&name).ok().filter(|v| !v.trim().is_empty())
}

/// Best-effort macOS Keychain read for Zed-stored keys. Never logs the secret.
fn lookup_keychain_key(provider_id: &str, api_url: &str, kind: &EditorLlmSourceKind) -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (provider_id, api_url, kind);
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        let mut candidates: Vec<(&str, &str)> = vec![
            ("Zed", provider_id),
            ("Zed", api_url),
            ("dev.zed.Zed", provider_id),
            ("Zed Language Model", provider_id),
            ("Zed OpenAI Compatible", provider_id),
        ];
        if matches!(kind, EditorLlmSourceKind::Continue) {
            candidates.push(("Continue", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::OpenCode) {
            candidates.push(("OpenCode", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::OpenClaw) {
            candidates.push(("OpenClaw", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Cline) {
            candidates.push(("Cline", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Aider) {
            candidates.push(("Aider", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Codex) {
            candidates.push(("Codex", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::ClaudeCode) {
            candidates.push(("Claude Code", provider_id));
            candidates.push(("Claude Code-credentials", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Cursor) {
            candidates.push(("Cursor", provider_id));
            candidates.push(("Cursor Safe Storage", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Trae) {
            candidates.push(("Trae", provider_id));
            candidates.push(("Trae Safe Storage", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::QwenCode) {
            candidates.push(("Qwen Code", provider_id));
            candidates.push(("qwen-code", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::IFlow) {
            candidates.push(("iFlow", provider_id));
        }
        if matches!(kind, EditorLlmSourceKind::Crush) {
            candidates.push(("Crush", provider_id));
        }
        for (service, account) in candidates {
            if let Some(key) = security_find_generic_password(service, account) {
                return Some(key);
            }
        }
        None
    }
}

#[cfg(target_os = "macos")]
fn security_find_generic_password(service: &str, account: &str) -> Option<String> {
    let out = Command::new("security")
        .args(["find-generic-password", "-s", service, "-a", account, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let key = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn resolve_provider_key_with_source(
    provider_id: &str,
    api_url: &str,
    kind: &EditorLlmSourceKind,
    inline_key: Option<&str>,
) -> (Option<String>, Option<&'static str>) {
    if let Some(k) = inline_key.filter(|k| !k.trim().is_empty()) {
        return (Some(k.to_string()), Some("config"));
    }
    if let Some(k) = lookup_env_key(provider_id) {
        return (Some(k), Some("env"));
    }
    if let Some(k) = lookup_keychain_key(provider_id, api_url, kind) {
        return (Some(k), Some("keychain"));
    }
    (None, None)
}

fn resolve_provider_key(
    provider_id: &str,
    api_url: &str,
    kind: &EditorLlmSourceKind,
    inline_key: Option<&str>,
) -> Option<String> {
    resolve_provider_key_with_source(provider_id, api_url, kind, inline_key).0
}

fn discover_id(source_label: &str, name: &str, source_path: &str) -> String {
    // Path keeps ids unique across mirrored OpenClaw/QClaw registry files.
    format!("{source_label}|{name}|{source_path}")
}

/// Distinguish product variants that share one scanner kind, so a provider
/// mirrored across several installs shows exactly where it came from.
fn source_display_label(kind: &EditorLlmSourceKind, source_path: &str) -> String {
    let p = source_path.replace('\\', "/");
    if p.contains("/.qclaw/") {
        return "QClaw".to_string();
    }
    for app in ["TRAE SOLO CN", "TRAE SOLO", "Trae CN", "Cursor Nightly"] {
        if p.contains(&format!("/{app}/")) {
            return app.to_string();
        }
    }
    kind.label().to_string()
}

#[derive(Debug)]
struct PendingDiscover {
    name: String,
    /// Every source (tool/variant) this entry was seen in; stacked in the UI.
    source_labels: Vec<String>,
    source_path: String,
    api_url: String,
    models: Vec<String>,
    key: Option<String>,
    key_source: Option<&'static str>,
}

fn pending_has_key(p: &PendingDiscover) -> bool {
    p.key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
}

/// Strictly richer info (more models, or a key when the other has none).
/// No directory/path preference — merged rows keep every source label anyway.
fn pending_richer(a: &PendingDiscover, b: &PendingDiscover) -> bool {
    let score = |p: &PendingDiscover| (p.models.len(), pending_has_key(p));
    score(a) > score(b)
}

/// Same provider identity: normalized URL + resolved key (URL-only when both
/// are keyless). Same URL with different keys = different accounts, kept apart.
fn pending_same_identity(a: &PendingDiscover, b: &PendingDiscover) -> bool {
    if norm_base_url(&a.api_url) != norm_base_url(&b.api_url) {
        return false;
    }
    match (
        a.key.as_deref().filter(|k| !k.is_empty()),
        b.key.as_deref().filter(|k| !k.is_empty()),
    ) {
        (Some(ka), Some(kb)) => ka == kb,
        (None, None) => true,
        // Keyed vs keyless at the same URL: keep both (keyed one is more useful).
        _ => false,
    }
}

/// Collapse mirrored copies of one provider (same URL + key) into a single row,
/// stacking every source label so the UI shows all places it was found.
fn dedupe_pending(items: Vec<PendingDiscover>) -> Vec<PendingDiscover> {
    let mut out: Vec<PendingDiscover> = Vec::new();
    for item in items {
        if let Some(i) = out.iter().position(|prev| pending_same_identity(prev, &item)) {
            let mut labels = out[i].source_labels.clone();
            for l in &item.source_labels {
                if !labels.contains(l) {
                    labels.push(l.clone());
                }
            }
            if pending_richer(&item, &out[i]) {
                out[i] = item;
            }
            out[i].source_labels = labels;
        } else {
            out.push(item);
        }
    }
    out
}

/// (normalized base URL, stored API key) per existing profile, for duplicate checks.
fn existing_url_keys(cfg: &config::Config) -> Vec<(String, String)> {
    cfg.profiles
        .iter()
        .map(|p| (norm_base_url(&p.base_url), p.api_key.clone()))
        .filter(|(u, _)| !u.is_empty())
        .collect()
}

/// A candidate is a duplicate of an existing profile when the URL matches AND the
/// keys can't be told apart. Same URL with a *different* resolved key is a distinct
/// account (e.g. two Zed providers sharing one endpoint with per-provider env keys)
/// and stays importable.
fn is_duplicate_of_existing(
    existing: &[(String, String)],
    api_url: &str,
    candidate_key: Option<&str>,
) -> bool {
    let norm = norm_base_url(api_url);
    existing.iter().any(|(url, key)| {
        url == &norm
            && match candidate_key {
                // Candidate key resolved: only an existing profile with no key yet
                // (fill it instead of importing a twin) or the same key is a dup.
                Some(ck) if !ck.is_empty() => key.is_empty() || key == ck,
                // No key resolved: URL match alone counts (conservative, as before).
                _ => true,
            }
    })
}

fn kind_for_path(path: &std::path::Path) -> Result<EditorLlmSourceKind, String> {
    editor_llm_sources::EditorLlmSourceKind::from_path(path)
        .ok_or_else(|| format!("unknown editor LLM source: {}", path.display()))
}

#[tauri::command]
pub(crate) fn discover_editor_llm_providers() -> Result<Vec<DiscoveredEditorLlm>, String> {
    let Some(home) = home_dir() else {
        return Ok(Vec::new());
    };
    let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
    let known = existing_url_keys(&cfg);
    let mut pending = Vec::new();
    for spec in editor_llm_sources::list_editor_llm_sources(&home) {
        let candidates = editor_llm_sources::scan_source(&spec)?;
        for c in candidates {
            let (key, key_source) =
                resolve_provider_key_with_source(&c.name, &c.api_url, &c.kind, c.api_key.as_deref());
            let source_path = c.source_path.display().to_string();
            let label = source_display_label(&spec.kind, &source_path);
            pending.push(PendingDiscover {
                name: c.name,
                source_labels: vec![label],
                source_path,
                api_url: c.api_url,
                models: c.models,
                key,
                key_source,
            });
        }
    }
    // One row per provider identity (URL + key). Mirrored copies (OpenClaw's
    // openclaw.json vs agents/*/models.json, Trae's four installs sharing one
    // store) collapse into a single row whose labels stack every origin.
    // Same URL + *different* key stays two rows: those are distinct accounts.
    let pending = dedupe_pending(pending);
    let mut found: Vec<DiscoveredEditorLlm> = pending
        .into_iter()
        .map(|p| {
            let already = is_duplicate_of_existing(&known, &p.api_url, p.key.as_deref());
            let has_key = pending_has_key(&p);
            let source_label = p.source_labels.join(" · ");
            DiscoveredEditorLlm {
                id: discover_id(&source_label, &p.name, &p.source_path),
                name: p.name,
                source_label,
                source_path: p.source_path,
                api_url: p.api_url,
                models: p.models,
                already_imported: already,
                has_key,
                needs_key: !has_key,
                key_source: p.key_source.map(str::to_string),
            }
        })
        .collect();
    found.sort_by(|a, b| {
        a.source_label
            .cmp(&b.source_label)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(found)
}

#[tauri::command]
pub(crate) fn preview_discovered_editor_llm(
    source_path: String,
    name: String,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(&source_path);
    let kind = kind_for_path(&path)?;
    let cfg = editor_llm_sources::preview_snippet(&path, &name, kind.clone())
        .map_err(|_| i18n_err("errEditorLlmNotFound", json!({ "name": name })))?;
    Ok(json!({
        "name": name,
        "sourceLabel": source_display_label(&kind, &source_path),
        "sourcePath": source_path,
        "config": cfg,
    }))
}

#[tauri::command]
pub(crate) fn import_discovered_editor_llm(
    lifecycle: tauri::State<'_, crate::SharedLifecycle>,
    source_path: String,
    name: String,
) -> Result<serde_json::Value, String> {
    lifecycle.with_serialized(|| import_discovered_editor_llm_inner(&source_path, &name))
}

fn import_discovered_editor_llm_inner(
    source_path: &str,
    name: &str,
) -> Result<serde_json::Value, String> {
    let path = PathBuf::from(source_path);
    let kind = kind_for_path(&path)?;
    let c = editor_llm_sources::resolve_import(&path, name, kind.clone())
        .map_err(|_| i18n_err("errEditorLlmNotFound", json!({ "name": name })))?;
    if c.api_url.trim().is_empty() {
        return Err(i18n_err("errMissingBaseUrl", json!({})));
    }
    let dir = config::default_dir();
    let existing = config::load_from(&dir).map_err(|e| e.to_string())?;
    let key = resolve_provider_key(&c.name, &c.api_url, &c.kind, c.api_key.as_deref());
    let norm = norm_base_url(&c.api_url);
    if let Some(p) = existing.profiles.iter().find(|p| {
        norm_base_url(&p.base_url) == norm
            && match key.as_deref() {
                // Same URL but a different resolved key = separate account; import it.
                Some(ck) if !ck.is_empty() => p.api_key.is_empty() || p.api_key == ck,
                _ => true,
            }
    }) {
        return Ok(json!({
            "skipped": true,
            "reason": "duplicate_base_url",
            "existingId": p.id,
            "existingName": p.name,
        }));
    }

    let mut models = c.models;
    let template_id = pick_template_id(&c.api_url, &c.kind);
    let has_key = key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);
    let models_total = models.len();
    let truncated = models_total > MAX_PLATTER_MODELS;
    models.truncate(MAX_PLATTER_MODELS);
    let default_model = models.first().cloned().unwrap_or_default();

    let id = create_profile_inner(
        &dir,
        template_id,
        &c.name,
        key.as_deref(),
        Some(&c.api_url),
        Some(default_model.as_str()),
    )?;
    if !models.is_empty() {
        update_profile_connection_inner(
            &dir,
            &id,
            Some(&c.api_url),
            None,
            Some(default_model.as_str()),
            Some(&models),
            Some(default_model.as_str()),
            None,
        )?;
    }

    Ok(json!({
        "skipped": false,
        "id": id,
        "name": c.name,
        "templateId": template_id,
        "modelsImported": models.len(),
        "modelsTotal": models_total,
        "modelsTruncated": truncated,
        "needsKey": !has_key,
        "hasKey": has_key,
        "sourceLabel": source_display_label(&c.kind, source_path),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_key_name_upper_snake() {
        assert_eq!(provider_env_key_name("Moonshot-CN"), "MOONSHOT_CN_API_KEY");
        assert_eq!(provider_env_key_name("Ollama-Cloud"), "OLLAMA_CLOUD_API_KEY");
    }

    #[test]
    fn infer_openai_v1_as_custom_openai() {
        assert_eq!(
            infer_template_id("https://api.moonshot.cn/v1"),
            "custom-openai"
        );
        assert_eq!(
            infer_template_id("https://open.bigmodel.cn/api/anthropic"),
            "glm"
        );
    }

    #[test]
    fn duplicate_check_same_url_different_key_is_importable() {
        let existing = vec![("https://integrate.api.nvidia.com/v1".to_string(), "key-A".to_string())];
        // Same URL + same key → duplicate.
        assert!(is_duplicate_of_existing(
            &existing,
            "https://integrate.api.nvidia.com/v1",
            Some("key-A")
        ));
        // Same URL + different key → separate account, importable.
        assert!(!is_duplicate_of_existing(
            &existing,
            "https://integrate.api.nvidia.com/v1",
            Some("key-B")
        ));
        // Same URL, no key resolved → conservative duplicate.
        assert!(is_duplicate_of_existing(
            &existing,
            "https://integrate.api.nvidia.com/v1/",
            None
        ));
        // Existing profile without a key: fill it instead of importing a twin.
        let keyless = vec![("https://api.x.com/v1".to_string(), String::new())];
        assert!(is_duplicate_of_existing(&keyless, "https://api.x.com/v1", Some("k")));
        // Different URL → never a duplicate.
        assert!(!is_duplicate_of_existing(&existing, "https://api.y.com/v1", Some("key-A")));
    }

    #[test]
    fn source_display_label_distinguishes_variants() {
        let k = EditorLlmSourceKind::Trae;
        assert_eq!(
            source_display_label(&k, "/U/L/Application Support/Trae/User/globalStorage/state.vscdb"),
            "Trae"
        );
        assert_eq!(
            source_display_label(&k, "/U/L/Application Support/TRAE SOLO CN/User/globalStorage/state.vscdb"),
            "TRAE SOLO CN"
        );
        assert_eq!(
            source_display_label(&EditorLlmSourceKind::OpenClaw, "/h/.qclaw/openclaw.json"),
            "QClaw"
        );
    }

    fn mk_pending(label: &str, path: &str, url: &str, key: &str, n_models: usize) -> PendingDiscover {
        PendingDiscover {
            name: "p".into(),
            source_labels: vec![label.to_string()],
            source_path: path.to_string(),
            api_url: url.to_string(),
            models: (0..n_models).map(|i| format!("m{i}")).collect(),
            key: if key.is_empty() { None } else { Some(key.to_string()) },
            key_source: None,
        }
    }

    #[test]
    fn dedupe_stacks_labels_for_mirrored_provider() {
        let out = dedupe_pending(vec![
            mk_pending("Trae", "/as/Trae/state.vscdb", "https://open.bigmodel.cn/api/coding/paas/v4", "sk-1", 3),
            mk_pending("TRAE SOLO CN", "/as/TRAE SOLO CN/state.vscdb", "https://open.bigmodel.cn/api/coding/paas/v4/", "sk-1", 1),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].models.len(), 3); // richer copy carried
        assert_eq!(out[0].source_labels, vec!["Trae".to_string(), "TRAE SOLO CN".to_string()]);
    }

    #[test]
    fn dedupe_keeps_same_url_different_keys_apart() {
        let out = dedupe_pending(vec![
            mk_pending("Zed", "/a", "https://api.x.com/v1", "key-A", 2),
            mk_pending("Zed", "/b", "https://api.x.com/v1", "key-B", 2),
        ]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn norm_base_url_strips_slash() {
        assert_eq!(
            norm_base_url("https://API.Example.com/v1/"),
            "https://api.example.com/v1"
        );
    }

}
