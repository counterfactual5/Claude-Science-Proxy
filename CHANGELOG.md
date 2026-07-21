# Changelog

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [2.2.0] — 2026-07-21

### Added
- **SSH config Include bridge**: Sandbox init now creates `~/.ssh/config` (0600) inside the sandbox HOME with an `Include` directive pointing to the real user's `~/.ssh/config`. This lets sandbox Science resolve SSH hosts / identity files exactly as the real user would — git clone over SSH, MCP via SSH, etc. — without copying keys or config. Fail-soft: if the real config doesn't exist or the sandbox `.ssh/` can't be created, the step is skipped (non-fatal).

### Fixed
- **Kimi K3 relay orphan tool blocks**: When Science replays multi-turn history after a failed tool call, orphan `tool_use` blocks (no matching `tool_result`) and orphan `tool_result` blocks (no matching `tool_use`) are now cleaned up before forwarding to Kimi on the relay path. Orphan `tool_result` is downgraded to a text block (content preserved as context); trailing orphan `tool_use` is removed. This prevents Kimi's `temporarily unavailable` loops caused by incomplete tool-call sequences.
- **Gemini `role: "model"` normalization**: Gemini sometimes returns `role: "model"` in assistant messages; `anthropic_to_openai` now normalizes it to `"assistant"` so strict OpenAI-compat endpoints don't 400.
- **Gemini/Grok safety finish_reason**: Non-standard `finish_reason` values (Gemini's `safety` / `recitaction` / `other`, Grok's `content_filter`) are no longer silently mapped to `end_turn`. A visible `[Content filtered by upstream: <reason>]` marker is appended to the response text so the user knows the upstream filtered or truncated.

### Changed
- **Strict model routing**: When the platter is active (registry + credentials present) and Science sends an unknown shell ID that is neither a registered route nor a known `FALLBACK_SHELL`, the proxy now returns HTTP 400 with a clear error message instead of silently falling back to `default_model`. This prevents the user from unknowingly using the wrong model after a Science update adds new shell IDs. To restore the old behavior, add the shell to the platter editor or `FALLBACK_SHELLS`.
- **Orphan org recovery UX**: When Science sign-out leaves multiple historical orgs with no `active-org.json`, the error message now lists the available org UUIDs ( `{org_list}`) so the user can manually write the desired one back without guessing.
- Version alignment (desktop bundle + Cargo + built-in web-search `SERVER_VERSION`) → **2.2.0**.

## [2.1.0] — 2026-07-21

### Added
- **Network pending approval UI**: When Operon denies CONNECT for an ungated host, `fetch_url` queues it; CSP → MCP →「待批准出网域名」lists pending hosts. Approve writes `~/.csp/network-allowlist.json`, merges org grants, and restarts Science so Operon reloads.
- **Common egress pre-grants**: Start merges a curated news/finance/US-gov/crypto/GitHub host set (Yahoo, Reuters, GovInfo, CoinDesk, CoinGecko, …) so everyday `fetch_url` needs fewer one-off approvals.
- **Fake-IP / egress status light**: Runtime status probes system DNS for Clash-style Fake-IP ranges and surfaces「出网受阻」when Operon would deny CONNECT.
- **Skill discover search**:「扫描导入」filters by name / description / source; match count shown. Already-owned skills (same path or name) badge「已有 · 默认保留」and stay unchecked so Import keeps CSP copies unless overwritten.
- **More Skill scan roots**: `~/.qoderworkcn/skills`, `~/.eigent/skills`, `~/.grok/skills`, `~/.mavis/skills`, `~/.stepfun/skills`, `~/.corust-agent/skills`.

### Fixed
- **CONNECT 403 messaging**: No longer always blames Fake-IP; prefers allowlist miss and points to CSP pending approval. Cloudflare challenge pages get a distinct hint.
- **`alwaysOrigins` grant path**: Network allowlist writes provenance under `approvalGrants.alwaysOrigins` (Science’s real path), with migration of misplaced stubs.
- **Skill discover `[hidden]` CSS**: `display:flex` no longer overrides filtered-out rows (search appeared broken).
- **Science rolling-compact forks**: Proxy detects compact/summarizer requests and strips the full tool list so third-party models (e.g. GLM) stop endless tool_use retries during context compaction.

### Changed
- Version alignment (desktop bundle + built-in web-search `SERVER_VERSION`) → **2.1.0**.

## [2.0.0] — 2026-07-19

