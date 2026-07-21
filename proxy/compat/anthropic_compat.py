"""Anthropic passthrough compatibility layer (S1a): exposes three entry points to the thin skeleton.
Calls provider_policy + dsml_shim internally.

Dependency direction: skeleton → this module → provider_policy; no reverse import of csp_proxy
(no circular deps). Stateless serializable entry points + injectable nonce + explicit ProviderState
→ prepares the S1b cross-language seam.
"""
import json
from dataclasses import dataclass
from datetime import datetime

from proxy.dsml import dsml_shim
from proxy.policy import provider_policy

_EMPTY_OBJECT_SCHEMA = {"type": "object", "properties": {}}

# Standing guidance for Science / OPERON under CSP virtual login.
# Bare Anthropic-native web_search / web_fetch are stripped by the subscription
# tier and are never top-level tools here. Local MCP tools are only reachable
# via `repl` → host.mcp("web-search", ...). Skills are progressive-disclosure
# and insufficient as standing prompt text — inject on every /v1/messages
# request that already carries a system prompt. Re-advertising names in MCP
# tools/list cannot intercept bare tool calls; this prompt injection is the fix.
# Current wall-clock date/time is generated at request time (Science has no
# reliable clock; knowledge cutoff ~early 2024).
CSP_WEB_ACCESS_GUIDANCE_SENTINEL = "<!-- CSP_WEB_ACCESS_GUIDANCE -->"


def _local_now_label(now=None):
    """Short host-local date/time label, e.g. ``2026-07-14 12:46 (Asia/Shanghai)``."""
    if now is None:
        now = datetime.now().astimezone()
    elif now.tzinfo is None:
        now = now.astimezone()
    else:
        now = now.astimezone()
    tz = now.tzinfo
    tz_label = getattr(tz, "key", None) or now.strftime("%Z") or now.strftime("%z") or "local"
    return now, f"{now.strftime('%Y-%m-%d %H:%M')} ({tz_label})"


def build_csp_web_access_guidance(now=None):
    """Build standing web-access guidance with a fresh wall-clock date line."""
    now, label = _local_now_label(now)
    date_only = now.strftime("%Y-%m-%d")
    year = now.year
    return (
        CSP_WEB_ACCESS_GUIDANCE_SENTINEL + "\n"
        f"Current local date/time: {label}. "
        "Treat this as \"today\" when answering date/time questions, ranking search "
        "freshness, and writing search queries — prefer the current calendar year "
        f"(e.g. {year}) and \"latest as of {date_only}\" over training-cutoff "
        "years (do not assume it is still 2024).\n"
        "CSP web access (standing rules):\n"
        "- This environment has NO native Anthropic `web_search` / `web_fetch` tools. "
        "Calling them as top-level tools fails with "
        "`Tool 'web_search' not found on agent 'OPERON'`.\n"
        "- Two search lanes (pick by host.mcp method name):\n"
        "  GENERAL (news/products/\"latest models\"/facts) — ONE public method:\n"
        "    data = host.mcp(\"web-search\", \"csp_web_search\", query=\"...\", max_results=N)\n"
        "    # auto: optional Brave/Serper/Tavily IF keyed → duckduckgo_ia → "
        "duckduckgo_lite (no key required; wikipedia is NOT on GENERAL)\n"
        "  LITERATURE (papers/DOI/scholarly/encyclopedic):\n"
        "    data = host.mcp(\"web-search\", \"search_literature\", query=\"...\", max_results=N)\n"
        "    # auto: wikipedia → Crossref → arXiv → PubMed\n"
        "    hits = data[\"results\"]  # list of {title, url, snippet, source, ...}\n"
        "    for r in hits: print(r.get(\"title\"), r.get(\"url\"))\n"
        "  Or just print(data). host.mcp returns a parsed dict with key \"results\" "
        "(NOT a bare list — do not enumerate(data) itself).\n"
        "- For product/news queries use csp_web_search — NOT search_literature "
        "(that lane is academic-only). Do not invent a second GENERAL engine.\n"
        "- Native Anthropic tool named web_search is unavailable and must NEVER "
        "be called top-level. The MCP method is csp_web_search only (publicly).\n"
        "- DuckDuckGo Instant Answer needs no API key. Empty IA / "
        "\"duckduckgo_ia: no results\" is common for news queries and does NOT mean "
        "keys are missing — free duckduckgo_lite follows automatically. "
        "If Lite warns about anti-bot, that is temporary — rephrase/retry; do NOT "
        "claim GENERAL fell back to Wikipedia (wikipedia is NOT on GENERAL after "
        "v1.6.7). Do NOT tell the user they must configure Brave/Serper/Tavily; "
        "those keys are optional quality upgrades only. Wikipedia-only hit lists "
        "come from search_literature (expected) — do not conflate the two lanes. "
        "For encyclopedic / academic topics use search_literature.\n"
        "- To fetch a page afterward:\n"
        "    page = host.mcp(\"web-search\", \"fetch_url\", url=\"...\")\n"
        "    print(page[\"content\"])  # dict with url, status, content\n"
        "- Do NOT call bare `web_search` / `web_fetch` as top-level tools.\n"
        "- Science built-in `search_skills` (UI: \"Searching for available skills "
        "and MCPs\"): ALWAYS pass a non-empty `query` OR `prefix` — empty calls fail "
        "with `Missing 'query' argument (or provide 'prefix')`. Examples: "
        "`search_skills(query=\"web search\")` or `search_skills(prefix=\"mcp-\")` "
        "(list connector skill docs). Never call `search_skills()` with no args.\n"
        "- Other CSP limits (short): never write to /mnt/data — save under the "
        "workspace cwd and persist with save_artifacts([...]); set CJK matplotlib "
        "fonts when plotting Chinese labels."
    )


