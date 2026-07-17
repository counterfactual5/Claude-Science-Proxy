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
            let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.eq_ignore_ascii_case(SKILL_FILE) {
                // Acceptance is exact `SKILL.md` only; this hint covers common
                // casing when the user pointed at the markdown file itself.
                return Err(format!(
                    "path points to a Skill markdown file ({}); select the parent directory that contains {} instead",
                    source.display(),
                    SKILL_FILE
                ));
            }
            return Err(format!(
                "source is not a directory: {} — provide the Skill folder (the directory containing {})",
                source.display(),
                SKILL_FILE
            ));
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
            ..Default::default()
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

        // Require exact `SKILL.md` at the directory root (case-sensitive on disk).
        let skill_md = source.join(SKILL_FILE);
        if skill_md.is_file() {
            if let Ok(content) = fs::read_to_string(&skill_md) {
                parse_skill_md(&content, &mut result);
            }
        } else {
            result.errors.push(format!(
                "No {} found in directory root — the Skill folder must contain {}",
                SKILL_FILE, SKILL_FILE
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
        self.import_with_logical_source(source, None)
    }

    /// Import from `source`, recording `logical_source` in inventory when set.
    ///
    /// Workspace ingress uses keys like `workspace://<org>/<ws>/file:foo.skill.md`
    /// so re-adopting the same Science draft updates the existing Skill instead
    /// of appending duplicates (filesystem canonical paths are not stable for
    /// ephemeral staging dirs).
    pub fn import_with_logical_source(
        &self,
        source: &Path,
        logical_source: Option<&str>,
    ) -> Result<Skill, String> {
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
        let dedupe_key = logical_source
            .map(PathBuf::from)
            .or_else(|| fs::canonicalize(source).ok())
            .unwrap_or_else(|| source.to_path_buf());
        let existing_ids: Vec<String> = inv
            .skills
            .iter()
            .filter(|(_, s)| source_paths_equal(&s.source_path, &dedupe_key))
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
            source_path: dedupe_key,
            enabled: true,
            size_bytes: inspection.total_size_bytes,
            imported_at: current_iso8601(),
            requirements: inspection.requirements,
            builtin: false,
        };

        // Update inventory
        inv.skills.insert(id.to_string(), skill.clone());
        self.save_inventory(&inv)?;

        Ok(skill)
    }

    /// Replace the on-disk files of an existing Skill from `source`, keeping id /
    /// enabled / source_path. Used when harvesting Science-library edits back
    /// into `~/.csp/skills/`.
    pub fn replace_from_source(&self, skill_id: &str, source: &Path) -> Result<Skill, String> {
        let inspection = Self::inspect_source(source)?;
        if !inspection.valid {
            return Err(format!(
                "Source validation failed: {}",
                inspection.errors.join("; ")
            ));
        }
        let mut inv = self.load_inventory()?;
        let Some(existing) = inv.skills.get(skill_id).cloned() else {
            return Err(format!("Skill not found: {skill_id}"));
        };
        let dest = existing.store_path.clone();
        if dest.exists() {
            fs::remove_dir_all(&dest).map_err(|e| format!("clear skill dir: {e}"))?;
        }
        copy_dir(source, &dest)?;
        let skill = Skill {
            id: existing.id,
            name: inspection.name,
            description: inspection.description,
            store_path: dest,
            source_path: existing.source_path,
            enabled: existing.enabled,
            size_bytes: inspection.total_size_bytes,
            imported_at: current_iso8601(),
            requirements: inspection.requirements,
            builtin: existing.builtin,
        };
        inv.skills.insert(skill_id.to_string(), skill.clone());
        self.save_inventory(&inv)?;
        Ok(skill)
    }

    /// Author a brand-new Skill from a `SKILL.md` `content` string.
    ///
    /// The Skill's name/description come from the content's YAML front-matter —
    /// the body is the single source of truth (the UI keeps the name/description
    /// fields synced into the front-matter before calling this). Rejects an
    /// empty name or a name already present in the store, then stages the content
    /// into a temp dir and reuses [`import`](Self::import) so dedupe, inventory,
    /// copy, and requirement-detection logic are all shared.
    pub fn create_from_content(&self, content: &str) -> Result<Skill, String> {
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
        parse_skill_md(content, &mut meta);
        let name = meta.name.trim().to_string();
        // `parse_skill_md` falls back to "Untitled Skill" when neither a
        // front-matter `name:` nor a `# heading` is present — treat that (and an
        // empty name) as "no name given".
        if name.is_empty() || name == "Untitled Skill" {
            return Err("Skill 名称不能为空".to_string());
        }

        // Duplicate-name guard: unlike `import` (which dedupes by source path),
        // an authored Skill has no stable source, so guard on the display name.
        let inv = self.load_inventory()?;
        if inv.skills.values().any(|s| s.name == name) {
            return Err(format!("同名 Skill 已存在：{name}"));
        }
        drop(inv);

        // Stage the SKILL.md into a throwaway dir, then import it. The staging
        // dir name is sanitized purely for filesystem safety; the stored Skill
        // name is re-derived from the front-matter by `import`, so the dir name
        // never leaks into the inventory.
        let staging = std::env::temp_dir().join(format!(
            "csp-skill-new-{}",
            SkillId::new().as_str().trim_start_matches("sk_")
        ));
        fs::create_dir_all(&staging).map_err(|e| format!("create staging dir: {e}"))?;
        let write_result = fs::write(staging.join(SKILL_FILE), content.as_bytes())
            .map_err(|e| format!("write staged SKILL.md: {e}"));
        let result = write_result.and_then(|()| self.import(&staging));
        // Best-effort cleanup: the import already copied the content into the
        // managed store, so a leftover temp dir is harmless.
        let _ = fs::remove_dir_all(&staging);
        result
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

    /// Workspace ingress keys already present in the inventory.
    pub fn workspace_source_keys(&self) -> Result<std::collections::BTreeSet<String>, String> {
        let inv = self.load_inventory()?;
        Ok(inv
            .skills
            .values()
            .filter_map(|s| {
                let p = s.source_path.to_string_lossy();
                if p.starts_with("workspace://") {
                    Some(p.into_owned())
                } else {
                    None
                }
            })
            .collect())
    }

    /// Get the list of enabled Skills (for sandbox deployment).
    pub fn enabled_skills(&self) -> Result<Vec<Skill>, String> {
        let inv = self.load_inventory()?;
        Ok(inv.skills.values().filter(|s| s.enabled).cloned().collect())
    }

    /// Seed a CSP-managed built-in Skill into the inventory exactly once, guarded
    /// by a sentinel file so a user who later disables or removes it is never
    /// resurrected on the next launch. The bundled `files` (relative path,
    /// contents) are written into managed storage. No-op (returns `false`) if the
    /// sentinel exists or a Skill with the same `name` is already present.
    ///
    /// `sentinel` is a dotfile name under the store root (e.g.
    /// `.seeded-csp-environment`). Mirrors `McpStore::seed_once`.
    pub fn seed_once(
        &self,
        sentinel: &str,
        name: &str,
        description: &str,
        files: &[(&str, &str)],
        requirements: &[&str],
    ) -> Result<bool, String> {
        let marker = self.root.join(sentinel);
        if marker.exists() {
            return Ok(false);
        }
        let mut inv = self.load_inventory()?;
        let seeded = if inv.skills.values().any(|s| s.name == name) {
            false
        } else {
            let id = SkillId::new();
            let dest = self.root.join(id.as_str());
            let size_bytes = write_builtin_files(&dest, files)?;
            let skill = Skill {
                id: id.clone(),
                name: name.to_string(),
                description: description.to_string(),
                store_path: dest,
                source_path: PathBuf::from(format!("builtin://{name}")),
                enabled: true,
                size_bytes,
                imported_at: current_iso8601(),
                requirements: requirements.iter().map(|s| s.to_string()).collect(),
                builtin: true,
            };
            inv.skills.insert(id.to_string(), skill);
            self.save_inventory(&inv)?;
            true
        };
        // Stamp the sentinel regardless, so a name collision does not make us
        // retry (and resurrect) on every launch.
        let _ = fs::write(&marker, b"1\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&marker, fs::Permissions::from_mode(0o600));
        }
        Ok(seeded)
    }

    /// Refresh the on-disk content of an already-seeded built-in Skill so app
    /// upgrades propagate new bundled files. Only touches a Skill still present
    /// in the inventory (so a user removal stays removed) and preserves its
    /// `enabled` state. No-op (returns `false`) if the named built-in Skill is
    /// absent.
    pub fn refresh_builtin(
        &self,
        name: &str,
        description: &str,
        files: &[(&str, &str)],
    ) -> Result<bool, String> {
        let mut inv = self.load_inventory()?;
        let Some(skill) = inv
            .skills
            .values_mut()
            .find(|s| s.builtin && s.name == name)
        else {
            return Ok(false);
        };
        let dest = skill.store_path.clone();
        let size_bytes = write_builtin_files(&dest, files)?;
        skill.description = description.to_string();
        skill.size_bytes = size_bytes;
        self.save_inventory(&inv)?;
        Ok(true)
    }

    /// Stamp a seed sentinel without seeding. Used to carry sticky opt-out
    /// across a built-in skill rename.
    pub fn stamp_sentinel(&self, sentinel: &str) -> Result<(), String> {
        let marker = self.root.join(sentinel);
        if marker.exists() {
            return Ok(());
        }
        fs::write(&marker, b"1\n").map_err(|e| format!("write seed sentinel: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&marker, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Whether a sentinel dotfile already exists under the store root.
    pub fn sentinel_exists(&self, sentinel: &str) -> bool {
        self.root.join(sentinel).exists()
    }

    /// Remove the first inventory entry whose display `name` matches (and
    /// delete its `store_path`). Returns `true` if something was removed.
    pub fn remove_by_name(&self, name: &str) -> Result<bool, String> {
        let mut inv = self.load_inventory()?;
        let Some((id, skill)) = inv
            .skills
            .iter()
            .find(|(_, s)| s.name == name)
            .map(|(id, s)| (id.clone(), s.clone()))
        else {
            return Ok(false);
        };
        inv.skills.remove(&id);
        if skill.store_path.exists() {
            fs::remove_dir_all(&skill.store_path).map_err(|e| format!("remove skill dir: {e}"))?;
        }
        self.save_inventory(&inv)?;
        Ok(true)
    }

    /// Whether the inventory currently has a **builtin** Skill with `name`.
    pub fn has_builtin_named(&self, name: &str) -> Result<bool, String> {
        let inv = self.load_inventory()?;
        Ok(inv.skills.values().any(|s| s.builtin && s.name == name))
    }

    /// Seed / refresh the built-in environment handbook Skill, migrating from
    /// the legacy `csp-web-access` name when needed while respecting sticky
    /// opt-out:
    ///
    /// 1. If `sentinel` already exists → only `refresh_builtin` (normal path).
    /// 2. Else if inventory has a builtin named `legacy_name` → remove it, then
    ///    `seed_once` the new skill (stamps `sentinel`).
    /// 3. Else if `legacy_sentinel` exists but no `legacy_name` in inventory →
    ///    user opted out; stamp `sentinel` without seeding.
    /// 4. Else → `seed_once` as usual.
    pub fn seed_or_migrate_environment_skill(
        &self,
        sentinel: &str,
        name: &str,
        description: &str,
        files: &[(&str, &str)],
        requirements: &[&str],
        legacy_name: &str,
        legacy_sentinel: &str,
    ) -> Result<bool, String> {
        // Path 1: already migrated / seeded under the new name.
        if self.sentinel_exists(sentinel) {
            return self.refresh_builtin(name, description, files);
        }

        // Path 2: replace the legacy builtin install with the renamed skill.
        if self.has_builtin_named(legacy_name)? {
            let _ = self.remove_by_name(legacy_name)?;
            return self.seed_once(sentinel, name, description, files, requirements);
        }

        // Path 3: legacy sentinel present but skill gone → sticky opt-out.
        // Check inventory for the legacy name (builtin or not) so we don't
        // confuse a still-present non-builtin collision; seed_once below also
        // name-guards. Opt-out is specifically: sentinel set, inventory empty
        // of that name.
        let inv = self.load_inventory()?;
        let legacy_still_present = inv.skills.values().any(|s| s.name == legacy_name);
        if self.sentinel_exists(legacy_sentinel) && !legacy_still_present {
            self.stamp_sentinel(sentinel)?;
            return Ok(false);
        }

        // Path 4: first-time seed.
        let seeded = self.seed_once(sentinel, name, description, files, requirements)?;
        if seeded {
            // Already written by seed_once; refresh is a no-op but keeps parity
            // with the historical seed-then-refresh callers.
            let _ = self.refresh_builtin(name, description, files);
        }
        Ok(seeded)
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
        out.sort_by_key(|a| a.name.to_lowercase());
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
        ..Default::default()
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
pub(crate) fn parse_skill_md(content: &str, result: &mut InspectionResult) {
    // Look for YAML front-matter between --- markers
    if let Some(start) = content.find("---\n") {
        if let Some(end) = content[start + 4..].find("\n---") {
            let frontmatter = &content[start + 4..start + 4 + end];
            for line in frontmatter.lines() {
                if let Some(name) = line.strip_prefix("name:") {
                    result.name = name.trim().trim_matches('"').to_string();
                }
            }
            if let Some(description) = parse_frontmatter_string(frontmatter, "description") {
                result.description = description;
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

/// Parse a top-level YAML string field, including folded/literal block scalars.
///
/// Skill descriptions commonly use `description: >-` followed by indented
/// lines. This intentionally stays small rather than pretending to parse all of
/// YAML; it covers the plain/quoted inline form and the `>` / `|` block forms
/// used by SKILL.md front-matter.
fn parse_frontmatter_string(frontmatter: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let prefix = format!("{key}:");
    for (index, line) in lines.iter().enumerate() {
        let Some(raw_value) = line.strip_prefix(&prefix) else {
            continue;
        };
        let value = raw_value.trim();
        let Some(style) = value.chars().next().filter(|c| *c == '>' || *c == '|') else {
            return Some(value.trim_matches('"').trim_matches('\'').to_string());
        };
        if !value[style.len_utf8()..]
            .chars()
            .all(|c| matches!(c, '-' | '+' | '0'..='9'))
        {
            return Some(value.trim_matches('"').trim_matches('\'').to_string());
        }

        let block_lines: Vec<&str> = lines[index + 1..]
            .iter()
            .copied()
            .take_while(|line| line.trim().is_empty() || line.starts_with([' ', '\t']))
            .collect();
        let indent = block_lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        let deindented: Vec<&str> = block_lines
            .iter()
            .map(|line| {
                if line.trim().is_empty() {
                    ""
                } else {
                    &line[indent.min(line.len())..]
                }
            })
            .collect();

        let mut parsed = if style == '|' {
            deindented.join("\n")
        } else {
            let mut folded = String::new();
            for line in deindented {
                if line.is_empty() {
                    folded.push('\n');
                } else {
                    if !folded.is_empty() && !folded.ends_with('\n') {
                        folded.push(' ');
                    }
                    folded.push_str(line);
                }
            }
            folded
        };
        if value.contains('-') {
            parsed = parsed.trim_end_matches('\n').to_string();
        } else if !parsed.is_empty() && !parsed.ends_with('\n') {
            parsed.push('\n');
        }
        return Some(parsed);
    }
    None
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

fn source_paths_equal(a: &Path, b: &Path) -> bool {
    let a_str = a.to_string_lossy();
    let b_str = b.to_string_lossy();
    if a_str.starts_with("workspace://") || b_str.starts_with("workspace://") {
        return a_str == b_str;
    }
    let a_canon = fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let b_canon = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    a_canon == b_canon
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

/// Write bundled built-in Skill files into `dest`, replacing any stale copy, and
/// return the total bytes written. Rejects unsafe relative paths (traversal /
/// absolute) so a bundled manifest can never escape the managed store dir.
fn write_builtin_files(dest: &Path, files: &[(&str, &str)]) -> Result<u64, String> {
    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|e| format!("clear builtin skill dir: {e}"))?;
    }
    fs::create_dir_all(dest).map_err(|e| format!("create builtin skill dir: {e}"))?;
    let mut total = 0u64;
    for (rel, contents) in files {
        if rel.is_empty() || rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
            return Err(format!("invalid builtin skill file path: {rel}"));
        }
        let path = dest.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create builtin skill subdir: {e}"))?;
        }
        fs::write(&path, contents.as_bytes())
            .map_err(|e| format!("write builtin skill file: {e}"))?;
        total += contents.len() as u64;
    }
    Ok(total)
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
    fn inspect_skill_md_file_hints_parent_directory() {
        let dir = env::temp_dir().join(format!("csp-skill-file-{}", rand_u64()));
        fs::create_dir_all(&dir).unwrap();
        let md = dir.join("SKILL.md");
        fs::write(&md, "---\nname: File\n---\n").unwrap();
        let err = SkillStore::inspect_source(&md).unwrap_err();
        assert!(
            err.contains("parent directory"),
            "expected parent-dir hint, got: {err}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_missing_skill_md_is_invalid() {
        let dir = env::temp_dir().join(format!("csp-skill-noskill-{}", rand_u64()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("helper.py"), "print(1)").unwrap();
        let result = SkillStore::inspect_source(&dir).unwrap();
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("SKILL.md")),
            "expected SKILL.md error, got: {:?}",
            result.errors
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_copies_companion_files_recursively() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-companions-{}", rand_u64()));
        fs::create_dir_all(src.join("scripts")).unwrap();
        fs::write(
            src.join("SKILL.md"),
            "---\nname: Companions\ndescription: with extras\n---\n# C",
        )
        .unwrap();
        fs::write(src.join("USAGE.md"), "how to use").unwrap();
        fs::write(src.join("requirements.txt"), "numpy").unwrap();
        fs::write(src.join("scripts/run.py"), "print('hi')").unwrap();

        let skill = store.import(&src).unwrap();
        assert!(skill.store_path.join("SKILL.md").is_file());
        assert!(skill.store_path.join("USAGE.md").is_file());
        assert!(skill.store_path.join("requirements.txt").is_file());
        assert!(skill.store_path.join("scripts/run.py").is_file());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&store.root);
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
    fn import_dedupes_same_logical_workspace_key() {
        let store = temp_store();
        let src = env::temp_dir().join(format!("csp-skill-ws-{}", rand_u64()));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: Workspace\n---\n").unwrap();

        let key = "workspace://org/ws/file:draft.skill.md";
        let first = store.import_with_logical_source(&src, Some(key)).unwrap();
        let second = store.import_with_logical_source(&src, Some(key)).unwrap();

        assert_ne!(first.id.to_string(), second.id.to_string());
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(!first.store_path.exists());
        assert!(second.store_path.exists());
        assert_eq!(second.source_path, PathBuf::from(key));

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&store.root);
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
        assert!(
            !dst.join("link.txt").exists(),
            "symlink not followed/copied"
        );

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
        fs::write(
            sk_a.join("SKILL.md"),
            "---\nname: Alpha\ndescription: A\n---\n",
        )
        .unwrap();
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
    fn parses_yaml_block_scalar_descriptions() {
        let mut folded = InspectionResult {
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
        parse_skill_md(
            "---\nname: Folded\ndescription: >-\n  Split current work into small\n  reviewable PRs.\nmetadata:\n  surface: ide\n---\n",
            &mut folded,
        );
        assert_eq!(
            folded.description,
            "Split current work into small reviewable PRs."
        );

        let mut literal = folded.clone();
        literal.description.clear();
        parse_skill_md(
            "---\nname: Literal\ndescription: |-\n  First line.\n  Second line.\n---\n",
            &mut literal,
        );
        assert_eq!(literal.description, "First line.\nSecond line.");
    }

    #[test]
    fn discover_skips_missing_roots() {
        let store = temp_store();
        let missing = env::temp_dir().join(format!("csp-disc-missing-{}", rand_u64()));
        let found = store.discover(&[(missing, "~/nope".to_string())]).unwrap();
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn seed_once_seeds_then_is_sticky() {
        let store = temp_store();
        let files = [("SKILL.md", "---\nname: csp-environment\n---\nguidance")];
        // First seed: creates an enabled builtin skill + stamps the sentinel.
        let seeded = store
            .seed_once(".seeded-test", "csp-environment", "desc", &files, &["mcp"])
            .unwrap();
        assert!(seeded);
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].builtin);
        assert!(list[0].enabled);
        assert_eq!(list[0].name, "csp-environment");

        // User removes it → gone from inventory.
        let id = list[0].id.clone();
        store.remove(&id).unwrap();
        assert!(store.list().unwrap().is_empty());

        // Second seed is sticky: sentinel present → no resurrection.
        let seeded2 = store
            .seed_once(".seeded-test", "csp-environment", "desc", &files, &["mcp"])
            .unwrap();
        assert!(!seeded2);
        assert!(store.list().unwrap().is_empty());

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn refresh_builtin_updates_content_and_preserves_disabled() {
        let store = temp_store();
        let v1 = [("SKILL.md", "v1")];
        store
            .seed_once(".seeded-test", "csp-environment", "d1", &v1, &["mcp"])
            .unwrap();
        let id = store.list().unwrap()[0].id.clone();
        // User disables it.
        store.set_enabled(&id, false).unwrap();

        // Refresh with new content: rewrites files, keeps it disabled.
        let v2 = [("SKILL.md", "v2-longer-content")];
        let refreshed = store.refresh_builtin("csp-environment", "d2", &v2).unwrap();
        assert!(refreshed);
        let skill = store.get(&id).unwrap().unwrap();
        assert!(
            !skill.enabled,
            "refresh must not re-enable a disabled skill"
        );
        assert_eq!(skill.description, "d2");
        let md = fs::read_to_string(skill.store_path.join("SKILL.md")).unwrap();
        assert_eq!(md, "v2-longer-content");

        // Refreshing a non-existent builtin name is a no-op.
        assert!(!store.refresh_builtin("nope", "x", &v2).unwrap());

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn migrate_replaces_legacy_builtin_with_environment() {
        let store = temp_store();
        let legacy = [("SKILL.md", "---\nname: csp-web-access\n---\nold")];
        store
            .seed_once(
                ".seeded-csp-web-access",
                "csp-web-access",
                "old desc",
                &legacy,
                &["mcp"],
            )
            .unwrap();
        assert!(store.has_builtin_named("csp-web-access").unwrap());

        let files = [("SKILL.md", "---\nname: csp-environment\n---\nnew handbook")];
        let migrated = store
            .seed_or_migrate_environment_skill(
                ".seeded-csp-environment",
                "csp-environment",
                "new desc",
                &files,
                &["network", "mcp"],
                "csp-web-access",
                ".seeded-csp-web-access",
            )
            .unwrap();
        assert!(migrated);

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "csp-environment");
        assert!(list[0].builtin);
        assert!(list[0].enabled);
        assert!(!store.has_builtin_named("csp-web-access").unwrap());
        assert!(store.sentinel_exists(".seeded-csp-environment"));
        let skill = store.get(&list[0].id).unwrap().unwrap();
        let md = fs::read_to_string(skill.store_path.join("SKILL.md")).unwrap();
        assert!(md.contains("new handbook"));

        // Second launch: new sentinel → refresh only, no duplicate.
        let again = store
            .seed_or_migrate_environment_skill(
                ".seeded-csp-environment",
                "csp-environment",
                "newer desc",
                &[("SKILL.md", "---\nname: csp-environment\n---\nrefreshed")],
                &["network", "mcp"],
                "csp-web-access",
                ".seeded-csp-web-access",
            )
            .unwrap();
        assert!(again);
        let list2 = store.list().unwrap();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].description, "newer desc");

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn migrate_respects_legacy_opt_out_without_seeding() {
        let store = temp_store();
        // Opt-out: legacy sentinel stamped, skill removed from inventory.
        store.stamp_sentinel(".seeded-csp-web-access").unwrap();
        assert!(store.list().unwrap().is_empty());

        let files = [(
            "SKILL.md",
            "---\nname: csp-environment\n---\nshould not seed",
        )];
        let migrated = store
            .seed_or_migrate_environment_skill(
                ".seeded-csp-environment",
                "csp-environment",
                "desc",
                &files,
                &["mcp"],
                "csp-web-access",
                ".seeded-csp-web-access",
            )
            .unwrap();
        assert!(!migrated);
        assert!(store.list().unwrap().is_empty());
        assert!(store.sentinel_exists(".seeded-csp-environment"));

        // Further launches stay empty (new sentinel → refresh no-op).
        let again = store
            .seed_or_migrate_environment_skill(
                ".seeded-csp-environment",
                "csp-environment",
                "desc",
                &files,
                &["mcp"],
                "csp-web-access",
                ".seeded-csp-web-access",
            )
            .unwrap();
        assert!(!again);
        assert!(store.list().unwrap().is_empty());

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn migrate_fresh_install_seeds_environment() {
        let store = temp_store();
        let files = [("SKILL.md", "---\nname: csp-environment\n---\nfresh")];
        let seeded = store
            .seed_or_migrate_environment_skill(
                ".seeded-csp-environment",
                "csp-environment",
                "desc",
                &files,
                &["mcp"],
                "csp-web-access",
                ".seeded-csp-web-access",
            )
            .unwrap();
        assert!(seeded);
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "csp-environment");
        assert!(store.sentinel_exists(".seeded-csp-environment"));

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn write_builtin_files_rejects_traversal() {
        let dir = env::temp_dir().join(format!("csp-builtin-{}", rand_u64()));
        assert!(write_builtin_files(&dir, &[("../evil", "x")]).is_err());
        assert!(write_builtin_files(&dir, &[("/abs", "x")]).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_from_content_writes_skill_and_lists() {
        let store = temp_store();
        let content = "---\nname: Authored Skill\ndescription: Made from scratch\n---\n\n# Authored Skill\n\nDo the thing.\n";
        let skill = store.create_from_content(content).unwrap();
        assert_eq!(skill.name, "Authored Skill");
        assert_eq!(skill.description, "Made from scratch");
        assert!(skill.enabled);
        // SKILL.md landed in managed storage.
        let md = fs::read_to_string(skill.store_path.join("SKILL.md")).unwrap();
        assert!(md.contains("Do the thing."));

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Authored Skill");

        let _ = fs::remove_dir_all(&store.root);
    }

    #[test]
    fn create_from_content_rejects_empty_and_duplicate_name() {
        let store = temp_store();
        // No name in front-matter and no heading → rejected.
        assert!(store.create_from_content("---\n---\njust a body").is_err());

        let content = "---\nname: Dup\n---\n# Dup\n";
        store.create_from_content(content).unwrap();
        // Second create with the same name is rejected.
        assert!(store.create_from_content(content).is_err());
        assert_eq!(store.list().unwrap().len(), 1);

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
