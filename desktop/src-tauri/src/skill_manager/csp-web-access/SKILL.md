---
name: csp-web-access
description: How to search the web and read pages inside Claude Science Proxy (CSP). This environment has NO hosted Web Search; for any web search or online lookup ALWAYS use CSP's local `web-search` MCP connector (tools `search_literature` / `csp_web_search` / `fetch_url`) and NEVER call the hosted `web_search` tool.
license: Apache-2.0
---

# CSP Web Access — always use the local `web-search` MCP

You are running inside **Claude Science Proxy (CSP)**, a sandboxed environment
that reaches the internet through a scholarly egress proxy. Treat this as
standing guidance for **every** session — the user should never have to repeat it.

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

## Summary

- Web search / online lookup / read a page → use `web-search`
  (`search_literature` / `csp_web_search`, then `fetch_url`).
- Hosted `web_search` tool → never call it; it does not exist in this environment.

## 中文提示

本环境没有托管版 Web Search。任何联网搜索、在线查询或网页读取，请始终使用本地
`web-search` 连接器的 `search_literature` / `csp_web_search`（搜索）与 `fetch_url`
（读取网页），不要调用托管的 `web_search` 工具——它在 CSP 下不可用，会报
`Tool 'web_search' not found on agent`。沙箱出网被限制为科研数据源白名单，默认使用
Crossref、arXiv、PubMed（及 OpenAlex / Semantic Scholar）等免密钥学术检索源。
