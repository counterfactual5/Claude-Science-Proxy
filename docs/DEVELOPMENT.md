# Claude Science Proxy（CSP）开发交接 / 现状

面向「零上下文接手」的开发者或独立开发任务。读完这份就能继续把 Tauri app 编译、测试、跑通、发版。

## 公开仓库 / 文档语言

对外入口以 **[`../README.md`](../README.md)**（简体中文）与 **[`../README.en.md`](../README.en.md)**（English）为准。

| 读者 | 建议先读 |
|---|---|
| 终端用户 | README 双语任选 |
| 贡献者 / 外审 | `README.en.md` → [`architecture-boundaries.md`](architecture-boundaries.md) → [`provider-capability-matrix.md`](provider-capability-matrix.md)（后两者为英文边界图） |
| 维护者（日常开发） | 本文、`known-issues.md`、`verified-facts.md`（中文为主） |

[`provider-support.md`](provider-support.md) 为早期调研封存，**非**当前支持矩阵。历史 `findings/` 保留旧称，见 [`../findings/README.md`](../findings/README.md)。

## 铁律（最高优先级，任何改动不得违反）

见 [`../CLAUDE.md`](../CLAUDE.md) 第一节。核心约束：
1. 绝不复制、读取、修改或删除真实 Claude 登录凭证、OAuth token、账号状态或用户数据（含 `.oauth-tokens`/`encryption.key`/`active-org.json`）。
2. 首次沙箱初始化允许只读克隆真实 `~/.claude-science` 下的运行时资源（`bin`/`conda`/`runtime`/`seed-assets`），但不得复制凭证或对话数据。
3. 绝不把真实 OAuth token 复制进沙箱；沙箱只用本地自造的虚拟令牌。
4. 绝不用改过的环境去起真实实例；真实端口 8765，沙箱一律独立 HOME+端口+data-dir。
5. 测试默认不碰 Science；整链冒烟须用户明确同意、且在场（碰 Science 的手测）。

## 命名规范

对外产品名统一写作 **Claude Science Proxy（CSP）**；简称 **CSP**。
- GitHub 仓库目录仍为 `CSSwitch`（磁盘名不改，避免断链）；app `productName`、窗口标题、README、CHANGELOG 一律用 Claude Science Proxy / CSP。
- bundle id：`com.csp.menubar`（Tauri identifier）。
- 用户数据目录：`~/.csp/`（`CSP.json`、`logs/`、`sandbox/home`）。
- **内部 IPC 保持不动**：`CSP_*` 环境变量、`csp_proxy.py` 文件名、`CSP_REPO`（开发态指定仓库根）。
- **对外文案脱敏**：用户可见文案不直说「越过 / 绕过登录」，主按钮用「一键开始」类中性说法；技术内部文档描述机制时可仍用「越过门票」。

### 遗留命名（故意保留，勿当漏改）

对外已统一 **CSP** / `~/.csp` / `csp_proxy.py` / `CSP_*` / `com.csp.menubar`。下列旧名**仅用于迁移、测试或历史文档**，删前须确认无用户数据断链：

| 旧名 | 用途 |
|---|---|
| 仓库目录名 `CSSwitch` | GitHub 路径稳定，避免断链 |
| `com.csswitch.maintenance` | 已 DEPRECATED；已从脚本中彻底清理移除 |
| 测试临时目录 / 日志前缀 `csp-*` | 单测 / 集成测隔离文件名（如 `/tmp/csp-auth-*.log`） |
| `findings/`、`CHANGELOG` 旧版本条目 | 历史证据，见 [`../findings/README.md`](../findings/README.md) |

**不要**把 `scripts/`、`doctor` 的用户可见中文 stdout 或 `proxy/` 运维 `log()` 当「遗留品牌」清掉——那是运维语言选择，见「用户可见文案（i18n）」未迁移清单。

## 代码注释（Code comments）

**生产代码注释默认英文**（Rust / Python / JS）。2026-07 已完成批量迁移；新代码一律英文注释，触到旧文件时若仍有中文注释应顺手对齐。

| 类型 | 约定 |
|---|---|
| 模块头（`//!` / `"""`） | 英文摘要：职责、输入输出、关键不变量 |
| 铁律 / 安全护栏 | 英文一句 + 链到 [`../CLAUDE.md`](../CLAUDE.md) 或 [`verified-facts.md`](verified-facts.md) |
| 补丁痕迹 | 不写「修 P1」「修 #9」等无上下文说法；用 issue/PR 链接或删掉 |
| 用户可见文案 | 只走 i18n 管线（见下节），不写进注释 |
| 逆向 / Science 行为 | 英文注释；见 `proxy/csp_proxy.py` provider 表 |

**有意保留中文的代码区**（非注释，触及时谨慎）：

