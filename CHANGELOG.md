# 更新日志 / Changelog

本项目遵循 [语义化版本](https://semver.org/) 规范，格式参考 [Keep a Changelog](https://keepachangelog.com/)。

> **约定**：已修问题从 [`docs/known-issues.md`](docs/known-issues.md)「毕业」到这里（发布即定稿）；未修/进行中留在 known-issues；硬 bug 的根因证据链存在 [`findings/`](findings/)。

## [1.0.0] — 2026-07-10

### 新增 Added
- **代码注释英文化**：Rust / Python / JS 生产路径与测试、shell 脚本的 `#` 注释默认英文；用户可见文案仍走 I18N
- **Capability catalog**：新增 `provider.virtual-model-registry`；`provider.relay.force-model-shell` 标注为无 registry 时的单模型回退
- **模板展示名**：`templates.rs` 使用英文规范 `name`；前端 `tplName_*` 与 `wizPresetLabel()` 按 cn/intl 版本本地化

### 变更 Changed
- **死代码清理（前后端对齐）**：面板 `#msg` 仅显示错误；移除 `get_config.pending_notice`、Tauri `status` command、`one_click_login` 成功 `msg_key`、切换成功 `hint_key` 等前端不再消费的字段
- **后端 i18n 统一**：`config`、`oauth_forge`、`scratch`、`capability_catalog`、`sandbox_session` 等模块的用户可见错误与一键启动成功提示改为 `i18n_err` / `msg_key` + `vars`；前端经 `resolveBackendErr` / `resolveHint` 渲染

### 修复 Fixed
- **Science 多模型选择器**：虚拟注册表 `display_name` 经 `science_safe_display_name()` 消毒，避免 Science `V2_` 过滤全小写连字符名（如 `glm-5`/`glm-5-turbo`）导致 8 个配置模型只显示 6 个

### 文档 Documentation
- **`docs/DEVELOPMENT.md`**：补充代码注释约定与 i18n 管线（错误串 / 成功消息 / hint、已迁移范围与有意保留中文的清单）
- **`docs/verified-facts.md` / `provider-capability-matrix.md`**：Science 选择器 operon 规则（`V2_`、8 壳上限）与 `CSP_MODEL_REGISTRY` 主路径
- **开源协作**：补齐 `.github/ISSUE_TEMPLATE` 与 PR 模板；`LICENSE` 追加维护者行；真机文档与 `prepare-legacy`（单槽 DeepSeek）对齐
- **公开前准备**：`docs/known-issues.md` 瘦身为用户向；`scripts/daily-maintenance.sh` 去除维护者 HOME 硬编码；`docs/PUBLIC_RELEASE_CHECKLIST.md`

## [Unreleased]