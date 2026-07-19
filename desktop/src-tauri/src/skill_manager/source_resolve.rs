//! Resolve a Skill import source: local directory, local `.zip`, or `https` URL.
//!
//! Downloads and extraction stay in the CSP desktop app (not Claude Science).
//! Staged trees live under the system temp dir until import completes.

use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::model::InspectionResult;
use super::store::SkillStore;

const SKILL_FILE: &str = "SKILL.md";
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXTRACT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 10_000;
const MAX_PATH_DEPTH: usize = 32;
const MAX_PATH_LEN: usize = 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedSkillSource {
    /// Directory containing root `SKILL.md`.
    pub import_root: PathBuf,
    /// Original user input (path or URL) for inventory de-dupe / display.
    pub logical_source: String,
    pub source_kind: SourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Directory,
    Zip,
    Url,
}

/// Resolve `raw` into a directory tree ready for inspection/import.
pub fn resolve_skill_source(raw: &str) -> Result<ResolvedSkillSource, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("source is empty".to_string());
    }
    if is_http_url(trimmed) {
        let staging = staging_dir(trimmed);
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        fs::create_dir_all(&staging).map_err(|e| format!("create staging dir: {e}"))?;
        let root = resolve_url_to_skill_root(trimmed, &staging)?;
        return Ok(ResolvedSkillSource {
            import_root: root,
            logical_source: trimmed.to_string(),
            source_kind: SourceKind::Url,
        });
    }

    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(format!("path does not exist: {trimmed}"));
    }

    if path.is_file() {
        if !is_zip_file(&path) {
            return Err(format!(
                "not a zip archive: {} — provide a Skill folder, a .zip file, or an https URL",
                path.display()
            ));
        }
        let logical = fs::canonicalize(&path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| trimmed.to_string());
        let staging = staging_dir(&logical);
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        fs::create_dir_all(&staging).map_err(|e| format!("create staging dir: {e}"))?;
        let root = extract_zip_to_skill_root(&path, &staging)?;
        return Ok(ResolvedSkillSource {
            import_root: root,
            logical_source: logical,
            source_kind: SourceKind::Zip,
        });
    }

    if !path.is_dir() {
        return Err(format!("unsupported source: {trimmed}"));
    }

    let logical = fs::canonicalize(&path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| trimmed.to_string());
    Ok(ResolvedSkillSource {
        import_root: path,
        logical_source: logical,
        source_kind: SourceKind::Directory,
    })
}

/// Resolve then inspect; returns inspection plus import metadata for the UI.
pub fn inspect_resolved_source(
    raw: &str,
) -> Result<(InspectionResult, ResolvedSkillSource), String> {
    let resolved = resolve_skill_source(raw)?;
    let mut inspection = SkillStore::inspect_source(&resolved.import_root)?;
    match resolved.source_kind {
        SourceKind::Url => inspection
            .warnings
            .push("Downloaded and extracted from URL".to_string()),
        SourceKind::Zip => inspection
            .warnings
            .push("Extracted from zip archive".to_string()),
        SourceKind::Directory => {}
    }
    Ok((inspection, resolved))
}

/// Prefer a validated `import_path` from inspect; otherwise resolve `source` again.
pub fn resolve_for_import(
    import_path: Option<&str>,
    source: &str,
) -> Result<ResolvedSkillSource, String> {
    if let Some(path) = import_path.filter(|p| !p.trim().is_empty()) {
        let root = PathBuf::from(path);
        if root.is_dir() && root.join(SKILL_FILE).is_file() {
            let trimmed = source.trim();
            let kind = if is_http_url(trimmed) {
                SourceKind::Url
            } else {
                let src_path = PathBuf::from(trimmed);
                if src_path.is_file() && is_zip_file(&src_path) {
                    SourceKind::Zip
                } else {
                    SourceKind::Directory
                }
            };
            return Ok(ResolvedSkillSource {
                import_root: root,
                logical_source: trimmed.to_string(),
                source_kind: kind,
            });
        }
    }
    resolve_skill_source(source)
}

fn is_http_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("skill"))
        || file_has_zip_magic(path)
}

fn file_has_zip_magic(path: &Path) -> bool {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf).is_ok() && buf[0] == b'P' && buf[1] == b'K'
}

