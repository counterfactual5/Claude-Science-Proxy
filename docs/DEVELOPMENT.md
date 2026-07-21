# Claude Science Proxy (CSP) Developer Guide

This document is aimed at developers taking over the project with zero prior context. Reading this will guide you through compiling the Tauri app, running tests, executing end-to-end sandbox verification, and packaging releases.

## Repository Documentation

All developer and user-facing documentation in this repository is maintained in **English** (with the exception of `README.zh.md` which serves as a bilingual quick-start entry point).

- **For users**: Start with `README.md` (or `README.zh.md`).
- **For contributors**: Read `README.md` and this `DEVELOPMENT.md` guide.
- **For AI coding assistants**: Read `AGENT.md` at the root of the repository for code style and safety rules.

---

## Iron Rules (Highest Priority)

Refer to the first section of `AGENT.md` for safety rules. Core constraints:
1. **Never copy, read, modify, or delete real Claude login credentials**, OAuth tokens, account state, or user data (including `.oauth-tokens`, `encryption.key`, `active-org.json` under `~/.claude-science`).
2. During the initial sandbox setup, you may clone runtime resources (`bin`, `conda`, `runtime`, `seed-assets`) in **read-only** mode from the real `~/.claude-science` directory, but never copy account credentials or conversation databases.
3. **Never copy real OAuth tokens into the sandbox**; the sandbox must only use locally forged virtual OAuth tokens.
4. **Never run the real instance under a modified environment**. The real instance runs on port 8765. All sandbox environments must use an **independent HOME, port, and data directory**.
5. Tests must not touch the live Claude Science app by default. End-to-end smoke testing involving Science requires explicit user consent.

---

## Naming Conventions

The public product name is **Claude Science Proxy (CSP)**.
- **Directories**: The local user data directory is `~/.csp/` (containing `CSP.json`, `logs/`, and `sandbox/home/`).
- **Identifiers**: The Tauri app bundle identifier is `com.csp.menubar`.
- **Environment Variables**: IPC and environment variables use the `CSP_*` prefix (e.g. `CSP_MODEL_REGISTRY`). The proxy script is named `csp_proxy.py`.
- **Desensitization**: Public-facing UI copy avoids terms like "bypass login" or "skip login" (preferring neutral phrases like "One-Click Start"). Technical internal documentation may still use "bypass login" to describe the mechanism.

### Legacy Wording (Intentionally Kept for Compatibility)

The following names are kept only where removing them would break test isolation or log grep workflows:
- Test log prefixes `csp-*` (e.g. `/tmp/csp-auth-*.log`): Used for isolating temporary test files.

---

## Repository Layout (by function)

Top-level directories are grouped by responsibility. **Rust, Tauri bundle paths, and shell scripts all use these nested paths** (not the old flat `proxy/*.py` / `scripts/*.sh` layout).

```
proxy/
  core/           csp_proxy.py, http_transport.py
  compat/         Anthropic / OpenAI protocol adapters
  registry/       model discovery, registry, sort
  policy/         provider_policy.py
  dsml/           DSML shim
scripts/
  sandbox/        launch / stop isolated Science
  maintenance/    doctor, daily-maintenance, launchd plist
  oauth/          make-virtual-oauth.mjs (dev parity with Rust forger)
  ci/             verify-proxy, self-test, clean-bundle-resources
test/
  runners/        S0 layered gate (run_all.sh, run-offline.sh, …)
  unit/           unittest by domain (proxy, capability, model, …)
  integration/    loopback / E2E proxy tests
  fixtures/       mock upstream, golden JSON, real-machine conf
  docs/           REAL_MACHINE_TEST.md, RM_RETEST_STEPS.md
  run_all.sh      thin shim → test/runners/run_all.sh
catalog/          capabilities.v1.json (read-only rules)
desktop/          Tauri menubar app
docs/             DEVELOPMENT.md, known-issues.md (+ assets)
findings/         local-only evidence archive (gitignored; created by maintenance scripts)
```

Entry commands (unchanged for contributors):

- `bash test/run_all.sh` — offline regression gate
- `python3 proxy/core/csp_proxy.py --provider deepseek --port 18991`
- `scripts/sandbox/launch-virtual-sandbox.sh` — full sandbox chain (dev)

---

## Code Comments

All production code comments (Rust, Python, JS) must be in **English**. If you modify an older file with Chinese comments, please refactor them to English.

- **Module Headers**: Document the module responsibilities, inputs/outputs, and invariants in English.
- **Safety Guards**: Keep them explicitly commented in English with a link to `AGENT.md`.
- **Bug Fixes**: Avoid writing vague phrases like "Fix P1" or "Fix #9"; use descriptive text or link to the GitHub issue.

---

## User-Visible Copy (i18n)

User-visible strings (errors, switch hints) are localized. The Rust backend returns localized error keys, and the frontend in `desktop/src/main.js` translates them using `I18N.cn` or `intl`.

### Error Responses