| 类别 | 位置 / 说明 |
|---|---|
| UI 文案 | `desktop/src/main.js` 的 `I18N.cn` / `intl` |
| 运维输出 | `proxy/`、`scripts/` 中 `log()` / `print()` 类消息（见下节「未迁移」） |
| 测试 | 断言消息、fixture 字符串 |
| Shell 脚本 | `echo` / `grep` 等匹配用户可见输出的模式 |
| 模板展示名 | `templates.rs` 中 `name` 字段（见下节「未迁移」） |
| 文档 | `*.md`（如 `REAL_MACHINE_TEST.md` 等） |

`scripts/` 的 stdout 可为国内运维保留中文；除非后续引入 `CSP_LANG`，否则不强制英文化。

## 用户可见文案（i18n）

2026-07 已完成 Rust 后端用户可见串迁移：错误、成功提示、切换 hint 均走 key + vars，前端在 `desktop/src/main.js` 的 `I18N.cn` / `intl` 落地文案。

### 错误串

| 层 | 约定 |
|---|---|
| Rust | `i18n_err("errKey", json!({ "var": value }))` → 序列化为 `{"i18n":"errKey","vars":{...}}` 字符串（见 `runtime/i18n.rs`） |
| 前端 | `resolveBackendErr(e)` 解析 JSON，调 `T(key, resolveBackendVars(vars))`；非 JSON 则原样显示（兼容旧串） |

`resolveBackendVars` 会展开嵌套：`rollback_key` → `T(rollback_key)` 填入 `rollback`；`vars.error` 递归走 `resolveBackendErr`。

### 成功消息

面板 `#msg` **仅显示错误**（校验失败、操作拒绝等）；成功与进度文案不再占用 UI 空间。

`one_click_login` 成功体返回 `{ url, action }`（`action` 为 `started` / `reopened`）；浏览器打开由后端执行，前端不展示成功 toast。

切换失败时 `set_active_profile` 等仍可通过 `hint_key` + `hint_vars` 给出可本地化原因（经 `resolveHint()` → `setMsg()`）。

### 切换 hint

`set_active_profile` 等**失败/拒绝**响应用 `hint_key` + `hint_vars`（`hint_payload()` 生成）：

```javascript
setMsg(resolveHint(r, "switchRejected"));   // 有 hint_key 则用，否则 fallback
```

成功切换仅返回 `{ committed: true, active_id }`，不再附带成功 hint。

### 已迁移模块

| 模块 | 内容 |
|---|---|
| `config.rs` | 读写/迁移/校验错误 |
| `oauth_forge.rs` | 虚拟登录伪造护栏与 I/O 错误 |
| `scratch.rs` | 候选连接临时代理探测错误 |
| `runtime/capability_catalog.rs` | catalog 解析与 rule 校验错误 |
| `lib.rs` `run_blocking` | `spawn_blocking` 失败 → `errBackgroundTaskFailed` |
| `runtime/sandbox_session.rs` 等 | 沙箱起停/探活错误；`one_click_login` 返回 `url` + `action` |
| `runtime/profile_switch.rs` 等 | 切换事务 hint |

其余 `runtime/*`、`commands/*` 中面向用户的 `Err(...)` 已对齐同一模式。

### 有意未迁移

| 类别 | 说明 |
|---|---|
| `scripts/` stdout | 运维/手测输出，可保留中文 |
| `proxy/` `log()` | 代理运行日志，非 UI |
| 测试 fixture / 断言串 | 不测 i18n 管线本身 |
| `sandbox.log` 运维行 | 如 `[oauth] 虚拟登录已就绪...`，写日志非面板 |
| 模板展示名 | `templates.rs` 的 `name` 仍为 Rust 侧 canonical（英文或历史中文）；**未**走 `i18n_err`。若改模板：Rust 保持英文 canonical `name`，前端用 `tplName_*`（`I18N.cn` / `intl`）做本地化展示，勿把展示文案硬编码进 Rust |

### 新增 key 流程

1. Rust：`i18n_err("errFooBar", json!({ "detail": ... }))` 或响应体 `hint_key`（切换拒绝等）。
2. `main.js`：**同时**在 `I18N.cn` 与 `intl` 加同名键；占位符用 `{var}`，与 `vars` 字段一致。
3. 命名：`err*`（错误）、`switch*` / `hint*`（切换拒绝 hint）；camelCase，与现有键对齐。
4. 前端展示错误处统一 `resolveBackendErr(e)`；切换拒绝走 `resolveHint()`。

## 分层

CSP = 翻译代理（Python）+ 虚拟登录伪造器（**Rust 原生**）+ 隔离脚本（shell）+ **正常窗口 app（Tauri）**。

