# Changelog

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [1.5.0] — 2026-07-14

### Fixed
- **Proxy injects standing web-access guidance (real fix for bare `web_search`)**: Under CSP virtual login, Anthropic-native `web_search` / `web_fetch` are **stripped from OPERON's toolset**, so bare top-level calls fail with `Tool 'web_search' not found on agent 'OPERON'`. Local MCP tools are **not** top-level model tools — they are only callable via `repl` → `host.mcp("web-search", "<method>", ...)`. Re-advertising names in MCP `tools/list` **cannot** intercept bare native calls (previous approach was the wrong layer). On every Anthropic-shaped `/v1/messages` request that already has a `system` prompt, `proxy/compat/anthropic_compat.py` now idempotently appends a short standing block (sentinel `<!-- CSP_WEB_ACCESS_GUIDANCE -->`) telling the model to use `host.mcp("web-search", "search_literature", …)` / `fetch_url` and never bare `web_search`/`web_fetch`. Applied on both Anthropic passthrough (`transform_request`) and OpenAI translation (`_handle_openai`). Skills (`csp-web-access`) remain progressive-disclosure and are insufficient alone.
- **Built-in web-search script rewrite triggers sandbox restart**: `write_web_search_server` now reports whether disk bytes changed; `deploy_sandbox_mcp` ORs that into `changed` so Start Claude Science / one_click_login restarts a running sandbox when the embedded `include_str` script was rewritten (MCP children otherwise keep the old script in memory). Quitting the CSP app alone still does not stop a leftover `claude-science serve` daemon — use **Stop Claude Science** in the UI (documented follow-up; no aggressive kill).

### Changed
- **Corrected misleading `web_search` copy**: `mcp_manager/builtin.rs`, `web_search_server.py`, and known-issues no longer claim that advertising `web_search` in `tools/list` makes model-native calls resolve locally. Truth: bare native tools are unavailable; `tools/list` names are `host.mcp` method names only; proxy prompt injection is the standing fix.
- **Built-in `csp-web-access` Skill broadened to full CSP environment conventions**: The standing-guidance Skill (bundled `SKILL.md`, seeded into `~/.csp/skills/` and deployed each launch) now teaches Claude Science the local sandbox conventions that differ from Anthropic's hosted environment, not just web search:
  - **Files/artifacts**: `/mnt/data` (and other `/mnt/...`) do not exist locally — save outputs to the current working directory (the active workspace `orgs/<org>/workspaces/<ws>/`) with relative paths, use `/tmp` only for scratch, and persist user-visible files by writing them in the workspace then calling `save_artifacts([...])`.
  - **Plotting/CJK**: matplotlib's default `DejaVu Sans` can't render CJK; set `plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]` and `axes.unicode_minus = False` before plotting non-Latin labels (guidance-only — the host already ships these fonts; no font binary is bundled).
  - **Web/network** and **skills/env** reminders: prefer the local `web-search` MCP over the hosted `web_search` tool; don't rely on `host.skills.publish()` for durable installs (draft in the workspace and adopt via the Skills tab); scientific packages live in the analysis `python` env, not necessarily the MCP env.
  - The Skill keeps its name `csp-web-access` (renaming would force a sentinel/inventory migration); only its content and one-line description were broadened. Its on-disk copy self-heals on every launch (`refresh_builtin` rewrites the bundled content while preserving enabled/disabled state), so this updated guidance propagates to already-seeded users on the next launch without resurrecting a removed Skill.

### Fixed (earlier in 1.5.0 — superseded)
- **Local `web-search` MCP re-advertises `web_search` / `web_fetch`** *(insufficient alone)*: `tools/list` still exposes these as `host.mcp` method names, but that does **not** intercept bare native calls. The proxy system-prompt injection above is the real fix.
- **Built-in `web-search` MCP tool-name conflict with the hosted `web_search`** *(superseded)*: An interim build advertised only distinct names (`search_literature` / `csp_web_search` / `fetch_url`) with `web_search` as a hidden dispatch alias.

