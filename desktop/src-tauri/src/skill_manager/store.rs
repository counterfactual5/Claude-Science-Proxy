//! Skill store - manages persistent Skill storage in `~/.csp/skills/`.
//!
//! Skills are stored as immutable copies in the managed store. The store
//! tracks a JSON inventory file with metadata for quick UI listing.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{DiscoveredSkill, InspectionResult, Skill, SkillId, SkillSummary};

const STORE_DIR: &str = "skills";
const INVENTORY_FILE: &str = "inventory.json";
const SKILL_FILE: &str = "SKILL.md";
const MAX_SOURCE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
const MAX_FILE_COUNT: u32 = 500;

/// In-memory + on-disk inventory of imported Skills.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Inventory {
    skills: BTreeMap<String, Skill>,
}

pub struct SkillStore {
    root: PathBuf,
}

impl SkillStore {
    /// Open the Skill store at `~/.csp/skills/`. Creates it if missing.
    pub fn open() -> Result<Self, String> {
        let root = crate::config::default_dir().join(STORE_DIR);
        fs::create_dir_all(&root).map_err(|e| format!("create store dir: {e}"))?;
        Ok(Self { root })
    }

    fn inventory_path(&self) -> PathBuf {
        self.root.join(INVENTORY_FILE)
    }

    fn load_inventory(&self) -> Result<Inventory, String> {
        let path = self.inventory_path();
        if !path.exists() {
            return Ok(Inventory::default());
        }
        let data = fs::read(&path).map_err(|e| format!("read inventory: {e}"))?;
        let inv: Inventory =
            serde_json::from_slice(&data).map_err(|e| format!("parse inventory: {e}"))?;
        Ok(inv)
    }

