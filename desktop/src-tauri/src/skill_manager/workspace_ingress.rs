//! Adopt Skill drafts that Claude Science writes under its workspace directory.
//!
//! Science cannot call `host.skills.edit()` under CSP virtual login, so it leaves
//! `*.skill.md` / companion files in `orgs/<org>/workspaces/<id>/`. CSP scans
//! those paths and imports selected drafts into `~/.csp/skills/` on user request.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{InspectionResult, Skill, SkillSummary};
use super::store::{parse_skill_md, SkillStore};

const MAX_WORKSPACES: usize = 256;
const MAX_CANDIDATES: usize = 256;
const MAX_SKILL_MD_SIZE: u64 = 1024 * 1024;
const SKILL_FILE: &str = "SKILL.md";

/// A Skill draft discovered under a Science workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSkillCandidate {
    /// Stable key for adopt (`workspace://…`).
    pub key: String,
    pub name: String,
    pub description: String,
    pub workspace_id: String,
    /// Relative paths bundled with this candidate (e.g. `SKILL.md`, `kernel.py`).
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    pub already_imported: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptWorkspaceSkillsResult {
    pub adopted: Vec<SkillSummary>,
    pub failures: Vec<String>,
    /// Sandbox was stopped so the user can restart with the new Skills applied.
    pub needs_restart: bool,
}

#[derive(Deserialize)]
struct ActiveOrg {
    org_uuid: String,
}

enum CandidateKind {
    Dir { rel: String },
    File { filename: String },
}

/// Scan the active org's Science workspaces for adoptable Skill drafts.
pub fn discover_workspace_skills() -> Result<Vec<WorkspaceSkillCandidate>, String> {
    let auth_dir = science_auth_dir();
    let Some(org_uuid) = read_org_uuid(&auth_dir) else {
        return Ok(Vec::new());
    };
    let workspaces_dir = auth_dir
        .join("orgs")
        .join(&org_uuid)
        .join("workspaces");
    if !workspaces_dir.is_dir() {
        return Ok(Vec::new());
    }

    let store = SkillStore::open()?;
    let imported_keys = workspace_source_keys(&store)?;

    let mut out = Vec::new();
    let workspace_entries = fs::read_dir(&workspaces_dir)
        .map_err(|e| format!("read workspaces dir: {e}"))?;
    let mut workspace_count = 0usize;
    for ws_entry in workspace_entries.flatten() {
        if workspace_count >= MAX_WORKSPACES {
            return Err("Science workspace count exceeds 256 limit".to_string());
        }
        let ws_path = ws_entry.path();
        if !ws_path.is_dir() || is_symlink(&ws_path) {
            continue;
        }
        let Some(workspace_id) = ws_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !valid_child_name(workspace_id) {
            continue;
        }
        workspace_count += 1;
        scan_workspace_dir(
            &ws_path,
            workspace_id,
            &org_uuid,
            &imported_keys,
            &mut out,
        )?;
        if out.len() > MAX_CANDIDATES {
            return Err("workspace Skill candidate count exceeds 256 limit".to_string());
        }
    }
    out.sort_by_key(|c| c.name.to_lowercase());
    Ok(out)
}

/// Stage and import one or more workspace drafts, then redeploy into the sandbox.
pub fn adopt_workspace_skills(keys: &[String]) -> Result<AdoptWorkspaceSkillsResult, String> {
    if keys.is_empty() {
        return Ok(AdoptWorkspaceSkillsResult {
            adopted: Vec::new(),
            failures: Vec::new(),
            needs_restart: false,
        });
    }
    let store = SkillStore::open()?;
    let auth_dir = science_auth_dir();
    let org_uuid = read_org_uuid(&auth_dir)
        .ok_or_else(|| "active-org.json missing or invalid; start Science once first".to_string())?;

    let mut adopted = Vec::new();
    let mut failures = Vec::new();
    for key in keys {
        match adopt_one(&store, &auth_dir, &org_uuid, key) {
            Ok(skill) => adopted.push(SkillSummary::from(&skill)),
            Err(e) => failures.push(format!("{key}: {e}")),
        }
    }

    let needs_restart = false;

    Ok(AdoptWorkspaceSkillsResult {
        adopted,
        failures,
        needs_restart,
    })
}

