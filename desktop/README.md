# CSSwitch 桌面 app（Tauri 菜单栏）

macOS 菜单栏 app，把 CSSwitch 的日常操作收进一个下拉面板：选 provider、填第三方 key、一键越过登录、起停代理与沙箱、三盏状态灯。

架构上它只是**进程管家**：Rust 后端负责起停子进程、注入环境变量、读写配置、探活；已验证的越权与翻译逻辑仍留在仓库的 Python / Node / shell 里被当作子进程调用（`proxy/csswitch_proxy.py`、`scripts/launch-virtual-sandbox.sh` 等），以保住铁律护栏与已验证行为。

## 组成

```
desktop/
  src/                    前端面板（原生 HTML/CSS/JS，无框架）
    index.html  styles.css  main.js
  src-tauri/
    src/lib.rs            后端入口：托盘 + 10 个 Tauri command（进程管家）
    src/config.rs         ~/.csswitch/config.json 读写（0700/0600、拒符号链接、原子写、掩码）
    src/proc.rs           探活 / which / 一次性 secret / 上游可达性（纯 std）
    tauri.conf.json       窗口配成菜单栏下拉面板（无边框、340×560、失焦即隐）
    Cargo.toml            tauri（tray-icon）+ serde
```

## 前置依赖

- **Rust**（rustup 安装）：<https://www.rust-lang.org/tools/install>
- **Node** 与 npm
- **Xcode Command Line Tools**（`xcode-select --install`）
- 已安装 **Claude Science**（一键越登录会启动其沙箱实例）
- 第三方 key（DeepSeek 或 DashScope），在面板里填即可（存本地 `~/.csswitch/config.json`，0600）

## 开发运行

```bash
cd desktop
npm install
npm run tauri dev
```

菜单栏出现 CSSwitch 图标，点击弹出面板。

后端定位 `proxy/` 与 `scripts/` 的顺序（`asset_root()`）：**① 打包后**优先用 Tauri 资源目录
（`Contents/Resources/`，见下「构建」——`proxy/`、`scripts/` 已被 bundle 进去）；**② 开发态**回退到
从可执行文件位置逐级上溯找仓库根（含 `proxy/csswitch_proxy.py`）。刻意**不看当前工作目录**，
避免据启动目录找到来路不明的脚本；开发时也可用 `CSSWITCH_REPO=/path/to/CSswitch` 显式指定。

## 构建

```bash
cd desktop
npm run tauri build
```

产物是 `.app` / `.dmg`。`proxy/` 与 `scripts/` 已通过 `tauri.conf.json` 的 `bundle.resources`
打进 `Contents/Resources/`，从 Finder 启动的正式 `.app` 也能找到并调用它们（自包含）。
沙箱运行状态落在可写的 `~/.csswitch/sandbox/home`（不写进只读的 `.app` 包内）。

> **签名/分发说明**：本版做 **ad-hoc 签名**（`bundle.macOS.signingIdentity: "-"`，正确封装资源），
> 但**未做 Apple 公证（notarization）**——那需要付费的 Apple Developer ID 证书。因此从 Finder 首次打开会被
> Gatekeeper 拦：请**右键 →「打开」**，或系统设置 → 隐私与安全性 →「仍要打开」。
> 产物目前是 **arm64（Apple Silicon）**；Intel Mac 需要额外的 x86_64 / universal 构建。

## 铁律保障

- 第三方 key 经**环境变量**注入代理子进程，绝不进命令行参数（避免 `ps` 泄露）；回显前端只给末 4 位掩码。
- 沙箱端口/目录护栏由被调脚本负责（对真实端口 8765 与真实目录 `~/.claude-science` 失败关闭）。
- 退 app 默认停代理、保留沙箱运行（见 spec §5.1）。
- 子进程 stderr/stdout 收进 `~/.csswitch/logs/`。

## 测试

后端纯逻辑（config / proc）有 Rust 单元测试：

```bash
cd desktop/src-tauri
cargo test
```

覆盖：0700/0600 权限、符号链接拒绝、原子写、key 掩码、探活、which、secret 生成。
面板与整链联调为手动冒烟（会启动沙箱 Science，须用户明确同意，守铁律）。
