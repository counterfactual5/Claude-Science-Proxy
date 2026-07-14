//! Built-in `csp-web-access` standing-guidance Skill.
//!
//! CSP bundles a small SKILL.md of **standing environment conventions** that
//! tells Claude Science, in **every** session, how the local CSP sandbox differs
//! from Anthropic's hosted Claude environment:
//!
//! - **Web access**: use the built-in local `web-search` MCP connector for any
//!   web/online search (via `search_literature` / `csp_web_search` / `fetch_url`)
//!   and never call Anthropic's hosted `web_search` tool — which is unavailable
//!   under CSP's virtual login and fails with `Tool 'web_search' not found on
//!   agent`.
//! - **Files/artifacts**: `/mnt/data` (and other `/mnt/...`) do not exist; save
//!   outputs to the workspace cwd with relative paths and persist user-visible
//!   files via `save_artifacts([...])`.
//! - **Plotting/CJK**: the default matplotlib font can't render CJK; set a
//!   CJK-capable `font.sans-serif` before plotting non-Latin labels.
//! - **Skills/env**: don't rely on `host.skills.publish()`; draft skills in the
//!   workspace and adopt them via CSP's Skills tab. Scientific packages live in
//!   the analysis `python` env, not necessarily the MCP env.
//!
//! The skill name stays `csp-web-access` (renaming would force a sentinel /
//! inventory migration); only the guidance content and description are broadened.
//!
//! This mirrors the built-in `web-search` MCP connector pattern
//! (`mcp_manager::builtin`): the content is bundled via `include_str!`, seeded
//! enabled-by-default on first run guarded by a **sticky sentinel**, and its
//! on-disk content is refreshed on every startup so app upgrades propagate new
//! guidance text — without ever resurrecting a skill the user removed.

/// Display name of the skill (also the sandbox deploy folder after
/// sanitization). Must NOT collide with any of Science's own bundled skill
/// folders (e.g. `alphafold2`, `boltz`, `self-awareness`,
/// `product-self-knowledge`), or the deployer would skip it as an unmanaged
/// collision.
pub const BUILTIN_WEB_ACCESS_NAME: &str = "csp-web-access";

/// One-line description shown in CSP's Skills tab (the SKILL.md body is what
/// Science actually reads as standing guidance). Covers the full set of CSP
/// local environment conventions, not only web search.
pub const BUILTIN_WEB_ACCESS_DESCRIPTION: &str = "CSP standing environment conventions: for any web search/page fetch use the local web-search MCP (search_literature / csp_web_search, then fetch_url — host.mcp returns a dict with key results, not a bare list), never the hosted web_search tool; don't write to /mnt/data — save to the workspace cwd and persist via save_artifacts([...]); set a CJK matplotlib font before plotting non-Latin labels; draft skills in the workspace (not host.skills.publish) and use the analysis python env for scientific packages.";

/// Sentinel dotfile under the skill store root recording the one-time seed. Once
/// present, the skill is never re-seeded, so a user who later disables or removes
/// it is respected across relaunches.
pub const WEB_ACCESS_SEED_SENTINEL: &str = ".seeded-csp-web-access";

/// The bundled SKILL.md, embedded at compile time.
pub const WEB_ACCESS_SKILL_MD: &str = include_str!("csp-web-access/SKILL.md");

/// Requirements tags surfaced in the UI for this skill.
pub const WEB_ACCESS_REQUIREMENTS: &[&str] = &["network", "mcp"];

/// The relative files that make up the bundled skill (currently just
/// `SKILL.md`). Returned as `(relative path, contents)` pairs so the store can
/// write them into managed storage.
pub fn bundled_files() -> Vec<(&'static str, &'static str)> {
    vec![("SKILL.md", WEB_ACCESS_SKILL_MD)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skill_md_is_standing_guidance() {
        // Mentions the exact connector + tool names and the hosted tool to avoid.
        assert!(WEB_ACCESS_SKILL_MD.contains("web-search"));
        assert!(WEB_ACCESS_SKILL_MD.contains("search_literature"));
        assert!(WEB_ACCESS_SKILL_MD.contains("csp_web_search"));
        assert!(WEB_ACCESS_SKILL_MD.contains("fetch_url"));
        assert!(WEB_ACCESS_SKILL_MD.contains("web_search"));
        // Return-shape guidance so models don't iterate the wrapper dict as hits.
        assert!(WEB_ACCESS_SKILL_MD.contains("data[\"results\"]"));
        assert!(WEB_ACCESS_SKILL_MD.contains("Return shape"));
        // Front-matter name matches the deploy folder name.
        assert!(WEB_ACCESS_SKILL_MD.contains(&format!("name: {BUILTIN_WEB_ACCESS_NAME}")));
    }

    #[test]
    fn bundled_skill_md_covers_environment_conventions() {
        // Files/artifacts: the /mnt/data prohibition and the workspace-cwd +
        // save_artifacts persistence rule must be present.
        assert!(
            WEB_ACCESS_SKILL_MD.contains("/mnt/data"),
            "must warn that /mnt/data does not exist locally"
        );
        assert!(
            WEB_ACCESS_SKILL_MD.contains("save_artifacts"),
            "must tell Science to persist user-visible files via save_artifacts"
        );
        assert!(
            WEB_ACCESS_SKILL_MD.contains("workspaces/<workspace_uuid>")
                || WEB_ACCESS_SKILL_MD.contains("current working directory"),
            "must point outputs at the workspace working directory"
        );
        // Plotting/CJK guidance.
        assert!(WEB_ACCESS_SKILL_MD.contains("font.sans-serif"));
        assert!(WEB_ACCESS_SKILL_MD.contains("axes.unicode_minus"));
        // Skills/env conventions.
        assert!(WEB_ACCESS_SKILL_MD.contains("host.skills.publish"));
        // The one-line description was broadened beyond web search.
        assert!(BUILTIN_WEB_ACCESS_DESCRIPTION.contains("/mnt/data"));
        assert!(BUILTIN_WEB_ACCESS_DESCRIPTION.contains("save_artifacts"));
    }

    #[test]
    fn bundled_files_contains_skill_md() {
        let files = bundled_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "SKILL.md");
        assert_eq!(files[0].1, WEB_ACCESS_SKILL_MD);
    }

    #[test]
    fn name_does_not_collide_with_science_bundled_skills() {
        for reserved in ["alphafold2", "boltz", "self-awareness", "product-self-knowledge"] {
            assert_ne!(BUILTIN_WEB_ACCESS_NAME, reserved);
        }
    }
}
