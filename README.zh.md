<p align="center">
  <img src="docs/assets/social-preview.png" alt="Claude Science Proxy" width="760">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License">
  <img src="https://img.shields.io/badge/status-early%20preview-orange.svg" alt="Early preview">
  <img src="https://img.shields.io/badge/platform-macOS%20Apple%20Silicon-1d1d1f.svg" alt="macOS Apple Silicon">
  <img src="https://img.shields.io/badge/built%20with-Tauri%202-C25A34.svg" alt="Tauri 2">
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh.md">简体中文</a>
</p>

# Claude Science Proxy（CSP）

Claude Science Proxy（CSP）是一个给 Claude Science 使用的本地模型切换器。它把 Science 的推理请求接到你自己的第三方模型 API 上，让没有 Claude 订阅的用户也能在 Science 里使用 DeepSeek、Kimi、MiniMax、GLM、OpenRouter、中转站或自定义兼容端点。

它面向的不只是开发者：你只需要准备 Claude Science、一个第三方 API Key，然后在桌面面板里新建配置、点击一条切换为当前生效，再点「启动 Claude Science」。

> **公开预览（early preview）**：源码与 Release 以 `v0.1.0-public-preview` 为首次公开标记；桌面 app 构建线约为 **0.3.x**，配置格式与行为可能变更。问题与建议请走 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues)。

> 当前版本主要支持 macOS Apple Silicon。首次打开未公证的 `.dmg` 应用时，macOS 可能需要你右键选择「打开」。

[下载最新版](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest) · [更新日志](./CHANGELOG.md)

## 目录

