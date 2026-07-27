"""Kimi / relay multi-turn history repair + fail-closed validation.

Soft-repairs Science-replayed orphan tool blocks, drops zero-information
placeholders, then refuses upstream POST when thinking signatures or tool
pairing remain inconsistent (avoids Kimi ``temporarily unavailable`` loops).
"""
from __future__ import annotations

import json

from proxy.policy import provider_policy

_MAX_KIMI_THINKING_BYTES = 1024 * 1024
_MAX_KIMI_SIGNATURE_BYTES = 64 * 1024
_MAX_RELAY_HISTORY_MESSAGES = 4096
_MAX_RELAY_HISTORY_BLOCKS = 4096


class RelayHistoryError(ValueError):
    """Fail-closed relay history / Kimi thinking validation before upstream POST."""


def _append_rule_id(rule_ids, rule_id):
    if rule_ids is not None and rule_id not in rule_ids:
        rule_ids.append(rule_id)


def kimi_signature_structurally_valid(signature):
    """Vendor thinking signatures must be non-empty and free of control/whitespace."""
    if not isinstance(signature, str) or not signature:
        return False
    if len(signature) > _MAX_KIMI_SIGNATURE_BYTES:
        return False
    return not any(ch.isspace() or ord(ch) < 32 for ch in signature)


def _zero_information_kimi_block(block):
    if not isinstance(block, dict):
        return False
    t = block.get("type")
    if t in ("server_tool_use", "web_search_tool_result"):
        return True
    if t == "text":
        return block.get("text") == ""
    if t == "thinking":
        return (block.get("thinking") or "") == "" and not (block.get("signature") or "")
    return False


def _message_has_no_information(message):
    if not isinstance(message, dict):
        return True
    content = message.get("content")
    if content is None:
        return True
    if isinstance(content, str):
        return content == ""
    if isinstance(content, list):
        return len(content) == 0
    return False


def normalize_kimi_failed_history_tail(messages, rule_ids=None):
    """Drop zero-info placeholders and empty failed assistant tails (Kimi only)."""
    if not isinstance(messages, list):
        return messages
    if len(messages) > _MAX_RELAY_HISTORY_MESSAGES:
        raise RelayHistoryError("Kimi history has too many messages")
    changed = False
    for message in messages:
        if not isinstance(message, dict) or message.get("role") != "assistant":
            continue
        content = message.get("content")
        if not isinstance(content, list):
            continue
        if len(content) > _MAX_RELAY_HISTORY_BLOCKS:
            raise RelayHistoryError("Kimi history message has too many content blocks")
        before = len(content)
        kept = [b for b in content if not _zero_information_kimi_block(b)]
        if len(kept) != before:
            message["content"] = kept
            changed = True

    if (
        messages
        and isinstance(messages[-1], dict)
        and messages[-1].get("role") == "assistant"
        and _message_has_no_information(messages[-1])
    ):
        messages.pop()
        changed = True
    if (
        len(messages) >= 2
        and isinstance(messages[-1], dict)
        and messages[-1].get("role") == "user"
        and isinstance(messages[-2], dict)
        and messages[-2].get("role") == "assistant"
        and _message_has_no_information(messages[-2])
    ):
        messages.pop(-2)
        changed = True
    if changed and not messages:
        raise RelayHistoryError("Kimi history is empty after removing a failed placeholder")
    if changed:
        _append_rule_id(rule_ids, provider_policy.RULE_HISTORY_KIMI_FAILED_TAIL_NORMALIZE)
    return messages


