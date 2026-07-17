//! Sync CSP's Skill store with Claude Science's on-disk skill library.
//!
//! Science's Edit-skill UI and in-chat edits write to
//! `orgs/<org>/skills/<name>/` (the runtime library). Workspace drafts under
//! `workspaces/` are only a secondary path for brand-new unpublished files.
//! This module discovers:
//! - **harvest**: `.csp_managed` folders that differ from `~/.csp/skills/`
//! - **import**: unmanaged Science folders with `SKILL.md` not yet in inventory
//!   (skips known bundled scientific skill names)
//! - **workspace**: unpublished workspace drafts whose name is not yet imported

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::deploy::MANAGED_MARKER;
use super::model::{InspectionResult, SkillSummary};
use super::store::{parse_skill_md, SkillStore};
use super::workspace_ingress::{
    discover_workspace_skills, WorkspaceSkillCandidate, WorkspaceSkillPreview,
};

const SKILL_FILE: &str = "SKILL.md";

/// Science-bundled scientific skills — never offer as "import".
const SCIENCE_BUNDLED: &[&str] = &[
    "alphafold2",
    "boltz",
    "borzoi",
    "chai1",
    "compute-env-setup",
    "customize",
    "diffdock",
    "esmfold2",
    "evo2",
    "fair-esm2",
    "figure-composer",
    "figure-style",
    "indication-dossier",
    "ligandmpnn",
    "literature-review",
    "managed-model-endpoints",
    "openfold3",
    "paper-narrative",
    "pdf-explore",
    "product-self-knowledge",
    "proteinmpnn",
    "remote-compute-modal",
    "remote-compute-ssh",
    "scgpt",
    "scvi-tools",
    "self-awareness",
    "skill-creator",
    "solublempnn",
    "using-model-endpoint",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScienceSkillSyncCandidate {
    pub key: String,
    /// `harvest` | `import` | `workspace`
    pub kind: String,
    pub name: String,
    pub description: String,
    pub skill_id: Option<String>,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    pub store_bytes: Option<u64>,
    pub science_bytes: Option<u64>,
    pub already_imported: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncScienceSkillsResult {
    pub synced: Vec<SkillSummary>,
    pub failures: Vec<String>,
    pub needs_restart: bool,
}

fn science_auth_dir() -> PathBuf {
    crate::runtime::science::sandbox_home().join(".claude-science")
}

fn read_org_uuid(auth_dir: &Path) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct ActiveOrg {
        org_uuid: String,
    }
    let v: ActiveOrg =
        serde_json::from_str(&fs::read_to_string(auth_dir.join("active-org.json")).ok()?).ok()?;
    let org = v.org_uuid;
    if org.len() == 36 && org.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        Some(org)
    } else {
        None
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn dir_fingerprint(root: &Path) -> Result<(u64, String), String> {
    let mut total = 0u64;
    let mut parts: Vec<String> = Vec::new();
    collect_fp(root, root, &mut total, &mut parts)?;
    parts.sort();
    Ok((total, parts.join("\n")))
}

fn collect_fp(
    root: &Path,
    dir: &Path,
    total: &mut u64,
    parts: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("readdir: {e}"))?;
        let path = entry.path();
        if is_symlink(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("meta: {e}"))?;
        if meta.is_dir() {
            collect_fp(root, &path, total, parts)?;
        } else if meta.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let len = meta.len();
            *total = total.saturating_add(len);
            parts.push(format!("{rel}:{len}"));
        }
    }
    Ok(())
}

fn list_rel_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut walk = |dir: &Path| {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if is_symlink(&path) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                // shallow: only top-level companions for the UI list
                continue;
            }
            if path.is_file() {
                out.push(name);
            }
        }
    };
    walk(root);
    out.sort();
    out
}

fn skill_meta(skill_md: &Path) -> (String, String) {
    let Ok(text) = fs::read_to_string(skill_md) else {
        return (String::new(), String::new());
    };
    let mut meta = InspectionResult {
        valid: false,
        name: String::new(),
        description: String::new(),
        file_count: 0,
        total_size_bytes: 0,
        requirements: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
        ..Default::default()
    };
    parse_skill_md(&text, &mut meta);
    (meta.name, meta.description)
}