# Back-compat alias for callers that still read a module-level string; prefer
# ``build_csp_web_access_guidance()`` so the date is request-fresh.
CSP_WEB_ACCESS_GUIDANCE = build_csp_web_access_guidance()

# Claude Science rolling-compact / summarizer forks (see operon binary
# callSite rc_fold_l1/l2, summarize_conversation, Literals: section). When these
# requests still advertise the session's full tool list, third-party models
# (esp. GLM) often answer with tool_use + empty text — Science then retries
# forever while the main turn is already over the upstream context limit.
_ROLLING_COMPACT_SYSTEM_MARKERS = (
    "Literals:",
    "Wrap your summary in <summary>",
    "summarize_conversation",
    "[rolling-summary",
    "OUTPUT FORMAT OVERRIDE",
    "Completionist means breadth",
    "RECORD_SUMMARY",
    "structured summary of the conversation",
    "Produce a structured summary",
)
_ROLLING_COMPACT_TOOL_NAMES = frozenset({
    "summarize_conversation",
    "record_summary",
    "emit_summary",
})


def _system_text_blob(system):
    """Flatten Anthropic ``system`` (str | list blocks) to one searchable string."""
    if isinstance(system, str):
        return system
    if isinstance(system, list):
        parts = []
        for blk in system:
            if isinstance(blk, str):
                parts.append(blk)
            elif isinstance(blk, dict):
                parts.append(blk.get("text") or "")
        return "\n".join(parts)
    return ""


def is_science_rolling_compact(body):
    """True when the Anthropic-shaped request looks like a Science compact fork."""
    if not isinstance(body, dict):
        return False
    blob = _system_text_blob(body.get("system"))
    if blob and any(m in blob for m in _ROLLING_COMPACT_SYSTEM_MARKERS):
        return True
    tools = body.get("tools")
    if isinstance(tools, list) and tools:
        names = {
            t.get("name") for t in tools
            if isinstance(t, dict) and isinstance(t.get("name"), str)
        }
        if names & _ROLLING_COMPACT_TOOL_NAMES:
            return True
        # Schema summarizer often ships a single record/summary tool.
        if len(names) == 1:
            only = next(iter(names)).lower()
            if "summary" in only or only.startswith("record_"):
                return True
    # User/harness text can also carry the Literals contract on retries.
    for msg in body.get("messages") or []:
        if not isinstance(msg, dict):
            continue
        content = msg.get("content")
        if isinstance(content, str):
            text = content
        elif isinstance(content, list):
            text = "\n".join(
                (b.get("text") or "") if isinstance(b, dict) else (b if isinstance(b, str) else "")
                for b in content
            )
        else:
            continue
        if "Literals:" in text and ("summary" in text.lower() or "<summary" in text.lower()):
            return True
    return False


