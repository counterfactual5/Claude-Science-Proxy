//! Deploy enabled Skills into the isolated Science sandbox.
//!
//! Current Claude Science builds discover user Skills by scanning the active org's
//! `<data-dir>/orgs/<org_uuid>/skills/<name>/SKILL.md` (pure disk scan; each
//! recognized folder gets a `.catalog_stamp`). Callers pass that org data-dir, and
//! enabled Skills are copied there before launch.
//!
//! Iron rules enforced here:
//! - Only ever write under the sandbox root; never the real `~/.claude-science`.
//! - Never touch Science's bundled Skills (alphafold2, boltz, ...). We only remove
//!   folders we previously deployed, marked with `.csp_managed`.
//! - Never clobber an existing unmanaged folder (a same-named bundled Skill is skipped).

use std::fs;
use std::path::Path;

use super::model::Skill;
use super::store::copy_dir;
use crate::oauth_forge::real_ancestor;

/// Marker file written inside each CSP-deployed Skill folder. Cleanup only removes
/// folders containing this marker, so bundled Skills are never deleted.
const MANAGED_MARKER: &str = ".csp_managed";

/// Sidecar manifest (in the skills dir) recording the signature of the last
/// deployment, so an unchanged redeploy can be a no-op.
const MANIFEST_FILE: &str = ".csp_manifest";

/// Summary of a deployment pass (used for launch-log observability).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DeployReport {
    /// Skill names successfully deployed this pass.
    pub deployed: Vec<String>,
    /// Skill names skipped due to collision with an unmanaged (bundled) folder
    /// or a sanitized-name clash with another enabled Skill.
    pub skipped: Vec<String>,
    /// Count of previously-managed folders removed before deploying.
    pub removed: usize,
    /// Whether anything on disk actually changed this pass. Lets a caller decide
    /// whether a running sandbox needs restarting for Science to re-read Skills.
    pub changed: bool,
}

/// Deploy `enabled` Skills into `<data_dir>/skills/`.
///
/// `data_dir` is the active org data-dir (normally
/// `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>`); `sandbox_root` is
/// `$SANDBOX_HOME`; `real_science_dir` is the real `~/.claude-science` used only
/// for the guard check.
pub fn deploy_enabled_skills(
    enabled: &[Skill],
    data_dir: &Path,
    sandbox_root: &Path,
    real_science_dir: &Path,
) -> Result<DeployReport, String> {
    // —— Iron-rule guards (mirror oauth_forge): resolve to nearest real ancestor,
    // then reject the real Science tree and anything outside the sandbox root. ——
    let resolved = real_ancestor(data_dir);
    let real_root = real_ancestor(real_science_dir);
    let root = real_ancestor(sandbox_root);
    if resolved.starts_with(&real_root) {
        return Err(format!(
            "refuse: sandbox skills dir resolves inside real Science dir ({})",
            resolved.display()
        ));
    }
    if !resolved.starts_with(&root) {
        return Err(format!(
            "refuse: sandbox skills dir resolves outside sandbox root ({} not under {})",
            resolved.display(),
            root.display()
        ));
    }

    let skills_dir = data_dir.join("skills");
    let mut report = DeployReport::default();

    // —— Build the deploy plan without touching disk: sanitize names, drop empty
    // ones, de-dupe sanitized clashes, and skip collisions with unmanaged
    // (bundled) folders. ——
    let mut claimed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut plan: Vec<(String, &Skill)> = Vec::new();
    for skill in enabled {
        let folder = sanitize_folder(&skill.name);
        if folder.is_empty() {
            report.skipped.push(skill.name.clone());
            continue;
        }
        if !claimed.insert(folder.clone()) {
            // Another enabled Skill already took this sanitized folder name.
            report.skipped.push(skill.name.clone());
            continue;
        }
        let dest = skills_dir.join(&folder);
        // Never clobber a bundled/unmanaged folder of the same name.
        if dest.exists() && !dest.join(MANAGED_MARKER).is_file() {
            report.skipped.push(skill.name.clone());
            continue;
        }
        plan.push((folder, skill));
    }

    // Signature of the intended deployment + the set of folders it will occupy.
    let signature = plan
        .iter()
        .map(|(folder, s)| format!("{}\t{}\t{}", folder, s.id, s.size_bytes))
        .collect::<Vec<_>>()
        .join("\n");
    let manifest_path = skills_dir.join(MANIFEST_FILE);
    let planned_folders: std::collections::BTreeSet<String> =
        plan.iter().map(|(f, _)| f.clone()).collect();

    // Idempotency: if the manifest matches and exactly the planned managed folders
    // are present, nothing changed — skip all destructive IO.
    if manifest_matches(&manifest_path, &signature)
        && managed_folders(&skills_dir) == planned_folders
    {
        report.deployed = plan.into_iter().map(|(_, s)| s.name.clone()).collect();
        return Ok(report);
    }

    // Remove only previously-managed deployments; leave bundled Skills untouched.
    if skills_dir.is_dir() {
        for entry in fs::read_dir(&skills_dir).map_err(|e| format!("read skills dir: {e}"))? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let p = entry.path();
            if p.is_dir() && p.join(MANAGED_MARKER).is_file() {
                fs::remove_dir_all(&p).map_err(|e| format!("remove managed skill: {e}"))?;
                report.removed += 1;
            }
        }
    }
    report.changed = report.removed > 0;

    if plan.is_empty() {
        // Nothing to deploy: drop a stale manifest so the dir is clean.
        if manifest_path.exists() {
            let _ = fs::remove_file(&manifest_path);
            report.changed = true;
        }
        return Ok(report);
    }
    fs::create_dir_all(&skills_dir).map_err(|e| format!("create skills dir: {e}"))?;

    for (folder, skill) in &plan {
        let dest = skills_dir.join(folder);
        // Fresh copy each pass (managed dest may linger if cleanup missed it).
        if dest.exists() {
            fs::remove_dir_all(&dest).map_err(|e| format!("clear stale managed skill: {e}"))?;
        }
        copy_dir(&skill.store_path, &dest)?;
        fs::write(dest.join(MANAGED_MARKER), b"csp\n")
            .map_err(|e| format!("write managed marker: {e}"))?;
        report.deployed.push(skill.name.clone());
    }
    fs::write(&manifest_path, signature.as_bytes()).map_err(|e| format!("write manifest: {e}"))?;
    report.changed = true;

    Ok(report)
}