/// Discover sync candidates: harvest drift, new library imports, workspace drafts.
pub fn discover_science_skill_sync() -> Result<Vec<ScienceSkillSyncCandidate>, String> {
    let store = SkillStore::open()?;
    let auth_dir = science_auth_dir();
    let org_uuid = read_org_uuid(&auth_dir).ok_or_else(|| {
        "active-org.json missing or invalid; start Science once first".to_string()
    })?;
    let skills_dir = auth_dir.join("orgs").join(&org_uuid).join("skills");

    let inventory = store.list()?;
    let mut by_name: BTreeMap<String, super::model::Skill> = BTreeMap::new();
    for s in &inventory {
        if let Ok(Some(full)) = store.get(&s.id) {
            by_name.insert(full.name.to_lowercase(), full);
        }
    }
    let imported_names: BTreeSet<String> = by_name.keys().cloned().collect();

    let mut out: Vec<ScienceSkillSyncCandidate> = Vec::new();

    if skills_dir.is_dir() && !is_symlink(&skills_dir) {
        for entry in fs::read_dir(&skills_dir).map_err(|e| format!("read skills dir: {e}"))? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if !path.is_dir() || is_symlink(&path) {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().to_string();
            if folder.starts_with('.') {
                continue;
            }
            let skill_md = path.join(SKILL_FILE);
            if !skill_md.is_file() {
                continue;
            }
            let managed = path.join(MANAGED_MARKER).is_file();
            let (meta_name, description) = skill_meta(&skill_md);
            let name = if meta_name.is_empty() {
                folder.clone()
            } else {
                meta_name
            };
            let files = list_rel_files(&path);
            let (science_bytes, science_fp) = dir_fingerprint(&path).unwrap_or((0, String::new()));

            if managed {
                let Some(existing) = by_name.get(&name.to_lowercase()) else {
                    // Managed on disk but missing from inventory — treat as import.
                    out.push(ScienceSkillSyncCandidate {
                        key: format!("science://import/{folder}"),
                        kind: "import".into(),
                        name: name.clone(),
                        description,
                        skill_id: None,
                        files,
                        warnings: vec![
                            "Marked .csp_managed but missing from CSP inventory".into(),
                        ],
                        store_bytes: None,
                        science_bytes: Some(science_bytes),
                        already_imported: false,
                    });
                    continue;
                };
                let store_path = PathBuf::from(&existing.store_path);
                let (store_bytes, store_fp) = if store_path.is_dir() {
                    dir_fingerprint(&store_path).unwrap_or((0, String::new()))
                } else {
                    (0, String::new())
                };
                if store_fp != science_fp {
                    let mut warnings = Vec::new();
                    if science_bytes > store_bytes {
                        warnings.push("Science library is newer/larger than CSP store".into());
                    } else {
                        warnings.push("Science library differs from CSP store".into());
                    }
                    out.push(ScienceSkillSyncCandidate {
                        key: format!("science://harvest/{}", existing.id),
                        kind: "harvest".into(),
                        name: name.clone(),
                        description,
                        skill_id: Some(existing.id.to_string()),
                        files,
                        warnings,
                        store_bytes: Some(store_bytes),
                        science_bytes: Some(science_bytes),
                        already_imported: true,
                    });
                }
            } else {
                if SCIENCE_BUNDLED
                    .iter()
                    .any(|b| b.eq_ignore_ascii_case(&folder) || b.eq_ignore_ascii_case(&name))
                {
                    continue;
                }
                if imported_names.contains(&name.to_lowercase()) {
                    continue;
                }
                out.push(ScienceSkillSyncCandidate {
                    key: format!("science://import/{folder}"),
                    kind: "import".into(),
                    name,
                    description,
                    skill_id: None,
                    files,
                    warnings: vec!["Present in Science library but not in CSP store".into()],
                    store_bytes: None,
                    science_bytes: Some(science_bytes),
                    already_imported: false,
                });
            }
        }
    }

    // Workspace drafts: only names not already in inventory (or not already listed).
    let harvest_names: BTreeSet<String> = out.iter().map(|c| c.name.to_lowercase()).collect();
    match discover_workspace_skills() {
        Ok(drafts) => {
            for d in drafts {
                if imported_names.contains(&d.name.to_lowercase()) {
                    continue;
                }
                if harvest_names.contains(&d.name.to_lowercase()) {
                    continue;
                }
                out.push(workspace_to_sync(d));
            }
        }
        Err(_) => {
            // Org may exist without workspaces; library sync still works.
        }
    }

    out.sort_by(|a, b| {
        kind_rank(&a.kind)
            .cmp(&kind_rank(&b.kind))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "harvest" => 0,
        "import" => 1,
        _ => 2,
    }
}

fn workspace_to_sync(d: WorkspaceSkillCandidate) -> ScienceSkillSyncCandidate {
    ScienceSkillSyncCandidate {
        key: d.key,
        kind: "workspace".into(),
        name: d.name,
        description: d.description,
        skill_id: None,
        files: d.files,
        warnings: d.warnings,
        store_bytes: None,
        science_bytes: None,
        already_imported: d.already_imported,
    }
}

/// Apply selected sync keys (harvest / import / workspace adopt).
pub fn sync_science_skills(keys: &[String]) -> Result<SyncScienceSkillsResult, String> {
    let store = SkillStore::open()?;
    let auth_dir = science_auth_dir();
    let org_uuid = read_org_uuid(&auth_dir).ok_or_else(|| {
        "active-org.json missing or invalid; start Science once first".to_string()
    })?;
    let skills_dir = auth_dir.join("orgs").join(&org_uuid).join("skills");

    let mut synced = Vec::new();
    let mut failures = Vec::new();

    for key in keys {
        match sync_one(&store, &skills_dir, key) {
            Ok(s) => synced.push(SkillSummary::from(&s)),
            Err(e) => failures.push(format!("{key}: {e}")),
        }
    }

    Ok(SyncScienceSkillsResult {
        synced,
        failures,
        needs_restart: false,
    })
}

fn sync_one(
    store: &SkillStore,
    skills_dir: &Path,
    key: &str,
) -> Result<super::model::Skill, String> {
    if let Some(id) = key.strip_prefix("science://harvest/") {
        let skill = store
            .get(id)?
            .ok_or_else(|| format!("Skill not found: {id}"))?;
        let folder = sanitize_folder_name(&skill.name);
        let src = skills_dir.join(&folder);
        if !src.is_dir() {
            return Err(format!("Science skill folder missing: {folder}"));
        }
        return store.replace_from_source(id, &src);
    }
    if let Some(folder) = key.strip_prefix("science://import/") {
        if !valid_folder(folder) {
            return Err("invalid Science skill folder name".into());
        }
        let src = skills_dir.join(folder);
        if !src.is_dir() {
            return Err(format!("Science skill folder missing: {folder}"));
        }
        let logical = format!("science://library/{folder}");
        return store.import_with_logical_source(&src, Some(&logical));
    }
    if key.starts_with("workspace://") {
        let result = super::workspace_ingress::adopt_workspace_skills(&[key.to_string()])?;
        if let Some(summary) = result.adopted.into_iter().next() {
            return store
                .get(&summary.id)?
                .ok_or_else(|| "adopted skill missing from inventory".into());
        }
        if let Some(err) = result.failures.into_iter().next() {
            return Err(err);
        }
        return Err("workspace adopt produced no skill".into());
    }
    Err(format!("unknown sync key: {key}"))
}

fn sanitize_folder_name(name: &str) -> String {
    // Mirror deploy sanitization roughly: keep alnum, dash, underscore.
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out = "skill".into();
    }
    out
}

