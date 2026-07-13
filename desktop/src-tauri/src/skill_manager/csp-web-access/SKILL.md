---
name: csp-web-access
description: Standing environment conventions for Claude Science Proxy (CSP), the local sandbox you run in. Covers (1) web access — this environment has NO hosted Web Search, so for any web search or online lookup ALWAYS use CSP's local `web-search` MCP connector (tools `search_literature` / `csp_web_search` / `fetch_url`) and NEVER call the hosted `web_search` tool; and (2) local filesystem, plotting/CJK fonts, and env conventions — never write to `/mnt/data`, save outputs to the workspace cwd then `save_artifacts([...])`, set a CJK matplotlib font before plotting non-Latin labels, and don't rely on `host.skills.publish()`.
license: Apache-2.0
---

# CSP environment conventions (web access + local sandbox)

You are running inside **Claude Science Proxy (CSP)**, a sandboxed environment on
the user's local machine that reaches the internet through a scholarly egress
proxy. Treat this as standing guidance for **every** session — the user should
never have to repeat it. This skill covers two things: how to reach the web (the
local `web-search` MCP, below) and the **local environment conventions**
(filesystem, plotting/CJK fonts, network, and skill/env edits) that differ from
Anthropic's hosted Claude environment.

## The one rule

For ANY web search, online lookup, "search the web", news/fact check, or
literature / paper search, and for reading any web page, ALWAYS use the local
MCP connector named **`web-search`**. NEVER call Anthropic's hosted
**`web_search`** tool.

The hosted `web_search` tool is **not available** under CSP's virtual login. If
you try it, the planner fails with:

```
Tool 'web_search' not found on agent
```

That wastes a turn. Do not attempt it, and do not tell the user that web search
is unavailable — it IS available, through the local connector described below.

## Which tool to call

The `web-search` connector is already connected and enabled. It exposes:

- **`search_literature`** — primary search. Use it for any query: papers,
  topics, facts, current events, "search for X". (Alias: **`csp_web_search`** —
  identical behavior; use whichever name your planner surfaces.)
- **`fetch_url`** — fetch a specific URL and read it back as clean, readable
  text. Use this to open a search result, or any link the user gives you.

Typical flow: call `search_literature` (or `csp_web_search`) to find sources,
then `fetch_url` to read the most relevant ones.

## What the sandbox can reach

Egress is limited to an allowlist that favors **scholarly sources**, so searches
default to reliable, no-key scholarly providers:

- Crossref, arXiv, PubMed (and OpenAlex / Semantic Scholar), with automatic
  fallback between them.

General search engines (DuckDuckGo / Wikipedia) and paid providers (Brave /
Serper / Tavily, if API keys are set in CSP's MCP tab) are selectable but are
usually blocked by the sandbox allowlist. Prefer scholarly queries; if a general
page is blocked, say so and suggest a scholarly source — do NOT fall back to the
hosted `web_search` tool.

## Local environment conventions

CSP is **not** the hosted Claude environment. The following conventions apply to
every session; following them avoids failed writes, blank/□□□ plots, wasted
tool calls, and skills that never persist.

### Files and artifacts

- `/mnt/data` **does not exist here** — neither do any other `/mnt/...` paths
  such as `/mnt/user-data`. Never write there; a write will fail or vanish.
- Save all outputs to the **current working directory** — the active Science
  workspace, `orgs/<org_uuid>/workspaces/<workspace_uuid>/` — using **relative
  paths** (e.g. `./result.csv`, `figures/plot.png`). Do not hard-code absolute
  paths.
- Use `/tmp` only for **disposable scratch** you don't need to keep.
- To persist a **user-visible** file: write it in the workspace (cwd), then call
  `save_artifacts([...])` with the relative path(s). Writing a file alone does
  not surface it to the user — the `save_artifacts` call is what does.

### Plotting and CJK (Chinese/Japanese/Korean) text

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

### Web and network

- Don't call the hosted `web_search` tool — use the local `web-search` MCP
  (`search_literature` / `csp_web_search`, then `fetch_url`), as described above.
- Egress is a **scholarly allowlist**: Crossref, arXiv, PubMed, OpenAlex,
  Semantic Scholar, Notion, and PyPI are reliable; general search engines
  (DuckDuckGo/Wikipedia/Google/Bing) and paid search APIs are usually blocked.
  Prefer scholarly sources; if a general page is blocked, say so rather than
  retrying the hosted tool.

### Skills and environment edits

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

## Summary

- Web search / online lookup / read a page → use `web-search`
  (`search_literature` / `csp_web_search`, then `fetch_url`).
- Hosted `web_search` tool → never call it; it does not exist in this environment.
- Files → write to the workspace cwd with relative paths; never `/mnt/data`;
  persist user-visible files with `save_artifacts([...])`; `/tmp` is scratch only.
- CJK plots → set a CJK `font.sans-serif` (Arial Unicode MS / Songti SC / STHeiti)
  and `axes.unicode_minus = False` before plotting.
- Durable skills → draft in the workspace and adopt via CSP's Skills tab, not
  `host.skills.publish()`. Scientific packages live in the analysis `python` env.

## 中文提示

本环境没有托管版 Web Search。任何联网搜索、在线查询或网页读取，请始终使用本地
`web-search` 连接器的 `search_literature` / `csp_web_search`（搜索）与 `fetch_url`
（读取网页），不要调用托管的 `web_search` 工具——它在 CSP 下不可用，会报
`Tool 'web_search' not found on agent`。沙箱出网被限制为科研数据源白名单，默认使用
Crossref、arXiv、PubMed（及 OpenAlex / Semantic Scholar）等免密钥学术检索源。

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
