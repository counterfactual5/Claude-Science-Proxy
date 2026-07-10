# Desktop App (Tauri) — Developer Guide

This document covers the `desktop/` subproject only — the Tauri desktop application that wraps the CSP proxy and provides a GUI for configuration.

## Architecture

| Layer | Technology |
|-------|------------|
| Frontend | Vanilla JS + Vite (`desktop/src/main.js`, `desktop/src/index.html`) |
| Backend | Rust (Tauri commands) `desktop/src-tauri/src/` |
| Proxy | Embedded Python (`proxy/csp_proxy.py`) + shell scripts |
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
proxy/csp_proxy.py
proxy/http_transport.py
proxy/dsml_shim.py
proxy/provider_policy.py
proxy/capability_catalog.py
proxy/model_sort.py
scripts/*.sh
```

At runtime, Rust resolves the bundle root via `asset_root()` (`src-tauri/src/lib.rs`), so the same binary works in:
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
│   └── style.css
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Tauri entry, command router
│   │   ├── lib.rs            # asset_root(), sandbox paths
│   │   ├── config.rs         # Config schema + migrations
│   │   ├── commands/         # Tauri command modules
│   │   ├── runtime/          # Proxy process management
│   │   ├── oauth_forge.rs    # OAuth PKCE flow
│   │   └── templates.rs      # Provider templates
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── vite.config.js
└── README.md          # ← This file
```

---

## Common Issues

| Symptom | Fix |
|---------|-----|
| `csp_proxy.py` not found | Ensure `tauri.conf.json` `bundle.resources` includes `proxy/` |
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