# Claude Science Proxy (CSP)

让 Claude Science 的模型推理走第三方 API（DeepSeek、GLM、Kimi、MiniMax、小米 MiMo、OpenRouter，或任意 OpenAI / Anthropic 兼容端点），保留 Science 的 agent 科研体验。类比 CC Switch 之于 Claude Code。

> 本文件只留**铁律 + 架构 + 速查指针**。详细内容：
> 逆向与已验证事实 → [`docs/verified-facts.md`](docs/verified-facts.md)；
> 已知问题 → [`docs/known-issues.md`](docs/known-issues.md)；
> 命令与构建 → [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) / [`desktop/README.md`](desktop/README.md)；
> 历史证据 → [`findings/`](findings/)。

## 一、铁律（最高优先级，任何会话都不得违反）

1. **绝不影响用户真实的订阅与登录状态。** 真实 Claude Science 的数据目录是 `~/.claude-science`，登录凭证在 `~/.claude-science/.oauth-tokens`、`active-org.json`、`encryption.key`、`orgs/`、`.key-backups/`。这些文件**只读都要谨慎，绝不复制、绝不修改、绝不删除**。
2. **绝不把真实 OAuth token 复制进任何沙箱。** 复制后两个实例共享同一 token，刷新时 Anthropic 可能轮换刷新令牌，导致用户真实实例被登出。要给沙箱登录，只能在沙箱里**全新独立登录**（另一套会话 token，对真实登录零影响），且由用户手动完成，Claude 不代做登录。
3. **绝不用改过的环境变量去启动用户的真实实例。** 真实实例跑在端口 8765。所有实验用的沙箱必须用**独立 data-dir + 独立端口 + 独立 HOME**，与 8765 完全隔开。
4. **测试默认不碰 Science。** 能用「代理↔上游」单独验证的，就不启动 Science（见 `test/`）。只有到最终整链联调、且用户明确同意时，才启动沙箱 Science，并且仍然遵守第 2、3 条。
5. 动任何有状态的东西前，先确认它不在铁律清单里；拿不准就停下来问用户。

## 二、架构

**两层 provider 模型（勿混淆）：**

| 层 | 位置 | 作用 |
|----|------|------|
| 面板模板 | `desktop/src-tauri/src/templates.rs` | 用户看到的 DeepSeek / GLM / Kimi / …（`template_id`） |
| 代理运行时 | `proxy/csp_proxy.py` 的 `PROVIDERS` | 仅 **4** 个 `--provider`：`deepseek` · `relay` · `openai-custom` · `openai-responses` |

映射：DeepSeek → `deepseek`；GLM / Kimi / MiniMax / MiMo / OpenRouter / Custom Anthropic → `relay`；Custom OpenAI → `openai-custom`；Custom OpenAI Responses → `openai-responses`。

```
Claude Science（沙箱 · 虚拟登录；登录仅当启动门票，推理不走 Anthropic）
   │  ANTHROPIC_BASE_URL=http://127.0.0.1:<port>/<secret>
   ▼
翻译代理 proxy/csp_proxy.py（--provider 四选一；默认 deepseek 原生透传）
   │  剥离入站 OAuth Bearer，注入第三方 key，按需 Anthropic ↔ OpenAI 互转，path-secret 鉴权
   ▼
上游 API（由模板 + env 决定）
   deepseek          → DeepSeek 原生 Anthropic 端点
   relay             → 任意 Anthropic 兼容 base（GLM / Kimi / OpenRouter / …）
   openai-custom     → OpenAI Chat Completions 兼容根
   openai-responses  → OpenAI Responses 兼容根
```

关键点：Claude 登录只是**启动 Science 的门票**，推理被 `ANTHROPIC_BASE_URL` 导去本地代理后，Anthropic 服务端不经手推理。门票用**本地伪造的虚拟 OAuth** 越过（零真实凭证、无需任何 Anthropic 账号）。

## 三、速查（详情见 docs/）