- **Rust**: Uses `i18n_err("errKey", json!({ "var": value }))` which serializes to `{"i18n":"errKey","vars":{...}}` (defined in `runtime/i18n.rs`).
- **Frontend**: Calls `resolveBackendErr(e)` to parse the JSON and display the message via `T(key, resolveBackendVars(vars))`. Non-JSON strings are displayed raw.

`resolveBackendVars` recursively expands nested i18n keys if they are present in variables (e.g. `rollback_key`).

### Switch Hints

For transaction failures, commands like `set_active_profile` return a localization payload via `hint_key` + `hint_vars` (generated by `hint_payload()`):

```javascript
setMsg(resolveHint(r, "switchRejected")); // Uses hint_key if present, fallbacks to default
```

Successful profile changes return `{ committed: true, active_id }` without any success hints, keeping the message panel clean.

---

## System Architecture

CSP consists of the following layers:
1. **Local Proxy (Python)**: A translation proxy running `csp_proxy.py` that intercepts outbound `/v1/messages` requests, strips Science's virtual OAuth token, injects your configured API keys, and translates Anthropic ↔ OpenAI protocol.
2. **Virtual OAuth Forger (Rust)**: A native Rust module (`src-tauri/src/oauth_forge.rs`) that writes sandbox-compliant fake token files under `.sandbox/` so that Science starts logged in. Node.js is **not** required to run the app.
3. **Sandbox Controller (Shell)**: Helper shell scripts to launch and stop the sandboxed Science client under an isolated `$HOME`. Since v2.2.0, the sandbox HOME's `.ssh/config` is bridged to the real `~/.ssh/config` via an `Include` directive (see `sandbox_session.rs`), enabling git/MCP over SSH without copying keys.
4. **Desktop App (Tauri/Rust)**: A normal window wrapper (340×700) that manages the lifecycle of the proxy and sandbox sub-processes, validates configuration profiles, and monitors loopback health.

---

## Project Status

