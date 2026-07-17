#!/usr/bin/env python3
"""csp-web-search: a free, no-API-key web search + page fetch MCP server.

Bundled into Claude Science Proxy (CSP) and deployed as the built-in
``web-search`` stdio connector so Claude Science can do real web search/fetch
even though Anthropic's hosted ``web_search`` tool is unavailable under CSP's
virtual login.

**How models must call this:** bare ``web_search`` / ``web_fetch`` are Anthropic
*native server tools*, not local MCP entry points. Under CSP virtual login they
are stripped from OPERON's toolset — top-level calls fail with
``Tool 'web_search' not found on agent 'OPERON'``. Local MCP tools are **not**
top-level model tools; call them only via ``repl`` as::

    host.mcp("web-search", "csp_web_search", query="...", max_results=N)
    host.mcp("web-search", "search_literature", query="...", max_results=N)
    host.mcp("web-search", "fetch_url", url="...")

Names in ``tools/list`` (``csp_web_search``, ``search_literature``,
``fetch_url``, ``web_fetch``) are **method names for ``host.mcp`` only**.
``web_search`` remains a **dispatch-only** alias of ``csp_web_search`` (not
listed) so old sessions/skills do not hard-fail. Re-advertising names cannot
intercept bare native calls; the CSP proxy injects standing system guidance.

Design notes
------------
* **Transport.** Newline-delimited JSON-RPC 2.0 over stdio (the MCP stdio
  transport). Implemented by hand with the standard library only — no
  dependency on the ``mcp`` SDK — so it runs on any Python 3.8+ interpreter the
  sandbox happens to ship, and never needs a ``pip install``.
* **Egress.** Claude Science runs every MCP child in an OS sandbox that denies
  all outbound loopback egress except to its own "operon" proxy, injected as
  ``HTTPS_PROXY``. Standard Python HTTP clients (``requests`` and, as a
  fallback, ``urllib``) honour that env var and issue a proper ``CONNECT``
  tunnel for HTTPS — unlike the Node/axios stacks that need CSP's shim — so
  they reach the internet through operon without any extra work.
* **Multi-provider with automatic fallback (OpenClaw-style).** One server hosts
  several search providers behind a single search implementation, exposed under
  several ``host.mcp`` method aliases. ``csp_web_search`` is the **canonical
  GENERAL** method (``web_search`` is an unlisted dispatch alias of the same
  handler); ``search_literature`` is the LITERATURE lane. ``provider=auto``
  (the default) tries key-based providers
  first *iff* their API key is present in the environment, then falls back to
  the free/no-key providers. Any single provider failure is captured as a
  warning and the next provider is tried; the call only fails if *every*
  candidate fails. Empty Instant Answer is **not** a missing-key error.

Providers
---------
Free / no key, and reachable through the sandbox egress allowlist (these are
the ``auto`` defaults — verified via operon: arXiv/Crossref/PubMed/OpenAlex/
Semantic Scholar return 200/429, i.e. the CONNECT tunnel is permitted):
  * ``crossref``        — Crossref works API (broad scholarly metadata).
  * ``arxiv``           — arXiv Atom API (preprints).
  * ``pubmed``          — NCBI E-utilities (biomedical literature).
  * ``openalex``        — OpenAlex works (all fields; small metered budget).
  * ``semanticscholar`` — Semantic Scholar graph (rate-limited without a key).
Free general-web (CSP pre-grants these hosts into Science network allowlist on
Start; no API key required):
  * ``duckduckgo_ia``   — DuckDuckGo Instant Answer API (entity/curated abstracts;
    often empty for news/"latest …" queries — that is normal, not a missing key).
  * ``duckduckgo_lite`` — DuckDuckGo Lite HTML (broader web results; preferred
    free fallback when Instant Answer is empty; end of GENERAL auto).
  * ``duckduckgo``      — DuckDuckGo full HTML endpoint (scraped, often anti-bot;
    explicit ``provider=`` only — not in auto).
  * ``wikipedia``       — MediaWiki search API (LITERATURE-lane auto home; also
    selectable via explicit ``provider=`` on either tool — not in GENERAL auto).
Key based (optional quality upgrade via MCP env — configure in CSP's MCP tab;
CSP also pre-grants ``api.search.brave.com`` / ``google.serper.dev`` /
``api.tavily.com`` so keys work once set — extra hosts:
``~/.csp/network-allowlist.json``). Keys are NOT required for general search:
  * ``brave``  — Brave Search API    (env ``BRAVE_SEARCH_API_KEY``).
  * ``serper`` — Serper.dev (Google) (env ``SERPER_API_KEY``).
  * ``tavily`` — Tavily Search API   (env ``TAVILY_API_KEY``).

Secrets are read only from the process environment (CSP injects them from its
0600 inventory); nothing is ever hardcoded or logged.
"""

import http.cookiejar
import json
import os
import re
import sys
import time
import html as _html
import urllib.error
import urllib.parse
import urllib.request