/// True when the manifest file exists and its contents equal `signature`.
fn manifest_matches(manifest_path: &Path, signature: &str) -> bool {
    fs::read_to_string(manifest_path)
        .map(|s| s == signature)
        .unwrap_or(false)
}

/// The set of directory names under `skills_dir` that carry our managed marker.
fn managed_folders(skills_dir: &Path) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    if let Ok(entries) = fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join(MANAGED_MARKER).is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

/// Sanitize a Skill name into a safe single path segment (no traversal, no separators).
fn sanitize_folder(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if c == ' ' || c == '.' {
            out.push('-');
        }
        // drop anything else (including '/', '\\', control chars)
    }
    let trimmed = out.trim_matches('-').to_string();
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_manager::model::{Skill, SkillId};
    use std::env;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn uniq() -> u64 {
        static C: AtomicU64 = AtomicU64::new(0);
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        n.wrapping_mul(1_000_003)
            .wrapping_add(C.fetch_add(1, Ordering::Relaxed))
    }

    /// Build a fake store skill dir with a SKILL.md and return a Skill record.
    fn make_skill(store_root: &Path, name: &str) -> Skill {
        let sp = store_root.join(format!("sk-{}", sanitize_folder(name)));
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
        Skill {
            id: SkillId::new(),
            name: name.to_string(),
            description: String::new(),
            store_path: sp,
            source_path: PathBuf::from("/tmp/src"),
            enabled: true,
            size_bytes: 0,
            imported_at: String::new(),
            requirements: vec![],
        }
    }

    struct Fixture {
        sandbox_root: PathBuf,
        data_dir: PathBuf,
        real_dir: PathBuf,
        store_root: PathBuf,
    }

    fn fixture() -> Fixture {
        let base = env::temp_dir().join(format!("csp-deploy-{}", uniq()));
        let sandbox_root = base.join("sandbox/home");
        let data_dir = sandbox_root.join(".claude-science");
        let real_dir = base.join("real/.claude-science");
        let store_root = base.join("store");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&real_dir).unwrap();
        fs::create_dir_all(&store_root).unwrap();
        Fixture {
            sandbox_root,
            data_dir,
            real_dir,
            store_root,
        }
    }

    #[test]
    fn deploys_enabled_and_writes_marker() {
        let f = fixture();
        let skills = vec![make_skill(&f.store_root, "my-skill")];
        let r = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert_eq!(r.deployed, vec!["my-skill".to_string()]);
        let dest = f.data_dir.join("skills").join("my-skill");
        assert!(dest.join("SKILL.md").is_file());
        assert!(dest.join(MANAGED_MARKER).is_file());
    }

    #[test]
    fn cleanup_preserves_bundled_removes_managed() {
        let f = fixture();
        let skills_dir = f.data_dir.join("skills");
        // Simulate a bundled skill (no marker) and a stale managed one (with marker).
        let bundled = skills_dir.join("alphafold2");
        fs::create_dir_all(&bundled).unwrap();
        fs::write(bundled.join("SKILL.md"), "---\nname: alphafold2\n---\n").unwrap();
        let stale = skills_dir.join("old-managed");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join(MANAGED_MARKER), b"csp\n").unwrap();

        let skills = vec![make_skill(&f.store_root, "fresh")];
        let r = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        assert_eq!(r.removed, 1, "stale managed folder removed");
        assert!(bundled.join("SKILL.md").is_file(), "bundled untouched");
        assert!(!stale.exists(), "managed folder removed");
        assert!(skills_dir.join("fresh").join(MANAGED_MARKER).is_file());
    }

    #[test]
    fn skips_collision_with_unmanaged() {
        let f = fixture();
        let skills_dir = f.data_dir.join("skills");
        let bundled = skills_dir.join("boltz");
        fs::create_dir_all(&bundled).unwrap();
        fs::write(bundled.join("SKILL.md"), "---\nname: boltz\n---\n").unwrap();

        let skills = vec![make_skill(&f.store_root, "boltz")];
        let r = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();

        assert_eq!(r.deployed.len(), 0);
        assert_eq!(r.skipped, vec!["boltz".to_string()]);
        // Bundled SKILL.md preserved, no marker injected.
        assert!(bundled.join("SKILL.md").is_file());
        assert!(!bundled.join(MANAGED_MARKER).exists());
    }

    #[test]
    fn empty_enabled_only_cleans_up() {
        let f = fixture();
        let skills_dir = f.data_dir.join("skills");
        let stale = skills_dir.join("old");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join(MANAGED_MARKER), b"csp\n").unwrap();

        let r = deploy_enabled_skills(&[], &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert_eq!(r.removed, 1);
        assert!(!stale.exists());
    }

    #[test]
    fn rejects_real_science_dir() {
        let f = fixture();
        let skills = vec![make_skill(&f.store_root, "x")];
        // data_dir == real dir → must be rejected.
        let r = deploy_enabled_skills(&skills, &f.real_dir, &f.sandbox_root, &f.real_dir);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("real Science dir"));
    }

    #[test]
    fn rejects_outside_sandbox() {
        let f = fixture();
        let outside = env::temp_dir().join(format!("csp-outside-{}", uniq()));
        fs::create_dir_all(&outside).unwrap();
        let skills = vec![make_skill(&f.store_root, "x")];
        let r = deploy_enabled_skills(&skills, &outside, &f.sandbox_root, &f.real_dir);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("outside sandbox root"));
    }

    #[test]
    fn dedupes_sanitized_name_clash() {
        let f = fixture();
        // Two distinct Skills whose names sanitize to the same folder ("My-Skill").
        let skills = vec![
            make_skill(&f.store_root, "My Skill"),
            make_skill(&f.store_root, "My-Skill"),
        ];
        let r = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert_eq!(r.deployed.len(), 1, "only the first claims the folder");
        assert_eq!(r.skipped.len(), 1, "the clashing one is skipped");
        assert!(f.data_dir.join("skills").join("My-Skill").is_dir());
    }

    #[test]
    fn second_identical_deploy_reports_no_change() {
        let f = fixture();
        let skills = vec![make_skill(&f.store_root, "stable")];
        let r1 = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(r1.changed);
        let r2 = deploy_enabled_skills(&skills, &f.data_dir, &f.sandbox_root, &f.real_dir).unwrap();
        assert!(!r2.changed, "identical redeploy is a no-op");
        assert_eq!(r2.removed, 0, "no destructive churn when unchanged");
    }

    #[test]
    fn sanitize_blocks_traversal() {
        assert_eq!(sanitize_folder("../evil"), "evil");
        assert_eq!(sanitize_folder("a/b"), "ab");
        assert_eq!(sanitize_folder("  spaced name "), "spaced-name");
        assert_eq!(sanitize_folder("///"), "");
    }
}