fn valid_folder(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name.len() <= 255
}

/// Preview a sync candidate (Science library or workspace draft).
pub fn preview_science_skill(
    key: &str,
    file: Option<&str>,
) -> Result<WorkspaceSkillPreview, String> {
    if key.starts_with("workspace://") {
        return super::workspace_ingress::preview_workspace_skill(key, file);
    }
    let auth_dir = science_auth_dir();
    let org_uuid = read_org_uuid(&auth_dir).ok_or_else(|| {
        "active-org.json missing or invalid; start Science once first".to_string()
    })?;
    let skills_dir = auth_dir.join("orgs").join(&org_uuid).join("skills");

    let (folder, skill_id, already) = if let Some(id) = key.strip_prefix("science://harvest/") {
        let store = SkillStore::open()?;
        let skill = store
            .get(id)?
            .ok_or_else(|| format!("Skill not found: {id}"))?;
        (sanitize_folder_name(&skill.name), Some(id.to_string()), true)
    } else if let Some(folder) = key.strip_prefix("science://import/") {
        if !valid_folder(folder) {
            return Err("invalid folder".into());
        }
        (folder.to_string(), None, false)
    } else {
        return Err(format!("unknown preview key: {key}"));
    };

    let root = skills_dir.join(&folder);
    if !root.is_dir() {
        return Err(format!("Science skill folder missing: {folder}"));
    }
    let files = list_rel_files(&root)
        .into_iter()
        .map(|name| {
            let size = root.join(&name).metadata().map(|m| m.len()).unwrap_or(0);
            super::workspace_ingress::WorkspaceSkillPreviewFile {
                name,
                size_bytes: size,
            }
        })
        .collect::<Vec<_>>();
    let active = file
        .map(str::to_string)
        .or_else(|| {
            files
                .iter()
                .find(|f| f.name == SKILL_FILE)
                .map(|f| f.name.clone())
        })
        .or_else(|| files.first().map(|f| f.name.clone()))
        .unwrap_or_else(|| SKILL_FILE.into());
    if active.contains("..") || active.contains('/') || active.contains('\\') {
        return Err("invalid preview file".into());
    }
    let path = root.join(&active);
    let raw = fs::read_to_string(&path).map_err(|e| format!("read preview: {e}"))?;
    let char_count = raw.chars().count();
    let truncated = char_count > 200_000;
    let content: String = if truncated {
        raw.chars().take(200_000).collect()
    } else {
        raw
    };
    let (name, description) = skill_meta(&root.join(SKILL_FILE));
    Ok(WorkspaceSkillPreview {
        key: key.to_string(),
        name: if name.is_empty() { folder } else { name },
        description,
        workspace_id: skill_id.unwrap_or_default(),
        already_imported: already,
        open_path: root.display().to_string(),
        files,
        active_file: active,
        content,
        truncated,
        char_count,
    })
}