SERVER_NAME = "web-search"
SERVER_VERSION = "1.9.0"
DEFAULT_PROTOCOL = "2025-06-18"
USER_AGENT = "csp-web-search/1.0 (+https://github.com/; Claude Science Proxy built-in)"
# Browser-like UA for DuckDuckGo HTML endpoints (Lite / full HTML). CSP UA
# alone is often classified as botnet (anomaly.js?cc=botnet).
_DDG_BROWSER_UA = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15"
)
HTTP_TIMEOUT = 15
# Empty Instant Answer / exhausted free providers — do NOT tell the user they
# "must" configure Brave/Serper/Tavily. Keys are optional quality upgrades.
_EMPTY_GENERAL_HINT = (
    "No free general-web hits for this query (DuckDuckGo Instant Answer may be "
    "empty for news/\"latest …\" questions; Lite may briefly hit anti-bot — "
    "retry or rephrase). That does NOT mean an API key is missing — keys are "
    "NOT required. GENERAL does NOT fall back to Wikipedia (wikipedia lives on "
    "search_literature only). Do NOT tell the user GENERAL \"fell back to "
    "Wikipedia\" or that they must configure Brave/Serper/Tavily. Optional "
    "BRAVE_SEARCH_API_KEY / SERPER_API_KEY / TAVILY_API_KEY in the MCP tab can "
    "improve reliability — paid APIs are optional upgrades, not a prerequisite. "
    "For encyclopedic / academic topics use search_literature."
)
_EMPTY_LITERATURE_HINT = (
    "No literature hits from wikipedia/Crossref/arXiv/PubMed for this query. "
    "Try a paper title, DOI, author+year, or a narrower scholarly phrase. "
    "Paid Brave/Serper/Tavily keys are unrelated to this lane."
)


def log(msg):
    """Diagnostics go to stderr; stdout is reserved for JSON-RPC frames."""
    try:
        sys.stderr.write("[csp-web-search] " + str(msg) + "\n")
        sys.stderr.flush()
    except Exception:
        pass


# --------------------------------------------------------------------------- #
# HTTP helper — prefers requests (robust proxy CONNECT + auth), falls back to  #
# urllib. Both honour HTTPS_PROXY from the environment.                        #
# --------------------------------------------------------------------------- #
try:
    import requests as _requests  # noqa: F401

    _HAVE_REQUESTS = True
except Exception:
    _HAVE_REQUESTS = False


class HttpError(Exception):
    pass


def http_request(method, url, headers=None, params=None, data=None, timeout=HTTP_TIMEOUT):
    """Perform an HTTP(S) request and return (status, text).

    ``params`` is a dict appended as a query string; ``data`` may be a dict
    (form-encoded), bytes, or a str. Raises ``HttpError`` on transport failure.
    """
    headers = dict(headers or {})
    headers.setdefault("User-Agent", USER_AGENT)
    if params:
        sep = "&" if ("?" in url) else "?"
        url = url + sep + urllib.parse.urlencode(params)

    body = data
    if isinstance(data, dict):
        body = urllib.parse.urlencode(data).encode("utf-8")
        headers.setdefault("Content-Type", "application/x-www-form-urlencoded")
    elif isinstance(data, str):
        body = data.encode("utf-8")

    if _HAVE_REQUESTS:
        try:
            resp = _requests.request(
                method, url, headers=headers, data=body, timeout=timeout
            )
            return resp.status_code, resp.text
        except Exception as exc:  # network/proxy/TLS error
            raise HttpError(str(exc))

    # urllib fallback — trust_env proxies are picked up automatically.
    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as fp:
            charset = fp.headers.get_content_charset() or "utf-8"
            return fp.getcode(), fp.read().decode(charset, "replace")
    except urllib.error.HTTPError as exc:
        try:
            text = exc.read().decode("utf-8", "replace")
        except Exception:
            text = ""
        return exc.code, text
    except Exception as exc:
        raise HttpError(str(exc))


def http_json(method, url, headers=None, params=None, data=None, timeout=HTTP_TIMEOUT):
    status, text = http_request(method, url, headers=headers, params=params,
                                data=data, timeout=timeout)
    if status >= 400:
        raise HttpError("HTTP %d: %s" % (status, (text or "")[:200]))
    try:
        return json.loads(text)
    except Exception as exc:
        raise HttpError("invalid JSON response: %s" % exc)


# --------------------------------------------------------------------------- #
# HTML helpers                                                                 #
# --------------------------------------------------------------------------- #
_TAG_RE = re.compile(r"<[^>]+>")
_WS_RE = re.compile(r"[ \t\r\f\v]+")
_MULTINL_RE = re.compile(r"\n{3,}")
_SCRIPT_STYLE_RE = re.compile(
    r"<(script|style|noscript|template)[^>]*>.*?</\1>", re.I | re.S
)


def strip_tags(fragment):
    return _html.unescape(_TAG_RE.sub("", fragment or "")).strip()


def html_to_text(document, max_chars):
    doc = _SCRIPT_STYLE_RE.sub(" ", document or "")
    doc = re.sub(r"<br\s*/?>", "\n", doc, flags=re.I)
    doc = re.sub(r"</(p|div|li|h[1-6]|tr|table|section|article)>", "\n", doc, flags=re.I)
    text = _html.unescape(_TAG_RE.sub("", doc))
    text = _WS_RE.sub(" ", text)
    text = _MULTINL_RE.sub("\n\n", text)
    text = "\n".join(line.strip() for line in text.split("\n"))
    text = text.strip()
    if max_chars and len(text) > max_chars:
        text = text[:max_chars].rstrip() + "\n…[truncated]"
    return text


def _ddg_unwrap(href):
    """DuckDuckGo wraps result links in /l/?uddg=<encoded-real-url>."""
    if not href:
        return href
    if href.startswith("//"):
        href = "https:" + href
    try:
        parsed = urllib.parse.urlparse(href)
        if parsed.path.endswith("/l/") or "uddg=" in (parsed.query or ""):
            qs = urllib.parse.parse_qs(parsed.query)
            if "uddg" in qs:
                return qs["uddg"][0]
    except Exception:
        pass
    return href


