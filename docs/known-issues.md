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

<a id="openai-custom-streaming"></a>

## OpenAI-compatible providers: buffered streaming and long sessions

**Provider paths:** `openai-custom` and `openai-responses` translate Science's Anthropic-shaped `/v1/messages` into OpenAI APIs. Science always asks for SSE, but these paths **buffer the upstream completion** (`stream: false` upstream) and **replay** the result as Anthropic SSE events.

**Fixed in v1.7.1 — `stream idle: no events for 120000ms`.** Science's client runs a 120s idle watchdog that counts **yielded protocol events** (`message_start`, `content_block_delta`, …), not raw TCP bytes. SSE comment lines and `event: ping` are ignored (`ping` is explicitly skipped in Science's Anthropic SSE parser). While waiting for a slow upstream (long tool-heavy history, Resume, GLM Coding Plan with `msgs=200+`), the proxy must open `message_start` + a text block immediately and emit **empty `text_delta` keepalives** so the watchdog resets. Without that, UI shows **Connection issue — retrying…** even though the proxy is healthy. See `proxy/core/http_transport.py` (`_COUNTED_TEXT_DELTA_KEEPALIVE`) and `csp_proxy.py` (`_emit_openai_stream_preamble`). v1.6.10's earlier "SSE keepalive" note was incomplete (comments/`ping` did not count).

**Still open — upstream rate limits and slowness.** GLM Coding Plan and other shared endpoints may return **HTTP 429** (`code 1313` fair-use policy) or take minutes on very long contexts. That is **not** a CSP stream-idle bug; Science may still show connection retries until the upstream accepts the request or you pause other sessions. Native Anthropic passthrough (`deepseek`, `relay` with `mode: anthropic`) uses real upstream streaming and is unaffected by this buffered-path fix.

**Mitigated — GLM `1261` Prompt 超长 + rolling-compact tool_use.** Science auto-compacts long chats (`rolling-compact`), but the summarizer fork often still ships the session tool list. Third-party models (notably GLM) then return `tool_use` with empty text, Science retries, and the main turn keeps hitting upstream **`code 1261`**. CSP now (1) detects Science summarizer/compact requests (Literals / `<summary>` / `summarize_conversation` markers) and **strips tools + forces `tool_choice: none`** (also skips standing web-access inject so it does not pollute the Literals contract), and (2) maps upstream 1261 / context-too-long to **HTTP 400 `invalid_request_error`** with an explicit “start a new chat” message instead of a retried 502. This improves compact success rate but **cannot guarantee** Science’s Literals format check always passes — if the session is already far over the limit, open a new chat.

**Scope:** Applies to any profile using **Custom OpenAI** / **Custom OpenAI Responses** (including GLM when configured with an OpenAI Chat base URL). Does **not** apply to native Anthropic-compatible relays.

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