- **必知三条**：① `ANTHROPIC_BASE_URL` 无条件生效；② 手动填 API key 被 operon 写死拒绝，**必须有 OAuth 门票**；③ 门票用本地伪造虚拟 OAuth 越过。完整证据 → `docs/verified-facts.md`。
- **发布态 / 待办**：仓库 [github.com/counterfactual5/Claude-Science-Proxy](https://github.com/counterfactual5/Claude-Science-Proxy)；欢迎 issue 与 PR。桌面 app 在 `desktop/`（Tauri **正常窗口**，已去托盘）。版本以 GitHub Releases / [`CHANGELOG.md`](CHANGELOG.md) 为准；待办 → [`docs/known-issues.md`](docs/known-issues.md)。
- **对外文案（脱敏）**：用户**可见**文案不露骨，别直说「越过 / 绕过登录」；主按钮用「一键开始」类中性说法。技术内部文档可仍用「越过门票」。见 `docs/DEVELOPMENT.md`「对外文案脱敏」。
- **默认上游**：DeepSeek（`--provider deepseek`，原生 Anthropic 透传）。虚拟模型注册表与壳 ID → `proxy/model_registry.py`；模板表 → `templates.rs`。
- **每日维护巡检**：launchd 每天 09:00/21:00（Asia/Shanghai）跑受限 `claude -p`，**只读仓库 + 抓公开网页 + 只往 `findings/auto-maint/` 写规划报告**（白名单工具、硬禁 commit/push/rm、禁读写 `~/.claude-science`、不启动 Science）。装卸：`scripts/install-maintenance.sh {install|uninstall|status|run}`。

## 四、目录与常用命令

```
proxy/csp_proxy.py              【主】代理（--provider 四选一；默认 deepseek）
proxy/model_registry.py         虚拟模型注册表（8 壳 ID）
scripts/make-virtual-oauth.mjs  虚拟 OAuth 伪造器（只写沙箱，护栏拒真实目录）
scripts/launch-virtual-sandbox.sh  起沙箱 Science + 虚拟登录 + 指向代理（推荐）
scripts/stop-science-sandbox.sh    停沙箱（绝不影响真实 8765）
scripts/doctor.sh                  只读环境诊断
scripts/verify-proxy.sh            校验运行中代理（/health + /v1/models）
scripts/self-test.sh               → test/run_all.sh 包装
desktop/                           Tauri 桌面 app（templates.rs = 面板模板源）
test/                              隔离回归（默认只打代理，不碰 Science）
findings/                          证据归档；auto-maint/ 巡检输出（git 忽略）
.sandbox/                          沙箱 HOME/data-dir（git 忽略）
```

**起代理（开发调试）**

```bash
# deepseek（默认）
DEEPSEEK_API_KEY=... python3 proxy/csp_proxy.py --provider deepseek --port 18991

# relay（面板 GLM / Kimi / OpenRouter 等预设走这条）
CSP_RELAY_BASE_URL=https://open.bigmodel.cn/api/anthropic \
CSP_RELAY_KEY=... CSP_RELAY_MODEL=glm-5.2 \
  python3 proxy/csp_proxy.py --provider relay --port 18991

# openai-custom
CSP_OPENAI_BASE_URL=https://example.com/v1 CSP_OPENAI_KEY=... \
  python3 proxy/csp_proxy.py --provider openai-custom --port 18991

# openai-responses
CSP_OPENAI_BASE_URL=https://example.com/v1 CSP_OPENAI_KEY=... \
  python3 proxy/csp_proxy.py --provider openai-responses --port 18991
```

**测试**

```bash
bash test/run_all.sh                        # 离线全量回归（最常用）
bash test/run_all.sh --require-release-ready  # 发版前门禁
bash scripts/self-test.sh                   # 同上（薄包装）
```

**整链（虚拟登录，须用户同意）**

```bash
# 先起代理，再：
scripts/launch-virtual-sandbox.sh --port 8990 --proxy-url http://127.0.0.1:18991/<secret>
scripts/stop-science-sandbox.sh
```

更多见 `docs/DEVELOPMENT.md`。

## 五、环境备忘

- 真实 Science：`~/.claude-science`、端口 **8765**，绝对不碰。
- 用户数据：`~/.csp/CSP.json`、`logs/`、`sandbox/home`。
- 上游 key 经环境变量注入代理（`DEEPSEEK_API_KEY`、`CSP_RELAY_*`、`CSP_OPENAI_*`），不入库、不进 argv。
- **代理环境变量**：若本机 `HTTPS_PROXY` 与 `https_proxy` 指向不同端口，Go 系工具（`gh`）可能连 GitHub 失败。发版前确认一致或 `unset` 大写变量。

---

## Iron Rules (English)

**Highest priority — never violate in any session:**

1. **Never read, copy, modify, or delete** real Claude login state under `~/.claude-science` (OAuth tokens, `active-org.json`, `encryption.key`, `orgs/`, etc.).
2. **Never copy real OAuth tokens into a sandbox.** Sandboxes use locally forged virtual tickets only.
3. **Never start the user's real Science instance** with modified env vars. Real instance uses port **8765**. All experiments need isolated HOME + port + data-dir.
4. **Tests default to proxy-only.** Start sandbox Science only for end-to-end checks with explicit user consent.
5. When unsure whether an action touches real credentials or port 8765 — **stop and ask**.

## Architecture (English)

CSP runs Claude Science in an **isolated sandbox** with a **local launch ticket**, then routes inference to a **loopback proxy** (`proxy/csp_proxy.py`).

**Two layers — do not conflate:**

- **UI templates** (`desktop/src-tauri/src/templates.rs`): DeepSeek, GLM, Kimi, etc.
- **Proxy runtime** (`PROVIDERS` in `csp_proxy.py`): only `--provider` **`deepseek` | `relay` | `openai-custom` | `openai-responses`**.

Flow: Science → `ANTHROPIC_BASE_URL=http://127.0.0.1:<port>/<secret>` → CSP proxy → upstream API. Login is a local ticket only; inference does not use Anthropic hosted models.

**Repo:** [counterfactual5/Claude-Science-Proxy](https://github.com/counterfactual5/Claude-Science-Proxy) · **Tests:** `bash test/run_all.sh`