# --------------------------------------------------------------------------- #
# Providers. Each returns a list of {title, url, snippet, source, published?}. #
# --------------------------------------------------------------------------- #
def provider_duckduckgo(query, max_results, env):
    # Full HTML endpoint: titles + snippets; scraped, rate-limited, and often
    # blocked by anti-bot challenges. Prefer duckduckgo_lite for auto.
    status, text = http_request(
        "POST", "https://html.duckduckgo.com/html/",
        data={"q": query, "kl": "us-en"},
    )
    if status >= 400:
        raise HttpError("duckduckgo HTTP %d" % status)
    if re.search(r"anomaly|challenge|captcha", text, re.I):
        raise HttpError("duckduckgo anti-bot challenge (use duckduckgo_lite)")
    results = []
    # Each result: an <a class="result__a" href="...">title</a> optionally
    # followed by an <a class="result__snippet">snippet</a>.
    blocks = re.split(r'class="result__a"', text)
    for block in blocks[1:]:
        m = re.search(r'href="([^"]+)"', block)
        if not m:
            continue
        url = _ddg_unwrap(m.group(1))
        tm = re.search(r'>(.*?)</a>', block, re.S)
        title = strip_tags(tm.group(1)) if tm else url
        sm = re.search(r'class="result__snippet"[^>]*>(.*?)</a>', block, re.S)
        snippet = strip_tags(sm.group(1)) if sm else ""
        if url.startswith("http"):
            results.append({"title": title or url, "url": url,
                            "snippet": snippet, "source": "duckduckgo"})
        if len(results) >= max_results:
            break
    return results


def _ddg_lite_is_challenge(status, text):
    """True when DuckDuckGo Lite served an anti-bot / anomaly interstitial.

    Prefer ``anomaly.js`` (botnet gate) over a bare ``challenge`` word match —
    normal result pages can mention "challenge" in titles/snippets. HTTP 202
    alone is *not* decisive: Instant Answer also returns 202 with valid JSON.
    """
    if status is not None and status >= 400:
        return False  # caller treats as hard HTTP error
    body = text or ""
    if re.search(r"anomaly\.js|cc=botnet|/anomaly", body, re.I):
        return True
    if re.search(r"\bcaptcha\b", body, re.I) and "result-link" not in body:
        return True
    # 202 + no parseable results is usually the gated interstitial.
    if status == 202 and "result-link" not in body:
        return True
    return False


def _ddg_lite_browser_headers(referer="https://lite.duckduckgo.com/"):
    return {
        "User-Agent": _DDG_BROWSER_UA,
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
        "Referer": referer,
    }


def _ddg_lite_parse(text, max_results):
    """Parse DuckDuckGo Lite HTML into result dicts."""
    results = []
    # Prefer explicit result-link anchors (skips Zero-click "More at Wikipedia").
    for m in re.finditer(
        r'<a[^>]*class=["\']result-link["\'][^>]*href=["\'](https?://[^"\']+)["\'][^>]*>'
        r'(.*?)</a>'
        r'|'
        r'<a[^>]*href=["\'](https?://[^"\']+)["\'][^>]*class=["\']result-link["\'][^>]*>'
        r'(.*?)</a>',
        text or "", re.S | re.I,
    ):
        url = _ddg_unwrap(m.group(1) or m.group(3))
        title_html = m.group(2) if m.group(1) else m.group(4)
        if not url or not url.startswith("http") or "duckduckgo.com" in url:
            continue
        title = strip_tags(title_html) or url
        tail = (text or "")[m.end(): m.end() + 1200]
        sm = re.search(
            r"class=['\"]result-snippet['\"][^>]*>(.*?)</td>",
            tail, re.S | re.I,
        )
        snippet = strip_tags(sm.group(1)) if sm else ""
        results.append({
            "title": title,
            "url": url,
            "snippet": snippet,
            "source": "duckduckgo_lite",
        })
        if len(results) >= max_results:
            break
    if results:
        return results
    # Fallback: any nofollow external link (older markup / partial pages).
    for m in re.finditer(
        r'<a[^>]*rel=["\']nofollow["\'][^>]*href=["\'](https?://[^"\']+)["\'][^>]*>'
        r'(.*?)</a>',
        text or "", re.S | re.I,
    ):
        url = _ddg_unwrap(m.group(1))
        if not url.startswith("http") or "duckduckgo.com" in url:
            continue
        title = strip_tags(m.group(2)) or url
        results.append({
            "title": title,
            "url": url,
            "snippet": "",
            "source": "duckduckgo_lite",
        })
        if len(results) >= max_results:
            break
    return results


