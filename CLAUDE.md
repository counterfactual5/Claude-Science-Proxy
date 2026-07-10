# Claude Science Proxy (CSP)

让 Claude Science 的模型推理走第三方 API（DeepSeek、GLM、Kimi、MiniMax、小米 MiMo、OpenRouter，或任意 OpenAI / Anthropic 兼容端点），保留 Science 的 agent 科研体验，模型换成你自己的。

> 本文件只留**铁律 + 架构 + 速查指针**。详细内容分散在：
> 逆向与已验证事实 → [`docs/verified-facts.md`](docs/verified-facts.md)；
> 已知问题/待修队列 → [`docs/known-issues.md`](docs/known-issues.md)；
> 命令与构建 → [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) / [`desktop/README.md`](desktop/README.md)。

---

## 一、铁律（最高优先级，任何会话都不得违反）

1. **绝不影响用户真实的订阅与登录状态。** 真实 Claude Science 的数据目录是
   `~/.claude-science`，登录凭证在 `~/.claude-science/.oauth-tokens`、
   `active-org.json`、`encryption.key`、`orgs/`、`.key-backups/`。
   这些文件**只读都要谨慎，绝不复制、绝不修改、绝不删除**。
2. **绝不把真实 OAuth token 复制进任何沙箱。** 复制后两个实例共享同一 token，
   刷新时 Anthropic 可能轮换刷新令牌，导致用户真实实例被登出。要给沙箱登录，
   只能在沙箱里**全新独立登录**（另一套会话 token，对真实登录零影响），且由用户
   手动完成，Claude 不代做登录。
3. **绝不用改过的环境变量去启动用户的真实实例。** 真实实例跑在端口 8765。
   所有实验用的沙箱必须用**独立 data-dir + 独立端口 + 独立 HOME**，与 8765 完全隔开。
4. **测试默认不碰 Science。** 能用「代理↔上游」单独验证的，就不启动 Science
   （见 `test/`）。只有到最终整链联调、且用户明确同意时，才启动沙箱 Science，
   并且仍然遵守第 2、3 条。
5. 动任何有状态的东西前，先确认它不在铁律清单里；拿不准就停下来问用户。

---

## 二、架构

```
Claude Science（沙箱 · 虚拟登录；登录仅当启动门票，推理不走 Anthropic）
   │  ANTHROPIC_BASE_URL=http://127.0.0.1:<port>/<secret>
   ▼
翻译代理 proxy/csp_proxy.py（默认 deepseek 原生透传，可切）
   │  剥离入站 OAuth Bearer，注入第三方 key，按需 Anthropic ↔ OpenAI 互转，path-secret 鉴权
   ▼
DeepSeek 原生 Anthropic 端点          --provider deepseek
任意 OpenAI Chat Completions 兼容端点  --provider openai-custom
任意 OpenAI Responses 兼容端点         --provider openai-responses
自定义 Anthropic 兼容中继端点           --provider relay
```

关键点：Claude 登录只是**启动 Science 的门票**，推理被 `ANTHROPIC_BASE_URL`
导去本地代理后，Anthropic 服务端不经手推理。门票用**本地伪造的虚拟 OAuth**
越过（零真实凭证、无需任何 Anthropic 账号）。

---

## 三、速查

- **必知三条**：① `ANTHROPIC_BASE_URL` 无条件生效；② 手动填 API key 被 operon
  写死拒绝，**必须有 OAuth 门票**；③ 门票用本地伪造虚拟 OAuth 越过。
  完整证据与格式 → `docs/verified-facts.md`。
