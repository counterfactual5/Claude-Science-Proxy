"""OpenAI Chat Completions compatibility helpers."""

import json

from proxy.policy import provider_policy

# GLM's OpenAI-compat endpoint 400s with `code 1210` ("API 调用参数有误") on
# pathologically large single message bodies (e.g. a tool_result that dumps an
# entire file/log). This is a soft, rarely-triggered safety net, not a token
# budgeter — real context-window limits are enforced upstream independently.
_MAX_SINGLE_TEXT_CHARS = 200_000

# Anthropic extended-thinking blocks never own a `tool_use_id`/`id` that any
# pairing logic depends on, and GLM's OpenAI-compat schema has no field for
# them, so dropping them outright (no placeholder) is intentionally silent
# and safe — unlike image/server-tool blocks below, they can't desync
# tool_call <-> tool_result matching.
_SILENT_DROP_BLOCK_TYPES = {"thinking", "redacted_thinking"}


def _cap_text(s):
    if isinstance(s, str) and len(s) > _MAX_SINGLE_TEXT_CHARS:
        overflow = len(s) - _MAX_SINGLE_TEXT_CHARS
        return s[:_MAX_SINGLE_TEXT_CHARS] + f"\n...[truncated {overflow} chars]"
    return s


def _placeholder_for_block(blk):
    """Textual stand-in for an Anthropic content block this translation can't carry.

    Anthropic block types with no OpenAI-compat equivalent (`image`,
    `server_tool_use`, `web_search_tool_result`, and any future type) used to be
    silently skipped. If such a block sat between a `tool_use` and its
    `tool_result` in the original transcript, silently vanishing it could look
    to a naive reader (or a later drift in this code) like the surrounding
    tool_use/tool_result pairing had shifted. Keep a short marker instead so the
    message body is never emptied out from under an otherwise well-formed turn.
    """
    t = blk.get("type") or "unknown"
    if t == "image":
        return "[image omitted: not representable in this OpenAI-compat translation]"
    if t in ("server_tool_use", "web_search_tool_result"):
        name = blk.get("name") or t
        return f"[{name} block omitted: server-side tool not exposed via this translation]"
    return f"[{t} block omitted: unsupported Anthropic content type]"


def _stringify_tool_result_content(content):
    """Flatten Anthropic `tool_result.content` into the bare string GLM requires.

    GLM's OpenAI-compat endpoint only accepts a plain string for a `tool`
    message's `content`; a list or dict body (Anthropic tool_result content can
    be a list of text/image blocks) is a common 1210 trigger. Every branch here
    always returns a `str`, never a list/dict, and non-text nested blocks (e.g.
    images embedded in a tool's own output) get a placeholder instead of being
    dropped, so multi-block tool results don't silently collapse to "".
    """
    if content is None:
        return ""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                if item.get("type") == "text":
                    parts.append(item.get("text") or "")
                elif item.get("type") == "image":
                    parts.append("[image omitted: not representable in this OpenAI-compat translation]")
                else:
                    # Unrecognized nested block: keep a textual trace (its raw
                    # JSON) rather than letting it vanish and shrink the result.
                    parts.append(json.dumps(item, ensure_ascii=False))
            elif item is not None:
                parts.append(str(item))
        return "".join(parts)
    if isinstance(content, dict):
        if content.get("type") == "text":
            return content.get("text") or ""
        return json.dumps(content, ensure_ascii=False)
    return str(content)


def map_tool_choice(tc, tools):
    if not isinstance(tc, dict):
        return None
    t = tc.get("type")
    if t == "auto":
        return "auto"
    if t == "none":
        return "none"
    if t == "tool" and tc.get("name"):
        return {"type": "function", "function": {"name": tc["name"]}}
    if t == "any":
        names = [x["name"] for x in (tools or []) if x.get("name")]
        if len(names) == 1:
            return {"type": "function", "function": {"name": names[0]}}
        return "required"
    return None