fn adopt_one(
    store: &SkillStore,
    auth_dir: &Path,
    org_uuid: &str,
    key: &str,
) -> Result<Skill, String> {
    let (workspace_id, kind) = parse_workspace_key(key, org_uuid)?;
    let ws_dir = auth_dir
        .join("orgs")
        .join(org_uuid)
        .join("workspaces")
        .join(&workspace_id);
    if !ws_dir.is_dir() || is_symlink(&ws_dir) {
        return Err("workspace directory not found".to_string());
    }

    let staging = stage_candidate(&ws_dir, &kind)?;
    let result = store.import_with_logical_source(&staging, Some(key));
    let _ = fs::remove_dir_all(&staging);
    result
}

fn stage_candidate(ws_dir: &Path, kind: &CandidateKind) -> Result<PathBuf, String> {
    let staging = PathBuf::from("/private/tmp").join(format!(
        "csp-workspace-skill-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir(&staging).map_err(|e| format!("create staging dir: {e}"))?;
    fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("chmod staging dir: {e}"))?;

    let stage_result = match kind {
        CandidateKind::Dir { rel } => {
            let source = if rel == "." {
                ws_dir.to_path_buf()
            } else {
                ws_dir.join(rel)
            };
            if !source.is_dir() || is_symlink(&source) {
                Err("skill directory not found".to_string())
            } else if !source.join(SKILL_FILE).is_file() {
                Err(format!("{SKILL_FILE} missing in skill directory"))
            } else {
                copy_tree_bounded(&source, &staging, 0)
            }
        }
        CandidateKind::File { filename } => {
            let doc = ws_dir.join(filename);
            if !doc.is_file() || is_symlink(&doc) {
                return Err("skill document not found".to_string());
            }
            let content = read_bounded_file(&doc, MAX_SKILL_MD_SIZE)?;
            write_staged_file(&staging, SKILL_FILE, &content)?;
            for companion in companion_files(ws_dir, filename)? {
                let src = ws_dir.join(&companion);
                if src.is_file() && !is_symlink(&src) {
                    let data = read_bounded_file(&src, MAX_SKILL_MD_SIZE)?;
                    write_staged_file(&staging, &companion, &data)?;
                }
            }
            Ok(())
        }
    };

    if stage_result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    stage_result?;
    Ok(staging)
}

fn scan_workspace_dir(
    ws_dir: &Path,
    workspace_id: &str,
    org_uuid: &str,
    imported_keys: &std::collections::BTreeSet<String>,
    out: &mut Vec<WorkspaceSkillCandidate>,
) -> Result<(), String> {
    // Directory candidates: workspace root or immediate subdirs with SKILL.md.
    if ws_dir.join(SKILL_FILE).is_file() && !is_symlink(&ws_dir.join(SKILL_FILE)) {
        push_dir_candidate(ws_dir, ".", workspace_id, org_uuid, imported_keys, out)?;
    }
    for entry in fs::read_dir(ws_dir).map_err(|e| format!("read workspace: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if p.is_dir() && !is_symlink(&p) && p.join(SKILL_FILE).is_file() {
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if valid_child_name(name) {
                push_dir_candidate(ws_dir, name, workspace_id, org_uuid, imported_keys, out)?;
            }
        }
    }

    // Single-file skill documents at workspace root.
    for entry in fs::read_dir(ws_dir).map_err(|e| format!("read workspace: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if !p.is_file() || is_symlink(&p) {
            continue;
        }
        let Some(filename) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if filename == SKILL_FILE {
            continue; // handled as directory candidate above
        }
        if !is_skill_doc_filename(filename) {
            continue;
        }
        push_file_candidate(ws_dir, filename, workspace_id, org_uuid, imported_keys, out)?;
    }
    Ok(())
}

fn push_dir_candidate(
    ws_dir: &Path,
    rel: &str,
    workspace_id: &str,
    org_uuid: &str,
    imported_keys: &std::collections::BTreeSet<String>,
    out: &mut Vec<WorkspaceSkillCandidate>,
) -> Result<(), String> {
    let source = if rel == "." {
        ws_dir.to_path_buf()
    } else {
        ws_dir.join(rel)
    };
    let skill_md = source.join(SKILL_FILE);
    let content = read_bounded_file(&skill_md, MAX_SKILL_MD_SIZE)?;
    let (name, description) = skill_meta_from_bytes(&content);
    let key = workspace_key(org_uuid, workspace_id, &CandidateKind::Dir { rel: rel.to_string() });
    let mut files = vec![SKILL_FILE.to_string()];
    collect_relative_files(&source, &source, &mut files)?;
    let warnings = companion_warnings_for_files(&files);
    out.push(WorkspaceSkillCandidate {
        key: key.clone(),
        name,
        description,
        workspace_id: workspace_id.to_string(),
        files,
        warnings,
        already_imported: imported_keys.contains(&key),
    });
    Ok(())
}

fn push_file_candidate(
    ws_dir: &Path,
    filename: &str,
    workspace_id: &str,
    org_uuid: &str,
    imported_keys: &std::collections::BTreeSet<String>,
    out: &mut Vec<WorkspaceSkillCandidate>,
) -> Result<(), String> {
    let doc = ws_dir.join(filename);
    let content = read_bounded_file(&doc, MAX_SKILL_MD_SIZE)?;
    let (name, description) = skill_meta_from_bytes(&content);
    let key = workspace_key(
        org_uuid,
        workspace_id,
        &CandidateKind::File {
            filename: filename.to_string(),
        },
    );
    let mut files = vec![format!("{filename} → {SKILL_FILE}")];
    for c in companion_files(ws_dir, filename)? {
        files.push(c);
    }
    let warnings = companion_warnings_for_files(&files);
    out.push(WorkspaceSkillCandidate {
        key,
        name,
        description,
        workspace_id: workspace_id.to_string(),
        files,
        warnings,
        already_imported: imported_keys.contains(&workspace_key(
            org_uuid,
            workspace_id,
            &CandidateKind::File {
                filename: filename.to_string(),
            },
        )),
    });
    Ok(())
}

fn companion_files(ws_dir: &Path, doc_filename: &str) -> Result<Vec<String>, String> {
    let base = skill_doc_base(doc_filename);
    let mut picked = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    let mut consider = |name: &str| -> Result<(), String> {
        if !seen.insert(name.to_string()) {
            return Ok(());
        }
        let p = ws_dir.join(name);
        if p.is_file() && !is_symlink(&p) {
            picked.push(name.to_string());
        }
        Ok(())
    };

    for fixed in [
        "kernel.py",
        "crypto_kernel.py",
        "requirements.txt",
        "test_report.md",
    ] {
        consider(fixed)?;
    }

    for entry in fs::read_dir(ws_dir).map_err(|e| format!("read workspace: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if !p.is_file() || is_symlink(&p) {
            continue;
        }
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == doc_filename || name == SKILL_FILE || is_skill_doc_filename(name) {
            continue;
        }
        if name.starts_with("demo_") && name.ends_with(".py") {
            consider(name)?;
            continue;
        }
        if name.ends_with(".py") {
            if name.starts_with(&base) {
                consider(name)?;
                continue;
            }
            if let Some(prefix) = base.split('-').next() {
                if !prefix.is_empty() && name.starts_with(prefix) {
                    consider(name)?;
                }
            }
        }
    }
    picked.sort();
    Ok(picked)
}

fn companion_warnings_for_files(files: &[String]) -> Vec<String> {
    let has_py = files.iter().any(|f| f.ends_with(".py"));
    let mut warnings = Vec::new();
    if !has_py {
        warnings.push("未发现 .py 伴随文件；导入后可能只有文档、无可执行 kernel".to_string());
    }
    warnings
}

fn collect_relative_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read dir: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if is_symlink(&p) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("metadata: {e}"))?;
        if meta.is_dir() {
            if dir == root {
                // Only one level of nesting under a skill directory.
                collect_relative_files(root, &p, files)?;
            }
            continue;
        }
        if meta.is_file() {
            let rel = p
                .strip_prefix(root)
                .map_err(|_| "path strip failed".to_string())?
                .to_string_lossy()
                .to_string();
            files.push(rel);
        }
    }
    files.sort();
    Ok(())
}

