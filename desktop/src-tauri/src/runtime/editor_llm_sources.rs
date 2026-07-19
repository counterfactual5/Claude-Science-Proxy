//! Parsers for editor/agent LLM provider configs.
//! Sources mirror the breadth of Skills/MCP discover (HOME-level known roots).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::runtime::jsonc::{read_jsonc, strip_jsonc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditorLlmSourceKind {
    Zed,
    Continue,
    OpenCode,
    OpenClaw,
    Factory,
    Cline,
    Aider,
    Codex,
    ClaudeCode,
    Cursor,
    Trae,
    QwenCode,
    IFlow,
    Crush,
}

impl EditorLlmSourceKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Zed => "Zed",
            Self::Continue => "Continue",
            Self::OpenCode => "OpenCode",
            Self::OpenClaw => "OpenClaw",
            Self::Factory => "Factory",
            Self::Cline => "Cline",
            Self::Aider => "Aider",
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Trae => "Trae",
            Self::QwenCode => "Qwen Code",
            Self::IFlow => "iFlow",
            Self::Crush => "Crush",
        }
    }

    /// Anthropic-format endpoints (vs OpenAI-compatible). Governs the imported
    /// CSP template's api_format when no host-specific template matches.
    pub(crate) fn is_anthropic(&self) -> bool {
        matches!(self, Self::ClaudeCode)
    }

    pub(crate) fn from_path(path: &Path) -> Option<Self> {
        let s = path.to_string_lossy();
        if s.contains("/zed/") || s.ends_with("zed/settings.json") {
            return Some(Self::Zed);
        }
        if s.contains("/.continue/") {
            return Some(Self::Continue);
        }
        if s.contains("opencode") && (s.ends_with(".json") || s.ends_with(".jsonc")) {
            return Some(Self::OpenCode);
        }
        // ~/.qclaw is Tencent QClaw's OpenClaw state dir (identical schema).
        if (s.contains("/.openclaw/") || s.contains("/.qclaw/"))
            && (s.ends_with("openclaw.json") || s.ends_with("models.json"))
        {
            return Some(Self::OpenClaw);
        }
        if s.contains("/.factory/") && s.ends_with("settings.json") {
            return Some(Self::Factory);
        }
        if s.contains("/.cline/") && s.ends_with("globalState.json") {
            return Some(Self::Cline);
        }
        if s.contains("aider.conf") {
            return Some(Self::Aider);
        }
        if s.contains("/.codex/") && s.ends_with("config.toml") {
            return Some(Self::Codex);
        }
        if s.contains("/.claude/")
            && (s.ends_with("settings.json") || s.ends_with("settings.local.json"))
        {
            return Some(Self::ClaudeCode);
        }
        if s.contains("/.qwen/") && s.ends_with("settings.json") {
            return Some(Self::QwenCode);
        }
        if s.contains("/.iflow/") && s.ends_with("settings.json") {
            return Some(Self::IFlow);
        }
        if s.ends_with("crush.json") || s.ends_with(".crush.json") {
            return Some(Self::Crush);
        }
        // Cursor / Trae persist custom LLM endpoints inside a VS Code-style
        // SQLite `state.vscdb` (ItemTable key/value), not a plain JSON file.
        if s.ends_with("state.vscdb") {
            if s.contains("/Cursor/") || s.contains("/Cursor ") {
                return Some(Self::Cursor);
            }
            if s.contains("/Trae/")
                || s.contains("/Trae ")
                || s.contains("/Trae CN/")
                || s.contains("/TRAE SOLO") // TRAE SOLO / TRAE SOLO CN
                || s.contains("/.trae")
            {
                return Some(Self::Trae);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EditorLlmCandidate {
    pub name: String,
    pub kind: EditorLlmSourceKind,
    pub source_path: PathBuf,
    pub api_url: String,
    pub models: Vec<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct EditorLlmSourceSpec {
    pub kind: EditorLlmSourceKind,
    pub path: PathBuf,
}

pub(crate) fn norm_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_lowercase()
}

/// Known HOME-level roots — same spirit as MCP_SOURCES / Skills DISCOVERY_ROOTS.
/// Covers apps that persist a **custom LLM endpoint** (base URL + models), whether
/// in a JSON/YAML/TOML file (Zed, Continue, …) or a VS Code-style `state.vscdb`
/// SQLite store (Cursor / Trae). Claude Code's custom `ANTHROPIC_BASE_URL` counts;
/// only pure account-login models with no custom endpoint have nothing to import.
pub(crate) fn list_editor_llm_sources(home: &Path) -> Vec<EditorLlmSourceSpec> {
    let mut out = Vec::new();
    let files: &[(&str, EditorLlmSourceKind)] = &[
        // Coding editors / agents with OpenAI-compatible catalogs
        (".config/zed/settings.json", EditorLlmSourceKind::Zed),
        (".continue/config.yaml", EditorLlmSourceKind::Continue),
        (".continue/config.yml", EditorLlmSourceKind::Continue),
        (".continue/config.json", EditorLlmSourceKind::Continue),
        (".config/opencode/opencode.json", EditorLlmSourceKind::OpenCode),
        (".config/opencode/opencode.jsonc", EditorLlmSourceKind::OpenCode),
        (".opencode.json", EditorLlmSourceKind::OpenCode),
        // Agent runtimes
        (".openclaw/openclaw.json", EditorLlmSourceKind::OpenClaw),
        // Tencent QClaw (小龙虾) is a desktop wrapper around OpenClaw; its state
        // dir is ~/.qclaw with the exact same openclaw.json / models.json schema.
        (".qclaw/openclaw.json", EditorLlmSourceKind::OpenClaw),
        (".factory/settings.json", EditorLlmSourceKind::Factory),
        (".cline/data/globalState.json", EditorLlmSourceKind::Cline),
        (".aider.conf.yml", EditorLlmSourceKind::Aider),
        (".aider.conf.yaml", EditorLlmSourceKind::Aider),
        (".codex/config.toml", EditorLlmSourceKind::Codex),
        // Claude Code custom endpoint (env.ANTHROPIC_BASE_URL). settings.local.json wins.
        (".claude/settings.local.json", EditorLlmSourceKind::ClaudeCode),
        (".claude/settings.json", EditorLlmSourceKind::ClaudeCode),
        // Qwen Code / iFlow CLI (OpenAI-compatible custom endpoints in settings.json).
        (".qwen/settings.json", EditorLlmSourceKind::QwenCode),
        (".iflow/settings.json", EditorLlmSourceKind::IFlow),
        // Crush (Charm) global config.
        (".config/crush/crush.json", EditorLlmSourceKind::Crush),
    ];
    let mut seen_continue = false;
    let mut seen_opencode = false;
    let mut seen_aider = false;
    let mut seen_claude = false;
    for (rel, kind) in files {
        if *kind == EditorLlmSourceKind::Continue && seen_continue {
            continue;
        }
        if *kind == EditorLlmSourceKind::OpenCode && seen_opencode {
            continue;
        }
        if *kind == EditorLlmSourceKind::Aider && seen_aider {
            continue;
        }
        if *kind == EditorLlmSourceKind::ClaudeCode && seen_claude {
            continue;
        }
        let p = home.join(rel);
        if p.is_file() {
            match kind {
                EditorLlmSourceKind::Continue => seen_continue = true,
                EditorLlmSourceKind::OpenCode => seen_opencode = true,
                EditorLlmSourceKind::Aider => seen_aider = true,
                EditorLlmSourceKind::ClaudeCode => seen_claude = true,
                _ => {}
            }
            out.push(EditorLlmSourceSpec {
                kind: kind.clone(),
                path: p,
            });
        }
    }
    // Cursor / Trae store custom endpoints in a SQLite state.vscdb (per-OS roots).
    for (path, kind) in vscdb_sources(home) {
        if path.is_file() {
            out.push(EditorLlmSourceSpec { kind, path });
        }
    }
    // OpenClaw/QClaw: agents/*/agent/models.json is just a runtime snapshot the
    // agent merges from the main config — scanning both would double every
    // provider. The main openclaw.json is authoritative; only fall back to the
    // per-agent registry when the main config is absent.
    for (main_rel, agents_rel) in [
        (".openclaw/openclaw.json", ".openclaw/agents"),
        (".qclaw/openclaw.json", ".qclaw/agents"),
    ] {
        if home.join(main_rel).is_file() {
            continue;
        }
        let agents = home.join(agents_rel);
        if !agents.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&agents) {
            for entry in entries.flatten() {
                let models = entry.path().join("agent/models.json");
                if models.is_file() {
                    out.push(EditorLlmSourceSpec {
                        kind: EditorLlmSourceKind::OpenClaw,
                        path: models,
                    });
                }
            }
        }
    }
    out
}

/// Per-OS `state.vscdb` roots for VS Code-derived editors (Cursor / Trae).
fn vscdb_sources(home: &Path) -> Vec<(PathBuf, EditorLlmSourceKind)> {
    let rel = "User/globalStorage/state.vscdb";
    #[cfg(target_os = "macos")]
    let root = home.join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let root = home.join(".config");
    let mut out = Vec::new();
    for app in ["Cursor", "Cursor Nightly"] {
        out.push((root.join(app).join(rel), EditorLlmSourceKind::Cursor));
    }
    // TRAE SOLO (agent-first shell) keeps the same store schema as Trae IDE.
    for app in ["Trae", "Trae CN", "TRAE SOLO", "TRAE SOLO CN"] {
        out.push((root.join(app).join(rel), EditorLlmSourceKind::Trae));
    }
    out
}

pub(crate) fn scan_source(spec: &EditorLlmSourceSpec) -> Result<Vec<EditorLlmCandidate>, String> {
    match spec.kind {
        EditorLlmSourceKind::Zed => scan_zed(&spec.path),
        EditorLlmSourceKind::Continue => scan_continue(&spec.path),
        EditorLlmSourceKind::OpenCode => scan_opencode(&spec.path),
        EditorLlmSourceKind::OpenClaw => scan_openclaw(&spec.path),
        EditorLlmSourceKind::Factory => scan_factory(&spec.path),
        EditorLlmSourceKind::Cline => scan_cline(&spec.path),
        EditorLlmSourceKind::Aider => scan_aider(&spec.path),
        EditorLlmSourceKind::Codex => scan_codex(&spec.path),
        EditorLlmSourceKind::ClaudeCode => scan_claude_code(&spec.path),
        EditorLlmSourceKind::Cursor => scan_cursor(&spec.path),
        EditorLlmSourceKind::Trae => scan_trae(&spec.path),
        EditorLlmSourceKind::QwenCode => scan_qwen(&spec.path),
        EditorLlmSourceKind::IFlow => scan_iflow(&spec.path),
        EditorLlmSourceKind::Crush => scan_crush(&spec.path),
    }
}

fn scan_zed(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let Some(block) = root
        .pointer("/language_models/openai_compatible")
        .and_then(|v| v.as_object())
    else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (name, cfg) in block {
        let api_url = cfg
            .get("api_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if api_url.is_empty() {
            continue;
        }
        let models = model_names_from_available(cfg.get("available_models"));
        out.push(EditorLlmCandidate {
            name: name.clone(),
            kind: EditorLlmSourceKind::Zed,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key: None,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_continue(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let root: Value = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        let json = strip_jsonc(&text);
        serde_json::from_str(&json)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        serde_yaml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?
    };
    let mut groups: BTreeMap<String, (String, String, Vec<String>, Option<String>)> =
        BTreeMap::new();
    for m in root.get("models").and_then(|v| v.as_array()).into_iter().flatten() {
        let Some(obj) = m.as_object() else {
            continue;
        };
        let provider = str_field(obj, &["provider"]).unwrap_or_default();
        let model_id = str_field(obj, &["model"]).unwrap_or_default();
        if model_id.is_empty() || model_id.eq_ignore_ascii_case("autodetect") {
            continue;
        }
        let api_base = str_field(obj, &["apiBase", "api_base"])
            .or_else(|| continue_provider_default_base(&provider));
        let Some(api_base) = api_base.filter(|u| !u.is_empty()) else {
            continue;
        };
        let norm = norm_base_url(&api_base);
        let display = str_field(obj, &["name", "title"]).unwrap_or_else(|| model_id.clone());
        let key = continue_extract_key(obj);
        let entry = groups.entry(norm).or_insert_with(|| {
            (display.clone(), api_base.clone(), Vec::new(), key.clone())
        });
        if entry.3.is_none() {
            entry.3 = key;
        }
        if !entry.2.contains(&model_id) {
            entry.2.push(model_id);
        }
    }
    let mut out = Vec::new();
    for (_norm, (name, api_url, models, api_key)) in groups {
        out.push(EditorLlmCandidate {
            name,
            kind: EditorLlmSourceKind::Continue,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_opencode(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let provider_obj = root
        .get("provider")
        .or_else(|| root.get("providers"))
        .and_then(|v| v.as_object());
    let Some(block) = provider_obj else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (id, cfg) in block {
        if cfg.get("disabled").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        let api_url = cfg
            .pointer("/options/baseURL")
            .or_else(|| cfg.pointer("/options/baseUrl"))
            // OpenCode v2 nests the endpoint under `settings.baseURL`.
            .or_else(|| cfg.pointer("/settings/baseURL"))
            .or_else(|| cfg.pointer("/settings/baseUrl"))
            .or_else(|| cfg.get("baseURL"))
            .or_else(|| cfg.get("baseUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if api_url.is_empty() {
            continue;
        }
        let mut models = Vec::new();
        if let Some(mobj) = cfg.get("models").and_then(|v| v.as_object()) {
            for mid in mobj.keys() {
                if !mid.is_empty() {
                    models.push(mid.clone());
                }
            }
        }
        let name = cfg
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(id)
            .to_string();
        let api_key = cfg
            .pointer("/options/apiKey")
            .or_else(|| cfg.pointer("/settings/apiKey"))
            .or_else(|| cfg.get("apiKey"))
            .and_then(|v| v.as_str())
            .filter(|s| !is_secret_placeholder(s))
            .map(str::to_string);
        out.push(EditorLlmCandidate {
            name,
            kind: EditorLlmSourceKind::OpenCode,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_openclaw(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let block = root
        .pointer("/models/providers")
        .or_else(|| root.get("providers"))
        .and_then(|v| v.as_object());
    let Some(block) = block else {
        return Ok(Vec::new());
    };
    // Optional allowlist models under agents.defaults.models keyed as "provider/model".
    let agent_models: Vec<String> = root
        .pointer("/agents/defaults/models")
        .and_then(|v| v.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    let mut out = Vec::new();
    for (id, cfg) in block {
        let base = cfg
            .get("baseUrl")
            .or_else(|| cfg.get("baseURL"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if base.is_empty() {
            continue;
        }
        // Skip ChatGPT/Codex OAuth backends — not usable as CSP OpenAI-custom profiles.
        let low = base.to_lowercase();
        if low.contains("chatgpt.com") || low.contains("openai-codex") {
            continue;
        }
        let api_kind = cfg
            .get("api")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if api_kind.contains("codex") {
            continue;
        }
        let api_url = normalize_openclaw_base_url(&base, &api_kind);
        let mut models = Vec::new();
        if let Some(arr) = cfg.get("models").and_then(|v| v.as_array()) {
            for m in arr {
                let mid = m
                    .get("id")
                    .or_else(|| m.get("name"))
                    .and_then(|v| v.as_str())
                    .or_else(|| m.as_str())
                    .unwrap_or("")
                    .trim();
                if !mid.is_empty() {
                    models.push(mid.to_string());
                }
            }
        }
        if models.is_empty() {
            let prefix = format!("{id}/");
            for full in &agent_models {
                if let Some(rest) = full.strip_prefix(&prefix) {
                    if !rest.is_empty() && !models.contains(&rest.to_string()) {
                        models.push(rest.to_string());
                    }
                }
            }
        }
        let api_key = cfg
            .get("apiKey")
            .and_then(|v| v.as_str())
            .filter(|s| !is_secret_placeholder(s))
            .map(str::to_string);
        out.push(EditorLlmCandidate {
            name: id.clone(),
            kind: EditorLlmSourceKind::OpenClaw,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_cline(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let secrets = path
        .parent()
        .map(|p| p.join("secrets.json"))
        .and_then(|p| read_jsonc(&p).ok().flatten());

    // (display_name, base_url_field, model_field, secret_field)
    let slots: &[(&str, &str, &[&str], &str)] = &[
        (
            "Cline · OpenAI Compatible",
            "openAiBaseUrl",
            &["openAiModelId", "actModeOpenAiModelId", "planModeOpenAiModelId"],
            "openAiApiKey",
        ),
        (
            "Cline · Ollama",
            "ollamaBaseUrl",
            &["ollamaModelId", "actModeOllamaModelId"],
            "ollamaApiKey",
        ),
        (
            "Cline · LM Studio",
            "lmStudioBaseUrl",
            &["lmStudioModelId", "actModeLmStudioModelId"],
            "",
        ),
        (
            "Cline · LiteLLM",
            "liteLlmBaseUrl",
            &["liteLlmModelId", "actModeLiteLlmModelId"],
            "liteLlmApiKey",
        ),
        (
            "Cline · Anthropic",
            "anthropicBaseUrl",
            &["apiModelId", "actModeApiModelId"],
            "apiKey",
        ),
    ];

    let mut out = Vec::new();
    for (name, url_key, model_keys, secret_key) in slots {
        let Some(api_url) = root.get(*url_key).and_then(|v| v.as_str()).map(|s| s.trim().to_string()) else {
            continue;
        };
        if api_url.is_empty() {
            continue;
        }
        let mut models = Vec::new();
        for mk in *model_keys {
            if let Some(mid) = root.get(*mk).and_then(|v| v.as_str()).map(|s| s.trim().to_string()) {
                if !mid.is_empty() && !models.contains(&mid) {
                    models.push(mid);
                }
            }
        }
        let api_key = if secret_key.is_empty() {
            None
        } else {
            secrets
                .as_ref()
                .and_then(|s| s.get(*secret_key))
                .and_then(|v| v.as_str())
                .filter(|s| !is_secret_placeholder(s))
                .map(str::to_string)
        };
        let api_url = if api_url.ends_with("/v1") || name.contains("Anthropic") {
            api_url
        } else if name.contains("Ollama") || name.contains("OpenAI") || name.contains("LiteLLM") || name.contains("LM Studio") {
            format!("{}/v1", api_url.trim_end_matches('/'))
        } else {
            api_url
        };
        out.push(EditorLlmCandidate {
            name: (*name).to_string(),
            kind: EditorLlmSourceKind::Cline,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_aider(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let root: Value = serde_yaml::from_str(&text)
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    let api_url = root
        .get("openai-api-base")
        .or_else(|| root.get("openai_api_base"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if api_url.is_empty() {
        return Ok(Vec::new());
    }
    let mut models = Vec::new();
    if let Some(m) = root.get("model").and_then(|v| v.as_str()) {
        let t = m.trim();
        if !t.is_empty() {
            models.push(t.to_string());
        }
    }
    let api_key = root
        .get("openai-api-key")
        .or_else(|| root.get("openai_api_key"))
        .and_then(|v| v.as_str())
        .filter(|s| !is_secret_placeholder(s))
        .map(str::to_string);
    Ok(vec![EditorLlmCandidate {
        name: "Aider".to_string(),
        kind: EditorLlmSourceKind::Aider,
        source_path: path.to_path_buf(),
        api_url,
        models,
        api_key,
    }])
}

fn scan_codex(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    let mut out = Vec::new();
    if let Some(providers) = table.get("model_providers").and_then(|v| v.as_table()) {
        for (id, cfg) in providers {
            let Some(cfg) = cfg.as_table() else {
                continue;
            };
            let api_url = cfg
                .get("base_url")
                .or_else(|| cfg.get("baseUrl"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if api_url.is_empty() {
                continue;
            }
            let name = cfg
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(id)
                .to_string();
            let env_key = cfg
                .get("env_key")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let api_key = env_key
                .as_deref()
                .and_then(|k| std::env::var(k).ok())
                .filter(|v| !v.trim().is_empty());
            out.push(EditorLlmCandidate {
                name: format!("Codex · {name}"),
                kind: EditorLlmSourceKind::Codex,
                source_path: path.to_path_buf(),
                api_url,
                models: Vec::new(),
                api_key,
            });
        }
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_factory(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let Some(arr) = root.get("customModels").and_then(|v| v.as_array()) else {
        return Ok(Vec::new());
    };
    // Group by normalized baseUrl.
    let mut groups: BTreeMap<String, (String, String, Vec<String>, Option<String>)> =
        BTreeMap::new();
    for m in arr {
        let Some(obj) = m.as_object() else {
            continue;
        };
        let api_url = str_field(obj, &["baseUrl", "baseURL"]).unwrap_or_default();
        if api_url.is_empty() {
            continue;
        }
        let model_id = str_field(obj, &["model"]).unwrap_or_default();
        if model_id.is_empty() {
            continue;
        }
        let display = str_field(obj, &["displayName"])
            .unwrap_or_else(|| factory_name_from_url(&api_url));
        let key = str_field(obj, &["apiKey"]).filter(|s| !is_secret_placeholder(s));
        let norm = norm_base_url(&api_url);
        let entry = groups.entry(norm).or_insert_with(|| {
            (
                factory_name_from_url(&api_url),
                api_url.clone(),
                Vec::new(),
                key.clone(),
            )
        });
        if entry.0.starts_with("Factory ·") {
            // Prefer a human display name if first model has one.
            if !display.starts_with("Factory ·") {
                entry.0 = format!("Factory · {display}");
            }
        }
        if entry.3.is_none() {
            entry.3 = key;
        }
        if !entry.2.contains(&model_id) {
            entry.2.push(model_id);
        }
    }
    let mut out = Vec::new();
    for (_norm, (name, api_url, models, api_key)) in groups {
        out.push(EditorLlmCandidate {
            name,
            kind: EditorLlmSourceKind::Factory,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_claude_code(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let env = root.get("env").and_then(|v| v.as_object());
    let api_url = env
        .and_then(|e| e.get("ANTHROPIC_BASE_URL"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    // Only a *custom* endpoint is importable; the default anthropic.com backend
    // is an account-only client with nothing to map into a CSP profile.
    if api_url.is_empty() {
        return Ok(Vec::new());
    }
    let mut models = Vec::new();
    for k in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_SMALL_FAST_MODEL",
    ] {
        if let Some(m) = env.and_then(|e| e.get(k)).and_then(|v| v.as_str()) {
            let t = m.trim();
            if !t.is_empty() && !models.contains(&t.to_string()) {
                models.push(t.to_string());
            }
        }
    }
    if let Some(m) = root.get("model").and_then(|v| v.as_str()) {
        let t = m.trim();
        // Top-level `model` may be an alias (opus/sonnet) rather than a real id;
        // keep only when no env-derived ids were found.
        if models.is_empty() && !t.is_empty() {
            models.push(t.to_string());
        }
    }
    let api_key = env
        .and_then(|e| e.get("ANTHROPIC_AUTH_TOKEN").or_else(|| e.get("ANTHROPIC_API_KEY")))
        .and_then(|v| v.as_str())
        .filter(|s| !is_secret_placeholder(s))
        .map(str::to_string);
    Ok(vec![EditorLlmCandidate {
        name: "Claude Code".to_string(),
        kind: EditorLlmSourceKind::ClaudeCode,
        source_path: path.to_path_buf(),
        api_url,
        models,
        api_key,
    }])
}

const CURSOR_STORE_KEY: &str =
    "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";

fn scan_cursor(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(raw) = read_vscdb_item(path, CURSOR_STORE_KEY)? else {
        return Ok(Vec::new());
    };
    let store: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("parse Cursor store {}: {e}", path.display()))?;
    // "Override OpenAI Base URL" — Cursor's only custom OpenAI-compatible endpoint.
    let api_url = store
        .get("openAIBaseUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if api_url.is_empty() {
        return Ok(Vec::new());
    }
    let api_url = ensure_openai_v1(&api_url);
    let models = cursor_model_names(store.get("availableAPIKeyModels"));
    // Cursor stores the custom key via Electron safeStorage (encrypted `v10`
    // blob under secret://cursorAuth/openAIKey) — not recoverable here, so the
    // key is surfaced as "needs key" and resolved via env/keychain downstream.
    Ok(vec![EditorLlmCandidate {
        name: format!("Cursor · {}", host_of(&api_url)),
        kind: EditorLlmSourceKind::Cursor,
        source_path: path.to_path_buf(),
        api_url,
        models,
        api_key: None,
    }])
}

fn cursor_model_names(val: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(arr) = val.and_then(|v| v.as_array()) else {
        return out;
    };
    for m in arr {
        let mid = m
            .as_str()
            .or_else(|| m.get("modelName").and_then(|v| v.as_str()))
            .or_else(|| m.get("name").and_then(|v| v.as_str()))
            .unwrap_or("")
            .trim();
        if !mid.is_empty() && !out.contains(&mid.to_string()) {
            out.push(mid.to_string());
        }
    }
    out
}

fn scan_trae(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(raw) = read_vscdb_item_like(path, "%AI.agent.model.model_list_map%")? else {
        return Ok(Vec::new());
    };
    let store: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("parse Trae store {}: {e}", path.display()))?;
    let Some(groups) = store.as_object() else {
        return Ok(Vec::new());
    };
    // The same custom provider is repeated across every agent mode group; merge
    // by provider id and collect distinct model ids + base URL.
    let mut merged: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for items in groups.values() {
        for it in items.as_array().into_iter().flatten() {
            // is_preset == false marks a user-added custom model.
            if it.get("is_preset").and_then(|v| v.as_bool()) != Some(false) {
                continue;
            }
            let provider = it
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let base = it
                .get("base_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if provider.is_empty() || base.is_empty() {
                continue;
            }
            // name looks like "provider//model-id"; fall back to display_name.
            let raw_name = it.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let model_id = raw_name
                .rsplit("//")
                .next()
                .filter(|s| !s.is_empty())
                .or_else(|| it.get("display_name").and_then(|v| v.as_str()))
                .unwrap_or("")
                .trim()
                .to_string();
            let entry = merged
                .entry(provider)
                .or_insert_with(|| (ensure_openai_v1(&base), Vec::new()));
            if !model_id.is_empty() && !entry.1.contains(&model_id) {
                entry.1.push(model_id);
            }
        }
    }
    let mut out = Vec::new();
    for (provider, (api_url, models)) in merged {
        out.push(EditorLlmCandidate {
            name: format!("Trae · {provider}"),
            kind: EditorLlmSourceKind::Trae,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key: None,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_qwen(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    // New schema: modelProviders.openai is an array of {id,name,baseUrl,envKey};
    // an older variant nests them under modelProviders.openai.models[].
    let entries: Vec<&Value> = match root.pointer("/modelProviders/openai") {
        Some(Value::Array(arr)) => arr.iter().collect(),
        Some(Value::Object(obj)) => obj
            .get("models")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // Group models sharing one baseUrl into a single provider candidate.
    let mut groups: BTreeMap<String, (String, String, Vec<String>, Option<String>)> =
        BTreeMap::new();
    for e in entries {
        let Some(obj) = e.as_object() else { continue };
        let base = str_field(obj, &["baseUrl", "base_url"]).unwrap_or_default();
        if base.is_empty() {
            continue;
        }
        let model_id = str_field(obj, &["id", "model"]).unwrap_or_default();
        if model_id.is_empty() {
            continue;
        }
        let display = str_field(obj, &["name"]).unwrap_or_else(|| host_of(&base));
        // Key comes from the env var named by `envKey`; resolve eagerly.
        let key = obj
            .get("envKey")
            .and_then(|v| v.as_str())
            .and_then(|k| std::env::var(k).ok())
            .filter(|v| !v.trim().is_empty());
        let norm = norm_base_url(&base);
        let entry = groups
            .entry(norm)
            .or_insert_with(|| (display.clone(), base.clone(), Vec::new(), key.clone()));
        if entry.3.is_none() {
            entry.3 = key;
        }
        if !entry.2.contains(&model_id) {
            entry.2.push(model_id);
        }
    }
    let mut out = Vec::new();
    for (_norm, (name, api_url, models, api_key)) in groups {
        out.push(EditorLlmCandidate {
            name: format!("Qwen · {name}"),
            kind: EditorLlmSourceKind::QwenCode,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    // Legacy single endpoint: security.auth.{baseUrl,apiKey} + model.name.
    if out.is_empty() {
        if let Some(base) = root
            .pointer("/security/auth/baseUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let key = root
                .pointer("/security/auth/apiKey")
                .and_then(|v| v.as_str())
                .filter(|s| !is_secret_placeholder(s))
                .map(str::to_string);
            let mut models = Vec::new();
            if let Some(m) = root.pointer("/model/name").and_then(|v| v.as_str()) {
                if !m.trim().is_empty() {
                    models.push(m.trim().to_string());
                }
            }
            out.push(EditorLlmCandidate {
                name: format!("Qwen · {}", host_of(&base)),
                kind: EditorLlmSourceKind::QwenCode,
                source_path: path.to_path_buf(),
                api_url: base,
                models,
                api_key: key,
            });
        }
    }
    sort_candidates(&mut out);
    Ok(out)
}

fn scan_iflow(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    // iFlow uses a flat {baseUrl, modelName, apiKey}; env override is IFLOW_baseUrl etc.
    let api_url = root
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .or_else(|| std::env::var("IFLOW_baseUrl").ok())
        .or_else(|| std::env::var("IFLOW_BASE_URL").ok())
        .unwrap_or_default();
    if api_url.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut models = Vec::new();
    if let Some(m) = root
        .get("modelName")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        models.push(m);
    }
    let api_key = root
        .get("apiKey")
        .and_then(|v| v.as_str())
        .filter(|s| !is_secret_placeholder(s))
        .map(str::to_string)
        .or_else(|| std::env::var("IFLOW_apiKey").ok())
        .or_else(|| std::env::var("IFLOW_API_KEY").ok())
        .filter(|v| !v.trim().is_empty());
    Ok(vec![EditorLlmCandidate {
        name: format!("iFlow · {}", host_of(&api_url)),
        kind: EditorLlmSourceKind::IFlow,
        source_path: path.to_path_buf(),
        api_url,
        models,
        api_key,
    }])
}

fn scan_crush(path: &Path) -> Result<Vec<EditorLlmCandidate>, String> {
    let Some(root) = read_jsonc(path)? else {
        return Ok(Vec::new());
    };
    let Some(providers) = root.get("providers").and_then(|v| v.as_object()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (id, cfg) in providers {
        let Some(obj) = cfg.as_object() else { continue };
        // Only entries with an explicit base_url are custom endpoints; builtins
        // that merely override api_key (e.g. {"openai":{"api_key":"$X"}}) are skipped.
        let base = str_field(obj, &["base_url", "baseUrl", "api_endpoint"]).unwrap_or_default();
        if base.is_empty() {
            continue;
        }
        let api_url = resolve_shell_var(&base).unwrap_or(base);
        if api_url.trim().is_empty() {
            continue;
        }
        let mut models = Vec::new();
        if let Some(arr) = obj.get("models").and_then(|v| v.as_array()) {
            for m in arr {
                let mid = m
                    .get("id")
                    .and_then(|v| v.as_str())
                    .or_else(|| m.as_str())
                    .unwrap_or("")
                    .trim();
                if !mid.is_empty() && !models.contains(&mid.to_string()) {
                    models.push(mid.to_string());
                }
            }
        }
        let name = str_field(obj, &["name"]).unwrap_or_else(|| id.clone());
        // api_key may be a `$VAR` reference; resolve from env, else treat as inline.
        let api_key = str_field(obj, &["api_key", "apiKey"]).and_then(|raw| {
            if raw.starts_with('$') {
                resolve_shell_var(&raw).filter(|v| !v.trim().is_empty())
            } else if is_secret_placeholder(&raw) {
                None
            } else {
                Some(raw)
            }
        });
        out.push(EditorLlmCandidate {
            name: format!("Crush · {name}"),
            kind: EditorLlmSourceKind::Crush,
            source_path: path.to_path_buf(),
            api_url,
            models,
            api_key,
        });
    }
    sort_candidates(&mut out);
    Ok(out)
}

/// Resolve a leading `$VAR` / `${VAR}` reference against the environment.
/// Returns `None` when the string is a var reference that isn't set.
fn resolve_shell_var(s: &str) -> Option<String> {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix('$') {
        let name = rest.trim_start_matches('{').trim_end_matches('}');
        return std::env::var(name).ok();
    }
    Some(t.to_string())
}

/// Read one ItemTable value by exact key from a VS Code-style `state.vscdb`.
/// Opened immutable + read-only so a running editor's live DB is never locked.
fn read_vscdb_item(path: &Path, key: &str) -> Result<Option<String>, String> {
    read_vscdb(path, "SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1", key)
}

/// Read the first ItemTable value whose key matches a LIKE pattern.
fn read_vscdb_item_like(path: &Path, like: &str) -> Result<Option<String>, String> {
    read_vscdb(path, "SELECT value FROM ItemTable WHERE key LIKE ?1 LIMIT 1", like)
}

fn read_vscdb(path: &Path, sql: &str, param: &str) -> Result<Option<String>, String> {
    use rusqlite::{Connection, OpenFlags};
    // immutable=1 avoids WAL/lock contention with a running editor; mode=ro is a
    // second safety net. Value column may be TEXT or BLOB depending on editor.
    // The path is percent-encoded because roots like "Application Support" and
    // "Trae CN" contain spaces that would otherwise break SQLite URI parsing.
    let uri = format!("file:{}?immutable=1&mode=ro", uri_encode_path(path));
    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare {}: {e}", path.display()))?;
    let val: Option<String> = stmt
        .query_row([param], |row| {
            // Grab as text; fall back to BLOB decoded as UTF-8.
            row.get::<_, String>(0).or_else(|_| {
                row.get::<_, Vec<u8>>(0)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
            })
        })
        .optional_string()?;
    Ok(val)
}

/// Percent-encode a filesystem path for use inside a SQLite `file:` URI.
/// Keeps `/` as a path separator; encodes spaces and URI-significant chars.
fn uri_encode_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'/' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Host portion of a URL for display names, e.g. `https://x.com/v1` → `x.com`.
fn host_of(url: &str) -> String {
    let trimmed = url.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    rest.split('/').next().unwrap_or(rest).to_string()
}

/// Append `/v1` for OpenAI-compatible roots that have no versioned path segment
/// (e.g. `https://api.deepseek.com`). Leaves already-pathed URLs untouched.
fn ensure_openai_v1(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let low = trimmed.to_lowercase();
    let has_path = host_of(trimmed).len() + "https://".len() < trimmed.len()
        || low.contains("/v1")
        || low.contains("/v2")
        || low.contains("/v3")
        || low.contains("/v4")
        || low.contains("/paas/")
        || low.contains("/coding/")
        || low.contains("/anthropic")
        || low.contains("/compatible-mode")
        || low.contains("/openai");
    if has_path {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

/// `rusqlite`'s `OptionalExtension` for `query_row` returning `Option`.
trait OptionalString<T> {
    fn optional_string(self) -> Result<Option<T>, String>;
}

impl<T> OptionalString<T> for rusqlite::Result<T> {
    fn optional_string(self) -> Result<Option<T>, String> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("query vscdb: {e}")),
        }
    }
}

fn factory_name_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        let host = rest.split('/').next().unwrap_or(rest);
        return format!("Factory · {host}");
    }
    "Factory".to_string()
}

fn normalize_openclaw_base_url(base: &str, api_kind: &str) -> String {
    let mut url = base.trim().trim_end_matches('/').to_string();
    // CSP openai-custom expects OpenAI-compatible /v1 roots.
    let needs_v1 = !url.ends_with("/v1")
        && (api_kind.contains("openai")
            || api_kind == "ollama"
            || api_kind.is_empty());
    if needs_v1 {
        url.push_str("/v1");
    }
    url
}

pub(crate) fn preview_snippet(
    path: &Path,
    name: &str,
    kind: EditorLlmSourceKind,
) -> Result<Value, String> {
    match kind {
        EditorLlmSourceKind::Zed => {
            let root =
                read_jsonc(path)?.ok_or_else(|| format!("empty source: {}", path.display()))?;
            let cfg = root
                .pointer("/language_models/openai_compatible")
                .and_then(|v| v.as_object())
                .and_then(|o| o.get(name))
                .cloned()
                .ok_or_else(|| format!("provider not found: {name}"))?;
            Ok(cfg)
        }
        EditorLlmSourceKind::Continue
        | EditorLlmSourceKind::Factory
        | EditorLlmSourceKind::Cline
        | EditorLlmSourceKind::Aider
        | EditorLlmSourceKind::Codex
        | EditorLlmSourceKind::ClaudeCode
        | EditorLlmSourceKind::Cursor
        | EditorLlmSourceKind::Trae
        | EditorLlmSourceKind::QwenCode
        | EditorLlmSourceKind::IFlow
        | EditorLlmSourceKind::Crush => {
            let c = resolve_import(path, name, kind)?;
            Ok(json!({
                "name": c.name,
                "apiBase": c.api_url,
                "models": c.models,
            }))
        }
        EditorLlmSourceKind::OpenCode => {
            let root =
                read_jsonc(path)?.ok_or_else(|| format!("empty source: {}", path.display()))?;
            let block = root
                .get("provider")
                .or_else(|| root.get("providers"))
                .and_then(|v| v.as_object())
                .ok_or_else(|| format!("no provider block in {}", path.display()))?;
            let cfg = block
                .iter()
                .find(|(id, v)| {
                    v.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n == name)
                        .unwrap_or(false)
                        || *id == name
                })
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("provider not found: {name}"))?;
            Ok(redact_secret_fields(cfg))
        }
        EditorLlmSourceKind::OpenClaw => {
            let root =
                read_jsonc(path)?.ok_or_else(|| format!("empty source: {}", path.display()))?;
            let cfg = root
                .pointer("/models/providers")
                .or_else(|| root.get("providers"))
                .and_then(|v| v.as_object())
                .and_then(|o| o.get(name))
                .cloned()
                .ok_or_else(|| format!("provider not found: {name}"))?;
            Ok(redact_secret_fields(cfg))
        }
    }
}

pub(crate) fn resolve_import(
    path: &Path,
    name: &str,
    kind: EditorLlmSourceKind,
) -> Result<EditorLlmCandidate, String> {
    let candidates = scan_source(&EditorLlmSourceSpec {
        kind,
        path: path.to_path_buf(),
    })?;
    candidates
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("provider not found: {name}"))
}

fn redact_secret_fields(mut cfg: Value) -> Value {
    if let Some(opts) = cfg.get_mut("options").and_then(|v| v.as_object_mut()) {
        if opts.get("apiKey").and_then(|v| v.as_str()).is_some() {
            opts.insert("apiKey".to_string(), json!("***"));
        }
    }
    if let Some(obj) = cfg.as_object_mut() {
        if obj.get("apiKey").and_then(|v| v.as_str()).is_some() {
            obj.insert("apiKey".to_string(), json!("***"));
        }
    }
    cfg
}

fn sort_candidates(out: &mut [EditorLlmCandidate]) {
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

fn model_names_from_available(val: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(arr) = val.and_then(|v| v.as_array()) else {
        return out;
    };
    for m in arr {
        let mid = m
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| m.as_str())
            .unwrap_or("")
            .trim();
        if !mid.is_empty() {
            out.push(mid.to_string());
        }
    }
    out
}

fn str_field(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn continue_provider_default_base(provider: &str) -> Option<String> {
    match provider.to_lowercase().as_str() {
        "openai" => Some("https://api.openai.com/v1".to_string()),
        "ollama" => Some("http://localhost:11434/v1".to_string()),
        "openrouter" => Some("https://openrouter.ai/api/v1".to_string()),
        "deepseek" => Some("https://api.deepseek.com/v1".to_string()),
        "groq" => Some("https://api.groq.com/openai/v1".to_string()),
        "mistral" => Some("https://api.mistral.ai/v1".to_string()),
        _ => None,
    }
}

fn continue_extract_key(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(k) = str_field(obj, &["apiKey", "api_key"]) {
        if !is_secret_placeholder(&k) {
            return Some(k);
        }
    }
    if let Some(headers) = obj
        .get("requestOptions")
        .and_then(|v| v.get("headers"))
        .and_then(|v| v.as_object())
    {
        for key in ["Authorization", "X-Auth-Token", "x-api-key", "api-key"] {
            if let Some(v) = headers.get(key).and_then(|v| v.as_str()) {
                let t = v.trim();
                if t.is_empty() || is_secret_placeholder(t) {
                    continue;
                }
                if key == "Authorization" && t.to_lowercase().starts_with("bearer ") {
                    return Some(t[7..].trim().to_string());
                }
                return Some(t.to_string());
            }
        }
    }
    None
}

fn is_secret_placeholder(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t.contains("${{")
        || t.contains("${")
        || t.contains("{{")
        || t == "<YOUR_API_KEY>"
        || t.eq_ignore_ascii_case("your-api-key")
        || t.eq_ignore_ascii_case("OLLAMA_API_KEY")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_continue_yaml_groups_by_api_base() {
        let dir = std::env::temp_dir().join(format!("csp-continue-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.yaml");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            r#"
models:
  - name: Moonshot Chat
    provider: openai
    model: kimi-k2
    apiBase: https://api.moonshot.cn/v1
  - name: Moonshot Edit
    provider: openai
    model: kimi-k2-turbo
    apiBase: https://api.moonshot.cn/v1
"#
        )
        .unwrap();
        let found = scan_continue(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].models.len(), 2);
    }

    #[test]
    fn parse_openclaw_providers() {
        let dir = std::env::temp_dir().join(format!("csp-openclaw-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("openclaw.json");
        fs::write(
            &path,
            serde_json::json!({
                "agents": { "defaults": { "models": {
                    "ollama/minimax-m2.7:cloud": {},
                    "ollama/kimi-k2.5:cloud": {}
                }}},
                "models": {
                    "providers": {
                        "ollama": {
                            "api": "ollama",
                            "apiKey": "secret-key",
                            "baseUrl": "http://127.0.0.1:11434",
                            "models": []
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let found = scan_openclaw(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "ollama");
        assert!(found[0].api_url.ends_with("/v1"));
        assert_eq!(found[0].models.len(), 2);
        assert_eq!(found[0].api_key.as_deref(), Some("secret-key"));
    }

    #[test]
    fn openclaw_agent_snapshot_only_scanned_without_main_config() {
        let home = std::env::temp_dir().join(format!("csp-openclaw-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(".openclaw/agents/main/agent")).unwrap();
        fs::write(
            home.join(".openclaw/agents/main/agent/models.json"),
            "{\"providers\":{}}",
        )
        .unwrap();

        // No main config → the per-agent snapshot is the fallback source.
        let paths: Vec<String> = list_editor_llm_sources(&home)
            .into_iter()
            .map(|s| s.path.display().to_string())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("agents/main/agent/models.json")));

        // Main config present → it is authoritative; the snapshot is skipped.
        fs::write(home.join(".openclaw/openclaw.json"), "{}").unwrap();
        let paths: Vec<String> = list_editor_llm_sources(&home)
            .into_iter()
            .map(|s| s.path.display().to_string())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with(".openclaw/openclaw.json")));
        assert!(!paths.iter().any(|p| p.ends_with("agents/main/agent/models.json")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn parse_factory_groups_by_base_url() {
        let dir = std::env::temp_dir().join(format!("csp-factory-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            serde_json::json!({
                "customModels": [
                    {
                        "model": "deepseek-v4-pro",
                        "baseUrl": "https://api.deepseek.com",
                        "apiKey": "sk-test",
                        "displayName": "deepseek-v4-pro",
                        "provider": "generic-chat-completion-api"
                    },
                    {
                        "model": "deepseek-v4-flash",
                        "baseUrl": "https://api.deepseek.com/",
                        "apiKey": "sk-test",
                        "displayName": "deepseek-v4-flash",
                        "provider": "generic-chat-completion-api"
                    },
                    {
                        "model": "minimax-m2.7:cloud",
                        "baseUrl": "http://127.0.0.1:11434/v1",
                        "apiKey": "ollama",
                        "displayName": "minimax-m2.7:cloud",
                        "provider": "generic-chat-completion-api"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();
        let found = scan_factory(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 2);
        let deepseek = found.iter().find(|c| c.api_url.contains("deepseek")).unwrap();
        assert_eq!(deepseek.models.len(), 2);
        assert!(deepseek.api_key.is_some());
    }

    #[test]
    fn parse_claude_code_custom_endpoint() {
        let dir = std::env::temp_dir().join(format!("csp-claude-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            serde_json::json!({
                "env": {
                    "ANTHROPIC_BASE_URL": "https://open.bigmodel.cn/api/anthropic",
                    "ANTHROPIC_AUTH_TOKEN": "sk-live",
                    "ANTHROPIC_MODEL": "glm-5.2"
                },
                "model": "opus"
            })
            .to_string(),
        )
        .unwrap();
        let found = scan_claude_code(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_url, "https://open.bigmodel.cn/api/anthropic");
        assert_eq!(found[0].models, vec!["glm-5.2".to_string()]);
        assert_eq!(found[0].api_key.as_deref(), Some("sk-live"));
    }

    #[test]
    fn claude_code_default_backend_yields_nothing() {
        let dir = std::env::temp_dir().join(format!("csp-claude-def-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"model":"opus","env":{}}"#).unwrap();
        let found = scan_claude_code(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert!(found.is_empty());
    }

    fn write_vscdb(path: &Path, key: &str, value: &str) {
        use rusqlite::Connection;
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value BLOB)")
            .unwrap();
        conn.execute(
            "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )
        .unwrap();
    }

    #[test]
    fn parse_cursor_override_base_url() {
        let dir = std::env::temp_dir().join(format!("csp-cursor-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.vscdb");
        let store = serde_json::json!({
            "openAIBaseUrl": "https://open.bigmodel.cn/api/coding/paas/v4",
            "availableAPIKeyModels": ["glm-5.2", {"modelName": "glm-4.7"}],
            "useOpenAIKey": false
        })
        .to_string();
        write_vscdb(&path, CURSOR_STORE_KEY, &store);
        let found = scan_cursor(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_url, "https://open.bigmodel.cn/api/coding/paas/v4");
        assert_eq!(found[0].models, vec!["glm-5.2".to_string(), "glm-4.7".to_string()]);
        assert!(found[0].api_key.is_none());
        assert!(found[0].name.contains("bigmodel.cn"));
    }

    #[test]
    fn parse_trae_custom_models_merge_by_provider() {
        let dir = std::env::temp_dir().join(format!("csp-trae-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.vscdb");
        let store = serde_json::json!({
            "solo_coder": [
                {"name": "gpt-5.4", "is_preset": true, "provider": serde_json::Value::Null},
                {"name": "deepseek//deepseek-chat", "is_preset": false,
                 "provider": "deepseek", "base_url": "https://api.deepseek.com"},
                {"name": "deepseek//deepseek-reasoner", "is_preset": false,
                 "provider": "deepseek", "base_url": "https://api.deepseek.com"}
            ],
            "builder": [
                {"name": "deepseek//deepseek-chat", "is_preset": false,
                 "provider": "deepseek", "base_url": "https://api.deepseek.com"},
                {"name": "bigmodel-plan//glm-5.1", "is_preset": false,
                 "provider": "bigmodel-plan",
                 "base_url": "https://open.bigmodel.cn/api/coding/paas/v4"}
            ]
        })
        .to_string();
        write_vscdb(&path, "7561_AI.agent.model.model_list_map", &store);
        let found = scan_trae(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 2);
        let ds = found.iter().find(|c| c.name.contains("deepseek")).unwrap();
        // base without a versioned path gets /v1 appended
        assert_eq!(ds.api_url, "https://api.deepseek.com/v1");
        assert_eq!(ds.models.len(), 2);
        assert!(ds.models.contains(&"deepseek-chat".to_string()));
        let glm = found.iter().find(|c| c.name.contains("bigmodel")).unwrap();
        assert_eq!(glm.api_url, "https://open.bigmodel.cn/api/coding/paas/v4");
    }

    #[test]
    fn parse_qwen_model_providers() {
        let dir = std::env::temp_dir().join(format!("csp-qwen-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            serde_json::json!({
                "modelProviders": {
                    "openai": [
                        {"id": "kimi-k2", "name": "Moonshot",
                         "baseUrl": "https://api.moonshot.cn/v1", "envKey": "MOONSHOT_KEY"},
                        {"id": "kimi-k2-turbo", "name": "Moonshot",
                         "baseUrl": "https://api.moonshot.cn/v1", "envKey": "MOONSHOT_KEY"}
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();
        let found = scan_qwen(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_url, "https://api.moonshot.cn/v1");
        assert_eq!(found[0].models.len(), 2);
    }

    #[test]
    fn parse_iflow_flat() {
        let dir = std::env::temp_dir().join(format!("csp-iflow-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{"selectedAuthType":"iflow","apiKey":"sk-real","baseUrl":"https://apis.iflow.cn/v1","modelName":"Qwen3-Coder"}"#,
        )
        .unwrap();
        let found = scan_iflow(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_url, "https://apis.iflow.cn/v1");
        assert_eq!(found[0].models, vec!["Qwen3-Coder".to_string()]);
        assert_eq!(found[0].api_key.as_deref(), Some("sk-real"));
    }

    #[test]
    fn parse_crush_providers_with_env_var() {
        std::env::set_var("CSP_TEST_CRUSH_KEY", "sk-crush");
        let dir = std::env::temp_dir().join(format!("csp-crush-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crush.json");
        fs::write(
            &path,
            serde_json::json!({
                "providers": {
                    "openai": {"api_key": "$OPENAI_API_KEY"},
                    "deepseek": {
                        "type": "openai-compat",
                        "base_url": "https://api.deepseek.com/v1",
                        "api_key": "$CSP_TEST_CRUSH_KEY",
                        "models": [{"id": "deepseek-chat", "name": "V3"}]
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        let found = scan_crush(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("CSP_TEST_CRUSH_KEY");
        // Builtin openai (no base_url) skipped; only deepseek surfaces.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].api_url, "https://api.deepseek.com/v1");
        assert_eq!(found[0].models, vec!["deepseek-chat".to_string()]);
        assert_eq!(found[0].api_key.as_deref(), Some("sk-crush"));
    }

    #[test]
    fn ensure_openai_v1_rules() {
        assert_eq!(ensure_openai_v1("https://api.deepseek.com"), "https://api.deepseek.com/v1");
        assert_eq!(ensure_openai_v1("https://api.deepseek.com/"), "https://api.deepseek.com/v1");
        assert_eq!(
            ensure_openai_v1("https://open.bigmodel.cn/api/coding/paas/v4"),
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
        assert_eq!(ensure_openai_v1("https://x.com/v1"), "https://x.com/v1");
    }

    #[test]
    fn source_kind_from_path() {
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.openclaw/openclaw.json")),
            Some(EditorLlmSourceKind::OpenClaw)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new(
                "/home/u/.openclaw/agents/main/agent/models.json"
            )),
            Some(EditorLlmSourceKind::OpenClaw)
        );
        // Tencent QClaw wraps OpenClaw under ~/.qclaw with the same schema.
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.qclaw/openclaw.json")),
            Some(EditorLlmSourceKind::OpenClaw)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new(
                "/home/u/.qclaw/agents/main/agent/models.json"
            )),
            Some(EditorLlmSourceKind::OpenClaw)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new(
                "/Users/u/Library/Application Support/TRAE SOLO CN/User/globalStorage/state.vscdb"
            )),
            Some(EditorLlmSourceKind::Trae)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.factory/settings.json")),
            Some(EditorLlmSourceKind::Factory)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.cline/data/globalState.json")),
            Some(EditorLlmSourceKind::Cline)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.codex/config.toml")),
            Some(EditorLlmSourceKind::Codex)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.claude/settings.json")),
            Some(EditorLlmSourceKind::ClaudeCode)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new(
                "/Users/u/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
            )),
            Some(EditorLlmSourceKind::Cursor)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new(
                "/Users/u/Library/Application Support/Trae CN/User/globalStorage/state.vscdb"
            )),
            Some(EditorLlmSourceKind::Trae)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.qwen/settings.json")),
            Some(EditorLlmSourceKind::QwenCode)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.iflow/settings.json")),
            Some(EditorLlmSourceKind::IFlow)
        );
        assert_eq!(
            EditorLlmSourceKind::from_path(Path::new("/home/u/.config/crush/crush.json")),
            Some(EditorLlmSourceKind::Crush)
        );
    }
}