pub fn open_science_skill(key: &str) -> Result<String, String> {
    if key.starts_with("workspace://") {
        return super::workspace_ingress::open_workspace_skill(key);
    }
    let preview = preview_science_skill(key, None)?;
    let path = PathBuf::from(&preview.open_path);
    crate::runtime::system::reveal_path_in_finder(&path)?;
    Ok(preview.open_path)
}

/// If a managed Science folder differs from the CSP store copy, harvest it
/// before deploy would wipe it. Returns number of skills harvested.
pub fn harvest_drift_before_deploy(skills_dir: &Path) -> Result<usize, String> {
    if !skills_dir.is_dir() {
        return Ok(0);
    }
    let store = SkillStore::open()?;
    let inventory = store.list()?;
    let mut by_name: BTreeMap<String, super::model::Skill> = BTreeMap::new();
    for s in &inventory {
        if let Ok(Some(full)) = store.get(&s.id) {
            by_name.insert(full.name.to_lowercase(), full);
        }
    }
    let mut n = 0usize;
    for entry in fs::read_dir(skills_dir).map_err(|e| format!("read skills: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() || is_symlink(&path) || !path.join(MANAGED_MARKER).is_file() {
            continue;
        }
        let (meta_name, _) = skill_meta(&path.join(SKILL_FILE));
        let folder = entry.file_name().to_string_lossy().to_string();
        let name = if meta_name.is_empty() {
            folder.clone()
        } else {
            meta_name
        };
        let Some(existing) = by_name.get(&name.to_lowercase()) else {
            continue;
        };
        let store_path = PathBuf::from(&existing.store_path);
        let Ok((_, science_fp)) = dir_fingerprint(&path) else {
            continue;
        };
        let store_fp = if store_path.is_dir() {
            dir_fingerprint(&store_path)
                .map(|(_, f)| f)
                .unwrap_or_default()
        } else {
            String::new()
        };
        if science_fp != store_fp {
            store.replace_from_source(existing.id.as_str(), &path)?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_list_includes_common_science_skills() {
        assert!(SCIENCE_BUNDLED.contains(&"alphafold2"));
        assert!(SCIENCE_BUNDLED.contains(&"boltz"));
        assert!(SCIENCE_BUNDLED.contains(&"using-model-endpoint"));
    }

    #[test]
    fn sanitize_folder_keeps_simple_names() {
        assert_eq!(sanitize_folder_name("crypto-data-pro"), "crypto-data-pro");
    }
}
