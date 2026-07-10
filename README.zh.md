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
- [与 CC Switch 的差异](#与-cc-switch-的差异)
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

简单理解：CSP 之于 Claude Science，类似 CC Switch 之于 Claude Code，但 Science 多了登录门票和沙箱隔离这层复杂度。

## 与 CC Switch 的差异

两者**彼此独立**，无从属或背书关系。

| | [CC Switch](https://github.com/farion1231/cc-switch) | **CSP（本项目）** |
|---|---|---|
| 目标应用 | Claude **Code** | Claude **Science** |
| 核心问题 | 切换 API / 模型 | 启动门票 + 沙箱 + 模型转发 |
| 虚拟登录 | 通常不需要 | 需要（本地门票，不复制真实凭证） |
| 隔离程度 | 环境变量为主 | 独立 HOME、端口、`~/.csp/sandbox` |
| 多模型 | 多 profile | profile + **虚拟模型注册表**（8 壳 ID） |
| 平台 | 跨平台倾向 | 当前 **macOS Apple Silicon** |

## 可以做什么

- 桌面面板管理多套模型配置，切换前校验 Key。
- 单条生效配置可启用多个模型（虚拟注册表），Science 选择器显示真实模型名。
- 支持 DeepSeek、GLM、Kimi、MiniMax、OpenRouter 及自定义 Anthropic / OpenAI 兼容端点。
- 配置与日志保存在 `~/.csp/`。

## 快速开始

1. 安装 [Claude Science](https://claude.com) 与 `python3`。
2. 从 [Releases](https://github.com/counterfactual5/Claude-Science-Proxy/releases/latest) 下载 `Claude Science Proxy_*.dmg`。
3. 右键打开（首次 Gatekeeper 拦截时）。
4. 新建配置 → 设为当前 → **启动 Claude Science**。

## 支持的模型来源

| 来源 | 接入方式 |
|---|---|
| DeepSeek | 原生 Anthropic 端点 |
| 智谱 GLM / Kimi / MiniMax / 小米 MiMo / OpenRouter | Anthropic 兼容端点 |
| 自定义 Anthropic / OpenAI / OpenAI Responses | 自填 base root |

## 反馈与支持

仅 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues) 与 Pull Requests。勿粘贴 API Key。

## 许可

[MIT](./LICENSE)