def _ddg_lite_session_get_post(query):
    """Warm homepage cookies then POST search (requests.Session when available)."""
    headers = _ddg_lite_browser_headers()
    if _HAVE_REQUESTS:
        sess = _requests.Session()
        try:
            sess.get(
                "https://lite.duckduckgo.com/lite/",
                headers=headers, timeout=HTTP_TIMEOUT,
            )
        except Exception:
            pass
        try:
            resp = sess.post(
                "https://lite.duckduckgo.com/lite/",
                headers=dict(headers, **{
                    "Content-Type": "application/x-www-form-urlencoded",
                    "Origin": "https://lite.duckduckgo.com",
                    "Referer": "https://lite.duckduckgo.com/lite/",
                }),
                data=urllib.parse.urlencode({"q": query}).encode("utf-8"),
                timeout=HTTP_TIMEOUT,
            )
            return resp.status_code, resp.text
        except Exception as exc:
            raise HttpError(str(exc))
    # urllib + CookieJar fallback
    opener = urllib.request.build_opener(
        urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar())
    )
    try:
        req = urllib.request.Request(
            "https://lite.duckduckgo.com/lite/", headers=headers,
        )
        with opener.open(req, timeout=HTTP_TIMEOUT) as fp:
            fp.read()
    except Exception:
        pass
    body = urllib.parse.urlencode({"q": query}).encode("utf-8")
    post_headers = dict(headers)
    post_headers["Content-Type"] = "application/x-www-form-urlencoded"
    post_headers["Origin"] = "https://lite.duckduckgo.com"
    post_headers["Referer"] = "https://lite.duckduckgo.com/lite/"
    req = urllib.request.Request(
        "https://lite.duckduckgo.com/lite/", data=body, headers=post_headers,
        method="POST",
    )
    try:
        with opener.open(req, timeout=HTTP_TIMEOUT) as fp:
            charset = fp.headers.get_content_charset() or "utf-8"
            return fp.getcode(), fp.read().decode(charset, "replace")
    except urllib.error.HTTPError as exc:
        try:
            text = exc.read().decode("utf-8", "replace")
        except Exception:
            text = ""
        return exc.code, text
    except Exception as exc:
        raise HttpError(str(exc))


def provider_duckduckgo_lite(query, max_results, env):
    # DuckDuckGo Lite: no key, broader web results than Instant Answer.
    # Prefer this when IA returns empty for news / "latest …" queries.
    # Intermittently serves anomaly.js botnet interstitial — warm cookies and
    # retry once before raising (auto then ends GENERAL without Wikipedia).
    last_err = None
    for attempt in range(2):
        try:
            status, text = _ddg_lite_session_get_post(query)
        except HttpError as exc:
            last_err = exc
            if attempt == 0:
                time.sleep(1.2)
                continue
            raise
        if status >= 400:
            last_err = HttpError("duckduckgo_lite HTTP %d" % status)
            if attempt == 0:
                time.sleep(1.2)
                continue
            raise last_err
        if _ddg_lite_is_challenge(status, text):
            last_err = HttpError(
                "duckduckgo_lite anti-bot challenge "
                "(temporary; not a missing API key — GENERAL does not use Wikipedia)"
            )
            if attempt == 0:
                time.sleep(1.5)
                continue
            # Last attempt: try a plain GET with q= before giving up.
            status2, text2 = http_request(
                "GET", "https://lite.duckduckgo.com/lite/",
                headers=_ddg_lite_browser_headers(
                    "https://lite.duckduckgo.com/lite/"
                ),
                params={"q": query},
            )
            if status2 < 400 and not _ddg_lite_is_challenge(status2, text2):
                parsed = _ddg_lite_parse(text2, max_results)
                if parsed:
                    return parsed
            raise last_err
        parsed = _ddg_lite_parse(text, max_results)
        if parsed:
            return parsed
        # Not a challenge but no links — one GET retry then empty.
        if attempt == 0:
            time.sleep(0.8)
            continue
        return []
    if last_err:
        raise last_err
    return []


def provider_duckduckgo_ia(query, max_results, env):
    # Instant Answer API: no key. Returns curated abstracts/related topics for
    # entity-like queries; empty JSON for many news/"latest …" queries is
    # expected (not a network failure and not a missing API key) — GENERAL auto
    # falls through to duckduckgo_lite only (wikipedia lives on LITERATURE).
    data = http_json(
        "GET", "https://api.duckduckgo.com/",
        params={"q": query, "format": "json", "no_html": 1, "no_redirect": 1,
                "t": "csp-web-search"},
    )
    out = []
    abstract = data.get("AbstractText") or data.get("Abstract")
    answer = data.get("Answer")
    if abstract or answer:
        out.append({
            "title": data.get("Heading") or query,
            "url": data.get("AbstractURL", "") or data.get("AnswerURL", ""),
            "snippet": strip_tags(abstract or answer),
            "source": "duckduckgo_ia",
        })

    def _walk(topics):
        for t in topics:
            if len(out) >= max_results:
                return
            if isinstance(t, dict) and t.get("Topics"):
                _walk(t["Topics"])
            elif isinstance(t, dict) and t.get("FirstURL"):
                text = strip_tags(t.get("Text", ""))
                out.append({
                    "title": text.split(" - ")[0] if text else t["FirstURL"],
                    "url": t.get("FirstURL", ""),
                    "snippet": text,
                    "source": "duckduckgo_ia",
                })

    _walk(data.get("RelatedTopics", []) or [])
    # Also surface top-level "Results" (instant answer external links).
    for hit in (data.get("Results") or []):
        if len(out) >= max_results:
            break
        if not isinstance(hit, dict) or not hit.get("FirstURL"):
            continue
        text = strip_tags(hit.get("Text", ""))
        out.append({
            "title": text.split(" - ")[0] if text else hit["FirstURL"],
            "url": hit.get("FirstURL", ""),
            "snippet": text,
            "source": "duckduckgo_ia",
        })
    return out[:max_results]