def anthropic_to_openai(req, state):
    msgs = []
    sys_prompt = req.get("system")
    if isinstance(sys_prompt, list):
        sys_prompt = "\n".join(
            (b.get("text") or "") if isinstance(b, dict) else (b if isinstance(b, str) else "")
            for b in sys_prompt
        )
    if sys_prompt:
        msgs.append({"role": "system", "content": sys_prompt})

    # Running set of tool_call ids GLM has actually seen declared in a
    # preceding assistant `tool_calls[]` array, plus that array's own order.
    # Long-session replay after Science compacts/summarizes history can leave
    # a `tool_result` whose matching `tool_use` fell outside the replayed
    # window ("tools=25/26"-style drift is a symptom of the same class of
    # mismatch upstream). Emitting such an orphan as a `role: tool` message
    # is a reliable 1210 trigger because its tool_call_id matches nothing in
    # the immediately preceding assistant turn, so we re-file it as plain
    # text instead of a dangling tool message.
    known_tool_ids = set()
    last_call_order = []

    for m in req.get("messages", []):
        role = m.get("role")
        content = m.get("content")
        if isinstance(content, str):
            msgs.append({"role": role, "content": content})
            continue
        text_parts, tool_calls, tool_results, orphan_parts = [], [], [], []
        for blk in content or []:
            if not isinstance(blk, dict):
                continue
            t = blk.get("type")
            if t == "text":
                text_parts.append(blk.get("text") or "")
            elif t == "tool_use":
                # `function.arguments` must be a JSON *string* per OpenAI schema —
                # a bare dict here is another common 1210 cause. Also guard a
                # missing/non-dict `input` (`None` would otherwise serialize to
                # the literal string "null", which most tool-callers can't parse).
                raw_input = blk.get("input")
                if not isinstance(raw_input, dict):
                    raw_input = {}
                call_id = blk.get("id") or f"call_missing_id_{len(tool_calls)}"
                tool_calls.append({
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": blk.get("name") or "unknown_tool",
                        "arguments": json.dumps(raw_input, ensure_ascii=False),
                    },
                })
            elif t == "tool_result":
                call_id = blk.get("tool_use_id")
                text = _cap_text(_stringify_tool_result_content(blk.get("content")))
                if blk.get("is_error"):
                    text = "[ERROR] " + text if text else "[ERROR]"
                if call_id and call_id in known_tool_ids:
                    tool_results.append({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": text,
                    })
                else:
                    label = call_id or "unknown"
                    orphan_parts.append(
                        f"[tool_result for {label} (unmatched tool_call, "
                        f"omitted from replay)]: {text}"
                    )
            elif t in _SILENT_DROP_BLOCK_TYPES:
                continue
            else:
                placeholder = _placeholder_for_block(blk)
                if placeholder:
                    text_parts.append(placeholder)
        if role == "assistant" and tool_calls:
            # GLM / several OpenAI-compat gateways reject content:null with tool_calls
            # (HTTP 400 / "API 调用参数有误"). Prefer empty string over null.
            msgs.append({
                "role": "assistant",
                "content": _cap_text("".join(text_parts)),
                "tool_calls": tool_calls,
            })
            known_tool_ids.update(tc["id"] for tc in tool_calls)
            last_call_order = [tc["id"] for tc in tool_calls]
        elif tool_results or orphan_parts:
            if tool_results and last_call_order:
                # Preserve the assistant's original tool_calls order — GLM can be
                # strict about `tool` messages arriving in the same order the
                # calls were issued, and Anthropic's own content-block order is
                # not guaranteed to match after any upstream reordering/retries.
                order_index = {cid: i for i, cid in enumerate(last_call_order)}
                tool_results.sort(key=lambda tr: order_index.get(tr["tool_call_id"], len(order_index)))
            msgs.extend(tool_results)
            leftover = _cap_text("".join(text_parts))
            if orphan_parts:
                leftover = (leftover + "\n" if leftover else "") + "\n".join(orphan_parts)
            if leftover:
                msgs.append({"role": role, "content": leftover})
        else:
            msgs.append({"role": role, "content": _cap_text("".join(text_parts))})
    out = {
        "model": provider_policy.resolve_model(req.get("model"), state),
        "messages": msgs,
        "stream": False,
    }
    if req.get("max_tokens"):
        out["max_tokens"] = provider_policy.clamp_max_tokens(
            req["max_tokens"],
            out["model"],
            state,
        )
    if req.get("temperature") is not None:
        out["temperature"] = req["temperature"]
    if req.get("tools"):
        out["tools"] = [{
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t.get("description", ""),
                "parameters": t.get("input_schema", {}),
            },
        } for t in req["tools"] if t.get("name")]
    tcm = map_tool_choice(req.get("tool_choice"), req.get("tools"))
    if tcm is not None:
        out["tool_choice"] = tcm
    if req.get("stop_sequences"):
        out["stop"] = req["stop_sequences"]
    if req.get("top_p") is not None:
        out["top_p"] = req["top_p"]
    return out


def openai_to_anthropic(resp, model_id):
    choice = (resp.get("choices") or [{}])[0]
    msg = choice.get("message", {})
    blocks = []
    if msg.get("content"):
        blocks.append({"type": "text", "text": msg["content"]})
    for tc in msg.get("tool_calls") or []:
        fn = tc.get("function", {})
        try:
            args = json.loads(fn.get("arguments") or "{}")
        except Exception:
            args = {}
        blocks.append({
            "type": "tool_use",
            "id": tc.get("id"),
            "name": fn.get("name"),
            "input": args,
        })
    fr = choice.get("finish_reason")
    stop = {"stop": "end_turn", "length": "max_tokens", "tool_calls": "tool_use"}.get(
        fr,
        "end_turn",
    )
    usage = resp.get("usage", {})
    return {
        "id": resp.get("id", "msg_proxy"),
        "type": "message",
        "role": "assistant",
        "model": model_id,
        "content": blocks or [{"type": "text", "text": ""}],
        "stop_reason": stop,
        "stop_sequence": None,
        "usage": {
            "input_tokens": usage.get("prompt_tokens", 0),
            "output_tokens": usage.get("completion_tokens", 0),
        },
    }
