# Git 历史提交邮箱脱敏

公开仓库时，`git log` 与 GitHub 提交页会暴露**提交作者邮箱**。本仓库曾出现：

| 邮箱 | 提交数 | 处理建议 |
|------|--------|----------|
| `shanjunjie666@gmail.com` | 166 | 改写为 `63803490+SuperJJ007@users.noreply.github.com` |
| `cnhym@foxmail.com` | 1 | 改写为 `41800000+contributor@users.noreply.github.com`（外部贡献者，保留作者名 `yiminghua`） |
| `63803490+SuperJJ007@users.noreply.github.com` | 17 | 已是 GitHub noreply，**不改** |
| `108983446+counterfactual5@users.noreply.github.com` | 108+ | 已是 GitHub noreply，**不改** |

源码树内**已无**上述真实邮箱字符串；风险仅在 **git 对象历史**。

## 是否值得做？

| 做法 | 优点 | 缺点 |
|------|------|------|
| **不改历史**（常见） | 零风险、不断协作链 | GitHub 提交页仍可见 2 个真实邮箱 |
| **改写历史**（本指南） | 公开后搜索不到 gmail/foxmail | **所有 commit SHA 变化**；已 clone 的人需重 clone；**必须 force push**；已发 Release tag 需重打 |

**建议**：若仓库**尚未公开**或远程几乎只有你在用 → 值得在首次 push 前改写。若已有他人 fork / 开放 PR → 优先沟通再改。

## 前置条件

```bash
pip3 install git-filter-repo   # 或 brew install git-filter-repo
```

## 执行（本地，会重写整库历史）

在仓库根目录、**工作区干净**且已备份后：

```bash
cd /path/to/CSSwitch

# 可选：备份
git clone --mirror . ../CSSwitch.git.backup

git filter-repo --force --commit-callback '
REDACT = {
    b"shanjunjie666@gmail.com": b"63803490+SuperJJ007@users.noreply.github.com",
    b"cnhym@foxmail.com": b"41800000+contributor@users.noreply.github.com",
}
for attr in ("author_email", "committer_email"):
    old = getattr(commit, attr)
    if old in REDACT:
        setattr(commit, attr, REDACT[old])
'

# 验证：不应再出现真实邮箱
git log --format='%ae' | sort -u
```

`git filter-repo` 默认会**移除 `origin` 远程**（防误推）。恢复并强推：

```bash
git remote add origin https://github.com/counterfactual5/Claude-Science-Proxy.git

# ⚠️ 改写后所有 SHA 变了；协作方需 force pull 或重 clone
git push --force-with-lease origin feat/multi-model-registry
# 若也要更新 main：先确认分支保护允许 force（当前 main 禁强推，需临时关规则或只推 feature 分支）
```

## 与 Release tag 的关系

改写后旧 tag（如 `v0.3.0-beta.2`）指向的 commit **失效**。需要：

```bash
git tag -d v0.3.0-beta.2   # 本地删旧 tag
git push origin :refs/tags/v0.3.0-beta.2   # 远程删旧 tag（若曾推送）
# 在新 HEAD 上重新打 tag 再 push
```

公开预览 tag `v0.1.0-public-preview` 应在**改写完成后**再打。

## 本机是否已执行？

见仓库根目录 `.git/filter-repo/already_ran`（filter-repo 成功后会写入）。若你只看到本文档、未看到改写后的 `git log` 变化，说明**仅文档就绪，历史尚未改写**——由维护者在 push 前按上表决定。

## 不改写时的替代

- GitHub → Settings → 勾选 **Block command line pushes that expose email**（对今后提交有效，不改历史）
- 原作者/贡献者在 GitHub 设置里启用 **Keep my email addresses private**
