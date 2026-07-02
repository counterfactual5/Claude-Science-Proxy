# 沙箱卡在「Switching organization」的根因与修复方案

**状态**：根因已确认（2026-07-03，隔离沙箱复现）+ **实机整链复现并验证修复**（2026-07-03，用户在场，独立沙箱端口 8990 / 独立 HOME，全程未碰真实 8765 / `~/.claude-science`）。**关键修正：fast-fail 必须回 401（未登录）而非 403（禁止）**，详见文末「实机验证」。隔离测试 `test/test_proxy_connect.py` 已更新为断言 401。
**来源**：一位下游用户私信反馈「一键越过登录后，Science 卡在 `Switching organization` 进不去」。

## 一句话根因

沙箱 Science 启动时会向 **`claude.ai/api/oauth/profile`** 发一个**阻塞式**请求（组织 / owner / tier 解析）。这个请求的地址在 operon 里**硬编码**，无法用环境变量改。响应**快**（哪怕 401）就秒过；**挂住**（用户网络到不了 claude.ai）就超时重试 → UI 一直卡在 `Switching organization`。

## 证据链（静态分析 + 动态复现）

静态（二进制 `/Applications/Claude Science.app/.../claude-science`）：
- `Switching organization` 是**前端 web UI 的状态文案**（本地 operon 服务器自带，非远程 CDN），不是 CLI 硬错误。
- 组织身份磁盘校验 `Jb()` 只要求 `active-org.json` 的 `org_uuid` 是合法 UUID —— 我们的伪造器满足，所以**卡住不是磁盘校验**，与已跑通的 `auth_status: authenticated:true` 一致。
- 启动路径写死 Anthropic 域名：`api.anthropic.com`(13)、`claude.com`(19)、`platform.claude.com`(4)。
- 令牌刷新包装 `OJ()`（日志里的 `claudeAiFetch`）：`refresh_token` 为空则刷新直接返回 null → 每次启动都会走这条 profile 请求 → 日志固定出现 `claudeAiFetch: ... treating as logged-out`。
- **没有** claude.ai base-url 的环境变量覆盖（只有 `ANTHROPIC_BASE_URL`/`ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN`/`OPERON_BASE_PATH`）；operon 走 `EnvHttpProxyAgent`，**认 `http_proxy`/`https_proxy`（小写优先）**。

动态复现（本机隔离沙箱，proxy 18995 / sandbox 8995 / 黑洞 18999）：

| 场景 | claude.ai 可达性 | 观测 |
|---|---|---|
| 正常（走系统代理 7890） | 能到，返回 401 快 | health 首次即 200；日志 `401 ... treating as logged-out`；启动正常 |
| **黑洞**：`http(s)_proxy` 指向「接受连接但永不响应」的本地端口（模拟被墙） | 挂住 | 日志变 `/api/oauth/profile failed: The operation timed out`；8 条连接卡死在黑洞上；daemon **15s+** 才起、反复重试 → **复现卡住** |
| 快速失败：`http(s)_proxy` 指向立即拒绝的端口 | 秒失败 | 日志 `failed: Unable to connect`；daemon **1s** 起来，正常 |

关键结论：**决定「正常 vs 卡住」的唯一变量，是 claude.ai 请求「快速失败」还是「挂住」**。

为什么开发机不卡：本机有 Clash 系统代理（`https_proxy=127.0.0.1:7890`）能到 claude.ai，profile 秒拿 401。反馈的用户大概率**没有能到 claude.ai 的代理/VPN**（国内直连被墙、SYN 被丢 → 挂住）。

排除项：
- H3（UI 资源走远程 CDN 被墙）→ 排除，UI 由本地 operon 自带。
- H2（用户 Science 版本 ≠ 伪造器对齐的 `20260630` 构建）→ 本机单版本无法直接复现；H1 已完整解释该症状，但仍建议向反馈用户确认版本，排除叠加问题。

## 复现关键点（供回归 / 他人验证）