fn staging_dir(key: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let short = format!(
        "{:016x}",
        u64::from_be_bytes(digest[..8].try_into().unwrap())
    );
    std::env::temp_dir().join(format!("csp-skill-import-{short}"))
}

fn resolve_url_to_skill_root(raw: &str, staging: &Path) -> Result<PathBuf, String> {
    let (fetch_url, subpath) = plan_url_fetch(raw)?;
    let bytes = http_get_bytes(&fetch_url)?;
    if !has_zip_magic(&bytes) {
        return Err(
            "URL did not return a zip archive — use a direct .zip link or a public GitHub repo/tree URL"
                .to_string(),
        );
    }
    let zip_path = staging.join("download.zip");
    write_bytes_limited(&zip_path, &bytes, MAX_ARCHIVE_BYTES)?;
    let extracted = staging.join("extracted");
    extract_zip_tree(&zip_path, &extracted)?;
    let archive_root = single_top_level_dir(&extracted)?;
    let candidate = match subpath {
        Some(sub) => archive_root.join(sub),
        None => archive_root,
    };
    find_skill_root(&candidate)
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Claude-Science-Proxy/2.0.0")
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {} for {url}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| format!("read response: {e}"))?
        .to_vec();
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(format!(
            "archive too large: {} bytes (max {MAX_ARCHIVE_BYTES})",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn write_bytes_limited(path: &Path, data: &[u8], max: u64) -> Result<(), String> {
    if data.len() as u64 > max {
        return Err(format!(
            "archive too large: {} bytes (max {max})",
            data.len()
        ));
    }
    let mut f = File::create(path).map_err(|e| format!("write archive: {e}"))?;
    f.write_all(data)
        .map_err(|e| format!("write archive: {e}"))?;
    Ok(())
}

fn has_zip_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == b'P' && bytes[1] == b'K'
}

/// Returns (fetch_url, optional subpath within the archive's top-level folder).
fn plan_url_fetch(raw: &str) -> Result<(String, Option<String>), String> {
    if let Some((owner, repo, branch, subpath)) = parse_github_tree_url(raw) {
        let fetch = if branch.len() == 40 && branch.chars().all(|c| c.is_ascii_hexdigit()) {
            format!("https://codeload.github.com/{owner}/{repo}/zip/{branch}")
        } else {
            format!("https://codeload.github.com/{owner}/{repo}/zip/refs/heads/{branch}")
        };
        return Ok((fetch, subpath));
    }
    if !raw.starts_with("https://") {
        return Err("only https URLs are supported for remote Skill import".to_string());
    }
    Ok((raw.to_string(), None))
}

fn parse_github_tree_url(raw: &str) -> Option<(String, String, String, Option<String>)> {
    let without_scheme = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))?;
    let mut parts = without_scheme.split('/');
    if parts.next()? != "github.com" {
        return None;
    }
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    let next = parts.next();
    match next {
        None | Some("") => Some((owner, repo, "main".to_string(), None)),
        Some("tree") => {
            let branch = parts.next()?.to_string();
            let sub: String = parts.collect::<Vec<_>>().join("/");
            let subpath = if sub.is_empty() { None } else { Some(sub) };
            Some((owner, repo, branch, subpath))
        }
        Some("blob") => None,
        Some(_) => None,
    }
}

fn single_top_level_dir(extracted: &Path) -> Result<PathBuf, String> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(extracted).map_err(|e| format!("read extract dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        if entry.path().is_dir() {
            dirs.push(entry.path());
        }
    }
    match dirs.len() {
        0 => Ok(extracted.to_path_buf()),
        1 => Ok(dirs.remove(0)),
        _ => Err(
            "archive has multiple top-level folders — use a URL that points at one Skill directory"
                .to_string(),
        ),
    }
}

fn extract_zip_to_skill_root(zip_path: &Path, staging: &Path) -> Result<PathBuf, String> {
    let extracted = staging.join("extracted");
    extract_zip_tree(zip_path, &extracted)?;
    find_skill_root(&extracted)
}