The **Skills** tab imports local Skill folders (folders containing a `SKILL.md`), local `.zip` archives, or `https://` URLs (direct zip or public GitHub tree) into managed storage at `~/.csp/skills/`, where you can list, enable/disable, remove, and **discover** them from common agent locations (`~/.agents/skills`, `~/.codex/skills`, `~/.claude/skills`, `~/.cursor/skills`, `~/.cursor/skills-cursor`, `~/.kimi-code/skills`, `~/.minimax/skills`, `~/.gemini/config/skills`, `~/.windsurf/skills`, `~/.openclaw/skills`, `~/.qclaw/skills`, `~/.kiro/skills`, plus domestic tools `~/.trae/skills`, `~/.trae-cn/skills`, `~/.codebuddy/skills`, `~/.workbuddy/skills`, `~/.factory/skills`, `~/.qoder/skills`, `~/.qoderwork/skills`, `~/.qoderworkcn/skills`, `~/.eigent/skills`, `~/.grok/skills`, `~/.mavis/skills`, `~/.stepfun/skills`, `~/.corust-agent/skills`). The discover list is **searchable** (name / description / source). Already-owned Skills (same source path or same name) stay listed with an **Already owned · keep by default** badge and are unchecked so Import skips them unless you opt in to overwrite. **Sync Science skill library** (Skills `⋯` menu) compares the CSP store with the live Science library at `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/` — harvests `.csp_managed` folders that Science (Edit skill / chat) changed back into `~/.csp/skills/`, can import unmanaged library skills not yet in CSP, and still offers unpublished `workspaces/` drafts whose names are not already imported. Deploy auto-harvests managed drift before wipe so Science-side edits are not silently overwritten. Science cannot call `host.skills.edit()` / `host.skills.publish()` under CSP virtual login in a durable way; syncing the on-disk library is the supported path. Re-adopting the same workspace key updates the existing inventory entry instead of duplicating it. On each **Start Claude Science**, enabled Skills are deployed into the sandbox at `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/<name>/` — the org-scoped directory current Claude Science builds actually scan and stamp (pure disk scan, no allowlist). CSP also cleans up Skills that older builds deployed to the legacy root `…/.claude-science/skills/`. The deployer only manages folders it marks with `.csp_managed`, so Science's bundled scientific Skills (alphafold2, boltz, …) are never removed, and a user Skill whose name collides with a bundled one is skipped (logged in `~/.csp/logs/sandbox.log`). After launch, CSP checks whether Science wrote a `.catalog_stamp` into each deployed folder and logs `recognized_by_science=<n>` as a self-verification. Requirement detection (`python`, `node`, `rust`, `mcp`, …) is a filename/extension heuristic and may miss or over-report. Deployment is idempotent (a manifest signature skips unchanged redeploys); since Science reads Skills only at launch, changes apply on the next **Start Claude Science**, and a running sandbox is restarted automatically when the deployed set actually changed. Two Skills whose names sanitize to the same folder — and symlinks inside a Skill — are skipped for safety. Note Science loads Skills on demand (searched by relevance), so an imported Skill is indexed but not force-loaded into every conversation. Writes never touch real `~/.claude-science` state.

