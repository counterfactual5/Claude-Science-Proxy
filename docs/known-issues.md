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

The **Skills** tab imports local Skill directories (folders containing a `SKILL.md`) into managed storage at `~/.csp/skills/`, where you can list, enable/disable, remove, and **discover** them from common agent locations (`~/.agents/skills`, `~/.codex/skills`, `~/.claude/skills`, `~/.cursor/skills`, plus domestic tools `~/.trae/skills` and `~/.codebuddy/skills`). On each **Start Claude Science**, enabled Skills are deployed into the sandbox at `$SANDBOX_HOME/.claude-science/orgs/<org_uuid>/skills/<name>/` — the org-scoped directory current Claude Science builds actually scan and stamp (pure disk scan, no allowlist). CSP also cleans up Skills that older builds deployed to the legacy root `…/.claude-science/skills/`. The deployer only manages folders it marks with `.csp_managed`, so Science's bundled scientific Skills (alphafold2, boltz, …) are never removed, and a user Skill whose name collides with a bundled one is skipped (logged in `~/.csp/logs/sandbox.log`). After launch, CSP checks whether Science wrote a `.catalog_stamp` into each deployed folder and logs `recognized_by_science=<n>` as a self-verification. Requirement detection (`python`, `node`, `rust`, `mcp`, …) is a filename/extension heuristic and may miss or over-report. Deployment is idempotent (a manifest signature skips unchanged redeploys); since Science reads Skills only at launch, changes apply on the next **Start Claude Science**, and a running sandbox is restarted automatically when the deployed set actually changed. Two Skills whose names sanitize to the same folder — and symlinks inside a Skill — are skipped for safety. Note Science loads Skills on demand (searched by relevance), so an imported Skill is indexed but not force-loaded into every conversation. Writes never touch real `~/.claude-science` state.

---

<a id="local-mcp"></a>

## Local stdio MCP connectors

The **MCP** tab manages local **stdio** MCP connectors (custom `command` + `args` + `env`) stored at `~/.csp/mcp/inventory.json`, where you can add, edit, enable/disable, remove, and **discover** them from common AI clients (Cursor, Claude Desktop/Code, Codex `config.toml`, Devin Desktop, VS Code, Zed, and domestic tools Qoder / 通义灵码, Trae / TRAE SOLO, CodeBuddy). On each **Start Claude Science**, enabled connectors are written to the sandbox at `$SANDBOX_HOME/.claude-science/mcp/local-mcp.json` — the file Claude Science reads for user stdio connectors (confirmed against a live sandbox: they surface with `source: local-stdio`, `transport: stdio`). Because Science's restricted MCP child sandbox can only read paths granted via `config.toml`, CSP also merges the parent directory of every absolute path a connector references into `[sandbox] user_read_paths` (least privilege; only that key is owned, all other `config.toml` keys are preserved). Disabling all connectors removes `local-mcp.json` and CSP's read grants so nothing lingers.

**HTTPS-through-proxy fix for Node connectors.** Science injects `HTTPS_PROXY=http://localhost:<operon-port>` into every MCP child, and — confirmed live — the child's OS-level sandbox network policy denies (`EPERM`) any outbound connection to a loopback port *other than* that one Operon address; this rules out pointing connectors at CSP's own proxy instead. Operon's proxy does support a real CONNECT tunnel, but several bundled Node HTTP stacks (axios via `follow-redirects`, used by e.g. `@notionhq/notion-mcp-server`) never issue one for HTTPS targets — they relay the request in absolute form (`GET https://host/… HTTP/1.1` sent as plain HTTP), which Operon then forwards as plain HTTP onto the origin's port 443, producing Cloudflare's `400 The plain HTTP request was sent to HTTPS port`. CSP fixes this client-side: `mcp_http_tunnel_shim.cjs` (bundled via `include_str!`, written to `$SANDBOX_HOME/.claude-science/mcp/csp-http-tunnel-shim.cjs`, and granted a `user_read_paths` entry for that directory) is loaded into Node with `--require`. Live probe also showed Science **strips `NODE_OPTIONS` from `local-mcp.json` env** while keeping other keys (e.g. `NOTION_TOKEN`), so CSP wraps each connector with `/bin/bash` that re-exports `NODE_OPTIONS=--require <shim>` immediately before `exec` (any user `NODE_OPTIONS` is merged in; Operon proxy env is left untouched). The shim monkey-patches `http.request`/`http.get` to turn absolute-form-through-proxy into a real CONNECT + TLS tunnel to the already-permitted proxy address.

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
