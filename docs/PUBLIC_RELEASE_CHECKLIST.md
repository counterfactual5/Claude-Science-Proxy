# 公开仓库前检查清单

在 **push / 改仓库可见性 / 打 Release** 之前逐条确认。目标：**先公开、再完善，但不翻车**。

## 铁律（不做就不要 push）

- [ ] **无真实密钥**：`gitleaks detect --source .`（或等价扫描）0 泄露；README / findings / 测试文档里无真实 API key（测试用 `sk-definitely-invalid` 等已在 `.gitleaks.toml` allowlist）。
- [ ] **无个人机器路径**：无 `/Users/<真实用户名>/`、维护者 HOME、disallowedTools 硬编码路径（`scripts/daily-maintenance.sh` 已泛化为 `$HOME`）。
- [ ] **Git 历史提交者**：`git log --format='%ae' | sort -u` 无 `gmail.com` / `foxmail.com` / `SuperJJ007@users.noreply`；显示名仅 `shanjunjie` / `counterfactual5` / `contributor`。见 [`git-history-email-redaction.md`](git-history-email-redaction.md)（**已本地改写，push 须 `--force-with-lease`**）。
- [ ] **Release tag**：远程无 tag 时可直接打 `v0.1.0-public-preview`；本地旧 `v0.1.0`…`v0.3.6` tag 在改写后**失效**，公开前 `git tag -l | xargs git tag -d` 再重打。
- [ ] **`findings/`**：无遗漏密钥；端口仅为标准测试端口说明；见 [`findings/README.md`](../findings/README.md)。
- [ ] **真机 bundle id**：验收用 `com.csp.acceptance`（`test/tauri.real-machine.conf.json`），无 `com.csswitch.acceptance` 残留。
- [ ] **`.superpowers/`** 未进 git（`.gitignore` 已忽略）；`git ls-files` 无该目录。
- [ ] **无微信图 / 非 GitHub 支持渠道**（已移除 `docs/assets/wechat-group.jpg`）。

## 信息脱敏（建议）

- [ ] **`docs/known-issues.md`** 仅为用户向已知问题（已瘦身）；开发过程痕迹见 CHANGELOG / findings 归档。
- [ ] **`.gitignore`** 含：`.claude/`、`.playwright-cli/`、`.pytest_cache/`、`.ruff_cache/`、`.workbuddy/`、`.superpowers/`。
- [ ] **工作区根目录个人脚本**（如 `audit_summary.py`、`sync_shared.py`）不在本仓库 git 内。

## 第一印象（约 30 分钟）

- [ ] **`README.md` / `README.en.md`**：特性、5 分钟上手、与 CC Switch 差异、early preview 说明。
- [ ] **`LICENSE`**：MIT，含原作者与维护者署名行。
- [ ] **首条公开 tag**：`v0.1.0-public-preview`（与桌面 app 内部 0.3.x 构建号可并存；Release 正文写明 *early preview, APIs may change*）。
- [ ] **GitHub About**（仓库 Settings → General）：
  - **Description**（示例）：
    ```
    让 Claude Science 的推理走自选的第三方 API（DeepSeek / GLM / Kimi / OpenAI 或 Anthropic 兼容端点），含本地虚拟登录与沙箱隔离。macOS 桌面 app（Tauri）。
    ```
  - **Topics**：
    ```
    claude claude-science anthropic deepseek glm kimi minimax openai-compatible llm proxy mcp tauri macos apple-silicon multi-profile
    ```
  - **Social preview**：上传 `docs/assets/social-preview.png`（1280×640）。

## 推送与 Release 命令（需你本机 `gh auth`）

```bash
# 1. 离线门禁
bash test/run_all.sh

# 2. 可选：密钥扫描（安装 gitleaks 后）
gitleaks detect --source .

# 3. 推送当前分支（示例）
git push -u origin HEAD

# 4. 打公开预览 tag（在确认清单全绿后）
git tag -a v0.1.0-public-preview -m "Early public preview — APIs and config may change."
git push origin v0.1.0-public-preview

# 5. 创建 GitHub Release（附 dmg 若已构建）
gh release create v0.1.0-public-preview \
  --title "v0.1.0-public-preview — Early public preview" \
  --notes-file docs/release-notes-public-preview.md \
  --prerelease
```

Release 正文草稿见 [`release-notes-public-preview.md`](release-notes-public-preview.md)。

## 公开后

- [ ] 仓库 Settings → 勾选 **Issues**；可选 Projects / Discussions。
- [ ] 在 README 中确认 Releases 链接可访问。
- [ ] 关注首周 issue：端口占用、provider 兼容性、Gatekeeper / 公证。
