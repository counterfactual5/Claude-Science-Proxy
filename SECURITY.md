# Security Policy

Claude Science Proxy (CSP) launches Claude Science in an isolated sandbox and
routes model inference through a **local, loopback-only proxy**. Because it sits
in the request path and handles third-party API keys, we take security reports
seriously.

## Supported versions

CSP follows a rolling release model. Only the **latest release** on the
[Releases page](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest)
receives security fixes. Please upgrade before reporting.

| Version | Supported |
|---------|-----------|
| Latest release (`main`) | ✅ |
| Older releases | ❌ |

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please use GitHub's private reporting channel:

1. Go to the repository's **Security** tab →
   **[Report a vulnerability](https://github.com/counterfactual5/Claude-Science-Proxy/security/advisories/new)**.
2. Describe the issue, affected version, and reproduction steps.
3. Do **not** include real API keys, tokens, or launch secrets — redact them.

If private advisories are unavailable to you, open a minimal public issue that
says only "requesting a private security contact" (no details), and we will
follow up.

### What to expect

- Acknowledgement as soon as the report is reviewed.
- A fix or mitigation plan for confirmed, in-scope issues, released in the next
  version.
- Credit in the release notes if you'd like it.

This is a small, community-maintained project — please allow reasonable time for
a response before any public disclosure.

## Scope

CSP's security model rests on a few invariants. Reports that show these being
broken are especially valuable:

- **Credential isolation** — CSP must never read, copy, or leak real
  `~/.claude-science` OAuth tokens or account state. The sandbox uses only a
  locally forged launch ticket, not an Anthropic credential.
- **Loopback-only proxy** — the proxy binds to `127.0.0.1` behind a per-session
  path secret and must not be reachable off-host.
- **Key handling** — third-party API keys live in `~/.csp/CSP.json` (mode `0600`),
  are passed to the proxy via environment variables, and are masked before
  reaching the frontend. Science's inbound `Authorization` / `x-api-key` headers
  are stripped before your provider key is injected.
- **Sandbox/port isolation** — sandboxed Science runs under an independent
  `HOME`, port, and data directory; it must never collide with the real instance
  on port **8765**.

### Out of scope

- Vulnerabilities in Claude Science itself, or in third-party providers you
  route to (DeepSeek, GLM, Kimi, MiniMax, OpenRouter, etc.).
- Issues that require an already-compromised local machine or admin access.
- The app is **not Apple-notarized** yet; the first-launch Gatekeeper prompt is
  a known limitation, not a vulnerability.

## A note on keys

CSP never transmits your keys anywhere except the provider endpoint **you**
configure. If you believe a key was exposed (e.g. pasted into a log or issue),
rotate it immediately at your provider.

---

## 中文摘要

CSP 处于推理请求路径上并管理第三方 API key，安全问题请认真对待。

- **仅支持最新版本**，报告前请先升级到 [最新 Release](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest)。
- **请勿公开提交安全漏洞**。走仓库 **Security → [Report a vulnerability](https://github.com/counterfactual5/Claude-Science-Proxy/security/advisories/new)** 私有上报；描述里**不要包含真实 key/token/密钥**。
- **重点范围**：真实 `~/.claude-science` 凭证隔离、代理仅监听 `127.0.0.1`（带 path secret）、key 存于 `~/.csp/CSP.json`（`0600`）且经环境变量传递并脱敏、沙箱使用独立 `HOME`/端口（绝不占用真实端口 **8765**）。
- **不在范围**：Claude Science 本身或第三方 provider 的漏洞、需要本机已被攻陷的问题、未公证导致的首次启动提示（已知限制，非漏洞）。
- 若怀疑 key 泄露（如粘进日志/issue），请立即到 provider 处**轮换密钥**。