def validate_kimi_thinking_in_history(messages):
    """Fail-closed: nonempty thinking without a structurally valid signature."""
    if not isinstance(messages, list):
        return
    for message in messages:
        if not isinstance(message, dict):
            continue
        content = message.get("content")
        if not isinstance(content, list):
            continue
        for block in content:
            if not isinstance(block, dict) or block.get("type") != "thinking":
                continue
            thinking = block.get("thinking") or ""
            signature = block.get("signature") or ""
            if not isinstance(thinking, str) or not isinstance(signature, str):
                raise RelayHistoryError("Kimi thinking block fields are invalid")
            if len(thinking) > _MAX_KIMI_THINKING_BYTES:
                raise RelayHistoryError("Kimi thinking content is too large")
            if len(signature) > _MAX_KIMI_SIGNATURE_BYTES:
                raise RelayHistoryError("Kimi thinking signature is too large")
            has_sig = kimi_signature_structurally_valid(signature)
            if thinking and not has_sig:
                raise RelayHistoryError(
                    "Kimi nonempty thinking has no valid signature "
                    "(history may be tampered — refusing upstream POST)"
                )


def validate_relay_tool_history(messages):
    """Fail-closed tool_use / tool_result pairing after soft repair."""
    if not isinstance(messages, list):
        raise RelayHistoryError("relay history messages must be a list")
    seen = set()
    pending = set()
    for message in messages:
        if not isinstance(message, dict):
            raise RelayHistoryError("relay history message is invalid")
        role = message.get("role")
        if role not in ("user", "assistant"):
            raise RelayHistoryError("relay history message role is invalid")
        content = message.get("content")
        if isinstance(content, list):
            blocks = content
        elif content is None or isinstance(content, str):
            blocks = []
        else:
            raise RelayHistoryError("relay history message content is invalid")
        if len(blocks) > _MAX_RELAY_HISTORY_BLOCKS:
            raise RelayHistoryError("relay history message has too many content blocks")
        if role == "assistant":
            if pending:
                raise RelayHistoryError(
                    "relay history has unresolved tool calls before an assistant turn"
                )
            for block in blocks:
                if not isinstance(block, dict):
                    continue
                t = block.get("type")
                if t == "tool_result":
                    raise RelayHistoryError(
                        "relay history tool_result must have the user role"
                    )
                if t != "tool_use":
                    continue
                tid = block.get("id")
                name = block.get("name")
                if not isinstance(tid, str) or not tid or len(tid) > 256:
                    raise RelayHistoryError("relay history tool_use id is invalid")
                if not isinstance(name, str) or not name or len(name) > 256:
                    raise RelayHistoryError("relay history tool_use name is invalid")
                if not isinstance(block.get("input"), dict):
                    raise RelayHistoryError("relay history tool_use input is invalid")
                if tid in seen:
                    raise RelayHistoryError(
                        "relay history contains a duplicate tool_use id"
                    )
                seen.add(tid)
                pending.add(tid)
        else:
            results = set()
            for block in blocks:
                if not isinstance(block, dict):
                    continue
                t = block.get("type")
                if t == "tool_use":
                    raise RelayHistoryError(
                        "relay history tool_use must have the assistant role"
                    )
                if t != "tool_result":
                    continue
                tid = block.get("tool_use_id")
                if not isinstance(tid, str) or not tid or len(tid) > 256:
                    raise RelayHistoryError("relay history tool_result id is invalid")
                if tid not in pending or tid in results:
                    raise RelayHistoryError(
                        "relay history contains an orphan or duplicate tool_result"
                    )
                results.add(tid)
            if pending:
                if results != pending:
                    raise RelayHistoryError("relay history has incomplete tool results")
                pending.clear()
            elif results:
                raise RelayHistoryError("relay history contains an orphan tool_result")
    if pending:
        raise RelayHistoryError("relay history ends with unresolved tool calls")