def strip_tools_for_rolling_compact(body):
    """Force a text-only summarizer turn: no tools, tool_choice=none.

    Returns a shallow copy when a mutation is needed; otherwise ``body``.
    """
    if not isinstance(body, dict):
        return body
    tools = body.get("tools")
    tc = body.get("tool_choice")
    need = bool(tools) or tc not in (None, {"type": "none"}, "none")
    if not need:
        return body
    out = dict(body)
    out.pop("tools", None)
    out["tool_choice"] = {"type": "none"}
    return out


def prepare_inbound_messages_request(body, now=None):
    """Inbound Anthropic ``/v1/messages`` prep shared by Anthropic + OpenAI paths.

    - Science rolling-compact / summarizer: strip tools (do **not** inject the
      standing web-access guidance — it pollutes the Literals contract).
    - Normal turns: idempotent CSP web-access guidance inject.
    """
    if is_science_rolling_compact(body):
        return strip_tools_for_rolling_compact(body)
    return inject_csp_web_access_guidance(body, now=now)


@dataclass
class Ctx:
    """Request-level context produced by transform_request and passed to rewrite_nonstream /
    make_stream_rewriter."""
    src_model: str
    target_model: str
    known_tools: dict
    nonce: str
    shim_mode: str
    provider: str
    rule_ids: tuple = ()


def _normalize_relay_input_schema(schema):
    if not isinstance(schema, dict) or not schema:
        return dict(_EMPTY_OBJECT_SCHEMA)

    out = dict(schema)
    props = out.get("properties")
    typ = out.get("type")

    if typ is None and isinstance(props, dict):
        out["type"] = "object"
    elif typ != "object":
        out = dict(_EMPTY_OBJECT_SCHEMA)
        props = out["properties"]

    if not isinstance(out.get("properties"), dict):
        out["properties"] = {}
    if "required" in out and not isinstance(out["required"], list):
        out.pop("required", None)
    return out


def _degrade_missing_tool_choice(upstream):
    choice = upstream.get("tool_choice")
    if not (isinstance(choice, dict) and choice.get("type") == "tool"):
        return
    names = {t.get("name") for t in upstream.get("tools") or [] if isinstance(t, dict)}
    if choice.get("name") not in names:
        upstream["tool_choice"] = {"type": "auto"}


def _append_rule_id(rule_ids, rule_id):
    if rule_ids is not None and rule_id not in rule_ids:
        rule_ids.append(rule_id)


def _system_already_has_guidance(system):
    """Return True if the Anthropic `system` field already carries our sentinel."""
    if isinstance(system, str):
        return CSP_WEB_ACCESS_GUIDANCE_SENTINEL in system
    if isinstance(system, list):
        for blk in system:
            if isinstance(blk, dict) and CSP_WEB_ACCESS_GUIDANCE_SENTINEL in (blk.get("text") or ""):
                return True
            if isinstance(blk, str) and CSP_WEB_ACCESS_GUIDANCE_SENTINEL in blk:
                return True
    return False


def _refresh_guidance_text(text, guidance):
    """Replace an existing trailing guidance block (from sentinel to EOF) with ``guidance``.

    Guidance is always appended as a trailing block, so rewriting from the sentinel
    updates a prior day's date from earlier conversation turns without stacking a
    second sentinel.
    """
    if not isinstance(text, str):
        return text
    idx = text.find(CSP_WEB_ACCESS_GUIDANCE_SENTINEL)
    if idx < 0:
        return text.rstrip() + "\n\n" + guidance
    prefix = text[:idx].rstrip()
    return (prefix + "\n\n" + guidance) if prefix else guidance


