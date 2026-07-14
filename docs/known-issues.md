# Known issues (user-facing)

Current **limitations and open problems** visible to end users. Fixed items are in [`CHANGELOG.md`](../CHANGELOG.md).

> **Product:** Claude Science Proxy (CSP) · config `~/.csp/CSP.json` · feedback via [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues) only.

---

<a id="architecture-boundaries"></a>

## Architecture boundaries (not bugs)

These depend on a **real Anthropic / claude.ai account** and are **unavailable or fast-fail** under third-party models + local virtual login:

- Anthropic-hosted remote services (`*.mcp.claude.com`, directory connectors, cloud-hosted skills)
- Features that show session expired / unavailable on official hosted services

See also README [Current limitations](../README.md#current-limitations).

---

<a id="port-occupancy"></a>

## Port occupancy

When switching ports or starting the proxy, errors should **name port occupancy** clearly—not a vague “invalid key”. If you still see a mismatch, open an issue with a redacted snippet from `~/.csp/logs/`.

---

<a id="dsml-leak"></a>

## DeepSeek tool calls and DSML leakage

Some DeepSeek responses leak `tool_use` as plain text (`<｜｜DSML｜｜>` etc.), which can stall web search / tool chains. Root cause is **upstream model output**, not virtual login. Optional DSML shim defaults to **off**; see CHANGELOG and `proxy/dsml/dsml_shim.py`.

---

<a id="virtual-model-registry"></a>

## Virtual model registry and force-model shell

Science only accepts `claude-*` model IDs. With `CSP_MODEL_REGISTRY`, CSP maps up to eight shell IDs to real upstream models; without it, relay/OpenAI paths fall back to a single `claude-opus` shell with force-overridden outbound routing. See `proxy/registry/model_registry.py` and `proxy/registry/model_discovery.py`.

---

<a id="custom-endpoint-scratch"></a>

## Custom endpoint scratch probe (needs repro)

Some custom relay / OpenAI endpoints work in user `curl` but fail panel scratch probe with “network/upstream busy”. Needs concrete `base_url`, model, and probe logs to diagnose.

---

<a id="science-version-drift"></a>

## Science version drift

Claude Science binary updates can change virtual OAuth, routing, and package-proxy behavior. The capability catalog records known version boundaries; if something breaks after upgrading Science, report **Science version + CSP version**.

---

<a id="2a"></a>

## Science OAuth refresh outside ANTHROPIC_BASE_URL

Some Science builds (e.g. `0.1.15-dev` family) may refresh OAuth via hardcoded `api.anthropic.com/oauth/token` **outside** `ANTHROPIC_BASE_URL`. Virtual OAuth can then look “logged out”. CSP documents this as a version/auth boundary—not real-account success. CONNECT fast-fail to Anthropic hosts is intentional (`proxy/core/csp_proxy.py`).

---

<a id="11"></a>

## External MCP egress and CONNECT tunneling

Non-Anthropic `CONNECT` opens a **direct TCP tunnel** today. The CSP proxy also **forwards absolute-form proxied requests** (e.g. `GET https://api.notion.com/…` sent in plain HTTP to the proxy) to non-Anthropic upstreams and relays the response. This matters because the sandbox blocks direct DNS/egress for MCP child processes, so a local stdio MCP server must reach its own API through the loopback proxy — and some HTTP clients (notably **axios**, used by `@notionhq/notion-mcp-server` and others) do **not** CONNECT-tunnel when `https_proxy` is set; they send the origin request to the proxy in absolute form. Anthropic domains still fast-fail (401) on this path, and the forward proxy never injects provider keys (the MCP's own auth headers pass through unchanged). See `do_CONNECT` / `_maybe_forward_absolute` in `proxy/core/csp_proxy.py`.

---

<a id="directory-connectors"></a>

## Directory connectors, hosted skills, and remote official skills

Directory connectors and official remote skills depend on claude.ai hosted services and real account state. Virtual login cannot make them available. Local / GitHub skills need post-install discoverability checks—installing alone does not prove Science can use them.

---

<a id="skill-manager"></a>

## Local Skill Manager

The **Skills** tab imports local Skill directories (folders containing a `SKILL.md`) into managed storage at `~/.csp/skills/`, where you can list, enable/disable, remove, and **discover** them from common agent locations (`~/.agents/skills`, `~/.codex/skills`, `~/.claude/skills`, `~/.cursor/skills`, plus domestic tools `~/.trae/skills` and `~/.codebuddy/skills`). **From Science adopt** (Skills `⋯` menu) scans Skill drafts Claude Science left under `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/workspaces/` — `*.skill.md`, `*_SKILL.md`, or folders with `SKILL.md`, plus companion files such as `kernel.py` — and copies selected drafts into `~/.csp/skills/` on your confirmation. Science cannot call `host.skills.edit()` / `host.skills.publish()` under CSP virtual login; workspace adopt is the supported path to turn Science-generated drafts into managed Skills. Re-adopting the same workspace key updates the existing inventory entry instead of duplicating it. On each **Start Claude Science**, enabled Skills are deployed into the sandbox at `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/<name>/` — the org-scoped directory current Claude Science builds actually scan and stamp (pure disk scan, no allowlist). CSP also cleans up Skills that older builds deployed to the legacy root `…/.claude-science/skills/`. The deployer only manages folders it marks with `.csp_managed`, so Science's bundled scientific Skills (alphafold2, boltz, …) are never removed, and a user Skill whose name collides with a bundled one is skipped (logged in `~/.csp/logs/sandbox.log`). After launch, CSP checks whether Science wrote a `.catalog_stamp` into each deployed folder and logs `recognized_by_science=<n>` as a self-verification. Requirement detection (`python`, `node`, `rust`, `mcp`, …) is a filename/extension heuristic and may miss or over-report. Deployment is idempotent (a manifest signature skips unchanged redeploys); since Science reads Skills only at launch, changes apply on the next **Start Claude Science**, and a running sandbox is restarted automatically when the deployed set actually changed. Two Skills whose names sanitize to the same folder — and symlinks inside a Skill — are skipped for safety. Note Science loads Skills on demand (searched by relevance), so an imported Skill is indexed but not force-loaded into every conversation. Writes never touch real `~/.claude-science` state.

**Built-in `csp-web-access` Skill (progressive disclosure, enabled by default).** Because the hosted `web_search` tool is unavailable under CSP virtual login (see the built-in `web-search` MCP connector below), Science's planner would otherwise try bare `web_search` and fail with `Tool 'web_search' not found on agent`. CSP still **seeds a small CSP-managed Skill named `csp-web-access` into `~/.csp/skills/` on first run**, enabled by default, deployed to `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/csp-web-access/` on every **Start Claude Science**, with a **Built-in** badge and sticky opt-out via sentinel `~/.csp/skills/.seeded-csp-web-access`. Content self-heals on each launch. **Caveat:** Skills are progressive-disclosure (not in every session's system prompt) and alone are insufficient — the **real standing fix** is proxy system-prompt injection (see [env conventions](#env-conventions) / [local MCP](#local-mcp)). The Skill remains belt-and-suspenders for relevance-ranked loading.

---

<a id="env-conventions"></a>

## Hosted vs CSP local environment conventions

Claude Science running inside CSP is a **local sandbox**, not Anthropic's hosted Claude environment, so several habits carried over from the hosted product silently fail. The built-in **`csp-web-access`** Skill (see above) now bundles these as standing, per-session guidance; they are also documented here for operators.

- **No `/mnt/data` (and no other `/mnt/...`).** The hosted environment exposes `/mnt/data` and `/mnt/user-data`; **these do not exist locally**, so writes there fail or vanish. Save outputs to the **current working directory** — the active Science workspace `orgs/<org_uuid>/workspaces/<workspace_uuid>/` — using **relative paths**. Use `/tmp` only for disposable scratch. To surface a **user-visible** file, write it in the workspace and then call **`save_artifacts([...])`** (writing the file alone does not expose it).
- **CJK plotting fonts.** matplotlib's default `DejaVu Sans` cannot render Chinese/Japanese/Korean glyphs, so CJK labels render as tofu boxes (□□□). Before plotting non-Latin labels, set a CJK-capable font that exists on the macOS host, e.g. `plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]` and `plt.rcParams["axes.unicode_minus"] = False`. This is **guidance-only** (no font binary is bundled and CSP does not patch the sandbox matplotlib rc): the host already ships these fonts, so a two-line `rcParams` set is the low-risk fix. If you use the `figure-style` skill, pass it a CJK font the same way.
- **Hosted `web_search` is unavailable; use `repl` → `host.mcp`.** Under CSP virtual login the Anthropic-hosted `web_search` / `web_fetch` tools do not exist in OPERON's toolset and fail with `Tool 'web_search' not found on agent` if called bare. Local MCP tools are **not** top-level model tools. Search/fetch via the `repl` tool:
  - `host.mcp("web-search", "search_literature", query="...", max_results=N)` (aliases: `"web_search"`, `"csp_web_search"`) — returns a **dict** with `"results"` (list of hit dicts); use `data["results"]`, not `for x in data`
  - then `host.mcp("web-search", "fetch_url", url="...")` when needed — returns `{"url","status","content"}`
  The CSP proxy injects this as standing system guidance on every `/v1/messages` request that already has a `system` prompt (sentinel `<!-- CSP_WEB_ACCESS_GUIDANCE -->`), including a **request-time current local date/time** line so the model treats that as "today" for date answers and search-year freshness (Science itself has no reliable wall clock; cutoff ~early 2024). Re-advertising names in MCP `tools/list` alone **cannot** intercept bare native calls. Egress is a scholarly allowlist (Crossref / arXiv / PubMed / OpenAlex / Semantic Scholar / Notion / PyPI reliable; general and paid search engines usually blocked). See [Built-in `web-search` connector](#local-mcp).
- **No wall clock inside Science.** The model cannot read the host clock; without guidance it answers "can't get real-time date" or skews search years to the training cutoff. CSP's proxy standing injection supplies `Current local date/time: …` on each Anthropic-shaped `/v1/messages` transform (and refreshes that block if an older sentinel is already in `system`).
- **`host.skills.publish()` / `host.skills.edit()` don't persist.** These hosted skill-management calls do not take effect under CSP virtual login. To install a durable skill, draft the files in the workspace and adopt them via **Skills tab → 从 Science 采纳 (adopt from Science)**; CSP then deploys them into the sandbox. See [Local Skill Manager](#skill-manager).
- **Two Python environments.** The **analysis `python` env** carries the scientific stack (numpy / pandas / matplotlib / scipy, …) and is where computation and plotting should run. The **MCP Python env** (used by stdio MCP servers) may **not** have plotting or scientific packages, so don't assume they're importable from an MCP tool context.

### Why a CSP-managed Skill can log `recognized_by_science=0`

After launch CSP checks whether Science wrote a `.catalog_stamp` into each deployed folder and logs `recognized_by_science=<n>`. On current Science builds (audited on `0.1.17-dev`) this stamp is written **once, at the initial org catalog build** — every bundled Skill's `.catalog_stamp` shares one identical timestamp/value — and folders added **after** that build are **not** re-stamped on later launches. Consequently a CSP-managed Skill deployed into an **already-initialized** org (e.g. `crypto-data`, or `csp-web-access` on an existing install) can stay unstamped and log `recognized_by_science=0`. This is a **false negative of the stamp heuristic**, not proof Science can't load the Skill: Science's live catalog lives in `orgs/<org>/operon-cli.db` and on-disk Skills are searched by relevance regardless of the stamp. It is **not** a `SKILL.md` frontmatter/format problem (`crypto-data` has valid `name`/`description` frontmatter identical in shape to recognized bundled Skills, yet is unstamped) nor a directory-naming problem. On a **fresh** org, CSP deploys Skills *before* Science's first catalog build, so the built-in Skill is stamped and recognized normally. If an existing install shows `recognized_by_science=0` and you want the on-disk stamp to flip, start Science with a fresh org (new virtual login) so the catalog is rebuilt with the Skill already present.

---

<a id="local-mcp"></a>

## Local stdio MCP connectors

The **MCP** tab manages local **stdio** MCP connectors (custom `command` + `args` + `env`) stored at `~/.csp/mcp/inventory.json`, where you can add, edit, enable/disable, remove, and **discover** them from common AI clients (Cursor, Claude Desktop/Code, Codex `config.toml`, Devin Desktop, VS Code, Zed, and domestic tools Qoder / 通义灵码, Trae / TRAE SOLO, CodeBuddy). On each **Start Claude Science**, enabled connectors are written to the sandbox at `$SANDBOX_HOME/.claude-science/mcp/local-mcp.json` — the file Claude Science reads for user stdio connectors (confirmed against a live sandbox: they surface with `source: local-stdio`, `transport: stdio`). Because Science's restricted MCP child sandbox can only read paths granted via `config.toml`, CSP also merges the parent directory of every absolute path a connector references into `[sandbox] user_read_paths` (least privilege; only that key is owned, all other `config.toml` keys are preserved). Disabling all connectors removes `local-mcp.json` and CSP's read grants so nothing lingers.

**Built-in `web-search` connector (free, no API key).** Because Claude Science lacks Anthropic's hosted `web_search` tool under CSP's virtual login (the sandbox log shows `Tool 'web_search' not found on agent 'OPERON'`), CSP ships a bundled multi-provider search + fetch MCP server and **seeds it into the inventory on first run**, enabled by default. It is a small self-contained **Python** stdio server (`web_search_server.py`, bundled via `include_str!` and written to `$SANDBOX_HOME/.claude-science/mcp/csp-web-search-server.py` at deploy time, like the Node shim). Python is chosen deliberately: unlike the Node/axios stacks, Python's `requests`/`urllib` honour the injected `HTTPS_PROXY` and issue a proper `CONNECT` tunnel, so it needs no shim. The connector's interpreter is resolved to the sandbox's own `claude-science-mcp` conda env Python (falling back to the `python` env, then `python3`) and re-resolved on every deploy so it self-heals. Its inventory description is also refreshed on each launch / deploy for already-seeded users. Call via **`repl` → `host.mcp`** (not as bare top-level tools):

- `host.mcp("web-search", "search_literature"|"web_search"|"csp_web_search", query=..., max_results=5, provider="auto")` — scholarly / web search. **Return:** `host.mcp` parses tool JSON into a **dict** `{"provider", "query", "results": [{title, url, snippet, source, …}], "warnings"}`. Use `hits = data["results"]` then iterate; do not enumerate the dict itself (yields string keys → `AttributeError` on `.get`).
- `host.mcp("web-search", "fetch_url"|"web_fetch", url=..., max_chars=8000)` — fetch a page; returns dict `{"url", "status", "content"}`.

**Why `tools/list` re-advertising alone is insufficient.** An earlier fix re-advertised `web_search` / `web_fetch` in the local connector's `tools/list` hoping model-native calls would resolve there. That was the **wrong layer**: bare `web_search` is an Anthropic *native server tool*; under CSP virtual login it is stripped from OPERON's toolset before any MCP routing. Local MCP tools are never top-level model tools — they are only reachable via `host.mcp`. `tools/list` names remain useful as **`host.mcp` method names**, but standing reliability comes from the **proxy system-prompt injection** (`inject_csp_web_access_guidance` in `proxy/compat/anthropic_compat.py`) on every Anthropic-shaped `/v1/messages` request that already has a `system` prompt.

**Deploy / restart.** On each **Start Claude Science** / one_click_login, MCP deploy rewrites the sandboxed script from the embedded `include_str` when bytes differ, and that rewrite marks deploy as changed so a running sandbox is restarted (MCP children otherwise keep the old script in memory). **Quitting the CSP app alone does not stop a leftover `claude-science serve` daemon** — use **Stop Claude Science** in the CSP UI before Start again. Aggressive kill-on-quit is a known follow-up (not done here to avoid risky process matching).

**Egress reality — the operon allowlist.** A live probe of the operon per-child proxy (which is what MCP children get as `HTTPS_PROXY`, on a per-child ephemeral port, **with no token in the URL** — hence the shim's tokenless `CONNECT` works) showed that operon enforces a **curated egress allowlist of scientific sources**, mergeable with the org's Science network grants (`preferences.json` → `userAllowedDomains` plus `approvalGrants.always.allow.network` / `alwaysOrigins.network`). Built-in scholarly hosts tunnel without grants: `arxiv.org`, `api.crossref.org`, `eutils.ncbi.nlm.nih.gov`, `api.openalex.org`, `api.semanticscholar.org`, `api.notion.com`, `pypi.org` (200/401/429). Without user grants, general engines / paid APIs still get **`403 Forbidden` on CONNECT** (`duckduckgo.com`, `html.duckduckgo.com`, `api.duckduckgo.com`, `en.wikipedia.org`, `google.com`, `bing`, `api.search.brave.com`, `google.serper.dev`, `api.tavily.com`, …). **Plan A probe (2026-07-14):** writing `html.duckduckgo.com` + `api.duckduckgo.com` into those grant fields and **restarting** Science (disk edit alone does not hot-reload the live operon proxy) flipped DDG CONNECT from 403 → 200; Crossref/CoinGecko stayed 200. Through the same proxy path, `provider=duckduckgo_ia` returned real Instant Answer hits; `provider=duckduckgo` (HTML scrape) connected but got an anomaly/challenge page (HTTP 202, zero `result__a` hits) — allowlist unlocked, scraper still fragile. Root `duckduckgo.com` / `en.wikipedia.org` remain 403 until granted. **CSP network allowlist (auto + extensible).** On each **Start Claude Science** / MCP deploy, CSP merges host grants into the active org's Science `preferences.json`:
1. **Built-in** domains for every bundled `web-search` provider: `html.duckduckgo.com`, `api.duckduckgo.com`, `en.wikipedia.org`, `api.search.brave.com`, `google.serper.dev`, `api.tavily.com` — so configuring a Brave/Serper/Tavily API key in the MCP tab works without a separate manual grant.
2. **User extensions** from `~/.csp/network-allowlist.json` (`{ "version": 1, "domains": ["example.com"] }`). Create/open via MCP tab `⋯` → **网络授权配置**. Hostname-only entries; Stop → Start after edits (Operon does not hot-reload grants).

Scholarly baselines (Crossref / arXiv / PubMed / …) remain Science-builtin and need no CSP grant. **`provider=auto` order (v1.6.2+):** keyed Brave/Serper/Tavily (when env keys set) → `duckduckgo_ia` → `wikipedia` → Crossref → arXiv → PubMed. HTML `duckduckgo` stays out of auto (anti-bot). Proxy standing guidance mirrors this so product/news queries are not stuck on academic-only results. A user who disables/removes the built-in connector is respected via `~/.csp/mcp/.seeded-web-search`.

**HTTPS-through-proxy fix for Node connectors.** Science injects `HTTPS_PROXY=http://localhost:<operon-port>` into every MCP child, and — confirmed live — the child's OS-level sandbox network policy denies (`EPERM`) any outbound connection to a loopback port *other than* that one Operon address; this rules out pointing connectors at CSP's own proxy instead. Operon's proxy does support a real CONNECT tunnel, but several bundled Node HTTP stacks (axios via `follow-redirects`, used by e.g. `@notionhq/notion-mcp-server`) never issue one for HTTPS targets — they relay the request in absolute form (`GET https://host/… HTTP/1.1` sent as plain HTTP), which Operon then forwards as plain HTTP onto the origin's port 443, producing Cloudflare's `400 The plain HTTP request was sent to HTTPS port`. CSP fixes this client-side: `mcp_http_tunnel_shim.cjs` (bundled via `include_str!`, written to `$SANDBOX_HOME/.claude-science/mcp/csp-http-tunnel-shim.cjs`, and granted a `user_read_paths` entry for that directory) is loaded into Node with `--require`. Live probe also showed Science **strips `NODE_OPTIONS` from `local-mcp.json` env** while keeping other keys (e.g. `NOTION_TOKEN`), so CSP wraps each connector with `/bin/bash` that re-exports `NODE_OPTIONS=--require <shim>` immediately before `exec` (any user `NODE_OPTIONS` is merged in; Operon proxy env is left untouched). The shim monkey-patches `http.request`/`http.get` to turn absolute-form-through-proxy into a real CONNECT + TLS tunnel to the already-permitted proxy address.

**`#!/usr/bin/env node` shims.** Global npm-style binaries (e.g. `notion-mcp-server`) often ship with an `env node` shebang. Science's MCP child does **not** inherit the host `PATH`, so `env` fails with `No such file or directory` even when `user_read_paths` already grants the script and its package tree. On deploy, CSP rewrites such connectors to an absolute `node <script>` invocation, locating `node` via the shim's sibling `bin/node` or by walking up npm-style install roots (`…/bin/node` next to `…/lib/node_modules/…`).

Scope and caveats:

- **Local stdio only.** No remote HTTP/SSE or marketplace connectors — those depend on hosted Anthropic services (see [Directory connectors …](#directory-connectors)). Science's marketplace-plugin path explicitly rejects `stdio` (`only http/sse plugin servers are supported`), so it is not used here.
- **No `cwd`.** Science's local stdio schema is `{ name, command, args, env, description? }` with no working-directory field, so CSP does not expose one — reference scripts by absolute path in `args`.
- **Command whitelist.** Science's managed runtime resolves `node`/`npm`/`npx`/`python`/`python3`/`pip`/`pip3`/`uv`/`uvx`/`deno`/`bun`/`bunx` or an absolute path. Other commands are allowed but flagged with a warning; Science may reject them.
- **Secrets.** `env` values are stored in the local 0600 inventory and only ever returned to the UI masked (`••••tail`). On edit, a blank `KEY=` keeps the stored value, deleting the whole line removes the variable, and a new value overwrites — so masked secrets are never round-tripped. The deployed `local-mcp.json` (which necessarily carries plaintext `env` for the connector) is written `0600` as well.
- **Read-path granularity.** For an absolute path argument, CSP grants the directory itself if it is a directory, otherwise its parent — least privilege, no broad parent grants. Relative-path script args are flagged (the child sandbox has no cwd and cannot resolve them).
- **`config.toml` rewrite.** CSP read-modify-writes `config.toml` with a TOML library, preserving other keys/tables but **not comments or key ordering**. In the sandbox this file is CSP-managed, so this is low-risk; avoid hand-editing it with comments you need to keep.
- **Applying changes.** Science reads Skills/MCP config only at launch. Changes take effect on the next **Start Claude Science**; if the sandbox is already running, CSP detects the change (idempotent deploy) and restarts it automatically so the new config is applied.
- **Discovery is read-only.** Scanning other clients only reads their configs to offer connectors for import; nothing is written back to them, and only local stdio entries (with a `command`) are offered. Some discovered entries are client-internal tools that will not run under Science's sandbox — import selectively.
- Iron rules match the Skill deployer: writes only ever land under the sandbox root, never the real `~/.claude-science`. Deployment is logged to `~/.csp/logs/sandbox.log` as `[mcp] …`.

---

<a id="sandbox-host-access"></a>

## Sandbox `request_host_access` (under investigation)

In some environments Science self-check `request_host_access` reports “path does not exist”, possibly related to sandbox HOME layout or capability grants. Needs repro.

---

<a id="session-recovery"></a>

## Historical session recovery (#6b)

Idempotent virtual login prevents **new** chats from being orphaned. If you already had multiple `orgs/` directories on an older build, older chats may need manually pointing `active-org.json` at a historical `org_uuid` (advanced; sandbox `~/.csp/sandbox/home/.claude-science/orgs/`).

---

## Roadmap (no dates promised)

| Direction | Notes |
|-----------|--------|
| Proxy in Rust | Reduce `python3` runtime dependency |
| Skill sandbox deployment | [Completed] Deploy enabled local Skills into the Science sandbox (see [Local Skill Manager](#skill-manager)) |
| Local stdio MCP connectors | [Completed] Manage + deploy local stdio MCP servers into the Science sandbox (see [Local stdio MCP connectors](#local-mcp)) |
| Launch-and-stay-ready | Auto-prepare Science on app open (issue discussion) |
| Intel / Universal build | Primary release is Apple Silicon today |
| Apple notarization | Ad-hoc signed; first open via right-click → Open |

---

## How to report

1. Use the [issue templates](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose).
2. Include CSP version, macOS version, provider/model, and repro steps.
3. **Do not paste** API keys, path secrets, OAuth files, or full logs—redacted `~/.csp/logs/` snippets only.
