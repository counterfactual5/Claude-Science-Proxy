//! Built-in `csp-web-access` standing-guidance Skill.
//!
//! CSP bundles a tiny SKILL.md that tells Claude Science, in **every** session,
//! to use the built-in local `web-search` MCP connector for any web/online
//! search (via `search_literature` / `csp_web_search` / `fetch_url`) and to
//! never call Anthropic's hosted `web_search` tool — which is unavailable under
//! CSP's virtual login and fails with `Tool 'web_search' not found on agent`.
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
/// Science actually reads as standing guidance).
pub const BUILTIN_WEB_ACCESS_DESCRIPTION: &str = "CSP standing guidance: for any web search or page fetch, use the local web-search MCP (search_literature / csp_web_search, then fetch_url); never call the hosted web_search tool (unavailable under CSP).";

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
        // Front-matter name matches the deploy folder name.
        assert!(WEB_ACCESS_SKILL_MD.contains(&format!("name: {BUILTIN_WEB_ACCESS_NAME}")));
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
