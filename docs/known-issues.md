# 已知问题 / 待修队列

下一版（v0.1.3）计划修的问题。按用户反馈与自测记录，附已知根因与修复方向。修复流程见 [`error-reporting-plan.md`](./error-reporting-plan.md)「更新节奏」。

> **已修但未发布的条目**同时登记在 [`../CHANGELOG.md`](../CHANGELOG.md) 的 `[未发布] v0.1.3`；发布后从本文件删除、只留在 CHANGELOG。当前 #1–#4 均处「已实现·离线验过·待整链回验/未发布」状态。

## 1. 面板窗口仍无法鼠标拖动（v0.1.2 未修好）

> **状态（2026-07-03）：已修（随 #4 落地）。** 面板改成带原生标题栏的正常窗口后，标题栏自带拖动，无边框拖拽区问题自然消失。待本机冒烟确认可拖动（用户在场）。

- **现象**：用户装了 v0.1.2，面板弹出后**仍拖不动**。v0.1.2 已加 `anchor_top_right()`（锚到菜单栏下方）+ 关键区 `data-tauri-drag-region`，但对用户无效。
- **根因**：**未确认**。候选：拖拽区只挂在 `.hd` / `.brand` 上，其子元素（品牌点、文字、按钮）拦截了鼠标，用户抓到子元素就拖不动；或无边框窗口在该 macOS 版本需要额外的 `startDragging` / 窗口 movable 处理。
- **解法**：不再纠结无边框拖拽区，直接走 #4 —— `decorations:true` 的原生标题栏自带拖动。`anchor_top_right()` 与失焦隐藏一并移除。

## 2. 已装 node 仍报「缺少依赖 node」（GUI 启动 PATH 问题）

> **状态（2026-07-03）：治本已完成（v0.1.4）——app 不再需要 node。** 虚拟 OAuth 伪造器已用 Rust 原生密码学重写（`src/oauth_forge.rs`：HKDF-SHA256 + AES-256-GCM，与 `.mjs` 的 v2 格式**字节兼容**，node↔rust 双向对拍单测钉死），一键流程进程内伪造、启动脚本 `--skip-oauth-forge` 跳过 node 步。**不管用户装没装 node 都能用**（A 类 + B 类全解决）。`cargo test` 32 过。**另**保留两层 PATH 兜底（`find_exe`）用于定位 **python3**（代理仍需）。**待**：整链回验 Science 是否接受 Rust 伪造的令牌（在场联调；因与 `.mjs` 字节兼容、而 `.mjs` 已 e2e 验过，预期通过）。详见 [`dependency-analysis.md`](dependency-analysis.md)。

- **现象**：**好几个客户反馈** + **开发者自己第二台电脑复现**：装了 node（如 **22.14.0**），填完 API、点一键越过登录仍报 `缺少依赖 node（写虚拟登录需要）`。**已从「个别测试机」升级为「面广的真实客户问题」，本轮最高优先。**
- **重要分型（见 [`dependency-analysis.md`](dependency-analysis.md)）**：缺 node 分两类 —— **A 类**「装了但 GUI 最小 PATH 看不见」（PATH 兜底能治）与 **B 类**「根本没装」（PATH 兜底无效，只有伪造器移 Rust / bundle 能治）。目标用户多是研究者非开发者，B 类占比未知，**待实机测试用 `command -v node` 分清**——这决定 PATH 兜底够不够、node-ectomy 紧不紧急。
- **根因**：**高可信（经典 macOS GUI PATH 问题）**。`one_click_login` 用 `proc::which("node")` 查 `$PATH`；但从**访达 / .app 启动**的 GUI 进程拿到的是最小 PATH（`/usr/bin:/bin:/usr/sbin:/sbin`），**不含** Homebrew(`/usr/local/bin`、`/opt/homebrew/bin`)、nvm / volta / fnm / asdf 等 node 安装位置 → 查不到。`python3` 不报错是因为 `/usr/bin/python3`（Xcode CLT）在系统 PATH 里，node 一般不在。
- **下一步**：`which()` 兜底扩充常见安装目录（`/usr/local/bin`、`/opt/homebrew/bin`、`/opt/local/bin`、`$HOME/.volta/bin`、`$HOME/.nvm/versions/node/*/bin`、`$HOME/.local/bin` 等）；或用登录 shell（`zsh -lic 'command -v node'`）解析用户真实 PATH；或 `path_helper`。同一处对 `python3` 一并加固。**待测试机确认后再改**。

