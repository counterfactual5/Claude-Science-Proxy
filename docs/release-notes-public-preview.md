# v0.1.0-public-preview — Early public preview

这是 **Claude Science Proxy（CSP）** 的首次公开源码与预览发布。桌面 app 当前构建线约为 **0.3.x**；本 tag 表示「可以试用与贡献」，**不**承诺配置格式与 API 冻结。

## 能做什么

- 在隔离沙箱中启动 Claude Science，使用本地生成的启动门票（不复制真实 Claude 登录信息）。
- 将 Science 推理请求转发到自选的第三方模型：DeepSeek、GLM、Kimi、MiniMax、OpenRouter 或自定义 Anthropic / OpenAI 兼容端点。
- 桌面面板管理多 profile、切换前校验 Key、虚拟模型注册表（单条生效配置多模型壳 ID）。
- 内置 capability catalog 与状态诊断，便于理解 provider 边界。

## 安装（macOS · Apple Silicon）

1. 从本 Release 下载 `Claude Science Proxy_*_aarch64.dmg`（若已附）或按 README 从源码构建。
2. 拖入「应用程序」；首次打开若被 Gatekeeper 拦截，请 **右键 → 打开**。
3. 准备 Claude Science 与第三方 API Key，在面板新建配置并「一键开始」。

## 已知限制

- **Early preview**：问题请走 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues)；勿粘贴密钥。
- 仅 **Apple Silicon** 官方构建；Intel 需自行构建。
- **未 Apple 公证**；企业环境可能额外拦截。
- Anthropic 托管远程 MCP、部分云端能力不可用（架构边界，见 README）。
- 代理仍依赖本机 `python3`（计划移入 Rust）。

## 许可

MIT — 见仓库 [`LICENSE`](../LICENSE)。