### Added
- **Create new Skills from the UI**: The Skills tab now has a primary `新建` action for authoring a new `SKILL.md` directly in CSP, while importing an existing Skill directory moved into the row/header `⋯` menu.
- **Built-in `csp-web-access` Skill (standing web-search guidance, on by default)**: CSP now seeds a small CSP-managed Skill into `~/.csp/skills/` on first run, enabled by default, so Claude Science automatically prefers the local `web-search` MCP in **every** session — the user no longer has to say "use the local web-search MCP" each time. Its bundled `SKILL.md` (embedded via `include_str!`) instructs Science that for ANY web search or page fetch it must use the local **`web-search`** connector (`search_literature` / `csp_web_search` to search, `fetch_url` to read pages) and must NEVER call the hosted `web_search` tool (unavailable under CSP virtual login, which otherwise wastes a turn on `Tool 'web_search' not found on agent`), and notes the sandbox egress allowlist favours scholarly sources (Crossref / arXiv / PubMed / OpenAlex / Semantic Scholar).
  - **Seeded + deployed like the built-in connector**: it is deployed to `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/csp-web-access/` on every **Start Claude Science**, appears in the **Skills** tab with a **内置 / Built-in** badge, and its on-disk content self-heals on each launch so app upgrades propagate improved guidance.
  - **Sticky opt-out**: disabling or removing it is respected — a one-time sentinel (`~/.csp/skills/.seeded-csp-web-access`) prevents resurrection on later launches (mirrors the built-in `web-search` MCP seeding). Caveat: this is model-facing guidance, not a hard interception, so the planner *usually* — but not always — honours it. **Prefer the proxy system-prompt injection** (above) as the standing guarantee; the Skill remains belt-and-suspenders progressive disclosure.
- **Built-in `web-search` MCP connector (free, no API key)**: CSP now ships a bundled multi-provider web search + page fetch MCP server and seeds it into `~/.csp/mcp/inventory.json` on first run, enabled by default, so Claude Science has real search/fetch despite Anthropic's hosted `web_search` being unavailable under CSP virtual login. It is a self-contained **Python** stdio server (`web_search_server.py`, bundled via `include_str!` and deployed next to the Node shim); Python is used because its `requests`/`urllib` honour the injected `HTTPS_PROXY` and `CONNECT`-tunnel correctly, needing no shim. The interpreter is resolved to the sandbox's own Python and re-resolved on every deploy so the entry self-heals.
  - **Tools (via `host.mcp`, not bare top-level)**: `host.mcp("web-search", "web_search"|"search_literature"|"csp_web_search", …)` returns structured results; `fetch_url` / `web_fetch` return readable page text.
  - **Multi-provider with automatic fallback (OpenClaw-style)**: `provider="auto"` tries key-based providers first when their key is present, then the free scholarly providers, capturing a per-provider warning and falling through so one failing provider never fails the whole search.
  - **No-key defaults tuned to the sandbox**: a live probe showed Claude Science's operon proxy enforces a **scientific egress allowlist** (arXiv/Crossref/PubMed/OpenAlex/Semantic Scholar/pypi/notion tunnel through; DuckDuckGo/Wikipedia/Google/Bing and the paid search APIs are refused with `403`). The defaults are therefore the reliable no-key scholarly providers **Crossref, arXiv, PubMed** (with OpenAlex/Semantic Scholar selectable). General-web (`duckduckgo`/`wikipedia`) and paid (`brave`/`serper`/`tavily`) providers are implemented and selectable but best-effort — currently blocked in-sandbox by the allowlist.
  - **Optional API keys**: set `BRAVE_SEARCH_API_KEY`, `SERPER_API_KEY` or `TAVILY_API_KEY` in the connector's `env` via the MCP tab (edited like any other connector secret; never hardcoded). These providers are then preferred by `auto` and used once/if their domain becomes reachable.
  - **UI**: the MCP list labels built-in connectors with a **内置 / Built-in** badge and a tooltip explaining the free defaults, the optional keys, and the sandbox allowlist limitation. A one-time sentinel means disabling/removing the connector is respected on later launches.

### Docs
- **Known issues**: documented that proxy injects standing web-access guidance; corrected the false claim that `tools/list` advertising intercepts bare `web_search`; noted Stop-in-UI vs quit-app for stale Science daemon; kept *"Hosted vs CSP local environment conventions"* and `recognized_by_science=0` stamp caveat.

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