### Added
- **Scan local LLM configs**: From「我的配置 → ⋯ → 扫描导入」, discover providers from agent/coding apps that store **local custom endpoints**: Zed, Continue, OpenCode (incl. v2 `settings.baseURL`), OpenClaw (+ `agents/*/models.json`), Factory, Cline (`~/.cline/data`), Aider, Codex (`model_providers`), **Qwen Code** (`~/.qwen/settings.json` `modelProviders.openai[]`), **iFlow** (`~/.iflow/settings.json`), **Crush** (`~/.config/crush/crush.json` `providers`, with `$VAR` env expansion), plus custom endpoints from **Cursor** (Override OpenAI Base URL in `state.vscdb`), **Claude Code** (`~/.claude/settings.json` `env.ANTHROPIC_BASE_URL`), and **Trae** (custom models in `state.vscdb`). Only pure account-login models with no custom endpoint are out of scope. Keys: config → env → Keychain; Cursor/Trae encrypt their keys (Electron safeStorage) so those import as “needs key”. UI shows key source badge.
- **Scan coverage note**: Roo Code / Cline / Kilo Code VS Code extensions and Windsurf / VS Code Copilot BYOK store the endpoint + key in VS Code SecretStorage (encrypted), so their custom base URL is not readable from disk and is intentionally skipped.
- **MCP scan parity**: MCP discovery now also covers **OpenCode** (`mcp` / `mcp.servers` in `opencode.json(c)`, with array-`command` + `environment` shapes), **Crush** (`mcp` in `crush.json`), **Qwen Code** & **iFlow** (`mcpServers` in their `settings.json`), and **Cline** (`cline_mcp_settings.json` under each host editor's globalStorage: Code / Cursor / Windsurf / Insiders) — matching the LLM scanner's tool set.
- **Skill scan roots**: Added `~/.config/opencode/skills` (OpenCode) and `~/.qwen/skills` / `~/.iflow/skills` (Qwen Code / iFlow), which follow the shared `SKILL.md` (agentskills.io) spec. Cline / Kimi / Zed / Warp already resolve to the shared `~/.agents/skills` root.
- **Domestic (China) agent coverage**: **QClaw 小龙虾** (Tencent's OpenClaw desktop wrapper, state dir `~/.qclaw` — LLM providers via `openclaw.json` + `agents/*/models.json`, MCP via `mcp.servers`, skills via `~/.qclaw/skills` / `skillhub-skills` / `workspace/skills`); **TRAE SOLO / TRAE SOLO CN** custom models (`state.vscdb`, same schema as Trae IDE); **Baidu Comate / Zulu** MCP (`~/.comate/mcp.json` + `mcp.local.json`); **Alibaba Lingma** MCP (`Lingma/mcp-config.json` per-OS app dir); **Qoder** skills (`~/.qoder/skills`); OpenClaw default workspace skills (`~/.openclaw/workspace/skills`). Qoder custom models and Comate/Lingma/CodeGeeX custom LLMs are account-side or IDE-encrypted (no local plaintext endpoint), so they stay out of the LLM scan by design.
- **QoderWork & Antigravity coverage**: **QoderWork** skills (`~/.qoderwork/skills` — the app's sole skills root) and home-dir MCP probes (`~/.qoderwork/.qoder.json` user-level `mcpServers` à la Claude Code, plus documented `~/.qoderwork/settings.json`); **Google Antigravity** skills now cover every variant's global root (`~/.gemini/config/skills`, `~/.gemini/antigravity/skills` IDE, `~/.gemini/antigravity-cli/skills` CLI, `~/.gemini/skills` shared) — Antigravity MCP (`~/.gemini/antigravity/mcp_config.json`) was already scanned. Both products' models are account-side (Google login / Qoder pool) with no local custom endpoint, so LLM scan is n/a.

- **Platter「browse all models」**: Each provider group in the multi-provider picker gains a "Browse all models" entry that pulls the upstream `GET /v1/models` catalog with the stored key (short-timeout scratch probe); un-enabled models render in a distinct dashed style and can be picked directly. Endpoints without `/v1/models` degrade to a manual model-ID input. Typing in the picker's search box auto-loads every provider's catalog (cached per session), so searching reaches models you haven't enabled yet; the search box is always visible and case-insensitive.
- **Model search in profile editor**: The enabled-models list pins enabled entries on top **in their configured order** and adds a case-insensitive search box for long catalogs (9+ models).
- **Message severity styles**: Feedback area is now an alert card with four levels — red `err`, yellow `warn`, green `ok`, gray `info` (progress) — and every success/progress call site was re-audited to use the right one (platter saved, Skill/MCP imported/restarted, LLM import summary, …).

### Fixed
- **Active platter hot-reload**: Saving the platter or editing a platter-member connection while platter is active restarts the formal proxy so `CSP_MODEL_REGISTRY` / `CSP_PROFILE_CREDENTIALS` match disk.
- **Delete platter member**: Deleting a profile used by the active platter stops the proxy (same safety as deleting the exclusive active profile).
- **Platter FALLBACK routing**: When Science sends `claude-haiku-4-5` (etc.) not present in a ≤2-model platter, fast/default FALLBACK uses the owning profile of `fast_model` / `default_model`, not always the first provider.
- **Discover duplicate rule**: A discovered provider is "already imported" only when **normalized URL + resolved key** both match an existing profile; same URL with a different key is a distinct account and stays importable. Mirrored copies of one provider (same URL + key seen in several config files) merge into a single row whose source labels stack (e.g. `Trae · TRAE SOLO CN`), and product variants get distinct labels (Trae CN, TRAE SOLO / TRAE SOLO CN, Cursor Nightly, QClaw).
- **OpenClaw/QClaw scan de-dup**: `openclaw.json` is the authoritative LLM source; the per-agent `agents/*/agent/models.json` runtime snapshot is only scanned as a fallback when the main config is absent (it mirrors the main config and doubled every provider).
- **Model order preservation**: The config layer no longer re-sorts `active_models` by version on import/save/load — the source tool's (or your manual) order is kept, and the default model is preserved as long as it stays in the list. Shell mapping / More-models menus sort internal copies only.
- **Platter picks stay platter-only**: Selecting a catalog/manual model no longer writes into the profile's `active_models` — providers remain independent; draft-selected models outside the enabled list still render (and are searchable) in their group.
- **Feedback layout**: The feedback card sits in normal flow — lists shrink (and scroll) instead of painting over it, and the area takes no space when empty; stale messages clear on every view change; info/success toasts no longer scroll the form (errors still do); the platter list keeps its scroll position across re-renders.
- **Menus & list UI**: Bottom-row card menus flip upward correctly (clip boundary is now `#profileList`, not `.panel-body`); model-search filtering actually hides items (CSS `display:flex` was overriding `[hidden]`); Skill discover rows are fully clickable with long descriptions collapsed behind an expand toggle; import feedback reports per-item ok / skipped / failed counts.

### Changed
- **Version alignment**: Desktop bundle, Skill download `User-Agent`, and built-in web-search `SERVER_VERSION` synced to **2.0.0**.
- Skills header button renamed **「手动导入 / Manual import」** to distinguish it from 「扫描导入 / Scan & import」.

## [1.9.0] — 2026-07-17

### Added
- **Multi-provider · custom models**: Fixed card under「我的配置」to pick up to 8 models across saved providers. First selected model is the Science default; fast model is inferred. Mutually exclusive with single-provider「当前生效」. Schema v5 adds `model_platter` + `active_mode`.
- **Cross-adapter platter routing**: When the platter is active, the proxy resolves each Science shell to the owning profile’s credentials (Anthropic relay, DeepSeek, or OpenAI-custom / Responses).

### Fixed
- Platter activation scratch probe uses the first entry’s real adapter (e.g. `openai-custom`), not the host `relay` process adapter — avoids false “upstream busy” failures on OpenAI endpoints.
- Single-provider model picker hard-caps Science enables at 8; create flow no longer auto-enables eight models by default.

### Changed
- **Version alignment**: Desktop bundle, Skill download `User-Agent`, and built-in web-search `SERVER_VERSION` synced to **1.9.0**.

### Docs
- Requirements plan: `docs/plans/2026-07-17-multi-provider-model-platter.md`. Local editor LLM config scan deferred to **v2.0** (`docs/plans/2026-07-17-scan-local-editor-llm-configs.md`).

## [1.8.2] — 2026-07-17

### Added
- **Sync Science skill library**: Replaces “Adopt from workspace” as the primary path. Scans `orgs/…/skills/` for CSP-managed drift (harvest back into `~/.csp/skills/`), optional new library imports, and unpublished workspace drafts. Deploy auto-harvests drift before wipe so Science Edit/chat edits are not lost.

### Fixed
- Science bundled skill `using-model-endpoint` no longer appears as a false “import” candidate during library sync.

### Changed
- **Version alignment**: Desktop bundle, Skill download `User-Agent`, and built-in web-search `SERVER_VERSION` synced to **1.8.2**.

## [1.8.1] — 2026-07-17

### Added
- **MCP 扫描预览**：扫描导入列表可预览源配置中该 server 的 JSON/TOML 片段，并以居中紧凑小窗展示（区别于 Skills 采纳的全屏预览）。
- **从 Science 采纳全屏预览**：工作区 Skill 采纳预览改为覆盖整个窗口的全屏层，便于阅读长 `SKILL.md`。

### Fixed
- MCP 预览按钮点击无响应（事件选择器与 `data-*` 属性不匹配）。
- 预览层「在 Finder 打开」改为 `open -R` 在 Finder 中选中路径；失败时在预览层内显示错误。
- 预览顶栏按钮边框对比度与布局：关闭移至右上角 **×**，「打开配置文件」独立显示。

### Changed
- **MCP 编辑表单紧凑化**：缩短字段间距、文本框行数与各字段提示文案，stdio 配置尽量一页放下、少滚动。
- **Version alignment**：桌面 bundle、Skill 下载 `User-Agent`、内置 web-search `SERVER_VERSION` 同步至 **1.8.1**。

## [1.8.0] — 2026-07-16

### Added
- **Skills 统一导入**：「导入 Skill」支持本地目录、本地 `.zip`、以及 `https://` URL（直链 zip 或公开 GitHub 仓库/目录）。下载与解压在 CSP 桌面端完成。
- **浏览**：单一「浏览」按钮直接打开原生选择器（macOS 同一面板可选 Skill 文件夹或 `.zip`）；粘贴后可直接「导入」（自动校验）。
- **MCP 远程连接**：新建/编辑 MCP 表单支持连接类型选择——本地 stdio（command/args/env）或远程 `streamable_http` / `sse`（url + 可选 headers）。远程按 Science `custom_mcp_servers` schema 部署到 org `operon-cli.db`（非 `local-mcp.json`）；headers 经 `headers_helper` 下发。扫描导入同时识别远程条目。旧 stdio inventory 向后兼容。

### Changed
- Inspect/import API：`source` + `importPath`（inspect 返回的暂存目录，避免 URL 重复下载）。
- 导入表单文案精简；去掉独立「检查」按钮。
- **Skills 列表操作**：主按钮改为「导入」；手写「新建 Skill」移入 `⋯`（与导入频率更匹配）。
- **扫描导入**：Skills / MCP 发现页说明进一步精简为「从其他 Agent 软件导入」；新增经文档核对的家目录级根——OpenClaw（`mcp.servers`）、AWS Kiro、CodeBuddy 旧版单文件、**WorkBuddy**（`~/.workbuddy/{.mcp.json,mcp.json,skills}`）、**Factory Droid**（`~/.factory/{mcp.json,skills}`）。工作区级配置（如 `.trae/mcp.json`、项目 `.cursor/mcp.json`）不在 HOME，故不扫描。远程解析同时兼容 Windsurf/Cascade 的 `serverUrl` 字段。
- **从 Science 采纳预览**：采纳列表每项可「预览」工作区草稿正文（可切换 `SKILL.md` / 伴随文件），并支持「在 Finder 打开」；已采纳条目也会显示真实内容，便于确认是否与聊天中的改进一致。
- **扫描根扩展**：Skills / MCP 扫描补充 Kimi Code（`~/.kimi-code/skills`、`~/.kimi-code/mcp.json`）与 MiniMax（`~/.minimax/skills`、`~/.minimax/mcp.json`）的 HOME 级默认路径。

## [1.7.2] — 2026-07-16

### Added
- **Skills「导入目录」文件夹选择器**: Hybrid UX — keep the path text field and add **Choose folder** / **选择文件夹** (native macOS picker). Selected path auto-fills the input and runs inspect.

## [1.7.1] — 2026-07-15

### Fixed
- **OpenAI-custom stream idle (120s)**: Science’s idle watchdog counts yielded protocol events (`message_start`, `content_block_delta`, …), not SSE comments or `ping` (both are ignored). The buffered openai-custom path now opens `message_start` + a text block immediately, then emits empty `text_delta` keepalives while waiting for upstream—so long sessions and **Resume** no longer die at ~120s with `stream idle: no events`. Corrects the incomplete keepalive from v1.6.10.
- **Skill scan import descriptions**: `SKILL.md` frontmatter `description: >-` / `|-` YAML block scalars are parsed fully (not just the first line).

### Docs
- **`docs/known-issues.md`**: New [OpenAI-compatible buffered streaming](#openai-custom-streaming) section (scope, v1.7.1 fix, remaining 429/slowness).
- **`docs/DEVELOPMENT.md`**, **`README`**, **`AGENT.md`**: Cross-links and proxy streaming path table.

## [1.7.0] — 2026-07-15

### Added
- **Skills「导入目录」**: Restored top-level **Import folder** / **导入目录** in the Skills `⋯` menu as a dedicated full-page form (replacing the nested “advanced” path under scan-import). Paste a known Skill folder path, inspect, and import with recursive copy of companion files (`USAGE.md`, `requirements`, scripts, etc.) into `~/.csp/skills/`.

### Fixed
- **OpenAI-compat / GLM `code 1210` hardening** (`proxy/compat/openai_chat_compat.py`): Long-session Anthropic → OpenAI Chat translation now flattens list/dict `tool_result` content to plain strings, reorders tool results to match assistant `tool_calls` order, re-files orphan tool results as user text (avoids dangling `role: tool` after Science history compaction), caps oversized single-message bodies, ensures `function.arguments` is always a JSON string, prefixes `is_error` tool results, silently drops extended-thinking blocks, and inserts placeholders for unsupported block types (image, server tools) instead of silently emptying turns.
- **Skill import validation**: Missing root `SKILL.md` is now an inspect error (not a warning); pointing at a `SKILL.md` file hints to select the parent directory.

### Changed
- **Version alignment**: Desktop bundle, embedded web-search `SERVER_VERSION`, and builtin test assertion synced to **1.7.0**.
- **Runtime status clarity**: One-sided proxy/Science states use compact labels, tooltips, and warn/ready styling so partial readiness is obvious.

## [1.6.14] — 2026-07-14

### Fixed
- **Skill/MCP changes apply while Science is running**: Create, import, enable/disable, adopt (Skills), and create/import/update/enable (MCP) redeploy into the sandbox and, if the sandbox is running, stop it so the UI can restart (`needs_restart`) — edits no longer wait silently for the next manual Start.
- **MCP UI i18n**: MCP list/form/discover copy and restart-status toasts follow the locale dictionaries (EN/ZH), including renamed form title id (`mcpFormTitle`).
- **Version alignment**: Embedded web-search `SERVER_VERSION` and its builtin test assertion synced to the desktop release version.

### Changed
- **Naming / cleanup**: Dropped unused `#[allow(dead_code)]` on `SkillStore::get` (now referenced); MCP action commands return `McpActionResult` / Skill equivalents with `needs_restart`.

## [1.6.13] — 2026-07-14

### Changed
- **Skills “打开文件夹” per card only**: Removed the list-header `⋯` → open-all-skills-root action. Each Skill card’s `⋯` menu again has **打开文件夹**, which reveals that skill’s managed folder via `open_skill_folder`.
- **Removed unused `open_skills_root`**: Dropped the Tauri command and `SkillStore::root_dir` helper that only served the header action.

## [1.6.12] — 2026-07-14

### Changed
- **Skill/MCP meta layout**: Size/path (or MCP command) stays on one row; requirement/env tags move to their own row — no orphan `·` separators when tags wrap.
- **Skills header “打开文件夹”**: Moved from the per-skill row menu into the list-header `⋯` menu; opens `~/.csp/skills/` via new `open_skills_root` command (`SkillStore::root_dir`).

### Fixed
- **Skill/MCP card ⋯ menus buried under the next card**: Row cards no longer use `overflow-x: hidden` (that clipping + stacking context hid menus); open `.pmenu-wrap` lifts to `z-index: 40`, matching Profiles.

## [1.6.11] — 2026-07-14

### Changed
- **Unified `+ 新建` chrome**: Config / Skills / MCP list headers use the same `+ 新建` primary + `⋯` cluster (MCP “新增” → “新建”); emoji plus signs dropped.
- **Skill import aligned with MCP scan-import**: Top-level “导入” folder flow removed; **扫描导入** is the primary path (common local Skill roots, including `~/.cursor/skills-cursor`). Manual path + inspect lives under **手动路径导入（高级）** on the discover page.

### Fixed
- **Inspection preview `[hidden]`**: `.inspection-preview` used `display: flex`, so bare `[hidden]` left an empty bordered box on Skill/MCP create forms until content existed — force `display: none !important` when hidden.
- **Skill/MCP requirement tags**: Wrap/ellipsis under narrow panes (`min-width: 0`, overflow-x hidden) so long env/req chips no longer blow out the list layout.

## [1.6.10] — 2026-07-14

### Added
- **Runtime status row**: Panel footer shows proxy / Science running state via `get_runtime_status` (idle, starting/stopping, proxy-only, Science-only, or both).
- **Skill / MCP exclusive full-page forms**: Create, import, discover, adopt, and MCP add/edit no longer use modals — list hides while an opaque wizard-like page is open (same pattern as the config wizard). Menu item emoji icons removed.
- **Extra provider presets**: Groq, Gemini, Together, Fireworks, SiliconFlow, DashScope (CN/Intl), Doubao / Volcengine Ark, and Doubao Coding Plan in the config combobox.

### Fixed
- **OpenAI-compat / GLM tool-call content**: Assistant messages with `tool_calls` now send `content: ""` instead of `null` (several gateways reject null).
- **OpenAI-custom SSE keepalives**: Buffered upstream POST emits SSE keepalives during TTFT so Science does not idle-timeout with “Connection issue — retrying…”.
- **Dated Science shell ids**: Model registry strips trailing `YYYYMMDD` (e.g. `claude-haiku-4-5-20251001`) when resolving routes so background agents are not left unmapped.

### Changed
- **`search_skills` standing guidance**: Proxy injection, `csp-environment` Skill, and UI hints require non-empty `query` or `prefix` (never empty args; empty fails with `Missing 'query' argument`).
- **Skill description clamp**: Long skill/MCP descriptions can expand/collapse in the list (“更多” / “收起”).

## [1.6.9] — 2026-07-14

### Fixed
- **Stale sandbox MCP after app rebuild**: Rebuilding the `.app` does **not** reload a CSP process that was already running. An old desktop binary keeps embedding the previous `web_search_server.py` and a later **Start** can rewrite Wikipedia back onto GENERAL auto — matching reports of `duckduckgo_lite anti-bot` warnings followed by Wikipedia hits. Opening the desktop app now refreshes `~/.csp/sandbox/.../mcp/csp-web-search-server.py` immediately (same bytes as Start); after any rebuild you must **quit and reopen CSP**, then **Stop→Start** Science.

## [1.6.8] — 2026-07-14

### Fixed
- **DuckDuckGo Lite anti-bot (botnet / `anomaly.js`) on GENERAL**: Lite HTML often returns a temporary interstitial after rapid queries. `duckduckgo_lite` now warms a cookie session on the Lite homepage, retries once with backoff, falls back to GET `?q=`, parses `result-link` anchors first, and treats `anomaly.js`/`cc=botnet` as the challenge signal (not bare "challenge"). GENERAL still does **not** fall through to Wikipedia — empty + honest `hint` when free providers fail.
- **Wikipedia-only Instant Answer short-circuit**: When `duckduckgo_ia` returns only `wikipedia.org` AbstractURL hits, GENERAL continues to `duckduckgo_lite` for broader web results (keeps IA as soft fallback if Lite fails). This stops Science from treating entity Instant Answers as "GENERAL = Wikipedia".
- **False "fell back to Wikipedia / need API keys" narrative**: Proxy standing guidance, `csp-environment`, tool descriptions, and empty-GENERAL hints explicitly forbid claiming GENERAL fell back to Wikipedia or that Brave/Serper/Tavily keys are required when IA is empty or Lite is briefly challenged. Wikipedia-only lists from `search_literature` are expected — do not conflate lanes. **Stop→Start** Science after upgrading so the sandbox MCP script is rewritten (stale pre-1.6.7 scripts still had Wikipedia on GENERAL auto).

## [1.6.7] — 2026-07-14

### Changed
- **GENERAL auto no longer falls back to Wikipedia**: `csp_web_search` `provider=auto` is now optional keyed Brave/Serper/Tavily → `duckduckgo_ia` → `duckduckgo_lite` only. Wikipedia stays on the **LITERATURE** lane (`search_literature` auto: wikipedia → Crossref → arXiv → PubMed) where it belongs as an academic/encyclopedic source — not as a general-web fallback. Proxy guidance, `csp-environment`, inventory description, empty-result hints, and tests updated; empty GENERAL payloads still state API keys are **not** required (rephrase / optional paid APIs improve quality).

## [1.6.6] — 2026-07-14

### Changed
- **Unify GENERAL web search to one public MCP method `csp_web_search`**: `tools/list` no longer advertises both `web_search` and `csp_web_search` (that made models treat them as two search products). Canonical / advertised GENERAL name is **`csp_web_search` only**; `web_search` remains an **unlisted dispatch alias** for old sessions, proxy remnants, and skills. Native Anthropic OPERON tool `web_search` is still unavailable and must never be called top-level — that is a different layer from the MCP method. Proxy standing guidance, `csp-environment` Skill, inventory descriptions, and UI hints updated accordingly.

## [1.6.5] — 2026-07-14

### Fixed
- **GENERAL web search empty Instant Answer → false “need API key” story**: DuckDuckGo Instant Answer often returns empty JSON for news/"latest …" queries (not a missing key and not a network failure). Auto now falls through to free `duckduckgo_lite` (Lite HTML) then `wikipedia`; empty-result payloads include an explicit `hint`/`message` that keys are optional. Proxy + `csp-environment` guidance tell the model `csp_web_search` ≡ `web_search` (alias) and not to demand Brave/Serper/Tavily.
- **Allowlist**: pre-grant `lite.duckduckgo.com` alongside the other DuckDuckGo hosts.

## [1.6.4] — 2026-07-14

### Changed
- **Built-in Skill renamed `csp-web-access` → `csp-environment`**: The standing-guidance Skill is now the CSP **environment handbook** (dual web-search lanes, `/mnt/data` / `save_artifacts`, CJK fonts, `host.skills.publish` / analysis python env, network allowlist). Bundled path is `skill_manager/csp-environment/`; seed sentinel is `~/.csp/skills/.seeded-csp-environment`.
- **Sticky opt-out migration**: On launch, if the new sentinel is absent: replace an inventory `csp-web-access` builtin with a seed of `csp-environment`; if only the legacy `.seeded-csp-web-access` sentinel remains (user removed the skill), stamp the new sentinel **without** reseeding; otherwise seed normally. Proxy injection sentinel `CSP_WEB_ACCESS_GUIDANCE` is unchanged (injection, not the Skill name).

## [1.6.3] — 2026-07-14

### Changed
- **Split web-search into GENERAL vs LITERATURE lanes**: `web_search` / `csp_web_search` use auto Brave/Serper/Tavily → `duckduckgo_ia`; `search_literature` uses auto wikipedia → Crossref → arXiv → PubMed. Proxy + `csp-web-access` guidance updated so product/news queries stop defaulting into the academic tool.

## [1.6.2] — 2026-07-14

### Changed
- **`web-search` `provider=auto` prefers general web before scholarly**: With CSP network grants in place, free auto fallbacks are now `duckduckgo_ia` → `wikipedia` → Crossref → arXiv → PubMed (key-based Brave/Serper/Tavily still first when env keys are set). Proxy standing guidance and `csp-web-access` updated so product/news/"latest" queries are not stuck on academic-only results. HTML `duckduckgo` remains out of auto.

## [1.6.1] — 2026-07-14

### Added
- **Science network allowlist for built-in web-search (+ user JSON)**: On each Start / MCP deploy, CSP writes Operon network grants for all bundled `web-search` provider hosts (DuckDuckGo, Wikipedia, Brave, Serper, Tavily) into the active org `preferences.json`, so API keys configured in the MCP tab work without a separate manual grant. Extra hosts go in `~/.csp/network-allowlist.json` (MCP `⋯` → **网络授权配置**); Stop → Start after edits.

## [1.6.0] — 2026-07-14

### Added
- **Skills primary “新建”**: The Skills tab now has a primary **新建** action for authoring a new `SKILL.md` directly in CSP; importing an existing Skill directory moved into the header/row `⋯` menu.

### Fixed
- **Bare `web_search` OPERON not-found via proxy system injection**: Under CSP virtual login, Anthropic-native `web_search` / `web_fetch` are stripped from OPERON's toolset, so bare top-level calls fail with `Tool 'web_search' not found on agent 'OPERON'`. Local MCP tools are only callable via `repl` → `host.mcp("web-search", "<method>", ...)`. On every Anthropic-shaped `/v1/messages` request that already has a `system` prompt, the proxy idempotently appends a standing block (sentinel `<!-- CSP_WEB_ACCESS_GUIDANCE -->`) telling the model to use `host.mcp("web-search", "search_literature", …)` / `fetch_url` and never bare `web_search`/`web_fetch`. Applied on Anthropic passthrough and OpenAI translation paths.
- **Current date/time in standing web-access guidance**: Science/glm has no reliable wall clock and a ~early-2024 knowledge cutoff, so "今天什么时间" and year-sensitive searches went wrong. The same proxy injection now includes a request-time `Current local date/time: …` line (`datetime.now().astimezone()`) and tells the model to treat it as "today" for date answers, freshness ranking, and search-query years. If the sentinel is already present (e.g. a prior turn's date in a carried `system`), the block is rewritten rather than duplicated.
- **`host.mcp` return-shape guidance**: Search returns a dict `{"provider","query","results":[…],"warnings"}` — use `data = host.mcp(...); hits = data["results"]` (or `print(data)`), not iterate the dict as hit objects. Fetch returns `{"url","status","content"}`. Documented in tool descriptions, inventory copy, proxy standing guidance, and `csp-web-access`.
- **MCP script rewrite → sandbox restart**: `write_web_search_server` reports whether disk bytes changed; `deploy_sandbox_mcp` ORs that into `changed` so Start Claude Science / one_click_login restarts a running sandbox when the embedded script was rewritten.

### Changed
- **Corrected misleading `web_search` / native-vs-MCP copy**: Built-in MCP ads and known-issues no longer claim that advertising names in `tools/list` intercepts bare native calls. Truth: bare native tools are unavailable; `tools/list` names are `host.mcp` method names only; proxy prompt injection is the standing fix. Known-issues also notes Stop-in-UI vs quit-app for a leftover `claude-science serve` daemon.

### Docs
- **Known issues**: documented proxy standing web-access guidance, native vs MCP tool calling, return-shape pitfalls, and sandbox restart-on-script-rewrite behavior.

## [1.5.0] — 2026-07-14

### Changed
- **Built-in `csp-web-access` Skill broadened to full CSP environment conventions**: The standing-guidance Skill (bundled `SKILL.md`, seeded into `~/.csp/skills/` and deployed each launch) now teaches Claude Science the local sandbox conventions that differ from Anthropic's hosted environment, not just web search:
  - **Files/artifacts**: `/mnt/data` (and other `/mnt/...`) do not exist locally — save outputs to the current working directory (the active workspace `orgs/<org>/workspaces/<ws>/`) with relative paths, use `/tmp` only for scratch, and persist user-visible files by writing them in the workspace then calling `save_artifacts([...])`.
  - **Plotting/CJK**: matplotlib's default `DejaVu Sans` can't render CJK; set `plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]` and `axes.unicode_minus = False` before plotting non-Latin labels (guidance-only — the host already ships these fonts; no font binary is bundled).
  - **Web/network** and **skills/env** reminders: prefer the local `web-search` MCP over the hosted `web_search` tool; don't rely on `host.skills.publish()` for durable installs (draft in the workspace and adopt via the Skills tab); scientific packages live in the analysis `python` env, not necessarily the MCP env.
  - The Skill keeps its name `csp-web-access` (renaming would force a sentinel/inventory migration); only its content and one-line description were broadened. Its on-disk copy self-heals on every launch (`refresh_builtin` rewrites the bundled content while preserving enabled/disabled state), so this updated guidance propagates to already-seeded users on the next launch without resurrecting a removed Skill.

### Fixed
- **Built-in `web-search` MCP tool-name conflict with the hosted `web_search`**: The bundled connector previously advertised a tool literally named `web_search`, which collides with Anthropic's hosted `web_search` tool. Given the name clash, Claude Science's planner selected the hosted tool — unavailable under CSP virtual login — and failed with `Tool 'web_search' not found on agent`, then fell back to OpenAlex/literature tools **without routing to the local MCP**. The server now advertises **distinct, planner-friendly names** — `search_literature` (primary), its alias `csp_web_search`, and `fetch_url` — and each tool description explicitly instructs the model to *use this local tool instead of the hosted 'Web Search'*. The bare `web_search` name is retained only as a hidden, un-advertised dispatch alias for backward compatibility (never in `tools/list`), so it can no longer shadow or be shadowed by the hosted tool. To trigger the local connector, prompt Science with `search_literature` / `csp_web_search` / the `web-search` MCP by name rather than "Web Search".

### Added
- **Built-in `csp-web-access` Skill (standing web-search guidance, on by default)**: CSP now seeds a small CSP-managed Skill into `~/.csp/skills/` on first run, enabled by default, so Claude Science automatically prefers the local `web-search` MCP in **every** session — the user no longer has to say "use the local web-search MCP" each time. Its bundled `SKILL.md` (embedded via `include_str!`) instructs Science that for ANY web search or page fetch it must use the local **`web-search`** connector (`search_literature` / `csp_web_search` to search, `fetch_url` to read pages) and must NEVER call the hosted `web_search` tool (unavailable under CSP virtual login, which otherwise wastes a turn on `Tool 'web_search' not found on agent`), and notes the sandbox egress allowlist favours scholarly sources (Crossref / arXiv / PubMed / OpenAlex / Semantic Scholar).
  - **Seeded + deployed like the built-in connector**: it is deployed to `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/csp-web-access/` on every **Start Claude Science**, appears in the **Skills** tab with a **内置 / Built-in** badge, and its on-disk content self-heals on each launch so app upgrades propagate improved guidance.
  - **Sticky opt-out**: disabling or removing it is respected — a one-time sentinel (`~/.csp/skills/.seeded-csp-web-access`) prevents resurrection on later launches (mirrors the built-in `web-search` MCP seeding). Caveat: this is model-facing guidance, not a hard interception, so the planner *usually* — but not always — honours it.
- **Built-in `web-search` MCP connector (free, no API key)**: CSP now ships a bundled multi-provider web search + page fetch MCP server and seeds it into `~/.csp/mcp/inventory.json` on first run, enabled by default, so Claude Science has real `web_search`/`fetch_url` despite Anthropic's hosted `web_search` being unavailable under CSP virtual login. It is a self-contained **Python** stdio server (`web_search_server.py`, bundled via `include_str!` and deployed next to the Node shim); Python is used because its `requests`/`urllib` honour the injected `HTTPS_PROXY` and `CONNECT`-tunnel correctly, needing no shim. The interpreter is resolved to the sandbox's own Python and re-resolved on every deploy so the entry self-heals.
  - **Tools**: `search_literature(query, max_results=5, provider="auto")` (alias `csp_web_search`) returns structured results (`provider`, `title`, `url`, `snippet`, `published`/`source`); `fetch_url(url, max_chars=8000)` returns readable page text. (Distinct names avoid clashing with the hosted `web_search`; see the Fixed entry above.)
  - **Multi-provider with automatic fallback (OpenClaw-style)**: `provider="auto"` tries key-based providers first when their key is present, then the free scholarly providers, capturing a per-provider warning and falling through so one failing provider never fails the whole search.
  - **No-key defaults tuned to the sandbox**: a live probe showed Claude Science's operon proxy enforces a **scientific egress allowlist** (arXiv/Crossref/PubMed/OpenAlex/Semantic Scholar/pypi/notion tunnel through; DuckDuckGo/Wikipedia/Google/Bing and the paid search APIs are refused with `403`). The defaults are therefore the reliable no-key scholarly providers **Crossref, arXiv, PubMed** (with OpenAlex/Semantic Scholar selectable). General-web (`duckduckgo`/`wikipedia`) and paid (`brave`/`serper`/`tavily`) providers are implemented and selectable but best-effort — currently blocked in-sandbox by the allowlist.
  - **Optional API keys**: set `BRAVE_SEARCH_API_KEY`, `SERPER_API_KEY` or `TAVILY_API_KEY` in the connector's `env` via the MCP tab (edited like any other connector secret; never hardcoded). These providers are then preferred by `auto` and used once/if their domain becomes reachable.
  - **UI**: the MCP list labels built-in connectors with a **内置 / Built-in** badge and a tooltip explaining the free defaults, the optional keys, and the sandbox allowlist limitation. A one-time sentinel means disabling/removing the connector is respected on later launches.

### Docs
- **Known issues**: added a *"Hosted vs CSP local environment conventions"* section (`/mnt/data`, CJK fonts, hosted `web_search`, `host.skills.publish`, and the analysis-vs-MCP Python env split), plus an explanation of why a CSP-managed Skill can log `recognized_by_science=0`. Audit finding: Science writes each Skill's `.catalog_stamp` **once at the initial org catalog build** and does not re-stamp folders added afterward, so the post-launch stamp check is a false-negative for Skills added to an already-initialized org (the Skill is still deployed correctly and searched by relevance); a fresh org deploys Skills before the first catalog build and stamps them normally.

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