    fn save_inventory(&self, inv: &Inventory) -> Result<(), String> {
        let path = self.inventory_path();
        let tmp = path.with_extension("json.tmp");
        let data =
            serde_json::to_vec_pretty(inv).map_err(|e| format!("serialize inventory: {e}"))?;
        fs::write(&tmp, &data).map_err(|e| format!("write inventory tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename inventory: {e}"))?;
        // Set 0600 perms
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            let _ = fs::set_permissions(&path, perms);
        }
        Ok(())
    }

    /// List all imported Skills as summaries.
    pub fn list(&self) -> Result<Vec<SkillSummary>, String> {
        let inv = self.load_inventory()?;
        Ok(inv.skills.values().map(SkillSummary::from).collect())
    }

    /// Get a full Skill by id.
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Result<Option<Skill>, String> {
        let inv = self.load_inventory()?;
        Ok(inv.skills.get(id).cloned())
    }

    /// Inspect a Skill source directory without importing. Returns metadata
    /// and a validation report so the UI can show a preview before import.
    pub fn inspect_source(source: &Path) -> Result<InspectionResult, String> {
        if !source.exists() {
            return Err(format!("source path does not exist: {}", source.display()));
        }
        if !source.is_dir() {
            return Err(format!("source is not a directory: {}", source.display()));
        }

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

        // Walk the source directory
        let entries = walk_dir(source, 0, MAX_FILE_COUNT, &mut result)?;
        result.file_count = entries.0;
        result.total_size_bytes = entries.1;

        if result.total_size_bytes > MAX_SOURCE_SIZE {
            result.errors.push(format!(
                "Source too large: {} bytes (max {})",
                result.total_size_bytes, MAX_SOURCE_SIZE
            ));
            return Ok(result);
        }

        // Look for SKILL.md
        let skill_md = source.join(SKILL_FILE);
        if skill_md.exists() {
            if let Ok(content) = fs::read_to_string(&skill_md) {
                parse_skill_md(&content, &mut result);
            }
        } else {
            result.warnings.push(format!(
                "No {} found - using directory name as Skill name",
                SKILL_FILE
            ));
            result.name = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled Skill")
                .to_string();
        }

        // Detect requirements from file names
        detect_requirements(source, &mut result.requirements);

        result.valid = result.errors.is_empty();
        Ok(result)
    }

    /// Import a Skill from a source directory. Copies contents to managed
    /// storage and records metadata in the inventory.
    pub fn import(&self, source: &Path) -> Result<Skill, String> {
        let inspection = Self::inspect_source(source)?;
        if !inspection.valid {
            return Err(format!(
                "Source validation failed: {}",
                inspection.errors.join("; ")
            ));
        }

        let mut inv = self.load_inventory()?;

        // De-dupe by source path: re-importing the same directory is treated as
        // an update — drop the previous copy so the inventory does not bloat.
        let canonical_src = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
        let existing_ids: Vec<String> = inv
            .skills
            .iter()
            .filter(|(_, s)| {
                let prev =
                    fs::canonicalize(&s.source_path).unwrap_or_else(|_| s.source_path.clone());
                prev == canonical_src
            })
            .map(|(id, _)| id.clone())
            .collect();
        for old_id in existing_ids {
            if let Some(old) = inv.skills.remove(&old_id) {
                if old.store_path.exists() {
                    let _ = fs::remove_dir_all(&old.store_path);
                }
            }
        }

        let id = SkillId::new();
        let dest = self.root.join(id.as_str());

        // Copy directory tree
        copy_dir(source, &dest)?;

        // Build Skill record
        let skill = Skill {
            id: id.clone(),
            name: inspection.name,
            description: inspection.description,
            store_path: dest,
            source_path: source.to_path_buf(),
            enabled: true,
            size_bytes: inspection.total_size_bytes,
            imported_at: current_iso8601(),
            requirements: inspection.requirements,
        };

        // Update inventory
        inv.skills.insert(id.to_string(), skill.clone());
        self.save_inventory(&inv)?;

        Ok(skill)
    }

    /// Set the enabled state of a Skill.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Skill, String> {
        let mut inv = self.load_inventory()?;
        let skill = inv
            .skills
            .get_mut(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;
        skill.enabled = enabled;
        let updated = skill.clone();
        self.save_inventory(&inv)?;
        Ok(updated)
    }

    /// Remove a Skill from the store and delete its files.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut inv = self.load_inventory()?;
        let skill = inv
            .skills
            .remove(id)
            .ok_or_else(|| format!("Skill not found: {}", id))?;

        if skill.store_path.exists() {
            fs::remove_dir_all(&skill.store_path).map_err(|e| format!("remove skill dir: {e}"))?;
        }
        self.save_inventory(&inv)?;
        Ok(())
    }

    /// Get the list of enabled Skills (for sandbox deployment).
    pub fn enabled_skills(&self) -> Result<Vec<Skill>, String> {
        let inv = self.load_inventory()?;
        Ok(inv.skills.values().filter(|s| s.enabled).cloned().collect())
    }

    /// Discover importable Skills under the given `(dir, label)` roots.
    ///
    /// Scans each root's immediate subdirectories for a `SKILL.md`, reads its
    /// name/description, and flags whether the source is already imported (by
    /// canonical path against the inventory). Missing roots are skipped silently.
    /// Results are de-duplicated by canonical source path and sorted by name.
    pub fn discover(&self, roots: &[(PathBuf, String)]) -> Result<Vec<DiscoveredSkill>, String> {
        let inv = self.load_inventory()?;
        // Canonical source paths already in the inventory.
        let imported: std::collections::BTreeSet<PathBuf> = inv
            .skills
            .values()
            .map(|s| fs::canonicalize(&s.source_path).unwrap_or_else(|_| s.source_path.clone()))
            .collect();

        let mut seen: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
        let mut out: Vec<DiscoveredSkill> = Vec::new();
        for (root, label) in roots {
            let entries = match fs::read_dir(root) {
                Ok(e) => e,
                Err(_) => continue, // missing/unreadable root → skip
            };
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() || !dir.join(SKILL_FILE).is_file() {
                    continue;
                }
                let canonical = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
                if !seen.insert(canonical.clone()) {
                    continue; // same dir reachable via two roots
                }
                let (name, description) = read_skill_meta(&dir);
                out.push(DiscoveredSkill {
                    name,
                    description,
                    source_path: dir.to_string_lossy().to_string(),
                    source_label: label.clone(),
                    already_imported: imported.contains(&canonical),
                });
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }
}