app 是**进程管家**：Rust 后端起停子进程、注入环境变量、读写配置、探活、跑切换事务；已验证的越权/翻译逻辑仍留在 `proxy/`、`scripts/` 里被当子进程调用（保住护栏与已验证行为，Rust 侧最小）。虚拟 OAuth 伪造已移进 Rust（`src-tauri/src/oauth_forge.rs`，字节级一致、护栏拒真实目录），**app 运行不需要 Node.js**；`scripts/make-virtual-oauth.mjs` 是等价的 Node 独立版，仅命令行单独用时才需要 node。

## 现状（截至 2026-07-10，`feat/multi-model-registry` 工作树，最新发布线 v0.3.6）

**已发布**：当前发布事实以 [`../CHANGELOG.md`](../CHANGELOG.md) 与 GitHub Releases 为准；本文件只记录架构与开发流程，不作为发布状态唯一来源。

**能力面**：
- **多 profile 配置管理**（cc-switch 式）：9 家 provider 模板（DeepSeek / 智谱 GLM / Kimi / MiniMax / 小米 MiMo / OpenRouter / 自定义 Anthropic / 自定义 OpenAI / 自定义 OpenAI Responses）+ 自定义端点；同一家可保存多套（不同 key / 模型）；JSON 存储 `~/.csp/CSP.json`（schema v4，含 v1→v2→v3→v4 一次性迁移），key 明文 0600、只回掩码。旧版 `qwen` / `siliconflow` 模板已下架，存量 profile 的 `template_id` 回退为 relay。
- **provider 分型**：native 仅 **deepseek**（`--provider deepseek` 走固定 Anthropic 端点）；其余经 **relay** / **openai-custom** / **openai-responses** adapter（anthropic 兼容透传或 OpenAI 翻译，带 `base_url`）。`csp_proxy.py` 仍保留 `--provider qwen` CLI 路径供手动调试，面板向导不再提供千问模板。
- **模型选择（#9，v0.3.2）**：relay 家可在编辑页勾选多个启用模型；虚拟注册表分配壳 ID，Science 显示真实模型名。
- **UI**：正常窗口 340×700（`decorations:true`，已去托盘/菜单栏），配置列表 + 多选模型；同一时间仅一条 profile 生效。
- **切换事务**：`set_active`/连接编辑经串行器走「scratch 校验候选 → 起正式代理探活 → 健康才提交 active_id」，失败回滚不停沙箱。

**待办**（详见 [`known-issues.md`](known-issues.md)）：#12 自定义校验 scratch 误判（需复现）；#2/#6 DeepSeek DSML tool_use 泄漏兜底（shim 已成形、默认关）；轨道 2 代理移 Rust（axum）拔 python；Intel/Universal 构建 + 公证。

## 源码结构（`desktop/`）

```
desktop/src/                     前端面板（原生 HTML/CSS/JS，无框架，逻辑内联 main.js）
  index.html  styles.css  main.js
desktop/src-tauri/src/
  lib.rs           tauri command + 切换事务 + launcher（起代理/沙箱、注入 env）
  config.rs        ~/.csp/CSP.json 读写：dir 0700 / file 0600、lstat 拒符号链接、
                   原子写、key 掩码、schema v4、v1→v4 迁移、relay 空 model 回填
  config_legacy.rs v1（旧固定槽）结构，仅迁移用
  templates.rs     provider 模板注册表（单一来源）：adapter/base_url/是否必选模型/内置模型/thinking 策略
  lifecycle.rs     串行器（切换事务加锁）+ generation token
  scratch.rs       候选连接的临时代理探测（Models/Message），起完即杀，绝不碰正式链路
  oauth_forge.rs   Rust 原生虚拟 OAuth 伪造（护栏拒真实目录）
  proc.rs          纯 std：TCP /health 探活（带 path-secret）、which、/dev/urandom secret、上游可达性
  main.rs          入口
  tauri.conf.json  正常窗口 ~340×700；bundle.resources 打包运行所需 proxy/scripts allowlist
```

## 前后端 command 契约

前端只调 Rust command；key 完整值永不进前端，只回显掩码。**坑：Tauri 顶层多词命令参数用 camelCase**（`templateId`/`baseUrl`/`skipVerify` 等），serde 结构体入参（如 `req`）内部字段仍 snake_case。

- **配置读写**：`get_config`（→ `{profiles, templates, active_id, proxy_port, sandbox_port}`；key 掩码）、`create_profile`、`update_profile_connection`（改 base_url/model/key）、`update_profile_metadata`（改名/备注）、`delete_profile`、`set_active_profile`（切换当前，经切换事务）、`open_csp_json`、`set_settings`。
- **模型**：`fetch_models`（起临时代理探 `/v1/models`，回真实 id + 内置合并）。
- **运行控制**：`stop_all`、`one_click_login`（→ `{url, action}`）。

内部诊断：`runtime/diagnostics::runtime_status_snapshot` 供 Rust 单测与运维探针；**无**对应 Tauri command（前端不轮询状态灯）。