fn copy_tree_bounded(src: &Path, dst: &Path, depth: u32) -> Result<(), String> {
    if depth > 5 {
        return Err("skill directory nesting too deep".to_string());
    }
    fs::create_dir_all(dst).map_err(|e| format!("create dir: {e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let ft = entry.file_type().map_err(|e| format!("file type: {e}"))?;
        if ft.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let from = entry.path();
        let to = dst.join(&name);
        if ft.is_dir() {
            copy_tree_bounded(&from, &to, depth + 1)?;
        } else if ft.is_file() {
            let len = entry.metadata().map_err(|e| format!("metadata: {e}"))?.len();
            if len > MAX_SKILL_MD_SIZE {
                return Err(format!("file too large: {}", from.display()));
            }
            fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn write_staged_file(dir: &Path, name: &str, content: &[u8]) -> Result<(), String> {
    if !valid_child_name(name) {
        return Err(format!("invalid staged filename: {name}"));
    }
    let path = dir.join(name);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("create staged file: {e}"))?;
    f.write_all(content)
        .map_err(|e| format!("write staged file: {e}"))?;
    f.sync_all().map_err(|e| format!("sync staged file: {e}"))?;
    Ok(())
}

fn read_bounded_file(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("metadata {}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }
    if meta.len() > max {
        return Err(format!("file too large: {}", path.display()));
    }
    fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))
}

fn skill_meta_from_bytes(content: &[u8]) -> (String, String) {
    let text = String::from_utf8_lossy(content);
    let mut result = InspectionResult {
        valid: false,
        name: String::new(),
        description: String::new(),
        file_count: 0,
        total_size_bytes: 0,
        requirements: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };
    parse_skill_md(&text, &mut result);
    if result.name.is_empty() {
        result.name = "Untitled Skill".to_string();
    }
    (result.name, result.description)
}

fn is_skill_doc_filename(name: &str) -> bool {
    name.ends_with(".skill.md")
        || (name.ends_with("_SKILL.md") && name != "_SKILL.md")
        || (name.ends_with("SKILL.md") && name != SKILL_FILE)
}

fn skill_doc_base(filename: &str) -> String {
    if let Some(stripped) = filename.strip_suffix(".skill.md") {
        return stripped.to_string();
    }
    if let Some(stripped) = filename.strip_suffix("_SKILL.md") {
        return stripped.to_string();
    }
    if let Some(stripped) = filename.strip_suffix("SKILL.md") {
        return stripped.to_string();
    }
    filename.to_string()
}

fn workspace_key(org_uuid: &str, workspace_id: &str, kind: &CandidateKind) -> String {
    match kind {
        CandidateKind::Dir { rel } => {
            format!("workspace://{org_uuid}/{workspace_id}/dir:{rel}")
        }
        CandidateKind::File { filename } => {
            format!("workspace://{org_uuid}/{workspace_id}/file:{filename}")
        }
    }
}

fn parse_workspace_key(key: &str, org_uuid: &str) -> Result<(String, CandidateKind), String> {
    let prefix = format!("workspace://{org_uuid}/");
    let rest = key
        .strip_prefix(&prefix)
        .ok_or_else(|| "invalid workspace skill key".to_string())?;
    let (workspace_id, kind_part) = rest
        .split_once('/')
        .ok_or_else(|| "invalid workspace skill key".to_string())?;
    if !valid_child_name(workspace_id) {
        return Err("invalid workspace id in key".to_string());
    }
    if let Some(rel) = kind_part.strip_prefix("dir:") {
        if rel.is_empty() || rel.contains('/') || rel.contains("..") {
            return Err("invalid directory key".to_string());
        }
        return Ok((
            workspace_id.to_string(),
            CandidateKind::Dir {
                rel: rel.to_string(),
            },
        ));
    }
    if let Some(filename) = kind_part.strip_prefix("file:") {
        if filename.is_empty() || !valid_child_name(filename) || !is_skill_doc_filename(filename) {
            return Err("invalid file key".to_string());
        }
        return Ok((
            workspace_id.to_string(),
            CandidateKind::File {
                filename: filename.to_string(),
            },
        ));
    }
    Err("invalid workspace skill key suffix".to_string())
}

fn workspace_source_keys(store: &SkillStore) -> Result<std::collections::BTreeSet<String>, String> {
    store.workspace_source_keys()
}

fn science_auth_dir() -> PathBuf {
    crate::runtime::science::sandbox_home().join(".claude-science")
}

fn read_org_uuid(auth_dir: &Path) -> Option<String> {
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

fn valid_child_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('.')
        && value.len() <= 255
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
}

fn unique_suffix() -> u128 {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SALT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    nanos.wrapping_add(SALT.fetch_add(1, Ordering::Relaxed) as u128)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_skill_md(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n"),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "probe: runs against the real ~/.csp sandbox"]
    fn probe_real_sandbox_discovery() {
        let found = discover_workspace_skills().unwrap();
        eprintln!("discovered {} candidate(s):", found.len());
        for c in &found {
            eprintln!(
                "  - {} [{}] ws={} files={:?} warnings={:?}",
                c.name, c.key, c.workspace_id, c.files, c.warnings
            );
        }
    }

    #[test]
    fn is_skill_doc_filename_matches_patterns() {
        assert!(is_skill_doc_filename("crypto-data.skill.md"));
        assert!(is_skill_doc_filename("crypto-data-v2_SKILL.md"));
        assert!(!is_skill_doc_filename("SKILL.md"));
    }

    #[test]
    fn companion_files_picks_kernel_and_demo() {
        let base = std::env::temp_dir().join(format!("csp-ws-comp-{}", unique_suffix()));
        fs::create_dir_all(&base).unwrap();
        fs::write(base.join("crypto-data-v2_SKILL.md"), b"---\nname: x\n---\n").unwrap();
        fs::write(base.join("kernel.py"), b"# k").unwrap();
        fs::write(base.join("demo_crypto.py"), b"# d").unwrap();
        let files = companion_files(&base, "crypto-data-v2_SKILL.md").unwrap();
        assert!(files.contains(&"kernel.py".to_string()));
        assert!(files.contains(&"demo_crypto.py".to_string()));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn parse_workspace_key_roundtrip() {
        let org = "cb5efa38-cc27-4c33-b141-ba973f10037a";
        let key = workspace_key(
            org,
            "ws1",
            &CandidateKind::File {
                filename: "foo.skill.md".to_string(),
            },
        );
        let (ws, kind) = parse_workspace_key(&key, org).unwrap();
        assert_eq!(ws, "ws1");
        match kind {
            CandidateKind::File { filename } => assert_eq!(filename, "foo.skill.md"),
            _ => panic!("expected file kind"),
        }
    }

    #[test]
    fn stage_file_candidate_writes_skill_md() {
        let ws = std::env::temp_dir().join(format!("csp-ws-stage-{}", unique_suffix()));
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("draft.skill.md"), b"---\nname: Draft\n---\n").unwrap();
        fs::write(ws.join("kernel.py"), b"pass").unwrap();
        let staging = stage_candidate(
            &ws,
            &CandidateKind::File {
                filename: "draft.skill.md".to_string(),
            },
        )
        .unwrap();
        assert!(staging.join("SKILL.md").is_file());
        assert!(staging.join("kernel.py").is_file());
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn copy_tree_skips_symlinks() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let base = std::env::temp_dir().join(format!("csp-ws-symlink-{}", unique_suffix()));
            let src = base.join("src");
            let outside = base.join("outside");
            fs::create_dir_all(&src).unwrap();
            fs::create_dir_all(&outside).unwrap();
            write_skill_md(&src, "SymlinkSkill");
            fs::write(outside.join("secret.txt"), b"no").unwrap();
            symlink(outside.join("secret.txt"), src.join("link.txt")).unwrap();
            let dst = base.join("dst");
            copy_tree_bounded(&src, &dst, 0).unwrap();
            assert!(dst.join("SKILL.md").is_file());
            assert!(!dst.join("link.txt").exists());
            let _ = fs::remove_dir_all(&base);
        }
    }
}