def _refresh_system_with_guidance(system, guidance):
    """Return updated ``system`` with fresh guidance, or ``system`` if unchanged."""
    if isinstance(system, str):
        updated = _refresh_guidance_text(system, guidance)
        return system if updated == system else updated
    if isinstance(system, list):
        found = False
        blocks = []
        for blk in system:
            if isinstance(blk, dict) and CSP_WEB_ACCESS_GUIDANCE_SENTINEL in (blk.get("text") or ""):
                new_text = _refresh_guidance_text(blk.get("text") or "", guidance)
                if new_text != blk.get("text"):
                    blocks.append({**blk, "text": new_text})
                else:
                    blocks.append(blk)
                found = True
            elif isinstance(blk, str) and CSP_WEB_ACCESS_GUIDANCE_SENTINEL in blk:
                new_text = _refresh_guidance_text(blk, guidance)
                blocks.append(new_text if new_text != blk else blk)
                found = True
            else:
                blocks.append(blk)
        if not found:
            blocks.append({"type": "text", "text": guidance})
            return blocks
        # Identity-preserving when nothing changed (same-second re-inject).
        if len(blocks) == len(system) and all(a is b for a, b in zip(blocks, system)):
            return system
        return blocks
    return system


def inject_csp_web_access_guidance(body, now=None):
    """Idempotently append / refresh standing web-access guidance on Anthropic ``system``.

    Only touches requests that already have a non-empty ``system`` prompt / blocks
    (Science operon turns always do). Generates a fresh wall-clock date at call
    time. If the sentinel is already present (e.g. prior turn's date in a carried
    ``system``), rewrites that trailing guidance block instead of stacking a
    duplicate. Returns a shallow-copied body when a mutation is needed; returns
    ``body`` unchanged when there is nothing to do (including same-second refresh
    that yields identical text).
    """
    if not isinstance(body, dict):
        return body
    system = body.get("system")
    if system is None or system == "" or system == []:
        return body

    guidance = build_csp_web_access_guidance(now=now)

    if _system_already_has_guidance(system):
        refreshed = _refresh_system_with_guidance(system, guidance)
        if refreshed is system:
            return body
        out = dict(body)
        out["system"] = refreshed
        return out

    out = dict(body)
    if isinstance(system, str):
        out["system"] = system.rstrip() + "\n\n" + guidance
        return out
    if isinstance(system, list):
        blocks = list(system)
        # Prefer appending a new text block so we never mutate caller-owned
        # block dicts in place.
        blocks.append({"type": "text", "text": guidance})
        out["system"] = blocks
        return out
    return body


def _normalize_relay_tools(upstream, rule_ids=None):
    """Normalize Anthropic-compatible relay tool schemas before outbound.

    Some Anthropic-compatible relay providers reject Claude Science's empty or loose
    ``input_schema`` values with a provider-side 400. Keep this limited to relay
    passthrough; OpenAI/custom OpenAI Responses conversions have their own mapping rules.
    """
    tools = upstream.get("tools")
    if not isinstance(tools, list):
        if "tools" in upstream:
            upstream.pop("tools", None)
            _degrade_missing_tool_choice(upstream)
        return

    normalized = []
    for tool in tools:
        if not isinstance(tool, dict) or not tool.get("name"):
            continue
        clean = dict(tool)
        clean["input_schema"] = _normalize_relay_input_schema(tool.get("input_schema"))
        normalized.append(clean)
    _append_rule_id(rule_ids, provider_policy.RULE_TOOL_RELAY_INPUT_SCHEMA_NORMALIZE)
    if normalized:
        upstream["tools"] = normalized
    else:
        upstream.pop("tools", None)
    _degrade_missing_tool_choice(upstream)


def _filter_upstream_tools(upstream, target_model, provider, rule_ids=None):
    """Provider-specific tool compatibility before sending to upstream.

    Kimi's Anthropic endpoint treats a tool named ``web_search`` as its own server tool and
    streams ``server_tool_use`` / ``web_search_tool_result`` blocks. The local client path
    expects ordinary client tools, so those server-tool blocks make the stream
    retry. Keep the original known_tools in ctx, but do not advertise this one tool upstream.
    """
    if provider != "relay":
        return
    _normalize_relay_tools(upstream, rule_ids)
    if "kimi" in (target_model or "").lower():
        tools = upstream.get("tools")
        if isinstance(tools, list):
            filtered = [t for t in tools if not (isinstance(t, dict) and t.get("name") == "web_search")]
            if len(filtered) != len(tools):
                _append_rule_id(rule_ids, provider_policy.RULE_TOOL_KIMI_WEB_SEARCH_SERVER_TOOL_FILTER)
                if filtered:
                    upstream["tools"] = filtered
                else:
                    upstream.pop("tools", None)
                _degrade_missing_tool_choice(upstream)


