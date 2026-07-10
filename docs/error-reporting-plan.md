# 错误报告 / 反馈机制 规划

目标：把报错与反馈收拢到可跟踪、可复用、尊重隐私的公开渠道（GitHub Issues / PR），并支撑「保持更新」的节奏。

## 隐私底线（先定死）

本项目是「绕过登录」性质的工具，**绝不做任何自动遥测 / 后台崩溃上报 / 静默数据回传**。
所有诊断信息都由用户**手动**提交、内容由用户决定。这条是硬约束，任何反馈功能都不得违反。

## 现状（2026-07-10，公开协作）

- **GitHub Issue / PR**：`.github/ISSUE_TEMPLATE/`（`bug_report.yml`、`feature_request.yml`）+ `pull_request_template.md`；README 链到 [Issues](https://github.com/counterfactual5/Claude-Science-Proxy/issues) 与 [PRs](https://github.com/counterfactual5/Claude-Science-Proxy/pulls)。
- **日志**：代理与沙箱子进程 stdout/stderr 落 `~/.csp/logs/`（`proxy.log`、`sandbox.log`，0600）；用户手动附红acted 片段到 issue。
- **后端错误 i18n**：Rust 经 `i18n_err` 返回 key，面板用 `resolveBackendErr` 渲染（见 [`DEVELOPMENT.md`](DEVELOPMENT.md)）。
- **README**：反馈小节 + 隐私声明（已移除微信群等非公开渠道）。

> 用户数据：`~/.csp/CSP.json`；日志：`~/.csp/logs/`。

## 规划中（未落地）

- **面板内入口**：跳转 GitHub Issue 模板 + 访达打开日志目录（`error-reporting-plan` 原稿；当前面板尚无「反馈」按钮）。

1. **日志脱敏导出**：一键「导出诊断包」= doctor 输出 + 日志尾部，且**自动把疑似 key/token 打码**后再落一个 zip，降低用户误贴密钥的风险。
2. **报错更友好**：面板把后端错误归类成人话（端口占用 / key 无效 / 缺 python3 / Science 未安装），每类附「怎么办」一句话与对应链接。（OAuth 伪造已移 Rust，**不再**报「缺 node」。）
3. **崩溃可见（本地）**：app 自身 panic 时写一份本地崩溃日志到 `~/.csp/logs/`，用户可在 bug 反馈里附上（仍不自动上报）。
4. **更新提示常态化**：启动时静默查一次最新 Release（失败无声），有新版在页脚点一下小红点提示，不打扰。
5. **常见问题沉淀**：把重复问题整理进 README FAQ 或 GitHub Discussions。
6. **可选·真·自动更新**：走 `tauri-plugin-updater`，需要一套更新签名密钥 + 托管 `latest.json`；等有 Apple Developer ID / 公证后再上，属较后期。

## 更新节奏

- 语义化版本：修 bug 走 patch（0.1.x），加功能走 minor（0.x）。
- 每次发布：改 → `cargo test` + 离线套件 + 启动冒烟 → gitleaks（历史+工作树）→ 打包 ad-hoc 签名 dmg → `gh release create vX` 附 dmg。
- README 下载链接固定指向 `releases/latest`，无需每版改文档。
