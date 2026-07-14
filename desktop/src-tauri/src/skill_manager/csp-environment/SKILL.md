---
name: csp-environment
description: Standing CSP local-environment handbook for Claude Science Proxy. Covers (1) web access — no hosted Web Search; ALWAYS use the local `web-search` MCP (GENERAL lane `web_search`/`csp_web_search`, LITERATURE lane `search_literature`, then `fetch_url`) and NEVER the hosted `web_search` tool; (2) filesystem — never `/mnt/data`, save to workspace cwd then `save_artifacts([...])`; (3) CJK matplotlib fonts; (4) skills/env — don't rely on `host.skills.publish()`, use analysis `python` for science packages; (5) network allowlist via CSP Start + `~/.csp/network-allowlist.json`.
license: Apache-2.0
---

# CSP environment handbook

You are running inside **Claude Science Proxy (CSP)** — a sandboxed Claude Science
on the user's local machine, with internet reaches via a scholarly egress proxy.
Treat this as **standing guidance for every session**. It covers how CSP differs
from Anthropic's hosted Claude environment: web access, files/artifacts,
plotting/CJK, skills/env, and network.

## 1. Web access — the one rule

For ANY web search, online lookup, news/fact check, literature / paper search,
or reading a web page: ALWAYS use the local MCP connector named **`web-search`**.
NEVER call Anthropic's hosted **`web_search`** tool.

The hosted `web_search` tool is **not available** under CSP's virtual login. If
you try it, the planner fails with:

```
Tool 'web_search' not found on agent
```

That wastes a turn. Do not attempt it, and do not tell the user that web search
is unavailable — it IS available through the local connector below.

### Which tool to call (two search lanes)

The `web-search` connector is already connected and enabled. Pick the lane by
**method name** — do not guess from keywords alone:

| Lane | Methods | Use for | `provider="auto"` order |
|------|---------|---------|-------------------------|
| **GENERAL** | `web_search` / `csp_web_search` | news, products, "latest models", facts | keyed Brave/Serper/Tavily (if set) → `duckduckgo_ia` |
| **LITERATURE** | `search_literature` | papers, DOIs, scholarly metadata | wikipedia → Crossref → arXiv → PubMed |
| (fetch) | `fetch_url` | read a URL as clean text after either search | — |

Typical flows:

```python
# GENERAL / product / news
data = host.mcp("web-search", "web_search", query="...", max_results=5)
hits = data["results"]
for r in hits:
    print(r.get("title"), r.get("url"), r.get("snippet"))

# LITERATURE / academic only
papers = host.mcp("web-search", "search_literature", query="...", max_results=5)
for r in papers["results"]:
    print(r.get("title"), r.get("url"))

page = host.mcp("web-search", "fetch_url", url=hits[0]["url"])
print(page["content"])
```

Do **not** write `for r in data:` / `enumerate(data)` on the search return —
that iterates dict **keys** (strings) and raises
`AttributeError: 'str' object has no attribute 'get'`.

**Return shape:** `host.mcp` returns a **dict** with a `results` list
(`hits = data["results"]`), never a bare list of hits. Fetch returns
`{"url", "status", "content"}`.

Use the **correct lane** for the question type. Explicit `provider=` still
works on either tool. HTML `provider="duckduckgo"` is optional and fragile
(anti-bot).

## 2. Files and artifacts

CSP is **not** the hosted Claude environment:

- `/mnt/data` **does not exist here** — neither do other `/mnt/...` paths such
  as `/mnt/user-data`. Never write there; a write will fail or vanish.
- Save all outputs to the **current working directory** — the active Science
  workspace `orgs/<org_uuid>/workspaces/<workspace_uuid>/` — using **relative
  paths** (e.g. `./result.csv`, `figures/plot.png`). Do not hard-code absolute
  paths.
- Use `/tmp` only for **disposable scratch** you don't need to keep.
- To persist a **user-visible** file: write it in the workspace (cwd), then call
  `save_artifacts([...])` with the relative path(s). Writing a file alone does
  not surface it to the user — `save_artifacts` is what does.

## 3. Plotting and CJK (Chinese/Japanese/Korean) text