def _strip_orphan_tool_blocks(messages):
    """Remove orphan tool_use/tool_result blocks from relay (Anthropic-native) messages.

    After a failed tool call (e.g. Kimi K3 multi-turn reasoning + tool_use failure),
    Science may replay history with a tool_use block whose tool_result was never
    produced, or a tool_result whose tool_use fell outside the replayed window.
    Sending these orphans to Kimi/relay causes repeated `temporarily unavailable`
    because the model sees an incomplete tool-call sequence.

    Strategy: track tool_use IDs declared in each assistant turn. A tool_result
    whose tool_use_id is not in any preceding turn is downgraded to a text block
    (preserving its content as context). A trailing tool_use with no matching
    tool_result in a subsequent user turn is removed entirely — the model would
    try to continue from a phantom tool call.
    """
    if not isinstance(messages, list):
        return messages
    known_tool_ids = set()
    for msg in messages:
        if not isinstance(msg, dict):
            continue
        content = msg.get("content")
        if not isinstance(content, list):
            continue
        new_blocks = []
        for blk in content:
            if not isinstance(blk, dict):
                new_blocks.append(blk)
                continue
            t = blk.get("type")
            if t == "tool_use":
                new_blocks.append(blk)
                known_tool_ids.add(blk.get("id"))
            elif t == "tool_result":
                call_id = blk.get("tool_use_id")
                if call_id and call_id in known_tool_ids:
                    new_blocks.append(blk)
                else:
                    # Orphan tool_result: downgrade to text so content is not lost
                    raw = blk.get("content")
                    if isinstance(raw, str):
                        text = raw
                    elif isinstance(raw, list):
                        text = " ".join(
                            b.get("text", "") if isinstance(b, dict) else str(b)
                            for b in raw
                        )
                    else:
                        text = str(raw) if raw else ""
                    label = call_id or "unknown"
                    new_blocks.append({
                        "type": "text",
                        "text": f"[tool_result for {label} (orphaned, "
                                f"omitted from replay)]: {text}",
                    })
            else:
                new_blocks.append(blk)
        msg["content"] = new_blocks

    # Second pass: remove trailing tool_use blocks in the last assistant message
    # that have no matching tool_result in any subsequent user message.
    # (Already-processed tool_results are now text, so any remaining tool_use
    # IDs in the last message with no follower are orphans.)
    if messages:
        last = messages[-1]
        if isinstance(last, dict) and last.get("role") == "assistant":
            content = last.get("content")
            if isinstance(content, list):
                # Collect tool_use IDs in the last message
                last_tool_ids = {
                    blk.get("id") for blk in content
                    if isinstance(blk, dict) and blk.get("type") == "tool_use"
                }
                if last_tool_ids:
                    # Check if any subsequent message has a matching tool_result
                    # (there are none after the last message, so all are orphans)
                    has_result = False
                    # If there are messages after this one (there shouldn't be
                    # if it's truly the last), check them; otherwise strip.
                    last["content"] = [
                        blk for blk in content
                        if not (isinstance(blk, dict) and blk.get("type") == "tool_use"
                                and blk.get("id") in last_tool_ids)
                    ] or [{"type": "text", "text": ""}]
    return messages