## 3. 沙箱卡在「Switching organization」（越过登录后进不去）

> **状态（2026-07-03）：已实现 + 隔离测试过，待整链回验。** `csswitch_proxy.py` 加了 `do_CONNECT`：对 `claude.ai / *.claude.com / *.anthropic.com` 的 CONNECT 立即 403，其余隧道透传；`launch-virtual-sandbox.sh` 注入 `http(s)_proxy` 指向代理 hostport + `no_proxy=127.0.0.1`。隔离测试 `test/test_proxy_connect.py`（Anthropic 域名→403、本地 echo→隧道透传、子域拦截）已过，全程不碰 Science。**待**：① 本机黑洞法整链沙箱回归（用户在场）；② 反馈用户真实网络回验（顺带确认其 Science 版本与是否有梯子）。**注**：当前透传对非 Anthropic 域名走**直连**（不链用户原上游代理）；有梯子用户若需经其代理外联，留待整链验证时按需加 `CSSWITCH_UPSTREAM_PROXY` 链式透传。

- **现象**：一键越过登录后浏览器打开，Science 卡在 `Switching organization`。
- **根因**：**已确认**。启动时对 `claude.ai/api/oauth/profile` 的阻塞式请求，在到不了 claude.ai 的网络上超时重试 → UI 卡住。完整证据与复现见 [`../findings/switching-organization-hang.md`](../findings/switching-organization-hang.md)。
- **下一步**：**targeted fast-fail** —— 起沙箱时把 `http(s)_proxy` 指到本地小代理，对 Anthropic 域名的 `CONNECT` 立即拒绝、其余透传（倾向做进 `csswitch_proxy.py` 的 `do_CONNECT`）。修复后本机黑洞法回归，反馈用户真实网络回验。

## 4. 面板应是正常窗口（缩小/关闭 + 居中打开），而非仅菜单栏（体验变更）

> **状态（2026-07-03）：已实现（托盘按用户选择彻底移除）。** `tauri.conf.json`：`decorations:true`（标题栏三键）、`visible:true`（启动即显示）、`center:true`（居中）、`resizable:true`（min 320×520）、去 `alwaysOnTop`/`skipTaskbar`。`lib.rs`：去 `ActivationPolicy::Accessory`（进 Dock、正常生命周期）、**整块移除托盘**、去失焦即隐藏、去 `anchor_top_right`。新增：从标题栏红叉关窗走 `CloseRequested` → 与「退出」一致停代理、清 secret、保留沙箱（否则绕过 `quit_app` 会留孤儿代理）。`cargo test` 23 过。**待本机冒烟**确认居中弹出、三键可用、可拖动。

- **诉求**：现在装完只在菜单栏（工具栏）出一个图标，**不自动弹 GUI**，点图标才在菜单栏下方弹一个无边框小面板。用户希望**像正常软件**：有**最小化 / 关闭**按钮、**不需要全屏**、装完/打开就**直接居中显示在屏幕中间**。
- **关联**：这条一旦做了，**第 1 条「拖不动」自然消失**（正常标题栏自带拖动），最小化/关闭也随标题栏免费获得。属设计取向变更，不是纯 bug。
- **改法**（`desktop/src-tauri`）：
  - 去掉 `ActivationPolicy::Accessory` → 改回默认（进 Dock、正常应用生命周期）。
  - 窗口 `decorations: true`（macOS 标题栏三键：关闭/最小化/缩放）、`visible: true`（启动即显示）、`center: true`（居中）、非 fullscreen（默认可缩放窗口）。
  - 去掉失焦即隐藏（`WindowEvent::Focused(false) → hide`）与 `anchor_top_right`（那是菜单栏定位；正常窗口用居中）。
  - **待确认的设计点**：菜单栏托盘图标是**保留**（混合：Dock 窗口 + 托盘）还是**彻底移除**？用户诉求偏「正常软件」，倾向移除或降为可选，实现时跟用户确认。