def provider_wikipedia(query, max_results, env):
    data = http_json(
        "GET", "https://en.wikipedia.org/w/api.php",
        params={
            "action": "query", "list": "search", "srsearch": query,
            "srlimit": max_results, "format": "json", "srprop": "snippet",
        },
    )
    out = []
    for hit in (data.get("query", {}).get("search", []) or [])[:max_results]:
        title = hit.get("title", "")
        out.append({
            "title": title,
            "url": "https://en.wikipedia.org/wiki/" + urllib.parse.quote(title.replace(" ", "_")),
            "snippet": strip_tags(hit.get("snippet", "")),
            "source": "wikipedia",
        })
    return out


def provider_arxiv(query, max_results, env):
    status, text = http_request(
        "GET", "https://export.arxiv.org/api/query",
        params={"search_query": "all:" + query, "start": 0,
                "max_results": max_results},
    )
    if status >= 400:
        raise HttpError("arxiv HTTP %d" % status)
    out = []
    for entry in re.findall(r"<entry>(.*?)</entry>", text, re.S)[:max_results]:
        tm = re.search(r"<title>(.*?)</title>", entry, re.S)
        im = re.search(r"<id>(.*?)</id>", entry, re.S)
        sm = re.search(r"<summary>(.*?)</summary>", entry, re.S)
        pm = re.search(r"<published>(.*?)</published>", entry, re.S)
        title = strip_tags(tm.group(1)) if tm else ""
        url = (im.group(1).strip() if im else "")
        out.append({
            "title": title,
            "url": url,
            "snippet": strip_tags(sm.group(1)) if sm else "",
            "source": "arxiv",
            "published": pm.group(1).strip() if pm else None,
        })
    return out


def provider_crossref(query, max_results, env):
    data = http_json(
        "GET", "https://api.crossref.org/works",
        params={"query": query, "rows": max_results},
    )
    out = []
    for item in (data.get("message", {}).get("items", []) or [])[:max_results]:
        title = (item.get("title") or [""])[0]
        url = item.get("URL", "")
        container = (item.get("container-title") or [""])[0]
        parts = (item.get("published", {}) or {}).get("date-parts", [[None]])
        year = parts[0][0] if parts and parts[0] else None
        out.append({
            "title": strip_tags(title),
            "url": url,
            "snippet": strip_tags(item.get("abstract", "")) or container,
            "source": "crossref",
            "published": str(year) if year else None,
        })
    return out


def provider_pubmed(query, max_results, env):
    base = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils"
    common = {"db": "pubmed", "retmode": "json"}
    key = env.get("NCBI_API_KEY")
    if key:
        common["api_key"] = key
    search = http_json("GET", base + "/esearch.fcgi",
                       params=dict(common, term=query, retmax=max_results))
    ids = (search.get("esearchresult", {}) or {}).get("idlist", []) or []
    if not ids:
        return []
    summ = http_json("GET", base + "/esummary.fcgi",
                     params=dict(common, id=",".join(ids)))
    res = summ.get("result", {}) or {}
    out = []
    for pid in ids:
        item = res.get(pid) or {}
        date = item.get("pubdate", "")
        source = item.get("source", "")
        snippet = ". ".join(x for x in (source, date) if x)
        out.append({
            "title": strip_tags(item.get("title", "")),
            "url": "https://pubmed.ncbi.nlm.nih.gov/%s/" % pid,
            "snippet": snippet,
            "source": "pubmed",
            "published": date or None,
        })
    return out


def _openalex_abstract(index):
    if not index:
        return ""
    positioned = []
    for word, spots in index.items():
        for spot in spots:
            positioned.append((spot, word))
    positioned.sort()
    return " ".join(word for _, word in positioned)


def provider_openalex(query, max_results, env):
    # Reliable + no key, but OpenAlex now meters a small per-IP daily budget;
    # a contact email joins the "polite pool" and an API key lifts the budget.
    params = {"search": query, "per-page": max_results}
    mail = env.get("OPENALEX_MAILTO") or env.get("OPERON_CONTACT_EMAIL")
    if mail:
        params["mailto"] = mail
    key = env.get("OPENALEX_API_KEY")
    if key:
        params["api_key"] = key
    data = http_json("GET", "https://api.openalex.org/works", params=params)
    out = []
    for item in (data.get("results", []) or [])[:max_results]:
        out.append({
            "title": strip_tags(item.get("title") or ""),
            "url": item.get("doi") or item.get("id") or "",
            "snippet": _openalex_abstract(item.get("abstract_inverted_index"))[:500],
            "source": "openalex",
            "published": item.get("publication_date"),
        })
    return out


def provider_semanticscholar(query, max_results, env):
    headers = {}
    key = env.get("SEMANTIC_SCHOLAR_API_KEY")
    if key:
        headers["x-api-key"] = key
    data = http_json(
        "GET", "https://api.semanticscholar.org/graph/v1/paper/search",
        headers=headers,
        params={"query": query, "limit": max_results,
                "fields": "title,url,abstract,year,venue"},
    )
    out = []
    for item in (data.get("data", []) or [])[:max_results]:
        out.append({
            "title": item.get("title", ""),
            "url": item.get("url", ""),
            "snippet": item.get("abstract") or item.get("venue") or "",
            "source": "semanticscholar",
            "published": str(item["year"]) if item.get("year") else None,
        })
    return out