/// Read a Skill's name/description from `SKILL.md` front-matter, falling back to
/// the directory name when absent.
fn read_skill_meta(dir: &Path) -> (String, String) {
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
    if let Ok(content) = fs::read_to_string(dir.join(SKILL_FILE)) {
        parse_skill_md(&content, &mut result);
    }
    if result.name.is_empty() {
        result.name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled Skill")
            .to_string();
    }
    (result.name, result.description)
}

/// Walk a directory tree, counting files and total size. Enforces limits.
fn walk_dir(
    dir: &Path,
    depth: u32,
    max_files: u32,
    result: &mut InspectionResult,
) -> Result<(u32, u64), String> {
    if depth > 5 {
        result
            .warnings
            .push("Directory depth > 5, skipping nested".to_string());
        return Ok((0, 0));
    }
    let mut count = 0u32;
    let mut size = 0u64;
    for entry in fs::read_dir(dir).map_err(|e| format!("read dir: {e}"))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.is_file() {
            count += 1;
            size += metadata.len();
            if count > max_files {
                result
                    .errors
                    .push(format!("Too many files (> {})", max_files));
                return Ok((count, size));
            }
        } else if metadata.is_dir() {
            let (c, s) = walk_dir(&path, depth + 1, max_files, result)?;
            count += c;
            size += s;
        }
    }
    Ok((count, size))
}

/// Parse SKILL.md to extract name and description from front-matter.
fn parse_skill_md(content: &str, result: &mut InspectionResult) {
    // Look for YAML front-matter between --- markers
    if let Some(start) = content.find("---\n") {
        if let Some(end) = content[start + 4..].find("\n---") {
            let frontmatter = &content[start + 4..start + 4 + end];
            for line in frontmatter.lines() {
                if let Some(name) = line.strip_prefix("name:") {
                    result.name = name.trim().trim_matches('"').to_string();
                } else if let Some(desc) = line.strip_prefix("description:") {
                    result.description = desc.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    // Fallback: use first heading
    if result.name.is_empty() {
        for line in content.lines() {
            if let Some(heading) = line.strip_prefix("# ") {
                result.name = heading.trim().to_string();
                break;
            }
        }
    }
    if result.name.is_empty() {
        result.name = "Untitled Skill".to_string();
    }
}

/// Detect Skill requirements from file/folder names.
///
/// Uses exact filenames and exact extensions (not substring/`ends_with`) to
/// avoid false positives such as `colors.txt` matching an `rs`/`r` marker.
fn detect_requirements(source: &Path, requirements: &mut Vec<String>) {
    // Exact filename → requirement.
    let filename_markers: &[(&str, &str)] = &[
        ("requirements.txt", "python"),
        ("pyproject.toml", "python"),
        ("setup.py", "python"),
        ("environment.yml", "python"),
        ("package.json", "node"),
        ("Cargo.toml", "rust"),
    ];
    // Exact file extension → requirement (case-sensitive; `R` is a real R ext).
    let ext_markers: &[(&str, &str)] = &[
        ("py", "python"),
        ("ipynb", "python"),
        ("rs", "rust"),
        ("r", "r"),
        ("R", "r"),
    ];

    fn add(req: &str, requirements: &mut Vec<String>) {
        if !requirements.iter().any(|r| r == req) {
            requirements.push(req.to_string());
        }
    }

    for entry in walk_entries(source) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        for (marker, req) in filename_markers {
            if name == *marker {
                add(req, requirements);
            }
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            for (marker, req) in ext_markers {
                if ext == *marker {
                    add(req, requirements);
                }
            }
        }
    }
    // Network / MCP heuristics
    if source.join("mcp.json").exists() || source.join(".mcp").exists() {
        add("mcp", requirements);
    }
}

fn walk_entries(dir: &Path) -> Vec<std::fs::DirEntry> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            out.push(entry);
        }
    }
    out
}

/// Recursively copy a directory tree.
///
/// Symlinks are **skipped** (both file and directory links): following them would
/// let a crafted source directory pull arbitrary out-of-tree data into
/// `~/.csp/skills/` or the sandbox. `file_type()` uses `symlink_metadata`, so it
/// does not traverse links.
pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create dest dir: {e}"))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read src dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let file_type = entry.file_type().map_err(|e| format!("file type: {e}"))?;
        if file_type.is_symlink() {
            continue;
        }
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path).map_err(|e| format!("copy file: {e}"))?;
        }
    }
    Ok(())
}