- [为什么需要 CSP](#为什么需要-csp)
- [可以做什么](#可以做什么)
- [快速开始](#快速开始)
- [支持的模型来源](#支持的模型来源)
- [虚拟模型注册表（multi-model）](#虚拟模型注册表multi-model)
- [状态诊断与能力 catalog](#状态诊断与能力-catalog)
- [它如何保护你的真实账号](#它如何保护你的真实账号)
- [哪些能力暂时用不了](#哪些能力暂时用不了)
- [参与贡献](#参与贡献)
- [反馈与支持](#反馈与支持)
- [开发与构建](#开发与构建)
- [风险与免责声明](#风险与免责声明)

## 为什么需要 CSP

Claude Science 是 Anthropic 面向科研与分析场景的 AI Agent 应用，可以做文献分析、数据处理、代码执行、图表生成和论文写作等工作。但 Science 默认依赖 Claude 登录和 Anthropic 推理服务。

CSP 做的是本地运行控制：

- 在隔离环境里启动 Claude Science。
- 为 Science 准备一份本地生成的启动门票，不复制你的真实 Claude 登录信息。
- 把 Science 的模型请求转发到你选择的第三方 provider。
- 在需要时把 Anthropic Messages API 和 OpenAI 兼容接口互相转换。

```text
Claude Science sandbox
  -> CSP local proxy
  -> DeepSeek / Kimi / MiniMax / GLM / OpenRouter / custom endpoint
```

## 可以做什么

**给普通用户**

- 用桌面面板管理多套模型配置，不需要手改环境变量。
- 同一家 provider 可以保存多套配置，例如不同 Key、不同模型、不同中转地址。
- 点击一条配置切换为当前生效前会先验证 Key；失败不会悄悄切换到坏配置。
- 点击「启动 Claude Science」会自动启动代理、准备隔离环境、打开 Science。
- Science 顶部模型选择器会显示你选择的真实模型名，而不是笼统的 `claude` 或 `opus`。单条生效配置可启用多个模型（虚拟注册表分配壳 ID），Science 里可切换使用。

**给进阶用户**

- 支持原生 Anthropic 兼容端点、OpenAI Chat Completions 兼容端点、OpenAI Responses 兼容端点。
- 支持自定义 `base_url`、模型名和中转站。
- DeepSeek、Kimi、MiniMax 等原生 Anthropic 端点优先透传，尽量保留工具调用、thinking 和流式响应。
- 支持单条生效配置下启用多个模型（虚拟注册表），在 Science 模型选择器里切换。
- 配置和日志都保存在本机，便于自查和反馈。

## 快速开始

开始之前，请确认你已经安装：

- [Claude Science](https://claude.com)
- macOS Apple Silicon 设备
- 一个可用的第三方模型 API Key
- `python3`（当前代理仍需要；后续计划移入 Rust，减少运行时依赖）

1. 从 [GitHub Releases](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest) 下载最新的 `Claude Science Proxy_*.dmg`。
2. 将 Claude Science Proxy 拖入「应用程序」。
3. 第一次打开如果被 Gatekeeper 拦截，请右键应用并选择「打开」。
4. 点击「+ 新建」，选择 provider，填写 API Key、选择或填写模型（支持多选）和必要的 `base_url`。
5. 点击「创建」保存配置。
6. 在配置列表中点击一条配置切换为当前生效；同一时间只有一条 profile 生效。
7. 验证通过后点击「启动 Claude Science」。
8. CSP 会启动隔离 Science，并在浏览器中打开入口；Science 模型选择器里会显示你配置的真实模型名。

## 虚拟模型注册表（multi-model）

Science 只认 `claude-` 开头的模型 ID。CSP 内置了一个虚拟模型注册表，从固定的 8 个壳 ID（主列表 3 + More models 5）中分配，每个壳映射到一个真实上游模型。部分全小写连字符模型名（如 `glm-5-turbo`）会被 Science 隐藏，代理会自动改写为安全显示名（如 `glm-5.turbo`），出站仍用真实模型 ID。

- **单条生效配置**：点击配置卡片切换为当前生效 profile。可在编辑页勾选多个模型，虚拟注册表为每个模型分配壳 ID，Science 模型选择器显示真实模型名；后台 agent 按角色（主模型 / 快速模型）路由到对应模型。

## 支持的模型来源

| 来源 | 接入方式 | 说明 |
|---|---|---|
| DeepSeek | 原生 Anthropic 端点 | 默认来源，尽量保留 thinking、工具调用和流式能力 |
| 智谱 GLM | Anthropic 兼容端点 | 可编辑官方默认地址，可选择或自填模型 |
| 小米 MiMo | Anthropic 兼容端点 | 支持改到套餐或区域端点 |
| Kimi / Moonshot | Anthropic 兼容端点 | 可编辑官方默认地址，支持 Kimi 系列模型 |
| MiniMax | Anthropic 兼容端点 | 可编辑官方默认地址，支持 MiniMax 系列模型 |
| OpenRouter | Anthropic 兼容聚合入口 | 可选择或自填模型 |
| 自定义 Anthropic | 自填兼容端点 | 适合私有网关、Claude 兼容中转站、本地适配器 |
| 自定义 OpenAI | 自填 OpenAI Chat Completions base root | 代理自动补 `/chat/completions` 与 `/models` |
| 自定义 OpenAI Responses | 自填 OpenAI Responses base root | 代理自动补 `/responses` 与 `/models` |

> 如果你的地址是 `/anthropic` 端点，请选择「自定义 Anthropic」。如果选择「自定义 OpenAI」，请填写 OpenAI 兼容的 base root，例如 `https://example.com/v1`，不要填 Anthropic 端点。

OpenAI 兼容类 provider（含历史上的内置模板）请通过「自定义 OpenAI」/「自定义 OpenAI Responses」配置，面板不再单独提供 Qwen 预设。

## 状态诊断与能力 catalog

CSP 内置了只读的 capability catalog，用来把 provider、工具调用、MCP/skill、Science 版本和 transport 的已知兼容性边界显式化。诊断会返回当前 profile 命中的 catalog 规则和固定边界规则，便于定位「当前配置为什么这样处理」以及「哪些能力只能诊断或降级」。

这个 catalog 是诊断与可观测性入口，不是 live provider、真实 Claude 账号态、Science GUI E2E、DMG 签名/公证或官方托管能力的验证结果。

## 它如何保护你的真实账号

CSP 的核心边界是：第三方模型模式只把凭证、数据目录和网络代理放在隔离环境里，不接管你的真实 Claude 账号。

- 不复制、读取或修改真实 Claude 登录凭证、OAuth token、账号状态或用户数据。
- 首次初始化沙箱时，可能会从真实 `~/.claude-science` 只读克隆 Science 运行时资源；这些不是账号凭证或对话数据。
- 隔离 Science 使用独立 HOME、独立端口和独立数据目录。
- 第三方 API Key 保存在 `~/.csp/CSP.json`，文件权限为 `0600`。
- 代理只监听 `127.0.0.1`，并使用 path secret 验证请求。

## 哪些能力暂时用不了

- Anthropic 托管的远程 MCP 服务不可用（`*.mcp.claude.com`）。
- 依赖真实 Claude 账号授权的目录连接器、远程插件、云端能力可能会显示 session expired 或 unavailable。
- 当前 macOS 包尚未 Apple 公证；代理仍依赖 `python3`。

已知问题见 [docs/known-issues.md](./docs/known-issues.md)。

## 参与贡献

欢迎 issue 与 PR。开始前请读 [`CLAUDE.md`](./CLAUDE.md) 与 [`docs/DEVELOPMENT.md`](./docs/DEVELOPMENT.md)，并跑 `bash test/run_all.sh`。

## 反馈与支持

仅 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues) 与 Pull Requests。勿粘贴 API Key。

## 开发与构建

```bash
cd desktop && npm install && npm run tauri dev
bash test/run_all.sh
```

## 风险与免责声明

- 本项目仅供个人学习与研究使用，使用风险由用户自行承担。
- CSP 与 Anthropic 不存在从属、合作或背书关系。
- 推理请求会发送到你自行配置并付费的第三方模型服务。
- 软件按「现状」提供，不作任何形式的担保。

## 许可

[MIT](./LICENSE)