def provider_brave(query, max_results, env):
    key = env.get("BRAVE_SEARCH_API_KEY") or env.get("BRAVE_API_KEY")
    if not key:
        raise HttpError("BRAVE_SEARCH_API_KEY not set")
    data = http_json(
        "GET", "https://api.search.brave.com/res/v1/web/search",
        headers={"X-Subscription-Token": key, "Accept": "application/json"},
        params={"q": query, "count": max_results},
    )
    out = []
    for hit in (data.get("web", {}).get("results", []) or [])[:max_results]:
        out.append({
            "title": hit.get("title", ""),
            "url": hit.get("url", ""),
            "snippet": strip_tags(hit.get("description", "")),
            "source": "brave",
            "published": hit.get("age"),
        })
    return out


def provider_serper(query, max_results, env):
    key = env.get("SERPER_API_KEY")
    if not key:
        raise HttpError("SERPER_API_KEY not set")
    data = http_json(
        "POST", "https://google.serper.dev/search",
        headers={"X-API-KEY": key, "Content-Type": "application/json"},
        data=json.dumps({"q": query, "num": max_results}),
    )
    out = []
    for hit in (data.get("organic", []) or [])[:max_results]:
        out.append({
            "title": hit.get("title", ""),
            "url": hit.get("link", ""),
            "snippet": hit.get("snippet", ""),
            "source": "serper",
            "published": hit.get("date"),
        })
    return out


def provider_tavily(query, max_results, env):
    key = env.get("TAVILY_API_KEY")
    if not key:
        raise HttpError("TAVILY_API_KEY not set")
    data = http_json(
        "POST", "https://api.tavily.com/search",
        headers={"Content-Type": "application/json"},
        data=json.dumps({
            "api_key": key, "query": query,
            "max_results": max_results, "search_depth": "basic",
        }),
    )
    out = []
    for hit in (data.get("results", []) or [])[:max_results]:
        out.append({
            "title": hit.get("title", ""),
            "url": hit.get("url", ""),
            "snippet": hit.get("content", ""),
            "source": "tavily",
            "published": hit.get("published_date"),
        })
    return out


PROVIDERS = {
    # Free, no key, and reachable through Claude Science's sandbox proxy
    # allowlist (verified: arXiv/Crossref/PubMed/OpenAlex/Semantic Scholar).
    "crossref": provider_crossref,
    "arxiv": provider_arxiv,
    "pubmed": provider_pubmed,
    "openalex": provider_openalex,
    "semanticscholar": provider_semanticscholar,
    # Free general-web providers. CSP pre-grants their hosts on Start.
    "duckduckgo": provider_duckduckgo,
    "duckduckgo_lite": provider_duckduckgo_lite,
    "duckduckgo_ia": provider_duckduckgo_ia,
    "wikipedia": provider_wikipedia,
    # Key-based general search. Also subject to the sandbox allowlist (their API
    # domains are blocked in-sandbox today); kept for out-of-sandbox use and in
    # case the allowlist is widened.
    "brave": provider_brave,
    "serper": provider_serper,
    "tavily": provider_tavily,
}

# Key-based providers and the env var that activates each.
# Two search lanes (chosen by host.mcp method name, not by guessing the query):
#   general     — csp_web_search (canonical); web_search = unlisted dispatch alias
#                 keyed Brave/Serper/Tavily (optional) → duckduckgo_ia →
#                 duckduckgo_lite  (wikipedia is NOT a GENERAL fallback)
#   literature  — search_literature
#                 wikipedia → Crossref → arXiv → PubMed
# Explicit provider=<name> still works on either tool (overrides the lane's auto).
KEY_PROVIDERS = [
    ("brave", ("BRAVE_SEARCH_API_KEY", "BRAVE_API_KEY")),
    ("serper", ("SERPER_API_KEY",)),
    ("tavily", ("TAVILY_API_KEY",)),
]
GENERAL_FREE_FALLBACKS = ["duckduckgo_ia", "duckduckgo_lite"]
LITERATURE_FREE_FALLBACKS = ["wikipedia", "crossref", "arxiv", "pubmed"]
# Kept for tests / docs; prefer the lane-specific lists above.
FREE_FALLBACKS = GENERAL_FREE_FALLBACKS + LITERATURE_FREE_FALLBACKS


def auto_order(env, lane="general"):
    """Build provider try-order for ``provider=auto`` on a given lane."""
    order = []
    if lane == "literature":
        order.extend(LITERATURE_FREE_FALLBACKS)
        return order
    # general (default)
    for name, keys in KEY_PROVIDERS:
        if any(env.get(k) for k in keys):
            order.append(name)
    order.extend(GENERAL_FREE_FALLBACKS)
    return order


