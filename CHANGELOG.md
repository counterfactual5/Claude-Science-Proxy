# Changelog

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Domestic agent/IDE discovery sources**: Skill and MCP discovery now also scan popular China-market tools using their default config locations. MCP: Alibaba **Qoder / 通义灵码** (`~/Library/Application Support/<app>/SharedClientCache/mcp.json`), ByteDance **Trae / TRAE SOLO** (`~/Library/Application Support/<app>/User/mcp.json`), and Tencent **CodeBuddy** (`~/.codebuddy/.mcp.json`, plus its documented legacy `~/.codebuddy/mcp.json`). Skills: `~/.trae/skills` and `~/.codebuddy/skills`. All use the standard `mcpServers` / `SKILL.md` layouts, so no new parsing is required; remote (non-stdio) entries are still filtered out.

## [1.2.0] — 2026-07-12

### Added
- **Local Skill Manager**: New **Skills** tab imports local Skill directories (folders with a `SKILL.md`) into `~/.csp/skills/`, with list / enable-disable / remove, and a **Discover** action that scans common agent locations (`~/.agents/skills`, `~/.codex/skills`, `~/.claude/skills`, `~/.cursor/skills`) for selective import. Enabled Skills deploy into the sandbox on each **Start Claude Science**; only folders CSP marks with `.csp_managed` are managed, so bundled scientific Skills are never touched.
- **Local stdio MCP Manager**: New **MCP** tab manages local stdio MCP connectors (`command` + `args` + `env`) at `~/.csp/mcp/inventory.json`, with add / edit / enable-disable / remove and a **Discover** action that reads connectors from Cursor, Claude Desktop/Code, Codex (`config.toml`), Devin Desktop, VS Code, and Zed. Enabled connectors deploy to the sandbox `local-mcp.json`, and CSP grants least-privilege `[sandbox] user_read_paths` for referenced absolute paths.

### Changed
- **Launch-time deployment & auto-restart**: Skills and MCP connectors are read by Science only at launch; CSP now deploys them idempotently and, when the deployed set actually changes on a reopen, restarts the running sandbox so the new config takes effect.
- **Mutually exclusive tabs**: The top-right tab buttons now show exactly one pane (Profiles / Skills / MCP); panes marked `hidden` are no longer stacked together.

### Fixed
- **Skill discovery path**: Skills now deploy to the org-scoped `…/.claude-science/orgs/<org_uuid>/skills/<name>/` that current Science builds actually scan and stamp, and CSP cleans up Skills left in the legacy root `…/.claude-science/skills/` by earlier builds. Launch self-verifies via Science's `.catalog_stamp` (`recognized_by_science=<n>`).
- **Secret handling**: MCP `env` values are stored in a `0600` inventory and only returned to the UI masked; the deployed `local-mcp.json` is written `0600`. Editing merges `env` (blank keeps, deleted removes, new value overwrites) so masked secrets are never round-tripped. `create` / `update` / `set_enabled` return masked summaries only.
- **Path & deploy safety**: Symlinks inside Skills are skipped, sanitized folder-name collisions are skipped, `config.toml` is compared semantically to avoid spurious restarts, and read grants apply least privilege (directory itself if a dir, else its parent).

### Documentation
- Documented the Skill Manager and Local stdio MCP connectors in `docs/known-issues.md`, including scope, caveats, secret handling, and the launch-time apply/restart behavior.

## [1.0.0] — 2026-07-10

### Added
- **English Code Comments**: Aligned all code comments in Rust, Python, and JS production paths, tests, and shell scripts to English. User-visible UI copy continues to use localizable i18n keys.
- **Capability Catalog**: Added the `provider.virtual-model-registry` rule and annotated `provider.relay.force-model-shell` as the single-model fallback when no model registry is configured.
- **Template Display Names**: Configured canonical English `name` fields in `templates.rs` and localized UI preset labels via frontend dictionary mappings.

### Changed
- **Dead Code Pruning**: Cleaned up frontend-facing fields from backend commands that are no longer consumed, such as `get_config.pending_notice`, Tauri `status` command, and redundant success hints.
- **Unified Backend i18n**: Refactored user-visible errors and success messages across `config`, `oauth_forge`, `scratch`, `capability_catalog`, and `sandbox_session` to serialize via `i18n_err` keys, handled dynamically by the frontend.

### Fixed
- **Science Multi-Model Selector**: Sanitized virtual registry model display names using `science_safe_display_name()` to bypass Science's `V2_` lowercase multi-hyphen filter, preventing configured models from being hidden.

### Documentation
- **Repository Overhaul**: Pruned 12 obsolete/historical research and checklist documents from `docs/` to simplify the codebase, preserving only `README.md` (and the bilingual `README.zh.md`) and a translated, single developer handbook: `docs/DEVELOPMENT.md`.
- **Open Source Preparation**: Configured GitHub issue templates, pull request templates, and MIT license attributions. Aligned real-machine smoke testing guidelines.