**Built-in `csp-environment` Skill (progressive disclosure, enabled by default; formerly `csp-web-access`).** Because the hosted `web_search` tool is unavailable under CSP virtual login (see the built-in `web-search` MCP connector below), Science's planner would otherwise try bare `web_search` and fail with `Tool 'web_search' not found on agent`. CSP **seeds a CSP-managed Skill named `csp-environment` into `~/.csp/skills/` on first run**, enabled by default, deployed to `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/csp-environment/` on every **Start Claude Science**, with a **Built-in** badge and sticky opt-out via sentinel `~/.csp/skills/.seeded-csp-environment`. Existing installs of `csp-web-access` are migrated on launch (removed + reseeded under the new name); if the user had already removed the legacy skill (legacy sentinel present, inventory empty), the new sentinel is stamped **without** reseeding. Content self-heals on each launch. **Caveat:** Skills are progressive-disclosure (not in every session's system prompt) and alone are insufficient — the **real standing fix** is proxy system-prompt injection (see [env conventions](#env-conventions) / [local MCP](#local-mcp)). The Skill remains belt-and-suspenders for relevance-ranked loading.

---

<a id="env-conventions"></a>

## Hosted vs CSP local environment conventions

Claude Science running inside CSP is a **local sandbox**, not Anthropic's hosted Claude environment, so several habits carried over from the hosted product silently fail. The built-in **`csp-environment`** Skill (see above; formerly `csp-web-access`) bundles these as standing, per-session guidance; they are also documented here for operators.

- **No `/mnt/data` (and no other `/mnt/...`).** The hosted environment exposes `/mnt/data` and `/mnt/user-data`; **these do not exist locally**, so writes there fail or vanish. Save outputs to the **current working directory** — the active Science workspace `orgs/<org_uuid>/workspaces/<workspace_uuid>/` — using **relative paths**. Use `/tmp` only for disposable scratch. To surface a **user-visible** file, write it in the workspace and then call **`save_artifacts([...])`** (writing the file alone does not expose it).
- **CJK plotting fonts.** matplotlib's default `DejaVu Sans` cannot render Chinese/Japanese/Korean glyphs, so CJK labels render as tofu boxes (□□□). Before plotting non-Latin labels, set a CJK-capable font that exists on the macOS host, e.g. `plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]` and `plt.rcParams["axes.unicode_minus"] = False`. This is **guidance-only** (no font binary is bundled and CSP does not patch the sandbox matplotlib rc): the host already ships these fonts, so a two-line `rcParams` set is the low-risk fix. If you use the `figure-style` skill, pass it a CJK font the same way.
- **Hosted `web_search` is unavailable; use `repl` → `host.mcp`.** Under CSP virtual login the Anthropic-hosted / native OPERON `web_search` / `web_fetch` tools do not exist in OPERON's toolset and fail with `Tool 'web_search' not found on agent` if called bare. Local MCP tools are **not** top-level model tools. Search/fetch via the `repl` tool:
  - `host.mcp("web-search", "csp_web_search", query="...", max_results=N)` — **public GENERAL** method (news/products/facts); returns a **dict** with `"results"` (list of hit dicts); use `data["results"]`, not `for x in data`
  - `host.mcp("web-search", "search_literature", query="...", max_results=N)` — LITERATURE lane (papers/DOI)
  - then `host.mcp("web-search", "fetch_url", url="...")` when needed — returns `{"url","status","content"}`
  Native Anthropic `web_search` ≠ MCP `csp_web_search`. `tools/list` advertises only `csp_web_search` for GENERAL (undocumented dispatch alias `web_search` may still work for back-compat). The CSP proxy injects this as standing system guidance on every `/v1/messages` request that already has a `system` prompt (sentinel `<!-- CSP_WEB_ACCESS_GUIDANCE -->`), including a **request-time current local date/time** line so the model treats that as "today" for date answers and search-year freshness (Science itself has no reliable wall clock; cutoff ~early 2024). Re-advertising names in MCP `tools/list` alone **cannot** intercept bare native calls. Egress is a scholarly allowlist (Crossref / arXiv / PubMed / OpenAlex / Semantic Scholar / Notion / PyPI reliable; general and paid search engines usually blocked). See [Built-in `web-search` connector](#local-mcp).
- **No wall clock inside Science.** The model cannot read the host clock; without guidance it answers "can't get real-time date" or skews search years to the training cutoff. CSP's proxy standing injection supplies `Current local date/time: …` on each Anthropic-shaped `/v1/messages` transform (and refreshes that block if an older sentinel is already in `system`).
- **`host.skills.publish()` / `host.skills.edit()` don't persist.** These hosted skill-management calls do not take effect under CSP virtual login. To install a durable skill, draft the files in the workspace and sync them via **Skills tab → Sync Science skill library**; CSP then deploys them into the sandbox. See [Local Skill Manager](#skill-manager).
- **Two Python environments.** The **analysis `python` env** carries the scientific stack (numpy / pandas / matplotlib / scipy, …) and is where computation and plotting should run. The **MCP Python env** (used by stdio MCP servers) may **not** have plotting or scientific packages, so don't assume they're importable from an MCP tool context.

### Why a CSP-managed Skill can log `recognized_by_science=0`

After launch CSP checks whether Science wrote a `.catalog_stamp` into each deployed folder and logs `recognized_by_science=<n>`. On current Science builds (audited on `0.1.17-dev`) this stamp is written **once, at the initial org catalog build** — every bundled Skill's `.catalog_stamp` shares one identical timestamp/value — and folders added **after** that build are **not** re-stamped on later launches. Consequently a CSP-managed Skill deployed into an **already-initialized** org (e.g. `crypto-data`, or `csp-environment` on an existing install) can stay unstamped and log `recognized_by_science=0`. This is a **false negative of the stamp heuristic**, not proof Science can't load the Skill: Science's live catalog lives in `orgs/<org>/operon-cli.db` and on-disk Skills are searched by relevance regardless of the stamp. It is **not** a `SKILL.md` frontmatter/format problem (`crypto-data` has valid `name`/`description` frontmatter identical in shape to recognized bundled Skills, yet is unstamped) nor a directory-naming problem. On a **fresh** org, CSP deploys Skills *before* Science's first catalog build, so the built-in Skill is stamped and recognized normally. If an existing install shows `recognized_by_science=0` and you want the on-disk stamp to flip, start Science with a fresh org (new virtual login) so the catalog is rebuilt with the Skill already present.

---

<a id="local-mcp"></a>

## Local MCP connectors (stdio + remote)

The **MCP** tab manages MCP connectors stored at `~/.csp/mcp/inventory.json`: local **stdio** (`command` + `args` + `env`) and remote **sse / streamable_http** (`url` + optional `headers`). You can add, edit, enable/disable, remove, and **discover** them from common AI clients (Cursor, Claude Desktop/Code, Codex `config.toml`, Devin Desktop / Windsurf, VS Code, Zed, Continue, Kimi Code, MiniMax, OpenClaw, AWS Kiro, and domestic tools Qoder / 通义灵码, Trae / TRAE SOLO, CodeBuddy, WorkBuddy, Factory). Scan-import rows offer a **compact config preview** (pretty-printed JSON/TOML for that server entry, reveal source file in Finder). On each **Start Claude Science**:
- enabled **stdio** connectors are written to `$SANDBOX_HOME/.claude-science/mcp/local-mcp.json` (confirmed: `source: local-stdio`, `transport: stdio`), and CSP merges absolute-path parents into `[sandbox] user_read_paths`;
- enabled **remote** connectors are upserted into the active org's `operon-cli.db` table `custom_mcp_servers` (Science's real remote schema; attached to agent `OPERON` as user `local-dev`) and are **not** written to `local-mcp.json`.

**Built-in `web-search` connector (free, no API key).** Because Claude Science lacks Anthropic's hosted `web_search` tool under CSP's virtual login (the sandbox log shows `Tool 'web_search' not found on agent 'OPERON'`), CSP ships a bundled multi-provider search + fetch MCP server and **seeds it into the inventory on first run**, enabled by default. It is a small self-contained **Python** stdio server (`web_search_server.py`, bundled via `include_str!` and written to `$SANDBOX_HOME/.claude-science/mcp/csp-web-search-server.py` at deploy time, like the Node shim). Python is chosen deliberately: unlike the Node/axios stacks, Python's `requests`/`urllib` honour the injected `HTTPS_PROXY` and issue a proper `CONNECT` tunnel, so it needs no shim. The connector's interpreter is resolved to the sandbox's own `claude-science-mcp` conda env Python (falling back to the `python` env, then `python3`) and re-resolved on every deploy so it self-heals. Its inventory description is also refreshed on each launch / deploy for already-seeded users. Call via **`repl` → `host.mcp`** (not as bare top-level tools):

- `host.mcp("web-search", "csp_web_search"|"search_literature", query=..., max_results=5, provider="auto")` — GENERAL (`csp_web_search`, the only public GENERAL method) or LITERATURE search. **Return:** `host.mcp` parses tool JSON into a **dict** `{"provider", "query", "results": [{title, url, snippet, source, …}], "warnings"}`. Use `hits = data["results"]` then iterate; do not enumerate the dict itself (yields string keys → `AttributeError` on `.get`). Undocumented dispatch alias `"web_search"` still maps to the GENERAL handler for back-compat but is **not** listed in `tools/list`.
- `host.mcp("web-search", "fetch_url"|"web_fetch", url=..., max_chars=8000)` — fetch a page; returns dict `{"url", "status", "content"}`.

**Why `tools/list` re-advertising alone is insufficient.** An earlier fix re-advertised `web_search` / `web_fetch` in the local connector's `tools/list` hoping model-native calls would resolve there. That was the **wrong layer**: bare `web_search` is an Anthropic *native server tool*; under CSP virtual login it is stripped from OPERON's toolset before any MCP routing. Local MCP tools are never top-level model tools — they are only reachable via `host.mcp`. `tools/list` names remain useful as **`host.mcp` method names**, but standing reliability comes from the **proxy system-prompt injection** (`inject_csp_web_access_guidance` in `proxy/compat/anthropic_compat.py`) on every Anthropic-shaped `/v1/messages` request that already has a `system` prompt.

**Deploy / restart.** On each **Start Claude Science** / one_click_login, MCP deploy rewrites the sandboxed script from the embedded `include_str` when bytes differ, and that rewrite marks deploy as changed so a running sandbox is restarted (MCP children otherwise keep the old script in memory). **Quitting the CSP app alone does not stop a leftover `claude-science serve` daemon** — use **Stop Claude Science** in the CSP UI before Start again. Aggressive kill-on-quit is a known follow-up (not done here to avoid risky process matching).

**Egress reality — the operon allowlist.** A live probe of the operon per-child proxy (which is what MCP children get as `HTTPS_PROXY`, on a per-child ephemeral port, **with no token in the URL** — hence the shim's tokenless `CONNECT` works) showed that operon enforces a **curated egress allowlist of scientific sources**, mergeable with the org's Science network grants (`preferences.json` → `userAllowedDomains` plus `approvalGrants.always.allow.network` / `approvalGrants.alwaysOrigins.network`). Built-in scholarly hosts tunnel without grants: `arxiv.org`, `api.crossref.org`, `eutils.ncbi.nlm.nih.gov`, `api.openalex.org`, `api.semanticscholar.org`, `api.notion.com`, `pypi.org` (200/401/429). Without user grants, general engines / paid APIs still get **`403 Forbidden` on CONNECT** (`duckduckgo.com`, `html.duckduckgo.com`, `api.duckduckgo.com`, `en.wikipedia.org`, `google.com`, `bing`, `api.search.brave.com`, `google.serper.dev`, `api.tavily.com`, …). **Plan A probe (2026-07-14):** writing `html.duckduckgo.com` + `api.duckduckgo.com` into those grant fields and **restarting** Science (disk edit alone does not hot-reload the live operon proxy) flipped DDG CONNECT from 403 → 200; Crossref/CoinGecko stayed 200. Through the same proxy path, `provider=duckduckgo_ia` returned real Instant Answer hits; `provider=duckduckgo` (HTML scrape) connected but got an anomaly/challenge page (HTTP 202, zero `result__a` hits) — allowlist unlocked, scraper still fragile. Root `duckduckgo.com` / `en.wikipedia.org` remain 403 until granted. **CSP network allowlist (auto + extensible).** On each **Start Claude Science** / MCP deploy, CSP merges host grants into the active org's Science `preferences.json`:
1. **Built-in** domains for every bundled `web-search` provider: `html.duckduckgo.com`, `lite.duckduckgo.com`, `api.duckduckgo.com`, `en.wikipedia.org`, `api.search.brave.com`, `google.serper.dev`, `api.tavily.com` — so configuring a Brave/Serper/Tavily API key in the MCP tab works without a separate manual grant.
2. **Built-in common egress** (v2.1.0+): a curated news / finance / US-gov / crypto / GitHub host set is merged on Start so everyday `fetch_url` needs fewer one-off approvals.
3. **User extensions** from `~/.csp/network-allowlist.json` (`{ "version": 1, "domains": ["example.com"] }`). Create/open via MCP tab `⋯` → **网络授权配置（JSON）**. Hostname-only entries; Stop → Start after edits (Operon does not hot-reload grants).
4. **Pending approval UI** (v2.1.0+): when Operon denies CONNECT, `fetch_url` queues the hostname under `/private/tmp/csp-network-pending.json` (and `~/.csp/network-pending.json`). CSP → MCP → **待批准出网域名** lists them; approve merges grants and restarts Science. Leave unchecked / dismiss to skip. Cloudflare browser-challenge pages are **not** allowlist failures — they return HTTP 403 after CONNECT succeeds.

**Fake-IP DNS vs Operon SSRF (2026-07-21).** Even with grants present, Science Operon resolves the CONNECT hostname and **denies** any address in private/reserved ranges (`[proxy] denied api.duckduckgo.com → fdfe:dcba:9876::8c (private/reserved range)` → client `Tunnel connection failed: 403 Forbidden`). Clash/Surge/mihomo **Fake-IP** mode commonly returns `198.18.0.0/15` and ULA `fdfe:dcba:9876::/48` for *all* public hosts (including `arxiv.org`). CSP's status row previously only probed proxy + Science HTTP health, so the UI could show **「代理+Science 运行中」** while every MCP egress CONNECT failed. CSP now probes system DNS for Fake-IP samples and surfaces **「代理+Science 运行中（出网受阻）」** with a tip to disable Fake-IP (or bypass Claude Science), then Stop → Start. CSP cannot rewrite Operon's resolver from inside the MCP sandbox (children may only talk to the injected Operon loopback proxy).

Scholarly baselines (Crossref / arXiv / PubMed / …) remain Science-builtin and need no CSP grant. **Two search lanes (v1.6.3+; free GENERAL fallbacks expanded in v1.6.5; public GENERAL name unified to `csp_web_search` in v1.6.6; Wikipedia moved fully to LITERATURE auto in v1.6.7; Lite anti-bot hardened in v1.6.8):** GENERAL — advertise **`csp_web_search` only** in `tools/list` (not two engines; undocumented `web_search` dispatch alias for back-compat); auto = optional keyed Brave/Serper/Tavily → `duckduckgo_ia` → `duckduckgo_lite` (no key required; empty Instant Answer is common and ≠ missing key; temporary Lite `anomaly.js` anti-bot is retried — **Wikipedia is not a GENERAL fallback**; do not narrate "fell back to Wikipedia" or demand API keys). LITERATURE — `search_literature`; auto = wikipedia → Crossref → arXiv → PubMed. Full HTML `duckduckgo` stays out of auto (anti-bot). CSP also pre-grants `lite.duckduckgo.com`. Native Anthropic `web_search` remains unavailable and must never be called. Proxy + `csp-environment` standing guidance mirror this so product/news queries use the general lane without inventing an API-key requirement. A user who disables/removes the built-in connector is respected via `~/.csp/mcp/.seeded-web-search`.

**HTTPS-through-proxy fix for Node connectors.** Science injects `HTTPS_PROXY=http://localhost:<operon-port>` into every MCP child, and — confirmed live — the child's OS-level sandbox network policy denies (`EPERM`) any outbound connection to a loopback port *other than* that one Operon address; this rules out pointing connectors at CSP's own proxy instead. Operon's proxy does support a real CONNECT tunnel, but several bundled Node HTTP stacks (axios via `follow-redirects`, used by e.g. `@notionhq/notion-mcp-server`) never issue one for HTTPS targets — they relay the request in absolute form (`GET https://host/… HTTP/1.1` sent as plain HTTP), which Operon then forwards as plain HTTP onto the origin's port 443, producing Cloudflare's `400 The plain HTTP request was sent to HTTPS port`. CSP fixes this client-side: `mcp_http_tunnel_shim.cjs` (bundled via `include_str!`, written to `$SANDBOX_HOME/.claude-science/mcp/csp-http-tunnel-shim.cjs`, and granted a `user_read_paths` entry for that directory) is loaded into Node with `--require`. Live probe also showed Science **strips `NODE_OPTIONS` from `local-mcp.json` env** while keeping other keys (e.g. `NOTION_TOKEN`), so CSP wraps each connector with `/bin/bash` that re-exports `NODE_OPTIONS=--require <shim>` immediately before `exec` (any user `NODE_OPTIONS` is merged in; Operon proxy env is left untouched). The shim monkey-patches `http.request`/`http.get` to turn absolute-form-through-proxy into a real CONNECT + TLS tunnel to the already-permitted proxy address.

**`#!/usr/bin/env node` shims.** Global npm-style binaries (e.g. `notion-mcp-server`) often ship with an `env node` shebang. Science's MCP child does **not** inherit the host `PATH`, so `env` fails with `No such file or directory` even when `user_read_paths` already grants the script and its package tree. On deploy, CSP rewrites such connectors to an absolute `node <script>` invocation, locating `node` via the shim's sibling `bin/node` or by walking up npm-style install roots (`…/bin/node` next to `…/lib/node_modules/…`).

Scope and caveats:

- **Stdio + remote custom.** Local stdio still deploys to `local-mcp.json`. Remote connectors use Science's real path: org `operon-cli.db` table `custom_mcp_servers` with `transport ∈ {sse, streamable_http}` (confirmed in Science `0.1.17-dev`). Marketplace / directory hosted connectors are still out of scope (see [Directory connectors …](#directory-connectors)). Science's marketplace-plugin path explicitly rejects `stdio` (`only http/sse plugin servers are supported`), so it is not used here.
- **No static headers column.** Custom remotes authenticate via OAuth fields or a `headers_helper` shell command that prints a JSON object of string headers. CSP stores optional headers in the 0600 inventory and deploys them as that helper (base64-wrapped `python3 -c`); do not put secrets in the URL.
- **No `cwd`.** Science's local stdio schema is `{ name, command, args, env, description? }` with no working-directory field, so CSP does not expose one — reference scripts by absolute path in `args`.
- **Command whitelist.** Science's managed runtime resolves `node`/`npm`/`npx`/`python`/`python3`/`pip`/`pip3`/`uv`/`uvx`/`deno`/`bun`/`bunx` or an absolute path. Other commands are allowed but flagged with a warning; Science may reject them.
- **Secrets.** `env` / `headers` values are stored in the local 0600 inventory and only ever returned to the UI masked (`••••tail`). On edit, a blank `KEY=` keeps the stored value, deleting the whole line removes the variable, and a new value overwrites — so masked secrets are never round-tripped. The deployed `local-mcp.json` (which necessarily carries plaintext `env` for the connector) is written `0600` as well.
- **Read-path granularity.** For an absolute path argument, CSP grants the directory itself if it is a directory, otherwise its parent — least privilege, no broad parent grants. Relative-path script args are flagged (the child sandbox has no cwd and cannot resolve them).
- **`config.toml` rewrite.** CSP read-modify-writes `config.toml` with a TOML library, preserving other keys/tables but **not comments or key ordering**. In the sandbox this file is CSP-managed, so this is low-risk; avoid hand-editing it with comments you need to keep.
- **Applying changes.** Science reads Skills/MCP config only at launch. Changes take effect on the next **Start Claude Science**; if the sandbox is already running, CSP detects the change (idempotent deploy) and restarts it automatically so the new config is applied. Remote DB sync needs the org `operon-cli.db` to exist (after the first Science launch).
- **Discovery.** Scanning other clients reads their configs to offer connectors for import; nothing is written back. Both local stdio (`command`) and remote (`url` + `type`/`transport`) entries are offered; each row can preview the source server block before import. Some discovered entries are client-internal tools that will not run under Science's sandbox — import selectively. Only **HOME-level** config files are scanned (not project-scope `.cursor/mcp.json`, `.trae/mcp.json`, etc.).
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
| Local MCP connectors (stdio + remote) | [Completed] Manage + deploy stdio and custom remote MCP into the Science sandbox (see [Local MCP connectors](#local-mcp)) |
| Launch-and-stay-ready | Auto-prepare Science on app open (issue discussion) |
| Intel / Universal build | Primary release is Apple Silicon today |
| Apple notarization | Ad-hoc signed; first open via right-click → Open |

---

## How to report

1. Use the [issue templates](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose).
2. Include CSP version, macOS version, provider/model, and repro steps.
3. **Do not paste** API keys, path secrets, OAuth files, or full logs—redacted `~/.csp/logs/` snippets only.