def do_web_search(args, env, lane="general"):
    query = (args.get("query") or "").strip()
    if not query:
        raise ValueError("query is required")
    max_results = args.get("max_results", 5)
    try:
        max_results = max(1, min(20, int(max_results)))
    except Exception:
        max_results = 5
    provider = (args.get("provider") or "auto").strip().lower()

    if provider == "auto":
        candidates = auto_order(env, lane=lane)
    elif provider in PROVIDERS:
        candidates = [provider]
    else:
        raise ValueError(
            "unknown provider '%s' (available: auto, %s)"
            % (provider, ", ".join(sorted(PROVIDERS)))
        )

    warnings = []
    soft_ia_wiki = None  # IA hit that is Wikipedia-only; prefer free web after.
    for name in candidates:
        fn = PROVIDERS[name]
        try:
            results = fn(query, max_results, env)
        except Exception as exc:
            warnings.append("%s: %s" % (name, exc))
            log("provider %s failed: %s" % (name, exc))
            continue
        if results:
            # Instant Answer often surfaces a Wikipedia AbstractURL for entity
            # queries. That is still duckduckgo_ia — not a GENERAL wikipedia
            # fallback — but prefer broader Lite/html web hits when available.
            if (
                name == "duckduckgo_ia"
                and lane == "general"
                and all(
                    "wikipedia.org" in (r.get("url") or "")
                    for r in results
                )
            ):
                soft_ia_wiki = results
                warnings.append(
                    "%s: Instant Answer is Wikipedia-only; trying free web "
                    "results next (not a GENERAL Wikipedia fallback)"
                    % name
                )
                continue
            return {"provider": name, "query": query, "lane": lane,
                    "results": results[:max_results], "warnings": warnings}
        if name == "duckduckgo_ia":
            warnings.append(
                "%s: no Instant Answer hits (common for non-entity / news "
                "queries; not a missing API key — trying free fallbacks)"
                % name
            )
        else:
            warnings.append("%s: no results" % name)

    if soft_ia_wiki:
        return {
            "provider": "duckduckgo_ia",
            "query": query,
            "lane": lane,
            "results": soft_ia_wiki[:max_results],
            "warnings": warnings,
        }

    hint = (_EMPTY_GENERAL_HINT if lane == "general"
            else _EMPTY_LITERATURE_HINT)
    if not candidates:
        warnings = warnings or ["no providers available"]
    return {
        "provider": None,
        "query": query,
        "lane": lane,
        "results": [],
        "warnings": warnings,
        "hint": hint,
        # Keep message for models that only skim string fields.
        "message": hint,
    }


def do_general_web_search(args, env):
    return do_web_search(args, env, lane="general")


def do_literature_search(args, env):
    return do_web_search(args, env, lane="literature")


def do_fetch_url(args, env):
    url = (args.get("url") or "").strip()
    if not url:
        raise ValueError("url is required")
    if not re.match(r"^https?://", url, re.I):
        raise ValueError("url must be http(s)")
    max_chars = args.get("max_chars", 8000)
    try:
        max_chars = max(200, min(50000, int(max_chars)))
    except Exception:
        max_chars = 8000
    status, text = http_request("GET", url)
    if status >= 400:
        raise HttpError("HTTP %d fetching %s" % (status, url))
    return {"url": url, "status": status,
            "content": html_to_text(text, max_chars)}


# --------------------------------------------------------------------------- #
# MCP tool schemas                                                             #
# --------------------------------------------------------------------------- #
# Shared search input schema (all search aliases use it verbatim).
_SEARCH_INPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "query": {"type": "string", "description": "Search query."},
        "max_results": {
            "type": "integer", "default": 5, "minimum": 1, "maximum": 20,
            "description": "Maximum number of results (1-20).",
        },
        "provider": {
            "type": "string",
            "default": "auto",
            "enum": ["auto", "crossref", "arxiv", "pubmed", "openalex",
                     "semanticscholar", "duckduckgo", "duckduckgo_lite",
                     "duckduckgo_ia", "wikipedia", "brave", "serper", "tavily"],
            "description": "Search backend; 'auto' picks the best available.",
        },
    },
    "required": ["query"],
}

_FETCH_INPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "url": {"type": "string", "description": "Absolute http(s) URL."},
        "max_chars": {
            "type": "integer", "default": 8000, "minimum": 200,
            "maximum": 50000,
            "description": "Truncate the extracted text to this many chars.",
        },
    },
    "required": ["url"],
}

_RETURN_SHAPE = (
    "RETURN SHAPE (critical): host.mcp parses the JSON tool result into a "
    "Python dict — NOT a list. Schema: "
    "{\"provider\": str|null, \"query\": str, \"lane\": \"general\"|\"literature\", "
    "\"results\": [{\"title\", \"url\", \"snippet\", \"source\", \"published\"?}, ...], "
    "\"warnings\": [str, ...]}. Correct usage: "
    "data = host.mcp(...); hits = data[\"results\"]; "
    "for r in hits: print(r.get(\"title\"), r.get(\"url\"), r.get(\"snippet\")). "
    "Or simply print(data). Do NOT iterate the dict itself "
    "(for x in data yields string keys → AttributeError on .get). "
)

_CSP_NO_NATIVE = (
    "Under Claude Science Proxy (CSP) virtual login there is NO Anthropic-hosted "
    "web_search tool — calling it as a top-level tool fails with "
    "\"Tool 'web_search' not found on agent\". Use the repl tool + host.mcp. "
)

_GENERAL_DESCRIPTION = (
    "GENERAL web search (news, products, \"latest models\", facts) via "
    "host.mcp(\"web-search\", \"csp_web_search\", query=..., max_results=N). "
    "This is the ONE public GENERAL method name — do not treat any other name "
    "as a second search engine. " + _CSP_NO_NATIVE + _RETURN_SHAPE +
    "With provider='auto' (default): optional keyed Brave/Serper/Tavily IF "
    "env keys are set, then free duckduckgo_ia → duckduckgo_lite (stops there; "
    "wikipedia is on the LITERATURE lane, NOT GENERAL — never claim GENERAL "
    "\"fell back to Wikipedia\"). "
    "DuckDuckGo needs NO API key. Empty Instant Answer / temporary Lite "
    "anti-bot does NOT mean keys are missing — do not tell the user they must "
    "configure Brave/Serper/Tavily. If results are empty, report the warnings/"
    "hint honestly and rephrase or fetch_url a known URL. "
    "Do NOT use this for academic / encyclopedic topics — use "
    "search_literature (that lane may return Wikipedia first by design). "
    "Explicit provider= still allowed. Full HTML provider='duckduckgo' is fragile. "
    "CSP pre-grants search hosts on Start; extend via ~/.csp/network-allowlist.json."
)