def transform_request(body, state):
    """(body, ProviderState) -> (upstream_body, Ctx). Pure function: no network, no global reads.
    Equivalent to legacy _handle_anthropic :695-702 + :714-718."""
    body = prepare_inbound_messages_request(body)
    src = body.get("model", "?")
    target = provider_policy.resolve_model(src, state)
    rule_ids = []
    if getattr(state, "model_registry", None) is not None:
        _append_rule_id(rule_ids, provider_policy.RULE_PROVIDER_VIRTUAL_MODEL_REGISTRY)
    elif (
        state.prov_name in ("relay", "openai-custom", "openai-responses")
        and state.policy.force_model_override
        and state.relay_force_model
    ):
        _append_rule_id(rule_ids, provider_policy.RULE_PROVIDER_RELAY_FORCE_MODEL_SHELL)
    if (
        state.prov_name == "relay"
        and state.relay_thinking == "enabled"
        and "kimi" in (target or "").lower()
    ):
        _append_rule_id(rule_ids, provider_policy.RULE_PROVIDER_KIMI_RELAY_THINKING_ENABLED)
    upstream = dict(body)
    upstream["model"] = target
    # Relay (Anthropic-native) path: strip orphan tool_use/tool_result blocks
    # that pollute multi-turn conversations after failed tool calls (Kimi K3 fix).
    if state.prov_name == "relay":
        upstream["messages"] = _strip_orphan_tool_blocks(upstream.get("messages", []))
    if upstream.get("max_tokens"):
        upstream["max_tokens"] = provider_policy.clamp_max_tokens(
            upstream["max_tokens"], target, state)
    provider_policy.normalize_thinking(
        upstream,
        state.prov_name,
        state.relay_thinking,
        rule_ids=rule_ids,
    )
    _filter_upstream_tools(upstream, target, state.prov_name, rule_ids)
    known_tools = {t["name"]: (t.get("input_schema") or {})
                   for t in (body.get("tools") or [])
                   if isinstance(t, dict) and t.get("name")}
    ctx = Ctx(src_model=src, target_model=target, known_tools=known_tools,
              nonce=state.nonce_factory(), shim_mode=state.shim_mode,
              provider=state.prov_name, rule_ids=tuple(rule_ids))
    return upstream, ctx


def _shim_on(ctx):
    return ctx.shim_mode in ("detect", "rewrite") and bool(ctx.known_tools)


def rewrite_nonstream(body_bytes, ctx):
    """(body_bytes, Ctx) -> (body_bytes, stats). Equivalent to legacy :771-780.
    off / no tools: (original bytes, {}); detect: (original bytes, {"found": bool});
    rewrite: (rewritten bytes, {"rewritten": bool})."""
    if not _shim_on(ctx):
        return body_bytes, {}
    if ctx.shim_mode == "rewrite":
        new = dsml_shim.rewrite_nonstream_body(body_bytes, ctx.known_tools, nonce=ctx.nonce)
        return new, {"rewritten": new != body_bytes}
    det = dsml_shim.DsmlDetector()
    det.feed(body_bytes)
    return body_bytes, {"found": det.found}


class _RewriteFilter:
    """Streaming filter for rewrite mode: wraps DsmlStreamRewriter with feed/finalize/stats."""

    def __init__(self, known_tools, nonce):
        self._rw = dsml_shim.DsmlStreamRewriter(known_tools, nonce=nonce)

    def feed(self, chunk):
        return self._rw.feed(chunk)

    def finalize(self):
        return self._rw.finalize()

    def stats(self):
        return {"synthesized": self._rw.synthesized, "tool_n": self._rw.tool_n}


class _DetectFilter:
    """Streaming filter for detect mode: passthrough bytes + internal stats."""

    def __init__(self):
        self._det = dsml_shim.DsmlDetector()

    def feed(self, chunk):
        self._det.feed(chunk)
        return chunk

    def finalize(self):
        return b""

    def stats(self):
        return {"found": self._det.found}


