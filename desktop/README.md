# Desktop App (Tauri) — Developer Guide

This document covers the `desktop/` subproject only — the Tauri desktop application that wraps the CSP proxy and provides a GUI for configuration.

## Architecture

| Layer | Technology |
|-------|------------|
| Frontend | Vanilla JS + Vite (`desktop/src/main.js`, `desktop/src/index.html`) |
| Backend | Rust (Tauri commands) `desktop/src-tauri/src/` |
| Proxy | Embedded Python (`proxy/core/csp_proxy.py`) + shell scripts |
| Sandbox | Isolated home at `~/.csp/sandbox/home` |

**Bundle ID**: `com.csp.menubar`  
**Data dir**: `~/.csp/` (config `CSP.json`, logs, sandbox)

---

## Prerequisites

- Node.js 18+ (LTS)
- Rust 1.78+ (`rustup default stable`)
- Python 3.10+ (for embedded proxy)
- macOS 13+ (target platform; Linux/Windows not tested)

```bash
# One-time setup
cd desktop
npm install
```

---

## Development

```bash
# Hot-reload dev server (frontend + backend)
npm run tauri dev

# Frontend only (Vite)
npm run dev

# Rust unit tests (config, proc, crypto, sandbox)
cd src-tauri && cargo test
```

### Key Tauri Commands (Rust → JS)

| Command | Purpose |
|---------|---------|
| `get_config` | Read full config |
| `save_config` | Atomic write + 0600 perm |
| `start_proxy` | Launch embedded proxy |
| `stop_proxy` | Stop proxy, optional sandbox keep-alive |
| `list_providers` | Built-in provider templates |
| `oauth_start` / `oauth_callback` | One-click login flow |
| `resolve_model` | Model ID → provider + resolved name |

See `src-tauri/src/commands/` for full list.

---

## Resource Bundling

All runtime assets are bundled via `tauri.conf.json` → `bundle.resources`:

```
proxy/core/csp_proxy.py
proxy/core/http_transport.py
proxy/dsml/dsml_shim.py
proxy/policy/provider_policy.py
proxy/registry/model_sort.py
proxy/compat/*.py
scripts/sandbox/*.sh
scripts/maintenance/doctor.sh
scripts/ci/verify-proxy.sh
```

At runtime, Rust resolves the bundle root via `asset_root()` (`src-tauri/src/runtime/system.rs`), marker file `proxy/core/csp_proxy.py`. Same binary works in:
- `cargo tauri dev` (dev)
- `.app/Contents/Resources/` (release)

**Environment override**: `CSP_REPO=/path/to/repo` forces an external repo (useful for debugging).

---

## Build & Package

```bash
cd desktop
npm run tauri build
```

Outputs:
- `src-tauri/target/release/bundle/macos/*.app`
- `src-tauri/target/release/bundle/dmg/*.dmg` (arm64 only)

### Signing / Notarization

Current config: **ad-hoc signing only** (`bundle.macOS.signingIdentity: "-"`).
- ✅ Correctly bundles resources
- ❌ Not notarized → Gatekeeper blocks first launch
- **User workaround**: Right-click → "Open", or System Settings → Privacy & Security → "Open Anyway"

For distribution: set `signingIdentity` to a valid **Developer ID Application** certificate and enable notarization in `tauri.conf.json`.

---

## Sandbox & Data Paths

| Path | Purpose |
|------|---------|
| `~/.csp/CSP.json` | User config (atomic write, 0600) |
| `~/.csp/logs/` | Proxy stdout/stderr, Tauri logs |
| `~/.csp/sandbox/home` | Isolated Science home (claude config, auth) |
| `/tmp/csp-*.log` | Test run logs (auto-cleaned) |

**Hard guards** (enforced by shell scripts, not Tauri):
- Real port `8765` must be free
- Real dir `~/.claude-science` must NOT be touched
- Symlinks rejected on config write

---

## Testing

```bash
# Rust unit tests (config, proc, crypto, sandbox, model_sort)
cd desktop/src-tauri && cargo test

# Python proxy tests (require running Science sandbox)
cd ../../test && python -m pytest test_proxy_*.py -v

# Full smoke test (manual)
npm run tauri dev
# → Click "Start Proxy" → Verify SSE stream in DevTools
```

---

## Project Structure

```
desktop/
├── src/
│   ├── main.js          # Frontend entry (I18N, UI logic)
│   ├── index.html
│   └── styles.css
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Tauri entry, command router
│   │   ├── lib.rs            # asset_root(), sandbox paths
│   │   ├── config.rs         # Config schema + migrations
│   │   ├── commands/         # Tauri command modules
│   │   ├── runtime/          # Proxy / sandbox lifecycle, status, platter
│   │   ├── oauth_forge.rs    # Virtual OAuth forger (sandbox ticket; not Anthropic PKCE)
│   │   └── templates.rs      # Provider templates
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── README.md          # ← This file
```

---

## Common Issues

| Symptom | Fix |
|---------|-----|
| `proxy/core/csp_proxy.py` not found | Ensure `tauri.conf.json` `bundle.resources` includes nested `proxy/` paths |
| Port 8765 in use | Kill existing proxy: `pkill -f csp_proxy` |
| Config permission denied | `chmod 600 ~/.csp/CSP.json` (auto-fixed on next write) |
| Gatekeeper blocks `.app` | Right-click → Open, or allow in Privacy & Security |
| Sandbox dir not created | `mkdir -p ~/.csp/sandbox/home` (auto-created on first start) |

---

## Contributing

1. `cd desktop && npm run tauri dev`
2. Make changes (frontend in `src/`, backend in `src-tauri/src/`)
3. `cargo test` + `npm run lint` (if added)
4. PR with description of UI/UX or backend change

---

## License

MIT — same as root `LICENSE`.