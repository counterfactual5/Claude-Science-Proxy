<p align="center">
  <img src="docs/assets/social-preview.png" alt="Claude Science Proxy — run Claude Science on your own model APIs" width="760">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License">
  <img src="https://img.shields.io/badge/version-1.9.0-brightgreen.svg" alt="v1.9.0">
  <img src="https://img.shields.io/badge/platform-macOS%20Apple%20Silicon-1d1d1f.svg" alt="macOS Apple Silicon">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-C25A34.svg" alt="Tauri 2">
</p>

<p align="center">
  <strong>English</strong> · <a href="./README.zh.md">简体中文</a>
</p>

# Claude Science Proxy (CSP)

**Run [Claude Science](https://claude.com) on the model APIs you already pay for** — DeepSeek, GLM, Kimi, MiniMax, OpenRouter, or any Anthropic- / OpenAI-compatible endpoint — while keeping Science’s agent workflow: tool use, code execution, and skills (where supported).

CSP is a **macOS desktop app** (Tauri) that:

1. Starts Claude Science in an **isolated sandbox**
2. Prepares a **local launch ticket** (no copy of your real Claude login)
3. Routes inference through a **local proxy** on `127.0.0.1`
4. Verifies API keys **before** switching the active profile
5. Manages local **Skills** and **MCP connectors** (stdio + remote), deploying enabled ones into the sandbox on launch

> **v1.9.0** — Multi-provider custom model platter (up to 8 models across saved providers); plus v1.8.2 Science skill library sync. See [Releases](https://github.com/counterfactual5/Claude-Science-Proxy/releases/tag/v1.9.0) · [Changelog](./CHANGELOG.md).

> **Platform:** macOS **Apple Silicon** today. The app is **not notarized** yet; on first launch, right-click → **Open**.

[Download latest release](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest) · [Changelog](./CHANGELOG.md) · [Report a bug](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose)

---

## Why CSP exists

Claude Science is Anthropic’s research-oriented agent app (literature review, data analysis, plotting, coding, writing). By default it expects a Claude subscription and Anthropic-hosted inference.

**Claude Science Proxy** is a local control plane:

| Layer | What CSP does |
|-------|----------------|
| Sandbox | Separate HOME, ports, and data under `~/.csp/sandbox` |
| Launch ticket | Locally forged OAuth-shaped ticket so Science can start without your real Claude credentials |
| Proxy | Forwards `/v1/messages` (and related) to your chosen provider |
| Translation | Anthropic ↔ OpenAI Chat / Responses when the upstream is not native Anthropic |

```text
Claude Science (sandbox)
        │
        ▼
  CSP local proxy  (127.0.0.1:<port>/<secret>)
        │
        ▼
  DeepSeek / GLM / Kimi / MiniMax / OpenRouter / your endpoint
```

---

## Features

### For everyday use

- **Multiple profiles** — different keys, models, or relay URLs; only one active at a time
- **Verify before switch** — invalid keys are rejected; CSP does not silently activate a broken profile
- **One-click start** — launches proxy, prepares sandbox, opens Science
- **Real model names** in Science’s selector (not a generic `claude` / `opus` label)
- **Multi-model per profile** — virtual registry maps up to **8** `claude-*` shell IDs to real upstream models
- **Local Skills manager** — create; import from folder, zip, or URL; enable/disable; scan-and-import from other agents; sync Science skill library (harvest edits, full-screen preview); built-in **`csp-environment`** handbook; enabled Skills deploy into the sandbox on launch
- **Local MCP manager** — add/edit **stdio** or **remote** (sse / streamable_http) connectors; scan-and-import from other AI clients with JSON/TOML config preview; enabled connectors deploy into the sandbox on launch
- **Built-in web-search MCP** — no key required for the free path: GENERAL (`csp_web_search` → DuckDuckGo IA/Lite) and LITERATURE (`search_literature` → Wikipedia / Crossref / arXiv / PubMed); optional Brave/Serper/Tavily keys; Start auto-grants provider hosts (`~/.csp/network-allowlist.json` for extras)

### For power users

- Native **Anthropic-compatible** passthrough (DeepSeek, Kimi, MiniMax, GLM, …)
- **Custom Anthropic** relay URLs
- **Custom OpenAI Chat** and **OpenAI Responses** base roots (proxy adds `/chat/completions`, `/responses`, `/models`)
- Read-only **capability catalog** for known provider / Science version boundaries
- Local config: `~/.csp/CSP.json` (`0600`); logs under `~/.csp/logs/`; MCP inventory at `~/.csp/mcp/inventory.json` (`0600`)

---

## Quick start

**You need**

- [Claude Science](https://claude.com) installed
- macOS on **Apple Silicon**
- A third-party API key
- `python3` on PATH (proxy runtime; moving to Rust is planned)

**Steps**

1. Download `Claude Science Proxy_*.dmg` from [Releases](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest).
2. Drag the app to **Applications**. If Gatekeeper blocks it, **right-click → Open**.
3. Click **+ New**, pick a provider, enter your API key, models (multi-select), and `base_url` if needed.
4. Click **Create**, then select the profile card to make it **active**.
5. Click **Start Claude Science** after key verification succeeds.
6. Science opens in the sandbox; the model picker shows the names you configured.

---

## Supported providers

| Provider | Integration | Notes |
|----------|-------------|--------|
| DeepSeek | Native Anthropic API | Default; best effort on thinking, tools, streaming |
| GLM (Zhipu) | Anthropic-compatible | Editable default URL |
| Kimi / Moonshot | Anthropic-compatible | Editable default URL |
| MiniMax | Anthropic-compatible | Editable default URL |
| Xiaomi MiMo | Anthropic-compatible | Plan / regional endpoints supported |
| OpenRouter | Anthropic-compatible aggregate | Pick or type a model |
| Custom Anthropic | Your `/anthropic` or compatible URL | Private gateways, relays |
| Custom OpenAI | OpenAI Chat base root | Proxy appends `/chat/completions` |
| Custom OpenAI Responses | OpenAI Responses base root | Proxy appends `/responses` |

> Use **Custom Anthropic** for `/anthropic` URLs. Use **Custom OpenAI** only for OpenAI-shaped roots like `https://example.com/v1`.

OpenAI-compatible providers are configured through **Custom OpenAI** / **Custom OpenAI Responses**.

---

## Virtual model registry

Science only accepts model IDs starting with `claude-`. CSP allocates **up to eight shell IDs** (3 in the main list + 5 under “More models”) and maps each shell to a real upstream model. Display names are sanitized for Science’s `V2_` filter (e.g. `glm-5-turbo` → `glm-5.turbo` in the UI; outbound requests still use the real ID).

---

## How CSP protects your real Claude account

- Does **not** copy, read, modify, or delete real Claude OAuth tokens, account state, or conversation data
- May **read-only clone** runtime binaries (`bin`, `conda`, `runtime`, `seed-assets`) from `~/.claude-science` on first sandbox setup — not credentials
- Stores third-party keys only in `~/.csp/CSP.json`; passes them via **environment variables** to the proxy
- Proxy listens on **loopback only** and strips Science’s `Authorization` / `x-api-key` before injecting your provider key

---

## Current limitations

- **Anthropic-hosted cloud features** (remote/hosted MCP, directory connectors) and some cloud-only capabilities are unavailable or fast-fail — **local stdio and custom remote MCP connectors are supported** via the MCP tab
- **Hosted top-level `web_search` / `web_fetch` are unavailable** under CSP virtual login — use the built-in `web-search` MCP: `host.mcp("web-search", "csp_web_search" | "search_literature" | "fetch_url", …)` and read `data["results"]`
- Provider quality varies for tools, long context, thinking, images, and streaming
- **OpenAI-compatible profiles** (`openai-custom` / `openai-responses`, including GLM on OpenAI Chat URLs): long sessions and **Resume** are supported since **v1.7.1** (counted SSE keepalives); upstream **429 / fair-use** rate limits and very slow responses can still cause retries — see [known issues](./docs/known-issues.md#openai-custom-streaming)
- **Not Apple-notarized** — manual approval on first open
- Proxy still requires **`python3`** today

Details: [`docs/known-issues.md`](./docs/known-issues.md)

---

## Contributing

Issues and PRs welcome. Start with [`CONTRIBUTING.md`](./CONTRIBUTING.md), then read [`AGENT.md`](./AGENT.md) (safety rules) and [`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md). Security reports: [`SECURITY.md`](./SECURITY.md).

```bash
bash test/run_all.sh
(cd desktop/src-tauri && cargo test)   # if you touch Rust
```

Real-machine tests: [`test/docs/REAL_MACHINE_TEST.md`](./test/docs/REAL_MACHINE_TEST.md) — never touch real `~/.claude-science` or port **8765** without the guard scripts.

**Support:** [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues) only — no WeChat/QQ/DM. Do not paste API keys in issues.

---

## Development

```bash
cd desktop && npm install && npm run tauri dev
```

Further reading: [desktop/README.md](./desktop/README.md) · [docs/DEVELOPMENT.md](./docs/DEVELOPMENT.md)

---

## Disclaimer

For personal learning and research. **Not affiliated with Anthropic.** Inference goes to **your** third-party providers. The local launch ticket is not an Anthropic credential. Software is provided **as is**, without warranty. See full text in the [Chinese README](./README.zh.md#风险与免责声明) or project docs.

## License

[MIT](./LICENSE)