class _KimiServerToolFilter:
    """Drop Kimi server-tool SSE blocks that the local client cannot consume.

    Kimi may emit Anthropic server-tool blocks (currently web search) even when CSP
    does not advertise that tool upstream. The client-tool path expects ordinary
    content blocks with contiguous indexes, so we remove those blocks and compact indexes.
    """

    _DROP_TYPES = {"server_tool_use", "web_search_tool_result"}

    def __init__(self):
        self._buf = b""
        self._skip = set()
        self._index_map = {}
        self._next_index = 0
        self._dropped = 0

    @staticmethod
    def _split_frame(buf):
        candidates = [(buf.find(b"\n\n"), 2), (buf.find(b"\r\n\r\n"), 4)]
        candidates = [(i, n) for i, n in candidates if i >= 0]
        if not candidates:
            return None, None, buf
        i, n = min(candidates, key=lambda x: x[0])
        return buf[:i], buf[i:i + n], buf[i + n:]

    @staticmethod
    def _event_and_data(frame):
        event = None
        data = []
        for line in frame.replace(b"\r\n", b"\n").split(b"\n"):
            if line.startswith(b"event:"):
                event = line.split(b":", 1)[1].strip()
            elif line.startswith(b"data:"):
                data.append(line.split(b":", 1)[1].lstrip())
        return event, b"\n".join(data)

    @staticmethod
    def _render(event, obj):
        data = json.dumps(obj, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if event:
            return b"event: " + event + b"\n" + b"data: " + data + b"\n\n"
        return b"data: " + data + b"\n\n"

    def _mapped_index(self, idx):
        if idx not in self._index_map:
            self._index_map[idx] = self._next_index
            self._next_index += 1
        return self._index_map[idx]

    def _rewrite_frame(self, frame, sep):
        event, data = self._event_and_data(frame)
        if not data:
            return frame + sep
        try:
            obj = json.loads(data.decode("utf-8"))
        except Exception:
            return frame + sep
        if not isinstance(obj, dict):
            return frame + sep

        t = obj.get("type")
        if t == "content_block_start":
            idx = obj.get("index")
            block = obj.get("content_block") if isinstance(obj.get("content_block"), dict) else {}
            if block.get("type") in self._DROP_TYPES:
                self._skip.add(idx)
                self._dropped += 1
                return b""
            obj = dict(obj)
            obj["index"] = self._mapped_index(idx)
            return self._render(event, obj)
        if t in ("content_block_delta", "content_block_stop"):
            idx = obj.get("index")
            if idx in self._skip:
                return b""
            if idx in self._index_map:
                obj = dict(obj)
                obj["index"] = self._index_map[idx]
                return self._render(event, obj)
        return frame + sep

    def feed(self, chunk):
        self._buf += chunk
        out = []
        while True:
            frame, sep, rest = self._split_frame(self._buf)
            if frame is None:
                break
            self._buf = rest
            out.append(self._rewrite_frame(frame, sep))
        return b"".join(out)

    def finalize(self):
        if not self._buf:
            return b""
        frame = self._buf
        self._buf = b""
        return self._rewrite_frame(frame, b"\n\n")

    def stats(self):
        out = {"dropped_kimi_server_tool_blocks": self._dropped}
        if self._dropped:
            out["rule_ids"] = [provider_policy.RULE_TOOL_KIMI_WEB_SEARCH_SERVER_TOOL_FILTER]
        return out


class _PipelineFilter:
    def __init__(self, filters):
        self._filters = filters

    def feed(self, chunk):
        out = chunk
        for f in self._filters:
            out = f.feed(out)
        return out

    def finalize(self):
        out = b""
        for f in self._filters:
            out = f.feed(out) + f.finalize()
        return out

    def stats(self):
        out = {}
        for f in self._filters:
            stats = f.stats()
            if "rule_ids" in stats:
                seen = out.setdefault("rule_ids", [])
                for rule_id in stats["rule_ids"]:
                    if rule_id not in seen:
                        seen.append(rule_id)
                stats = {k: v for k, v in stats.items() if k != "rule_ids"}
            out.update(stats)
        return out


def make_stream_rewriter(ctx):
    """(Ctx) -> stream_filter | None. off / no tools → None (skeleton passthrough, zero overhead).
    Filter API: feed(chunk)->bytes / finalize()->bytes / stats(). Equivalent to legacy :735-737."""
    filters = []
    if ctx.provider == "relay" and "kimi" in (ctx.target_model or "").lower():
        filters.append(_KimiServerToolFilter())
    if _shim_on(ctx):
        if ctx.shim_mode == "rewrite":
            filters.append(_RewriteFilter(ctx.known_tools, ctx.nonce))
        else:
            filters.append(_DetectFilter())
    if not filters:
        return None
    if len(filters) == 1:
        return filters[0]
    return _PipelineFilter(filters)
