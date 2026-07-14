# Contributing to Claude Science Proxy (CSP)

Thanks for your interest in improving CSP. This guide covers how to file issues,
propose changes, and get a pull request merged.

> New to the codebase? Read [`README.md`](./README.md), then
> [`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md) (build/test/architecture) and
> [`AGENT.md`](./AGENT.md) (safety rules and code style).

---

## Ground rules (read first)

CSP launches Claude Science in an **isolated sandbox** and routes inference
through a **local proxy**. Contributions must respect these invariants:

- **Never** read, copy, modify, or delete real `~/.claude-science` credentials,
  OAuth tokens, account state, or conversation databases.
- **Never** run against the real Science instance on port **8765**. Sandboxed
  runs must use an independent `HOME`, port, and data directory.
- **Never** commit secrets (API keys, tokens, `.env`). A `gitleaks` scan runs
  before releases; keep the tree clean.
- Production code comments are written in **English**.

The full list lives in the "Iron Rules" section of
[`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md).

---

## Reporting issues

Use [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose)
and pick the right template.

- **Do not paste API keys, tokens, or launch secrets** into issues or logs.
- Include: macOS version, chip (Apple Silicon), CSP version, provider/profile
  type, and the exact steps to reproduce.
- For **security vulnerabilities**, do **not** open a public issue — see
  [`SECURITY.md`](./SECURITY.md).

Support is via GitHub Issues only. There is no WeChat/QQ/DM channel.

---

## Development setup

Requirements: macOS **Apple Silicon**, `python3`, Node.js (for the Tauri build
CLI), and the Rust toolchain.

```bash
# App (frontend + Rust backend)
cd desktop && npm install && npm run tauri dev

# Proxy (standalone, for iterating on protocol translation)
DEEPSEEK_API_KEY=sk-... python3 proxy/core/csp_proxy.py --provider deepseek --port 18991
```

See [`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md) for the full repository layout,
manual proxy invocations, and end-to-end sandbox smoke tests.

---

## Making changes

1. Create a feature branch off `main`.
2. Keep changes focused; avoid unrelated refactors in the same PR.
3. Match existing style: English comments, `CSP_*` env prefix, localized
   user-visible strings (see the i18n section of `docs/DEVELOPMENT.md`).
4. Update docs/`CHANGELOG.md` when behavior changes.

### Verify before pushing

```bash
bash test/run_all.sh                       # offline regression gate (required)
(cd desktop/src-tauri && cargo test)       # if you touch Rust
(cd desktop/src-tauri && cargo clippy --all-targets -- -D warnings && cargo fmt --check)
node --check desktop/src/main.js           # if you touch the frontend
```

Real-machine / Science E2E tests are opt-in and require isolated `HOME` + ports.
Follow [`test/docs/REAL_MACHINE_TEST.md`](./test/docs/REAL_MACHINE_TEST.md) — never
touch real `~/.claude-science` or port **8765** without the guard scripts.

---

## Pull requests

- Fill in the [PR template](./.github/pull_request_template.md), including the
  test plan and the iron-rules checklist.
- Link related issues (`Fixes #123`).
- Describe the fix in plain language; avoid vague messages like "fix P1".
- A green `bash test/run_all.sh` is expected for every PR.

By contributing, you agree that your contributions are licensed under the
project's [MIT License](./LICENSE).

---

## Cutting a GitHub Release

When shipping a public version (not only a local build):

1. Bump version in lockstep: `desktop/package.json`, `desktop/src-tauri/Cargo.toml`,
   `desktop/src-tauri/tauri.conf.json`, README badges, and `CHANGELOG.md`.
2. Update the README “what’s new” callout (EN + ZH) — do **not** leave
   “local build; no GitHub release” after you published.
3. Update the GitHub repo **About** description (and topics if needed) to the new
   version / capabilities:
   `gh repo edit --description "… vX.Y.Z."`
4. Push `main`, tag `vX.Y.Z`, and `gh release create` with the DMG + notes
   (Summary + Install; skip internal Test-plan checklists on public notes unless
   asked).
5. After release: rewrite notes if they understate the jump from the previous
   public tag.

---

## 中文摘要

欢迎贡献。提交前请先读 [`README.md`](./README.md)、[`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md)、[`AGENT.md`](./AGENT.md)。

- **红线**：绝不读取/复制/修改真实 `~/.claude-science` 凭证；绝不占用真实端口 **8765**；绝不提交密钥。沙箱运行必须使用独立 `HOME`、端口和数据目录。
- **提 Issue**：走 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose)，附 macOS 版本、芯片、CSP 版本、provider 类型和复现步骤；**不要粘贴 API key**。安全漏洞请看 [`SECURITY.md`](./SECURITY.md)，勿开公开 issue。
- **提交前自测**：`bash test/run_all.sh`（必跑）；改动 Rust 跑 `cargo test` / `clippy` / `fmt`；改动前端跑 `node --check`。
- **PR**：按模板填写测试计划和红线清单，关联相关 issue。贡献代码即视为接受 [MIT 许可](./LICENSE)。
- **发版**：升版本号 → 更新 README 亮点（中英）→ **同步 GitHub About 描述** → push / tag / Release + DMG。