def strip_orphan_tool_blocks(messages):
    """Soft-repair orphan tool_use/tool_result blocks before fail-closed validation."""
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
                        "text": (
                            f"[tool_result for {label} (orphaned, "
                            f"omitted from replay)]: {text}"
                        ),
                    })
            else:
                new_blocks.append(blk)
        msg["content"] = new_blocks

    # Strip unresolved tool_use rounds (failed / interrupted / compacted).
    i = 0
    while i < len(messages):
        msg = messages[i]
        if not (isinstance(msg, dict) and msg.get("role") == "assistant"):
            i += 1
            continue
        content = msg.get("content")
        if not isinstance(content, list):
            i += 1
            continue
        tool_ids = [
            b.get("id")
            for b in content
            if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("id")
        ]
        if not tool_ids:
            i += 1
            continue
        resolved = set()
        j = i + 1
        while j < len(messages):
            nxt = messages[j]
            if not isinstance(nxt, dict):
                j += 1
                continue
            if nxt.get("role") == "assistant":
                break
            if nxt.get("role") == "user":
                nc = nxt.get("content")
                if isinstance(nc, list):
                    for b in nc:
                        if isinstance(b, dict) and b.get("type") == "tool_result":
                            resolved.add(b.get("tool_use_id"))
            j += 1
        unresolved = set(tool_ids) - resolved
        if unresolved:
            filtered = [
                b
                for b in content
                if not (
                    isinstance(b, dict)
                    and b.get("type") == "tool_use"
                    and b.get("id") in unresolved
                )
            ]
            msg["content"] = filtered or [{"type": "text", "text": ""}]
        i += 1

    if messages:
        last = messages[-1]
        if isinstance(last, dict) and last.get("role") == "assistant":
            content = last.get("content")
            if isinstance(content, list):
                filtered = [
                    blk
                    for blk in content
                    if not (isinstance(blk, dict) and blk.get("type") == "tool_use")
                ]
                last["content"] = filtered or [{"type": "text", "text": ""}]
    return messages


def prepare_relay_messages(messages, target_model, rule_ids=None):
    """Soft-repair then fail-closed validate relay (esp. Kimi) multi-turn history."""
    # Copy shallow message dicts so we don't mutate the caller's request body.
    if isinstance(messages, list):
        messages = [
            (dict(m) if isinstance(m, dict) else m) for m in messages
        ]
        for m in messages:
            if isinstance(m, dict) and isinstance(m.get("content"), list):
                m["content"] = list(m["content"])
    messages = strip_orphan_tool_blocks(messages)
    is_kimi = "kimi" in (target_model or "").lower()
    if is_kimi:
        messages = normalize_kimi_failed_history_tail(messages, rule_ids)
        validate_kimi_thinking_in_history(messages)
    validate_relay_tool_history(messages)
    return messages


def filter_kimi_nonstream_response(body_bytes):
    """Drop zero-info thinking; fail-closed on nonempty unsigned thinking."""
    try:
        obj = json.loads(body_bytes)
    except Exception as e:
        raise RelayHistoryError(
            f"Kimi nonstream response is not valid JSON: {e}"
        ) from e
    content = obj.get("content")
    if not isinstance(content, list):
        raise RelayHistoryError("Kimi nonstream response content is invalid")
    if len(content) > _MAX_RELAY_HISTORY_BLOCKS:
        raise RelayHistoryError(
            "Kimi nonstream response has too many content blocks"
        )
    kept = []
    for block in content:
        if not isinstance(block, dict):
            raise RelayHistoryError("Kimi nonstream content block is invalid")
        if block.get("type") != "thinking":
            kept.append(block)
            continue
        thinking = block.get("thinking") or ""
        signature = block.get("signature") or ""
        if not isinstance(thinking, str) or not isinstance(signature, str):
            raise RelayHistoryError("Kimi thinking block fields are invalid")
        if len(thinking) > _MAX_KIMI_THINKING_BYTES:
            raise RelayHistoryError("Kimi thinking content is too large")
        if len(signature) > _MAX_KIMI_SIGNATURE_BYTES:
            raise RelayHistoryError("Kimi thinking signature is too large")
        has_sig = kimi_signature_structurally_valid(signature)
        if not thinking and not has_sig:
            continue
        if thinking and not has_sig:
            raise RelayHistoryError(
                "Kimi nonempty thinking has no valid signature"
            )
        kept.append(block)
    obj["content"] = kept
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