_LITERATURE_DESCRIPTION = (
    "ACADEMIC / literature search via host.mcp(\"web-search\", "
    "\"search_literature\", query=..., max_results=N). " + _CSP_NO_NATIVE +
    _RETURN_SHAPE +
    "With provider='auto' (default) this LITERATURE lane tries wikipedia → "
    "Crossref → arXiv → PubMed. Use for papers, DOIs, scholarly metadata. "
    "For product/news/\"latest GPT\" queries use csp_web_search instead "
    "(GENERAL lane). OpenAlex / Semantic Scholar remain explicitly selectable "
    "via provider=. CSP network allowlist applies; extend via "
    "~/.csp/network-allowlist.json."
)

_FETCH_DESCRIPTION = (
    "Fetch a web page (or text/JSON resource) and return its readable text "
    "with HTML stripped. Call via host.mcp(\"web-search\", \"fetch_url\"|"
    "\"web_fetch\", url=...). Useful after a search result. Local CSP MCP only "
    "— no Anthropic-hosted fetch. "
    "RETURN SHAPE: host.mcp returns a dict "
    "{\"url\": str, \"status\": int, \"content\": str}. "
    "Use: data = host.mcp(...); print(data[\"content\"]). "
    "Do not assume a bare string unless you read data[\"content\"]."
)

# Public tools/list — ONE GENERAL method name only. Listing both web_search and
# csp_web_search made models think there were two search products.
TOOLS = [
    {
        "name": "csp_web_search",
        "description": _GENERAL_DESCRIPTION,
        "inputSchema": _SEARCH_INPUT_SCHEMA,
    },
    {
        "name": "search_literature",
        "description": _LITERATURE_DESCRIPTION,
        "inputSchema": _SEARCH_INPUT_SCHEMA,
    },
    {
        "name": "fetch_url",
        "description": _FETCH_DESCRIPTION + " Alias: web_fetch.",
        "inputSchema": _FETCH_INPUT_SCHEMA,
    },
    {
        "name": "web_fetch",
        "description": _FETCH_DESCRIPTION + " Alias of fetch_url.",
        "inputSchema": _FETCH_INPUT_SCHEMA,
    },
]

# Dispatch still accepts web_search as an undocumented alias of csp_web_search
# (old sessions, proxy remnants, skills that still say web_search).
TOOL_DISPATCH = {
    "csp_web_search": do_general_web_search,
    "web_search": do_general_web_search,  # unlisted alias
    "search_literature": do_literature_search,
    "fetch_url": do_fetch_url,
    "web_fetch": do_fetch_url,
}


# --------------------------------------------------------------------------- #
# JSON-RPC / MCP stdio loop                                                    #
# --------------------------------------------------------------------------- #
def _result(msg_id, result):
    return {"jsonrpc": "2.0", "id": msg_id, "result": result}


def _error(msg_id, code, message):
    return {"jsonrpc": "2.0", "id": msg_id,
            "error": {"code": code, "message": message}}


def handle(msg, env):
    method = msg.get("method")
    msg_id = msg.get("id")
    # Notifications (no id) never get a response.
    if msg_id is None and method and method.startswith("notifications/"):
        return None

    if method == "initialize":
        params = msg.get("params") or {}
        proto = params.get("protocolVersion") or DEFAULT_PROTOCOL
        return _result(msg_id, {
            "protocolVersion": proto,
            "capabilities": {"tools": {"listChanged": False}},
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
        })
    if method == "ping":
        return _result(msg_id, {})
    if method == "tools/list":
        return _result(msg_id, {"tools": TOOLS})
    if method == "tools/call":
        params = msg.get("params") or {}
        name = params.get("name")
        args = params.get("arguments") or {}
        fn = TOOL_DISPATCH.get(name)
        if fn is None:
            return _error(msg_id, -32602, "unknown tool: %s" % name)
        try:
            payload = fn(args, env)
            text = json.dumps(payload, ensure_ascii=False, indent=2)
            return _result(msg_id, {
                "content": [{"type": "text", "text": text}],
                "isError": False,
            })
        except Exception as exc:
            return _result(msg_id, {
                "content": [{"type": "text",
                             "text": "%s error: %s" % (name, exc)}],
                "isError": True,
            })
    if msg_id is None:
        return None  # unknown notification
    return _error(msg_id, -32601, "method not found: %s" % method)


def main():
    env = os.environ
    log("started (requests=%s, proxy=%s)" % (
        _HAVE_REQUESTS, env.get("HTTPS_PROXY") or env.get("https_proxy") or "none"))
    stdin = sys.stdin
    for line in stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except Exception:
            continue
        try:
            resp = handle(msg, env)
        except Exception as exc:  # never crash the loop
            log("handler crashed: %s" % exc)
            resp = _error(msg.get("id"), -32603, "internal error: %s" % exc)
        if resp is not None:
            sys.stdout.write(json.dumps(resp, ensure_ascii=False) + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
