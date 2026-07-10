# 已知问题（用户向）

本文件只记录**当前对用户可见**的已知限制与开放问题。已修复项见 [`CHANGELOG.md`](../CHANGELOG.md)；技术证据与历史开发记录见 [`findings/`](../findings/)（归档，不保证与最新 `main` 一致）。

> **产品**：Claude Science Proxy（CSP）· 用户数据 `~/.csp/CSP.json` · 反馈仅通过 [GitHub Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues)。

## 架构边界（不是 bug）

以下能力依赖 **Anthropic 官方账号 / claude.ai 服务端**，在第三方模型 + 本地虚拟登录模式下**不可用或会 fast-fail**：

- Anthropic 托管远程 MCP（`*.mcp.claude.com`）
- Directory connectors、部分远程插件与云端技能
- 部分会显示 session expired / unavailable 的官方托管功能

详见 README「哪些能力暂时用不了」与 [`architecture-boundaries.md`](architecture-boundaries.md)。

## 开放问题

### 端口占用

切换端口或启动代理时，若测试端口已被占用，应看到**明确指向端口占用**的错误，而不是含糊的「key 无效」。若仍遇到误判，请带 `~/.csp/logs/` 脱敏片段开 issue。

### DeepSeek + 工具调用（DSML 泄漏）

部分 DeepSeek 响应会把 `tool_use` 泄漏成文本（`<｜｜DSML｜｜>` 等），导致 web search / 工具链卡住。根因在**上游模型输出**，非虚拟登录。可选 DSML shim 默认关闭；见 CHANGELOG 与 `proxy/dsml_shim.py`。

### 自定义端点切换校验（待复现）

个别自定义 relay / OpenAI 端点在用户侧 `curl` 可用，但面板 scratch 探测报「网络/上游繁忙」未切换。需要具体 `base_url`、模型与探测日志才能定位；欢迎 issue。

### Science 版本漂移

Claude Science 二进制版本变化可能影响虚拟 OAuth、路由与包源代理行为。CSP 通过 capability catalog 标注已知版本边界；升级 Science 后若异常，请注明 Science 版本与 CSP 版本。

### 沙箱内 `request_host_access`（待查）

个别环境下 Science 自检 `request_host_access` 报「路径不存在」，可能与沙箱 HOME 布局或能力授权有关，待复现。

### 历史会话恢复（#6b）

幂等虚拟登录已阻止**新**对话被孤儿化；若你在旧版本上已有多个 `orgs/` 目录，旧对话可能需手动把 `active-org.json` 指回历史 `org_uuid`（高级操作，见 `oauth_forge` 与沙箱 `~/.csp/sandbox/home/.claude-science/orgs/`）。

## 路线图（未承诺排期）

| 方向 | 说明 |
|------|------|
| 代理移入 Rust | 减少 `python3` 运行时依赖 |
| 启动即常驻 | 打开 app 自动准备 Science（issue 讨论中） |
| Intel / Universal 构建 | 当前主要发布 Apple Silicon |
| Apple 公证 | 当前 ad-hoc 签名，首次需右键打开 |

## 如何报告问题

1. 打开 [Bug 模板](https://github.com/counterfactual5/Claude-Science-Proxy/issues/new/choose)。
2. 附上 CSP 版本、macOS 版本、provider/模型、复现步骤。
3. **勿粘贴** API Key、path secret、OAuth 文件或完整日志；可附脱敏后的 `~/.csp/logs/` 片段。