- **下一步**：改窗口配置 + 入口逻辑，本机启动冒烟确认居中弹出、三键可用、可拖动。**待实现**。

## 5. 用户反馈「request 有但 console 报错」（信息不全，沟通中）

- **现象（用户口述 + 截图）**：「request 有，但是 console 报错」。console 显示 4 条。**功能是否真受影响未知**（"request 有" 确切含义待澄清）。
- **截图逐条分析（关键：多为浏览器扩展噪声，非本应用 bug）**：
  1. `[Intervention] Slow network is detected ... Fallback font will be used`（×2，来源 `express-utils.js:18`，`chrome-extension://efaidnbmnnnibpcajpcglclefindmkaj/.../Adobe...`）→ 来自 **Adobe Acrobat 浏览器扩展**（该扩展 ID 即 Acrobat），**不是** Science/CSSwitch 代码。但 **"Slow network is detected" 是有意义的信号**：印证用户网络慢/不通，与 [第 3 条 Switching organization](#3-沙箱卡在switching-organization进不去) 根因（到不了 claude.ai 就挂）一致。
  2. `Banner not shown: beforeinstallpromptevent.preventDefault() ...` → PWA 安装横幅提示，**无害信息**，非报错。
  3. 🔴 `Uncaught (in promise) Error: A listener indicated an asynchronous response by returning true, but the message channel closed before a response was received` → **典型的 Chrome 扩展消息通道报错**（扩展 content script 的 `onMessage` 返回 true 但端口先关），几乎总是**扩展噪声，非页面/本应用 bug**。
- **判断**：截图这几条**基本都不是 CSSwitch/Science 自身错误**，是用户浏览器扩展（Adobe Acrobat 等）+ PWA 横幅 + 慢网络干预。唯一值得注意的是「慢网络」（支持第 3 条根因）。
- **要向用户问清楚（沟通中）**：① "request 有" 到底指什么，发消息后模型有没有正常回复、还是卡住/无输出？② 实际卡在哪一步、什么表现？③ 让用户**用无痕窗口 / 禁用扩展**重开，或把 console 按页面来源（`localhost:<port>`）过滤，再看有没有**真正来自 Science 页面**的错误。④ 是否也遇到 Switching organization 卡住（可能同一网络根因）。
- **状态**：信息不全，**记录 + 沟通中**，不改。

---

**当前状态（2026-07-03，v0.1.3 开发中）**：
- **#1 拖不动**：已修（随 #4，原生标题栏自带拖动）。待冒烟。
- **#2 node PATH**：**已确认面广**（多客户 + 开发者第二台机复现）。已加固 `find_exe()` 两层兜底（常见目录 + 登录 shell 解析真实 PATH，覆盖 fnm 等），node/python3 共用，单测过。待客户/测试机回验新版消除报错。**本轮最高优先。**
- **#3 卡组织**：已实现代理 `do_CONNECT` fast-fail + 启动脚本注入 `http(s)_proxy`；隔离测试过（不碰 Science）。待整链黑洞法回归（用户在场）+ 反馈用户真实网络回验。
- **#4 正常窗口**：已实现（去 Accessory + 移除托盘 + decorations/center/visible + 关窗清代理）。`cargo test` 23 过。待冒烟确认三键/居中/拖动。
- **#5 console 报错**：未动（信息不全，多为浏览器扩展噪声，与用户沟通中）。

**离线回归**：`cargo test`（23 过）+ `test/run_all.sh`（Python 22 含新 CONNECT 测试 / node 5 / bash 全过）**ALL GREEN**。版本号 `tauri.conf.json` 与 `Cargo.toml` 已升 0.1.2→0.1.3；**尚未构建/发布**（dmg + gh release 是用户在场时的整链冒烟后步骤）。