## 命令与构建

```bash
# 起代理（默认 DeepSeek）
DEEPSEEK_API_KEY=... python3 proxy/csp_proxy.py --provider deepseek --port 18991
# 切千问（仅 CLI 调试，面板无千问模板）：--provider qwen + DASHSCOPE_API_KEY；relay 家：--provider relay + CSP_RELAY_BASE_URL/KEY/MODEL
# 也支持 --env-file

# 编译 / 跑 / 打包（desktop/，需 node 装 @tauri-apps/cli；产物含 .app / .dmg）
cd desktop && npm install         # 首次装 tauri CLI
npm run tauri dev                 # 开发跑
npm run tauri build               # 打包 → src-tauri/target/release/bundle/dmg/Claude Science Proxy_<ver>_aarch64.dmg
```

## 离线回归

```bash
bash test/run_all.sh                        # S0 分层门：offline / loopback / scripts / rust / frontend
bash test/run_all.sh --require-release-ready # 要求所有层 pass 且无 env-blocked
python3 -m unittest test.test_proxy_units test.test_provider_policy test.test_proxy_packaging -v
cd desktop/src-tauri && cargo test          # Rust 后端单测；配 cargo clippy --all-targets -- -D warnings + cargo fmt --check
node --check desktop/src/main.js            # 前端语法（不加 node 测试依赖，前端逻辑预览手验）
```

## 整链冒烟（碰 Science，须用户同意 + 在场，守沙箱隔离、凭证隔离和端口隔离）

不必驱动 GUI，直接跑 app 编排的同一条链（独立 HOME + 端口 + data-dir，绝不碰 8765）：
```bash
# 1. 起代理（带 path-secret，模拟 app）；relay 家用 CSP_RELAY_* env
SEC=$(python3 -c "import secrets;print(secrets.token_hex(16))")
CSP_RELAY_BASE_URL=https://open.bigmodel.cn/api/anthropic CSP_RELAY_KEY=<key> CSP_RELAY_MODEL=glm-5.2 \
  python3 proxy/csp_proxy.py --provider relay --port 18996 --auth-token "$SEC" &
# 2. 起沙箱，proxy-url 带 secret 前缀（= one_click_login 干的事；护栏用 --dry-run 走干跑）
scripts/launch-virtual-sandbox.sh --port 8990 --proxy-url "http://127.0.0.1:18996/$SEC"
# 3. 验：沙箱 /health；取带票 URL（daemon 重启后浏览器旧 session 失效，重取）
curl -s http://127.0.0.1:8990/health
HOME=<沙箱HOME> '/Applications/Claude Science.app/Contents/Resources/bin/claude-science' url --data-dir <沙箱data-dir>
# 4. 停
scripts/stop-science-sandbox.sh
```
关键观察点：Science 把推理打到 `ANTHROPIC_BASE_URL=.../$SEC` 后能否被 path-secret 认证放行；relay force 时顶部选择器显示真实模型名。**注意 `--dry-run` 是 flag（不是 `DRY_RUN=1` 环境变量），用错会真启动沙箱。**

## 怎么装 Rust（国内网络，实测坑）

官方源 `static.rust-lang.org` 极慢。首选镜像：
```bash
export RUSTUP_DIST_SERVER=https://rsproxy.cn   # 或 https://mirrors.ustc.edu.cn/rust-static
rustup toolchain install stable --profile minimal && rustup default stable
```
crates.io 也走镜像，写 `~/.cargo/config.toml`：
```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

## 发版流程（v0.3.2 实操记录）

```bash
# 0. 功能分支合 main（真机验证过后），跑全绿：run_all --require-release-ready / cargo test / clippy / fmt / node --check
# 1. 版本号 bump（5 处一致）：desktop/package.json + package-lock.json（根 + packages[""]）
#    + src-tauri/Cargo.toml + Cargo.lock（desktop 包）+ tauri.conf.json
# 2. CHANGELOG.md 加条目（已修问题从 known-issues「毕业」到这里）；README 若有能力面变化一并改
# 3. gh/git 前先： export HTTPS_PROXY=http://127.0.0.1:7890 HTTP_PROXY=http://127.0.0.1:7890 ALL_PROXY=http://127.0.0.1:7890
#    （大写默认 8001 是死的，gh 会误报 token invalid）
git push origin main
# 4. 打包 dmg：cd desktop && npm run tauri build
# 5. tag + Release
git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z
gh release create vX.Y.Z --title "..." --notes-file <notes> <dmg 路径>
# 6. 发布前建议 gitleaks 扫（工作树/暂存/历史三处）
```
当前 dmg 未 Apple 公证（无 APPLE_* 凭证），首次启动右键「打开」；仅 Apple Silicon（arm64）。