fn extract_zip_tree(zip_path: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        let _ = fs::remove_dir_all(dest);
    }
    fs::create_dir_all(dest).map_err(|e| format!("create extract dir: {e}"))?;
    let file = File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(format!("too many zip entries (max {MAX_ZIP_ENTRIES})"));
    }
    let mut total_written = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry: {e}"))?;
        let name = entry.name().to_string();
        if name.contains("__MACOSX") || name.ends_with(".DS_Store") {
            continue;
        }
        let safe = sanitize_zip_entry_path(&name)?;
        let out_path = dest.join(&safe);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| format!("mkdir: {e}"))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
        }
        total_written += entry.size();
        if total_written > MAX_EXTRACT_BYTES {
            return Err(format!(
                "extracted content too large (max {MAX_EXTRACT_BYTES})"
            ));
        }
        let mut outfile = File::create(&out_path).map_err(|e| format!("create file: {e}"))?;
        copy(&mut entry, &mut outfile).map_err(|e| format!("extract file: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if entry.unix_mode().unwrap_or(0) & 0o111 != 0 {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(0o700));
            } else {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(0o600));
            }
        }
    }
    Ok(())
}

fn sanitize_zip_entry_path(name: &str) -> Result<PathBuf, String> {
    if name.contains('\\') || name.contains('\0') {
        return Err(format!("unsafe zip path: {name}"));
    }
    if name.len() > MAX_PATH_LEN {
        return Err(format!("zip path too long: {name}"));
    }
    let path = Path::new(name);
    let mut depth = 0usize;
    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => return Err(format!("zip path escapes root: {name}")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("zip path must be relative: {name}"))
            }
            Component::Normal(seg) => {
                depth += 1;
                if depth > MAX_PATH_DEPTH {
                    return Err(format!("zip path too deep: {name}"));
                }
                clean.push(seg);
            }
            Component::CurDir => {}
        }
    }
    if clean.as_os_str().is_empty() {
        return Err(format!("empty zip path: {name}"));
    }
    Ok(clean)
}

fn find_skill_root(base: &Path) -> Result<PathBuf, String> {
    if base.join(SKILL_FILE).is_file() {
        return Ok(base.to_path_buf());
    }
    let mut candidates = Vec::new();
    if base.is_dir() {
        for entry in fs::read_dir(base).map_err(|e| format!("read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("read entry: {e}"))?;
            let path = entry.path();
            if path.is_dir() && path.join(SKILL_FILE).is_file() {
                candidates.push(path);
            }
        }
    }
    match candidates.len() {
        0 => Err(format!(
            "no {SKILL_FILE} found — source must be a Skill folder with {SKILL_FILE} at its root"
        )),
        1 => Ok(candidates.remove(0)),
        _ => Err(
            "multiple Skill folders found — point at a single Skill directory or zip one Skill per archive"
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_test_zip(dir: &Path, skill_name: &str, body: &str) -> PathBuf {
        let zip_path = dir.join(format!("{skill_name}.zip"));
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let prefix = format!("{skill_name}/");
        zip.add_directory(format!("{prefix}"), opts).unwrap();
        zip.start_file(format!("{prefix}{SKILL_FILE}"), opts)
            .unwrap();
        zip.write_all(body.as_bytes()).unwrap();
        zip.finish().unwrap();
        zip_path
    }

    #[test]
    fn parse_github_repo_root() {
        let u = "https://github.com/acme/demo";
        let (owner, repo, branch, sub) = parse_github_tree_url(u).unwrap();
        assert_eq!(owner, "acme");
        assert_eq!(repo, "demo");
        assert_eq!(branch, "main");
        assert!(sub.is_none());
    }

    #[test]
    fn parse_github_tree_with_subpath() {
        let u = "https://github.com/acme/demo/tree/dev/skills/foo";
        let (owner, repo, branch, sub) = parse_github_tree_url(u).unwrap();
        assert_eq!(owner, "acme");
        assert_eq!(repo, "demo");
        assert_eq!(branch, "dev");
        assert_eq!(sub.as_deref(), Some("skills/foo"));
    }

    #[test]
    fn resolve_local_zip_extracts_skill_root() {
        let tmp =
            std::env::temp_dir().join(format!("csp-skill-resolve-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let zip = write_test_zip(&tmp, "demo-skill", "---\nname: Demo\n---\n# Demo\n");
        let resolved = resolve_skill_source(zip.to_str().unwrap()).unwrap();
        assert_eq!(resolved.source_kind, SourceKind::Zip);
        assert!(resolved.import_root.join(SKILL_FILE).is_file());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sanitize_zip_rejects_parent_dir() {
        assert!(sanitize_zip_entry_path("../etc/passwd").is_err());
    }
}