/// Current UTC time as an RFC 3339 / ISO 8601 string (e.g. `2026-07-12T10:30:00Z`).
///
/// Dependency-free: converts epoch seconds to a civil date with Howard Hinnant's
/// `civil_from_days` algorithm, so the store stays free of a date crate.
pub(crate) fn current_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso8601_from_epoch_secs(secs)
}

/// Format epoch seconds (UTC) as `YYYY-MM-DDTHH:MM:SSZ`.
fn iso8601_from_epoch_secs(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, hh, mm, ss)
}

/// Convert days since the Unix epoch (1970-01-01) to a `(year, month, day)`
/// civil date. Based on Howard Hinnant's public-domain `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_store() -> SkillStore {
        let dir = env::temp_dir().join(format!("csp-skills-test-{}", rand_u64()));
        fs::create_dir_all(&dir).unwrap();
        SkillStore { root: dir }
    }

    fn rand_u64() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        // Combine epoch nanos with a process-wide atomic counter so parallel
        // tests never collide on a temp-dir name (a bare nanos clock can repeat
        // across threads and make one test delete another's store).
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        nanos.wrapping_mul(1_000_003).wrapping_add(n)
    }

    #[test]
    fn empty_store_lists_nothing() {
        let store = temp_store();
        let list = store.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = temp_store();
        let result = store.get("sk_nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn remove_nonexistent_errors() {
        let store = temp_store();
        let result = store.remove("sk_nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn set_enabled_nonexistent_errors() {
        let store = temp_store();
        let result = store.set_enabled("sk_nonexistent", true);
        assert!(result.is_err());
    }

    #[test]
    fn inspect_nonexistent_source_errors() {
        let result = SkillStore::inspect_source(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err());
    }

    #[test]
    fn import_and_list_roundtrip() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-src-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("SKILL.md"),
            "---\nname: Test Skill\ndescription: A test\n---\n# Test",
        )
        .unwrap();
        fs::write(src.join("hello.txt"), "hi").unwrap();

        let skill = store.import(&src).unwrap();
        assert_eq!(skill.name, "Test Skill");
        assert!(skill.enabled);
        assert!(skill.store_path.exists());

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);

        // Cleanup
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn set_enabled_toggles() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-src2-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: Toggle\n---\n# T").unwrap();

        let skill = store.import(&src).unwrap();
        let updated = store.set_enabled(&skill.id.to_string(), false).unwrap();
        assert!(!updated.enabled);

        // Cleanup
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn remove_cleans_files_and_inventory() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-src3-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: Removable\n---\n").unwrap();

        let skill = store.import(&src).unwrap();
        let store_path = skill.store_path.clone();

        store.remove(&skill.id.to_string()).unwrap();
        assert!(!store_path.exists());
        assert!(store.list().unwrap().is_empty());

        // Cleanup
        let _ = fs::remove_dir_all(&src);
    }

    #[test]
    fn iso8601_formats_known_epochs() {
        assert_eq!(iso8601_from_epoch_secs(0), "1970-01-01T00:00:00Z");
        // Well-known timestamp: 1_700_000_000 = 2023-11-14T22:13:20Z.
        assert_eq!(
            iso8601_from_epoch_secs(1_700_000_000),
            "2023-11-14T22:13:20Z"
        );
        // End-of-year boundary to exercise the civil conversion.
        assert_eq!(
            iso8601_from_epoch_secs(1_609_459_199),
            "2020-12-31T23:59:59Z"
        );
    }

    #[test]
    fn import_dedupes_same_source() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-dedup-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: Dupe\n---\n").unwrap();

        let first = store.import(&src).unwrap();
        let second = store.import(&src).unwrap();

        // Re-import replaces the previous copy, not append.
        assert_ne!(first.id.to_string(), second.id.to_string());
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(!first.store_path.exists());
        assert!(second.store_path.exists());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn detect_requirements_is_precise() {
        let src = env::temp_dir().join(format!("csp-skill-reqs-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        // Decoys that must NOT trigger rust/r via substring matching.
        fs::write(src.join("colors.txt"), "x").unwrap();
        fs::write(src.join("notes.md"), "x").unwrap();
        // Real markers.
        fs::write(src.join("requirements.txt"), "x").unwrap();
        fs::write(src.join("analysis.py"), "x").unwrap();
        fs::write(src.join("lib.rs"), "x").unwrap();

        let mut reqs = Vec::new();
        detect_requirements(&src, &mut reqs);
        assert!(reqs.contains(&"python".to_string()));
        assert!(reqs.contains(&"rust".to_string()));
        assert!(
            !reqs.contains(&"r".to_string()),
            "colors.txt must not imply R"
        );

        let _ = fs::remove_dir_all(&src);
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_skips_symlinks() {
        use std::os::unix::fs::symlink;
        let base = env::temp_dir().join(format!("csp-symlink-{}", rand_u64()));
        let src = base.join("src");
        let outside = base.join("outside");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&outside).unwrap();
        // A real file plus a symlink pointing outside the source tree.
        fs::write(src.join("real.txt"), "ok").unwrap();
        fs::write(outside.join("secret.txt"), "leak").unwrap();
        symlink(outside.join("secret.txt"), src.join("link.txt")).unwrap();

        let dst = base.join("dst");
        copy_dir(&src, &dst).unwrap();
        assert!(dst.join("real.txt").is_file(), "real file copied");
        assert!(!dst.join("link.txt").exists(), "symlink not followed/copied");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn discover_finds_and_marks_imported() {
        let store = temp_store();
        // Two roots, each with one SKILL.md dir; plus a non-skill dir to ignore.
        let root_a = env::temp_dir().join(format!("csp-disc-a-{}", rand_u64()));
        let root_b = env::temp_dir().join(format!("csp-disc-b-{}", rand_u64()));
        let sk_a = root_a.join("alpha");
        let sk_b = root_b.join("beta");
        let junk = root_a.join("not-a-skill");
        fs::create_dir_all(&sk_a).unwrap();
        fs::create_dir_all(&sk_b).unwrap();
        fs::create_dir_all(&junk).unwrap();
        fs::write(sk_a.join("SKILL.md"), "---\nname: Alpha\ndescription: A\n---\n").unwrap();
        fs::write(sk_b.join("SKILL.md"), "---\nname: Beta\n---\n").unwrap();
        fs::write(junk.join("readme.txt"), "x").unwrap();

        // Import alpha so it should be flagged already_imported.
        store.import(&sk_a).unwrap();

        let roots = vec![
            (root_a.clone(), "~/a".to_string()),
            (root_b.clone(), "~/b".to_string()),
        ];
        let found = store.discover(&roots).unwrap();
        assert_eq!(found.len(), 2, "two SKILL.md dirs, junk ignored");
        let alpha = found.iter().find(|d| d.name == "Alpha").unwrap();
        let beta = found.iter().find(|d| d.name == "Beta").unwrap();
        assert!(alpha.already_imported, "alpha was imported");
        assert!(!beta.already_imported, "beta not imported");
        assert_eq!(alpha.description, "A");

        let _ = fs::remove_dir_all(&root_a);
        let _ = fs::remove_dir_all(&root_b);
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn discover_skips_missing_roots() {
        let store = temp_store();
        let missing = env::temp_dir().join(format!("csp-disc-missing-{}", rand_u64()));
        let found = store
            .discover(&[(missing, "~/nope".to_string())])
            .unwrap();
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn enabled_skills_filters_correctly() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-src4-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: E\n---\n").unwrap();

        let skill = store.import(&src).unwrap();
        store.set_enabled(&skill.id.to_string(), false).unwrap();

        let enabled = store.enabled_skills().unwrap();
        assert!(enabled.is_empty());

        store.set_enabled(&skill.id.to_string(), true).unwrap();
        let enabled = store.enabled_skills().unwrap();
        assert_eq!(enabled.len(), 1);

        let _ = fs::remove_dir_all(&src);
    }
}