- **发布态 / 待办**：仓库
  [github.com/counterfactual5/Claude-Science-Proxy](https://github.com/counterfactual5/Claude-Science-Proxy)；
  欢迎 issue 与 PR。桌面 app 在 `desktop/`（Tauri 正常窗口进程管家）。
  **当前 Latest 版本以 GitHub Releases / [`CHANGELOG.md`](CHANGELOG.md) 为准**；
  **待办队列与排期** → [`docs/known-issues.md`](docs/known-issues.md)。
- **对外文案（脱敏）**：用户**可见**文案不露骨，别直说「越过 / 绕过登录」；
  主按钮用「一键开始」类中性说法。技术**内部**文档描述机制时可仍用「越过门票」。
  详见 `docs/known-issues.md` 第 1 条。
- **默认 provider**：DeepSeek（原生 Anthropic 透传）。模型映射与选择器 id 见
  `proxy/csp_proxy.py` 的 `PROVIDERS` 字典。
- **每日维护巡检**：launchd 每天 09:00/21:00（Asia/Shanghai）跑受限 `claude -p`，
  **只读仓库 + 抓公开网页 + 只往 `findings/auto-maint/` 写规划报告**
  （白名单工具、硬禁 commit/push/rm、禁读写 `~/.claude-science`、不启动 Science）。
  装卸看：`scripts/install-maintenance.sh {install|uninstall|status|run}`。

---

## 四、目录与常用命令

```
proxy/csp_proxy.py                provider 可切代理（deepseek 默认）【主入口】
scripts/make-virtual-oauth.mjs    虚拟 OAuth 伪造器（Node，字节级一致；只写沙箱，护栏拒真实目录）
scripts/launch-virtual-sandbox.sh 起沙箱 Science + 写虚拟登录 + 指向代理（推荐整链入口）
scripts/stop-science-sandbox.sh   停沙箱（按 data-dir，绝不影响真实 8765）
scripts/doctor.sh                 只读环境诊断（依赖/key 有无/端口/权限/铁律自检）
scripts/verify-proxy.sh           校验运行中的代理（/health + /v1/models，零上游花费）
scripts/self-test.sh              离线回归套件（test/run_all.sh 包装）
scripts/*maintenance*             每日巡检 wrapper/提示词/launchd/安装器
desktop/                          Tauri 桌面 app（正常窗口，构建见 desktop/README.md）
test/                             隔离回归测试（只打代理，不碰 Science）
findings/                         证据、二进制分析、诊断记录；auto-maint/ 是巡检输出（git 忽略）
.sandbox/                         沙箱 Science 独立 HOME/data-dir（git 忽略）
```

**常用命令**（更多见 `docs/DEVELOPMENT.md`）：

```bash
# 离线回归（最常用，自动起/停代理）
bash test/run_all.sh

# 起代理 — DeepSeek（原生 Anthropic 透传，默认）
DEEPSEEK_API_KEY=sk-... python3 proxy/csp_proxy.py --provider deepseek --port 18991

# 起代理 — 任意 OpenAI Chat Completions 兼容端点
CSP_OPENAI_KEY=sk-... python3 proxy/csp_proxy.py \
  --provider openai-custom \
  --base-url https://api.example.com/v1 \
  --model my-model --port 18991

# 起代理 — 自定义 Anthropic 兼容中继
CSP_RELAY_KEY=sk-... python3 proxy/csp_proxy.py \
  --provider relay \
  --base-url https://relay.example.com \
  --port 18991

# 整链（虚拟登录，推荐）
# 先起代理（见上），再：
scripts/launch-virtual-sandbox.sh --port 8990 \
  --proxy-url http://127.0.0.1:18991/<secret>
# 停：
scripts/stop-science-sandbox.sh
```

---

## 五、环境备忘

- 真实 Science 数据目录 `~/.claude-science`、端口 8765，绝对不碰。
- 上游 key 在用户 shell 环境：`DEEPSEEK_API_KEY` / `CSP_OPENAI_KEY` /
  `CSP_RELAY_KEY` 等（值不显示、不入库）。
- **代理环境变量**：若本机同时设了大写 `HTTPS_PROXY` 与小写 `https_proxy`
  且指向不同端口，`gh`（Go）会读大写变量，可能导致连 GitHub 失败或误报 token
  invalid。发版前请确认大小写变量**一致**（或临时 `unset` 大写变量再跑 `gh`/`git`）。
- Python 用 conda 环境，避免系统 3.9。

---

## Section 1 — Iron Rules (English summary)

> **Highest priority. No session may violate these.**

1. **Never touch the user's real Claude Science data.** Real data lives at
   `~/.claude-science`. Credentials (`oauth-tokens`, `encryption.key`, `orgs/`,
   etc.) must **never be read carelessly, copied, modified, or deleted**.
2. **Never copy a real OAuth token into any sandbox.** Sharing a token between
   two instances risks Anthropic rotating the refresh token, logging out the
   real instance. Sandbox login must be done via a fresh independent login inside
   the sandbox—never automated by the AI.
3. **Never start the real Science instance with modified env vars.** The real
   instance runs on port 8765. All sandboxes must use an **independent
   data-dir + port + HOME**, fully isolated from 8765.
4. **Tests must not touch Science by default.** Validate proxy↔upstream in
   isolation (`test/`). Only start a sandbox Science for end-to-end testing with
   explicit user consent, still obeying rules 2 and 3.
5. Before touching anything stateful, confirm it is not on the iron-rules list.
   When in doubt, stop and ask the user.

## Section 2 — Architecture (English)

```
Claude Science (sandbox · virtual login; login = ticket only, inference bypasses Anthropic)
   │  ANTHROPIC_BASE_URL=http://127.0.0.1:<port>/<secret>
   ▼
Translation proxy  proxy/csp_proxy.py  (default: deepseek native passthrough)
   │  Strips inbound OAuth Bearer, injects third-party key,
   │  translates Anthropic ↔ OpenAI as needed, path-secret auth
   ▼
DeepSeek native Anthropic endpoint     --provider deepseek
Any OpenAI Chat Completions endpoint   --provider openai-custom
Any OpenAI Responses endpoint          --provider openai-responses
Custom Anthropic-compatible relay      --provider relay
```

The login is only a **ticket** to start Science. Once `ANTHROPIC_BASE_URL`
redirects inference to the local proxy, Anthropic's servers never handle
inference. The ticket is bypassed via a **locally forged virtual OAuth token**
(zero real credentials, no Anthropic account required).
