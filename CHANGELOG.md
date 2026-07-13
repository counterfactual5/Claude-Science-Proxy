# Changelog

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [1.4.1] — 2026-07-13

### Changed
- **Row actions menus**: Skill and MCP rows now use a compact `⋯` menu instead of inline buttons, matching the Profiles row layout. Skill rows offer **编辑 / 打开文件夹 / 删除** (open `SKILL.md` in the default editor, reveal the managed folder in Finder, remove); MCP rows offer **编辑 / 删除**.

### Fixed
- **Version metadata**: `Cargo.toml` is bumped in lockstep so the binary's internal version string matches the bundle version.

## [1.4.0] — 2026-07-13

### Added
- **Science workspace Skill adopt**: Skills `⋯` → **从 Science 采纳** scans `$SANDBOX_HOME/.claude-science/orgs/<org>/workspaces/` for Skill drafts (`*.skill.md`, `*_SKILL.md`, or `SKILL.md` folders) and companion files (`kernel.py`, `demo_*.py`, …), imports selected drafts into `~/.csp/skills/`, and redeploys (restarting a running sandbox when needed). Science cannot publish skills under CSP virtual login; this is the supported ingress path for Science-generated drafts.

### Fixed
- **Workspace adopt file list**: Folder-based candidates no longer show `SKILL.md` twice in the adopt dialog.

## [1.3.1] — 2026-07-13

### Fixed
- **npm-style Node MCP shims in the Science sandbox**: Global MCP binaries such as `notion-mcp-server` often use `#!/usr/bin/env node`, but Science's MCP child sandbox does not inherit the host `PATH`, causing `env: node: No such file or directory`. CSP now rewrites those shims at deploy time to an absolute `node <script>` invocation when it can resolve the colocated Node runtime from the user's installation.

## [1.3.0] — 2026-07-13

### Added
- **Domestic agent/IDE discovery sources**: Skill and MCP discovery now also scan popular China-market tools using their default config locations. MCP: Alibaba **Qoder / 通义灵码** (`~/Library/Application Support/<app>/SharedClientCache/mcp.json`), ByteDance **Trae / TRAE SOLO** (`~/Library/Application Support/<app>/User/mcp.json`), and Tencent **CodeBuddy** (`~/.codebuddy/.mcp.json`, plus its documented legacy `~/.codebuddy/mcp.json`). Skills: `~/.trae/skills` and `~/.codebuddy/skills`. All use the standard `mcpServers` / `SKILL.md` layouts, so no new parsing is required; remote (non-stdio) entries are still filtered out.
- **MCP inventory quick edit**: The MCP tab can now open CSP's persistent MCP inventory at `~/.csp/mcp/inventory.json` for quick advanced edits.

### Changed
- **Simplified Skills / MCP headers**: Both tabs now match the Profiles layout — a single primary button plus a `⋯` overflow menu for secondary actions. The former "discover" action was relabeled "scan & import" with matching dialog titles.

### Fixed
- **Node MCP connectors reaching HTTPS APIs (e.g. Notion)**: Science's MCP-child sandbox permits outbound loopback connections only to its own injected Operon proxy — confirmed live that redirecting to any other local port (including CSP's own proxy) is denied with `EPERM`. Meanwhile several bundled Node HTTP stacks (axios via `follow-redirects`, used by `@notionhq/notion-mcp-server` and others) never issue a CONNECT for HTTPS targets; they relay the request in absolute form, which Operon forwards as plain HTTP onto the origin's port 443 (`400 The plain HTTP request was sent to HTTPS port`). CSP ships a Node shim (`mcp_http_tunnel_shim.cjs`) that turns that pattern into a real CONNECT+TLS tunnel. Live probe also showed Science strips `NODE_OPTIONS` from `local-mcp.json` env, so the shim is loaded by wrapping each connector with `/bin/bash` that re-exports `NODE_OPTIONS=--require <shim>` immediately before `exec`.

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