- **Configuration Management**: Supports multiple provider profiles (DeepSeek, GLM, Kimi, MiniMax, Xiaomi MiMo, OpenRouter, Custom Anthropic, Custom OpenAI, Custom OpenAI Responses). Saved under `~/.csp/CSP.json` (schema v4 with automatic schema migration). Keys are masked when sent to the frontend.
- **Provider Types**: `deepseek` connects to the native Anthropic endpoint. The other templates route through `relay`, `openai-custom`, or `openai-responses` adapters.
- **Model Selector**: The frontend allows selecting multiple active models for a profile. A virtual registry maps up to 8 shell IDs (`claude-*`) in Science to the real upstream model names.
- **Profile Transactions**: Activating a profile follows a strict sequence: launches a temporary scratch proxy → validates candidate connection health → switches the active ID. If verification fails, it rolls back gracefully.
- **Desktop Skills / MCP (1.8.x)**: Skills import from folder, zip, or URL; workspace adopt with preview; MCP stdio + remote connectors with scan-and-import config preview. See [`docs/known-issues.md`](known-issues.md#skill-manager) and [`#local-mcp`](known-issues.md#local-mcp).

### Proxy streaming paths (`csp_proxy.py`)

| Path | Providers | Upstream | Downstream to Science |
|------|-----------|----------|------------------------|
| Anthropic passthrough | `deepseek`, `relay` | Real SSE stream | Passthrough (optional DSML rewrite; relay path strips orphan tool_use/tool_result since v2.2.0) |
| Buffered OpenAI replay | `openai-custom`, `openai-responses` | Single JSON completion (`stream: false`) | Replayed Anthropic SSE |

For the **buffered OpenAI** path, Science's 120s stream-idle watchdog requires **counted** SSE events while waiting for upstream TTFT. Since v1.7.1 the proxy opens `message_start` + an empty text block, then sends empty `content_block_delta` keepalives every second (`http_transport._COUNTED_TEXT_DELTA_KEEPALIVE`). SSE comments and `ping` do **not** reset the watchdog. See [`docs/known-issues.md`](known-issues.md#openai-custom-streaming).

---

## Source Directory Structure (`desktop/`)

```
desktop/src/                     Frontend code (HTML/CSS/JS without frameworks)
  index.html, styles.css, main.js
desktop/src-tauri/src/           Rust Backend
  lib.rs           Tauri commands, profile switcher, and sub-process lifecycle manager
  config.rs        Atomic read/write for ~/.csp/CSP.json, permission checks (0600), and migrations
  config_legacy.rs Legacy v1 config structures kept for migration compatibility
  templates.rs     Provider template definitions (base URLs, models, thinking settings)
  lifecycle.rs     Serialization mutex for profile switching transactions
  scratch.rs       Temporary scratch proxy to validate credentials before switching
  oauth_forge.rs   Native Rust implementation of the virtual OAuth ticket generator
  proc.rs          Helper utilities for TCP health probes, URIs, and subprocess execution
  runtime/
    sandbox_session.rs  Sandbox launch orchestrator: skills/MCP deploy, SSH config bridge (v2.2.0+), health polling
    science.rs     Science binary discovery, sandbox HOME management, status checks
  main.rs          Entry point
  tauri.conf.json  Tauri build settings and bundle resources whitelists
```

---

## Frontend-Backend Command Contract

Frontend code only invokes Rust commands via `invoke()`. API keys are never returned as plain text (always masked). Note that Tauri expects arguments to be in `camelCase` (e.g. `templateId`, `baseUrl`), but inner fields in structures use `snake_case`.

- **Config**: `get_config`, `create_profile`, `update_profile_connection`, `update_profile_metadata`, `delete_profile`, `set_active_profile`, `open_csp_json`, `set_settings`.
- **Model Discovery**: `fetch_models` (spawns a temporary scratch proxy to query upstream `/v1/models` and returns verified model names).
- **Skills / MCP** (desktop **1.8.x**): `discover_skills`, `import_skill` (folder / zip / URL), `discover_workspace_skills`, `preview_workspace_skill`, `adopt_workspace_skills`; `discover_mcp_servers`, `preview_discovered_mcp`, remote `sse` / `streamable_http` via `custom_mcp_servers`. Env/header secrets are masked in API responses; edit form shows `KEY=` only.
- **Process Control**: `stop_all`, `one_click_login` (returns `{ url, action }` for launch actions).

---

## Commands & Development

### Starting the Proxy Manually

```bash
# Start DeepSeek (Native Anthropic passthrough)
DEEPSEEK_API_KEY=sk-... python3 proxy/core/csp_proxy.py --provider deepseek --port 18991

# Start Custom OpenAI Chat translation
CSP_OPENAI_KEY=sk-... python3 proxy/core/csp_proxy.py --provider openai-custom --base-url https://api.example.com/v1 --port 18991

# Start Custom Relay (Anthropic compatible base)
CSP_RELAY_KEY=sk-... python3 proxy/core/csp_proxy.py --provider relay --base-url https://relay.example.com --port 18991
```

### Developing and Building the App

```bash
# Install dependencies (requires Node.js to run the build CLI)
cd desktop
npm install

# Run the app in development mode
npm run tauri dev

# Build the release package (.dmg / .app)
npm run tauri build
# Outputs to: src-tauri/target/release/bundle/dmg/Claude Science Proxy_<version>_aarch64.dmg
```

---

## Testing & Regression

Always verify your changes before pushing. Run the following test commands:

```bash
# Run all offline/mock regression gates (offline, loopback, scripts, rust, and frontend check)
bash test/run_all.sh

# Run all test gates requiring 100% success and no skipped env-blocked tests
bash test/run_all.sh --require-release-ready

# Run specific Python proxy unit tests
python3 -m unittest test.test_proxy_units test.test_provider_policy test.test_proxy_packaging -v

# Run Rust backend unit tests
cd desktop/src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Check frontend JS syntax
node --check desktop/src/main.js
```

---

## End-to-End Sandbox Smoke Tests

To perform a complete integration test without running the Tauri UI (while respecting sandbox and port isolation rules):

```bash
# 1. Start the proxy manually with a path secret (simulating the Tauri app)
SEC=$(python3 -c "import secrets; print(secrets.token_hex(16))")
CSP_RELAY_BASE_URL=https://api.example.com/anthropic CSP_RELAY_KEY=sk-... CSP_RELAY_MODEL=my-model \
  python3 proxy/core/csp_proxy.py --provider relay --port 18996 --auth-token "$SEC" &

# 2. Launch the isolated sandbox pointing to your manual proxy
scripts/sandbox/launch-virtual-sandbox.sh --port 8990 --proxy-url "http://127.0.0.1:18996/$SEC"

# 3. Verify sandbox health and retrieve the virtual login URL
curl -s http://127.0.0.1:8990/health
HOME=.sandbox/home '/Applications/Claude Science.app/Contents/Resources/bin/claude-science' url --data-dir .sandbox/home/.claude-science

# 4. Stop the sandbox after validation
scripts/sandbox/stop-science-sandbox.sh
```

*Note: Always pass the `--dry-run` flag if you want to dry-run scripts. Avoid using environments like `DRY_RUN=1`.*

---

## Release Checklist

1. Merge your feature branch into `main` after local verification passes.
2. Bump the version number in all 5 places:
   - `desktop/package.json`
   - `desktop/package-lock.json`
   - `desktop/src-tauri/Cargo.toml`
   - `desktop/src-tauri/Cargo.lock`
   - `desktop/src-tauri/tauri.conf.json`
3. Add release entries to `CHANGELOG.md` (move fixed items from open tasks to Changelog).
4. Build the DMG: `cd desktop && npm run tauri build`.
5. Create the Git tag and GitHub Release:
   ```bash
   git tag -a vX.Y.Z -m "Release message"
   git push origin vX.Y.Z
   gh release create vX.Y.Z <dmg-path> --title "CSP vX.Y.Z"
   ```
6. Ensure no secrets are leaked using a `gitleaks` scan before pushing.
