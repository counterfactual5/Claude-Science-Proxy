# Git 历史提交者脱敏

公开仓库时，`git log` 与 GitHub 提交页会暴露**提交作者邮箱与显示名**。本仓库已完成**两轮**本地 `git filter-repo` 改写（2026-07-10）。

## 当前状态（改写后）

**作者显示名**（`git log --format='%an' | sort | uniq -c`）：

| 名称 | 约提交数 | 说明 |
|------|----------|------|
| `shanjunjie` | 183 | 原作者历史（MIT `LICENSE` 署名行保留；GitHub **不再**用可关联的 SuperJJ 邮箱） |
| `counterfactual5` | 113+ | 当前维护者 |
| `contributor` | 1 | 外部贡献者（原 `yiminghua` / foxmail 已脱敏） |

**邮箱**（`git log --format='%ae' | sort -u`）— 应**仅**含：

```
108983446+counterfactual5@users.noreply.github.com
41800000+contributor@users.noreply.github.com
41800001+legacy@users.noreply.github.com
```

不得出现 `gmail.com`、`foxmail.com`，也不得出现 `63803490+SuperJJ007@users.noreply.github.com`（该 noreply 会在 GitHub 上**关联到原作者账号**）。

## 两轮改写做了什么

### 第一轮：真实邮箱 → noreply

| 原邮箱 | 处理 |
|--------|------|
| `shanjunjie666@gmail.com` | → 后由第二轮再改为 `41800001+legacy@...` |
| `cnhym@foxmail.com` | → `41800000+contributor@users.noreply.github.com` |

### 第二轮：显示名 + 可关联 noreply

| 项 | 处理 |
|----|------|
| `SuperJJ` 显示名 | → `shanjunjie` |
| `yiminghua` 显示名 | → `contributor` |
| `63803490+SuperJJ007@users.noreply.github.com` | → `41800001+legacy@users.noreply.github.com`（避免 GitHub 自动链到 SuperJJ007 主页） |

源码树内**不应**粘贴上述真实邮箱；`LICENSE` 中 `Copyright (c) 2026 shanjunjie` **保留**（MIT 要求）。

## 是否值得做？

| 做法 | 优点 | 缺点 |
|------|------|------|
| **不改历史** | 零风险 | 暴露 gmail / foxmail；SuperJJ noreply 链到原作者 GitHub |
| **改写历史**（已完成本地） | 上述风险消除 | **所有 commit SHA 变化**；须 **force push**；本地旧 tag 失效 |

## 推送前验证

```bash
cd /path/to/Claude-Science-Proxy

git log --format='%ae' | sort -u    # 仅 3 个 418.../counterfactual5 noreply
git log --format='%an' | sort -u    # shanjunjie / counterfactual5 / contributor

git push --force-with-lease origin feat/multi-model-registry
```

`main` 若禁 force push，需临时调整分支保护或先只推 feature 分支。

## 本地 Release tag（重要）

历史改写后，**本地** `v0.1.0` … `v0.3.6` 等 tag 多半指向**已失效**的 commit（远程目前 **无 tag**，`gh api .../tags` 为空）。

公开前建议**删掉本地旧 tag**，仅在首次公开 Release 时打新 tag：

```bash
# 删除全部本地旧 tag（确认不需要旧 SHA 后再执行）
git tag -l | xargs git tag -d

# 首次公开预览（在 force push 之后、对应当前 HEAD）
git tag -a v0.1.0-public-preview -m "Early public preview — APIs may change."
git push origin v0.1.0-public-preview
```

若曾把旧 tag 推到远程，需 `git push origin :refs/tags/<name>` 删除后再重打。

## 复现改写（仅供新 clone 备份后使用）

需安装：`brew install git-filter-repo`

```bash
git clone --mirror . ../Claude-Science-Proxy.git.backup

# 第一轮 + 第二轮可合并为一次 callback（见 git 笔记或本文件历史）
git filter-repo --force --commit-callback '...'
git remote add origin https://github.com/counterfactual5/Claude-Science-Proxy.git
```

## 本机执行记录

- **2026-07-10**：两轮 `git filter-repo` 已在 `feat/multi-model-registry` 完成。
- **尚未 force push**（除非你已在重命名后自行推送）。

## 今后提交

- 维护者请继续使用 `108983446+counterfactual5@users.noreply.github.com`（或 GitHub 提供的私有 noreply）。
- Settings → **Block command line pushes that expose email**（防止新提交泄露真实邮箱）。
