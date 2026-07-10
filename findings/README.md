# findings/ 目录说明

本目录存放**带时间戳的逆向、真机验收与外审证据**，多为历史快照，**不保证与当前 `main` 措辞一致**。

阅读时注意：

| 文中常见旧称 | 当前（2026-07 起） |
|---|---|
| CSSwitch | **Claude Science Proxy（CSP）** |
| `~/.csswitch/config.json` | **`~/.csp/CSP.json`**（历史路径，2026-07 已移除自动迁移） |
| `~/.csswitch/sandbox` | **`~/.csp/sandbox/home`** |
| `com.csswitch.*` bundle / maintenance | **`com.csp.menubar`**；旧 maintenance label 安装时已卸载 |
| 菜单栏 / 托盘 app | **正常窗口** Tauri app（已去托盘） |
| `csswitch_proxy.py` | **`csp_proxy.py`** |
| `CSP_RELAY_MODEL` / 单壳 force | **`CSP_MODEL_REGISTRY` 虚拟注册表优先**（最多 8 模型）；无 registry 时仍 force 回退 |

**不要**根据 findings 里的路径或产品名去改运行时代码；当前路径与命名以 [`../docs/DEVELOPMENT.md`](../docs/DEVELOPMENT.md) 为准。

当前架构与 i18n 约定以 `docs/DEVELOPMENT.md`、`CHANGELOG.md` 为准。