- operon 认**小写** `https_proxy`（本机 profile 已导出 `https_proxy=127.0.0.1:7890`、大写 `HTTPS_PROXY=127.0.0.1:8001`），只改大写无效。
- `no_proxy=127.0.0.1,localhost,::1` 让推理仍走本地代理，只把外网 claude.ai 引去黑洞，精确模拟「claude.ai 不通、本地代理正常」。
- 黑洞 = 一个 accept 后既不读也不写也不关的 TCP 监听端口。

## 修复方案（已选：targeted fast-fail）

让沙箱到 `claude.ai / platform.claude.com / api.anthropic.com` 的请求**快速失败**而非挂住，与用户网络无关：

> 起沙箱时把 `http(s)_proxy` 指到一个**本地小代理**：对上述 Anthropic 域名的 `CONNECT` **立即拒绝/关闭**（→ operon 秒判 logged-out 秒过）；对其它域名**透传**到用户原本的上游（保留其它外联，如装包）。

- 倾向实现：作为 `proxy/csswitch_proxy.py` 的新模式（加 `do_CONNECT` 处理），少一个进程、逻辑集中。
- 备选（不推荐）：把所有外联都 fast-fail —— 对被墙用户反正外网都连不上，但会误伤有梯子用户的 MCP / 装包。
- 验证：本机可用上面的黑洞法造出「卡住」并证明修复后秒过；但**反馈用户的真实网络需其本人回验**（顺带确认其 Science 版本与是否有梯子）。

## 实机验证与关键修正（2026-07-03，v0.1.4，403 → 401）

**第一版（403）实机不通。** v0.1.4 首次实机整链（一键越过登录 → 起沙箱 8990）后，UI 仍卡「Switching organization / This is taking longer than expected」。抓 operon 自带日志 `~/.csswitch/sandbox/home/.claude-science/logs/server-*.log`：

```
claudeAiFetch: /api/oauth/profile → 403        （反复出现，无 treating as logged-out）
```

代理侧 `do_CONNECT` 确实在对 `claude.ai:443` fast-fail，但回的是 **403**。operon 每 15~30s（页面轮询/reload 时）重试，一直卡住。

**根因修正：401 vs 403 的语义差。** operon 的 `claudeAiFetch`（令牌刷新包装 `OJ()`）对 CONNECT 的状态码分支处理：
- **401 Unauthorized**（「没登录」）→ 日志 `claudeAiFetch: 401 and refresh failed — treating as logged-out` → **秒判 logged-out 放行**。
- **403 Forbidden**（「登录了但没权限」）→ 当成组织/权限问题 → **反复重试** → 卡 Switching organization。

原始根因文里「响应快（哪怕 401）就秒过」说的就是 401；当初实现却选了 403（以为只要「快速失败」即可），是**语义选错**。虚拟登录本就该表现为「未登录」，故应回 **401**。

**改动：** `csswitch_proxy.py` 的 `do_CONNECT` 对 blocked 域名 `self._connect_reply(401)`（原 403）。`test_proxy_connect.py` 断言同步改 401。

**实机验证（独立沙箱，未碰 8765）：** 换 401 后重启代理 + operon，operon 日志变为 `claudeAiFetch: 401 and refresh failed — treating as logged-out`，`/health` 返回 `{"status":"healthy",...,"agents_registered":4}`，不再卡 Switching organization。对照实验干净：唯一变量是 403→401（同一沙箱 data-dir、同一伪造登录、同一代理，仅状态码不同 → 前者卡、后者过）。

> 附带发现（与本 bug 无关）：实机时有一个**旧 release build**（`target/release/bundle/macos/CSSwitch.app`，v0.1.4 前带菜单栏托盘）仍在跑，与 v0.1.4 dev 窗口并存，造成「点状态栏图标唤醒的是托盘而非可挪动窗口」的错觉。v0.1.4 已无任何 tray 代码；杀掉旧进程即恢复。