matplotlib's default font `DejaVu Sans` **cannot render CJK glyphs** — CJK
labels come out as tofu boxes (□□□). Before plotting any non-Latin (CJK) text,
set a CJK-capable font that exists on this macOS host:

```python
import matplotlib.pyplot as plt
plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]
plt.rcParams["axes.unicode_minus"] = False  # keep the minus sign rendering
```

If you use the `figure-style` skill, pass it a CJK font the same way. Latin-only
plots need no change.

## 4. Skills and Python environments

- Don't rely on `host.skills.publish()` / `host.skills.edit()` for durable skill
  installs — they don't take effect under CSP's virtual login. Instead, **draft
  the skill files in the workspace** (a `SKILL.md` folder or `*.skill.md`) and
  let CSP's **Skills tab → "adopt from Science"** pick them up into managed
  storage; from there CSP deploys them into the sandbox.
- Two Python environments exist and differ: the **analysis `python` env** has
  the full scientific stack (numpy/pandas/matplotlib/scipy, etc.) — use it for
  computation and plotting. The **MCP Python env** may **not** have plotting or
  scientific packages, so don't assume they're importable from an MCP tool
  context.

## 5. Network allowlist

CSP pre-grants hosts for the bundled `web-search` providers into Science's
network allowlist on Start (DuckDuckGo Instant Answer, Wikipedia, Brave,
Serper, Tavily). Extra hosts can be added in `~/.csp/network-allowlist.json`.
If a host is still blocked, say so — do **not** retry the hosted Anthropic
`web_search` tool.

## Summary

- GENERAL web / news / products → `web_search` / `csp_web_search` (then `fetch_url`).
- LITERATURE / papers / DOI → `search_literature` (then `fetch_url`).
- Hosted `web_search` tool → never call it; it does not exist in this environment.
- Files → workspace cwd + relative paths; never `/mnt/data`; persist with
  `save_artifacts([...])`; `/tmp` is scratch only.
- CJK plots → set CJK `font.sans-serif` + `axes.unicode_minus = False` first.
- Durable skills → draft in workspace, adopt via CSP Skills tab (not
  `host.skills.publish()`). Scientific packages → analysis `python` env.
- Extra egress hosts → `~/.csp/network-allowlist.json` (then Stop → Start).

## 中文提示

本环境没有托管版 Web Search。联网请用本地 `web-search`：**通用/新闻/产品**用
`web_search` / `csp_web_search`（auto：Brave/Serper/Tavily → duckduckgo_ia）；
**论文/学术**用 `search_literature`（auto：wikipedia → Crossref → arXiv → PubMed）；
读页用 `fetch_url`。不要调用托管 `web_search`。`host.mcp` 搜索返回 **dict**
（含 `results`），正确写法：`data = host.mcp(...); hits = data["results"]`。

本地环境约定（与托管 Claude 不同，请每次遵守）：

- **文件/产物**：本地**不存在** `/mnt/data`（以及任何 `/mnt/...`、`/mnt/user-data`），
  切勿写入。请把输出保存到**当前工作目录**（即活动工作区
  `orgs/<org_uuid>/workspaces/<workspace_uuid>/`）并使用相对路径；`/tmp` 仅用于可丢弃的
  临时文件；要生成用户可见文件，请先写入工作区再调用 `save_artifacts([...])`。
- **绘图/中文字体**：matplotlib 默认字体 `DejaVu Sans` 无法渲染中日韩字符（会显示为
  方框）。绘制含中文标签的图前，请设置
  `plt.rcParams["font.sans-serif"] = ["Arial Unicode MS", "Songti SC", "STHeiti", "DejaVu Sans"]`
  与 `plt.rcParams["axes.unicode_minus"] = False`；使用 `figure-style` 时同样传入中文字体。
- **技能/环境修改**：不要依赖 `host.skills.publish()` 做持久安装；请把技能文件写在工作区，
  再用 CSP「Skills 标签 → 从 Science 采纳」纳入管理。科学计算包在**分析用 `python` 环境**里，
  MCP 的 Python 环境可能没有绘图/科学库。
- **网络授权**：Start 时 CSP 会预授权内置搜索域名；额外域名写在
  `~/.csp/network-allowlist.json`，改完后需 Stop → Start。
