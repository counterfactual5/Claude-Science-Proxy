"""CSP DSML fallback shim: recover tool_use when DeepSeek leaks DSML markers as plain text.
Pure-function segmenter (this file) + stream state machine + byte detector. No third-party deps."""
import codecs
import json
import os
import re

DSML_MARKER_BYTES = (
    "｜DSML｜".encode("utf-8"),
    "｜｜DSML｜｜".encode("utf-8"),
)


def shim_mode(prov_name, prov):
    """off | detect | rewrite. Relay is always off this round (deepseek-only); reads env only when
    deepseek and dsml_capable."""
    if prov_name == "relay":
        return "off"
    if not (prov or {}).get("dsml_capable"):
        return "off"
    m = os.environ.get("CSP_TOOLUSE_SHIM", "").lower()
    return m if m in ("detect", "rewrite") else "off"


class DsmlDetector:
    """detect mode: only decides whether DSML leak markers appear in this response; does not change
    a single byte. Phase-one telemetry (detection rate stats; no disk writes, no rewrite, no fix
    claims). Small tail buffer across chunks to avoid misses."""

    _K = max(len(m) for m in DSML_MARKER_BYTES)   # byte length of the longest marker

    def __init__(self):
        self.found = False
        self._tail = b""

    def feed(self, data):
        if self.found or not data:
            return
        buf = self._tail + data
        if any(mk in buf for mk in DSML_MARKER_BYTES):
            self.found = True
            self._tail = b""
            return
        # Keep only trailing bytes that might be half a marker, for cross-chunk matching.
        self._tail = buf[-(self._K - 1):] if len(buf) >= self._K else buf

# Delimiter: one or two fullwidth vertical bars U+FF5C (vLLM docs use one; issue #8 observed two).
_P = r"[｜]{1,2}"
_WRAP = r"(?:tool_calls|function_calls)"
_OPEN_RE = re.compile(r"<" + _P + r"DSML" + _P + _WRAP + r">")
_TOOLCALLS_RE = re.compile(
    r"<" + _P + r"DSML" + _P + _WRAP + r">(.*?)</" + _P + r"DSML" + _P + _WRAP + r">", re.S)
_INVOKE_RE = re.compile(
    r"<" + _P + r'DSML' + _P + r'invoke\s+name="([^"]+)"\s*>(.*?)</' + _P + r"DSML" + _P + r"invoke>",
    re.S)
_PARAM_RE = re.compile(
    r"<" + _P + r'DSML' + _P + r'parameter\s+name="([^"]+)"(?:\s+string="(true|false)")?\s*>'
    + r"(.*?)</" + _P + r"DSML" + _P + r"parameter>", re.S)


def _coerce_param(pname, string_attr, raw, prop_schema):
    """string="true" → raw string; string="false"/missing → coerce per schema type; on failure fall
    back to json.loads, then to string."""
    if string_attr == "true":
        return raw
    typ = (prop_schema or {}).get("type")
    if typ == "string":
        return raw
    try:
        if typ == "integer":
            return int(raw)
        if typ == "number":
            return float(raw)
        if typ == "boolean":
            low = raw.strip().lower()
            if low in ("true", "1", "yes"):
                return True
            if low in ("false", "0", "no"):
                return False
            # Unknown boolean literal (e.g. "maybe"): do not assume False; keep raw string,
            # let _type_ok reject → _validate_input returns False → discard whole segment
            # (conservative: prefer treating as plain text).
            return raw
        if typ in ("object", "array"):
            return json.loads(raw)
    except (ValueError, TypeError, json.JSONDecodeError):
        pass
    try:
        return json.loads(raw)
    except (ValueError, TypeError, json.JSONDecodeError):
        return raw


def _type_ok(val, typ):
    """Loose base-type check: only reject obvious conflicts (round 3 P2)."""
    if typ in (None, "string"):
        return isinstance(val, str) if typ == "string" else True
    if typ == "integer":
        return (isinstance(val, int) and not isinstance(val, bool)) or \
               (isinstance(val, str) and val.strip().lstrip("+-").isdigit())
    if typ == "number":
        if isinstance(val, bool):
            return False
        if isinstance(val, (int, float)):
            return True
        try:
            float(val)
            return True
        except (ValueError, TypeError):
            return False
    if typ == "boolean":
        return isinstance(val, bool) or (isinstance(val, str)
                and val.strip().lower() in ("true", "false", "1", "0", "yes", "no"))
    if typ == "object":
        return isinstance(val, dict)
    if typ == "array":
        return isinstance(val, list)
    return True


def _validate_input(inp, schema):
    """All required fields present + base types compatible; returns False on failure (caller treats
    whole segment as text)."""
    schema = schema or {}
    for req in schema.get("required") or []:
        if req not in inp:
            return False
    props = schema.get("properties") or {}
    for k, v in inp.items():
        if k in props and not _type_ok(v, props[k].get("type")):
            return False
    return True


def _parse_invoke(name, body, known_tools):
    """Parse one invoke → {"name","input"}; returns None if params fail schema (caller discards)."""
    schema = known_tools.get(name) or {}
    schema_props = schema.get("properties") or {}
    inp = {}
    for pn, sattr, raw in _PARAM_RE.findall(body):
        inp[pn] = _coerce_param(pn, sattr, raw, schema_props.get(pn))
    # Wrapper unwrap: single param named arguments/input that is not a real tool field → unwrap object
    if len(inp) == 1:
        only = next(iter(inp))
        if only in ("arguments", "input") and only not in schema_props:
            val = inp[only]
            if isinstance(val, str):
                try:
                    val = json.loads(val)
                except (ValueError, json.JSONDecodeError):
                    val = None
            if isinstance(val, dict):
                inp = val
    if not _validate_input(inp, schema):
        return None
    return {"name": name, "input": inp}


def parse_dsml_tool_calls(wrapper_region, known_tools):
    """Parse a tool_calls region. Any undeclared tool name or schema mismatch → [] (discard whole)."""
    known_tools = known_tools or {}
    out = []
    for m in _TOOLCALLS_RE.finditer(wrapper_region):
        invokes = _INVOKE_RE.findall(m.group(1))
        if not invokes:
            return []
        for name, body in invokes:
            if name not in known_tools:
                return []
            call = _parse_invoke(name, body, known_tools)
            if call is None:      # schema mismatch → discard whole segment
                return []
            out.append(call)
    return out


def segment_dsml_text(text, known_tools):
    """Split text into ordered segments by DSML tool_calls regions, preserving interleaving. No DSML
    → single text segment."""
    if not text:
        return []
    known_tools = known_tools or {}
    segs = []
    pos = 0
    for m in _TOOLCALLS_RE.finditer(text):
        calls = parse_dsml_tool_calls(m.group(0), known_tools)
        if not calls:
            continue           # unknown tool / bad format: do not split; keep as text below
        if m.start() > pos:
            segs.append({"type": "text", "text": text[pos:m.start()]})
        for c in calls:
            segs.append({"type": "tool_use", "name": c["name"], "input": c["input"]})
        pos = m.end()
    if pos < len(text):
        tail = text[pos:]
        if tail:
            segs.append({"type": "text", "text": tail})
    if not segs:
        return [{"type": "text", "text": text}]
    return segs


class DsmlStreamRewriter:
    """Streaming SSE rewrite state machine. Task 4: transparent remapping (own downstream indexes,
    generic delta/stop mapping, incremental UTF-8). Task 5 adds DSML detection on text_delta and
    tool_use synthesis."""

    def __init__(self, known_tools, nonce=""):
        self.known_tools = known_tools or {}
        self.nonce = nonce or "x"
        self._dec = codecs.getincrementaldecoder("utf-8")()
        self._buf = ""            # decoded text not yet framed
        self.next_out = 0
        self.cur_out = None       # index of the currently open downstream block
        self.cur_type = None      # type of the current upstream block
        self.synthesized = False
        self.tool_n = 0
        # Task 5:
        self.state = "PASS"
        self.scan_buf = ""
        self.cap_buf = ""

    # ---- public ----
    def feed(self, data):
        self._buf += self._dec.decode(data)
        return self._drain_frames()

    def finalize(self):
        # Flush decoder residue + unframed tail + Task 5 held-back text
        self._buf += self._dec.decode(b"", final=True)
        out = self._drain_frames(flush_tail=True)
        out += self._finalize_text()      # Task 5 override; Task 4 returns b""
        return out

    # ---- frame loop ----
    def _drain_frames(self, flush_tail=False):
        out = []
        while True:
            i_lf = self._buf.find("\n\n")
            i_crlf = self._buf.find("\r\n\r\n")
            cands = [(i, s) for i, s in ((i_lf, 2), (i_crlf, 4)) if i >= 0]
            if not cands:
                break
            idx, sep = min(cands)
            frame = self._buf[:idx]
            self._buf = self._buf[idx + sep:]
            out.append(self._handle_frame(frame))
        # On finalize: treat the last upstream frame without a trailing blank line (sudden EOF)
        # as complete, or message_stop / trailing deltas are silently dropped (Codex P1).
        if flush_tail and self._buf.strip():
            frame = self._buf
            self._buf = ""
            out.append(self._handle_frame(frame))
        return b"".join(out)

    # ---- per-frame handling ----
    def _handle_frame(self, frame):
        event, obj = self._parse_frame(frame)
        if obj is None or not isinstance(obj, dict):
            return self._raw(frame)              # comment/unknown/non-JSON: passthrough
        t = obj.get("type")
        if t == "content_block_start":
            self.cur_type = (obj.get("content_block") or {}).get("type")
            self.cur_out = self.next_out
            self.next_out += 1
            return self._emit("content_block_start",
                              {**obj, "index": self.cur_out})
        if t == "content_block_delta":
            dtype = (obj.get("delta") or {}).get("type")
            if self.cur_type == "text" and dtype == "text_delta":
                return self._on_text_delta(obj.get("delta", {}).get("text", ""))
            return self._emit("content_block_delta", {**obj, "index": self.cur_out})
        if t == "content_block_stop":
            return self._on_block_stop()
        if t == "message_delta":
            return self._flush_pending() + self._on_message_delta(obj)
        if t == "message_stop":
            return self._flush_pending() + self._raw(frame)
        # message_start / ping / other: passthrough
        return self._raw(frame)

    # Max possible opening-marker char count (<｜｜DSML｜｜function_calls>), for PASS holdback.
    _MAX_OPEN = len("<｜｜DSML｜｜function_calls>")
    _CAP = 256 * 1024

    def _on_text_delta(self, text):
        out = []
        if self.state == "PASS":
            self.scan_buf += text
            out.append(self._pass_scan())
        else:
            self.cap_buf += text
            out.append(self._capture_scan())
        return b"".join(out)

    def _pass_scan(self):
        out = []
        while True:
            m = _OPEN_RE.search(self.scan_buf)
            if m:
                before = self.scan_buf[:m.start()]
                if before:
                    out.append(self._text_delta(before))
                # Close the current text block
                if self.cur_out is not None:
                    out.append(self._emit("content_block_stop",
                              {"type": "content_block_stop", "index": self.cur_out}))
                    self.cur_out = None
                self.cap_buf = self.scan_buf[m.start():]   # includes OPEN for close-tag matching
                self.scan_buf = ""
                self.state = "CAPTURE"
                out.append(self._capture_scan())
                return b"".join(out)
            # No match: emit safe prefix, keep last _MAX_OPEN-1 chars as possible prefix
            keep = self._MAX_OPEN - 1
            if len(self.scan_buf) > keep:
                emit = self.scan_buf[:-keep]
                self.scan_buf = self.scan_buf[-keep:]
                if emit:
                    out.append(self._text_delta(emit))
            return b"".join(out)

    def _capture_scan(self):
        out = []
        cm = _TOOLCALLS_RE.search(self.cap_buf)
        if cm:
            calls = parse_dsml_tool_calls(cm.group(0), self.known_tools)
            if calls:
                for c in calls:
                    out.append(self._tool_use_events(c))
                self.synthesized = True
            else:
                # Unknown tool / bad format: treat whole region as literal text
                out.append(self._text_as_new_block(cm.group(0)))
            rest = self.cap_buf[cm.end():]
            self.cap_buf = ""
            self.state = "PASS"
            self.cur_out = None
            if rest:
                # Remainder back to PASS for more OPEN or plain text
                self.scan_buf = rest
                out.append(self._pass_scan())
            return b"".join(out)
        # No close tag: conservative fallback when over cap
        if len(self.cap_buf) > self._CAP:
            out.append(self._text_as_new_block(self.cap_buf))
            self.cap_buf = ""
            self.state = "PASS"
            self.cur_out = None
        return b"".join(out)

    def _finalize_text(self):
        # Finalize contract: after flush, must close any text block opened/still open
        # (emit content_block_stop), not only flush deltas.
        out = []
        if self.state == "CAPTURE" and self.cap_buf:
            out.append(self._text_as_new_block(self.cap_buf))   # opens + closes
            self.cap_buf = ""
            self.state = "PASS"
        if self.scan_buf:
            out.append(self._text_delta(self.scan_buf))         # lazy open
            self.scan_buf = ""
        if self.cur_out is not None:                            # close any still-open block
            out.append(self._emit("content_block_stop",
                      {"type": "content_block_stop", "index": self.cur_out}))
            self.cur_out = None
        return b"".join(out)

    # ---- boundary flush (round 3 P0): emit held text before stop/message; no dropped chars or index=None ----
    def _on_block_stop(self):
        out = []
        if self.state == "CAPTURE":
            if self.cap_buf:
                out.append(self._text_as_new_block(self.cap_buf))
            self.cap_buf = ""
            self.state = "PASS"
        elif self.scan_buf and self.cur_out is not None:
            # PASS holdback tail: block is closing, emit into current open block (no lazy open)
            out.append(self._emit("content_block_delta", {"type": "content_block_delta",
                      "index": self.cur_out, "delta": {"type": "text_delta", "text": self.scan_buf}}))
            self.scan_buf = ""
        if self.cur_out is not None:
            out.append(self._emit("content_block_stop",
                      {"type": "content_block_stop", "index": self.cur_out}))
            self.cur_out = None
        return b"".join(out)

    def _flush_pending(self):
        # Before message_delta/message_stop: emit held text and close block; no dangling text or
        # blocks open across message boundaries.
        out = []
        if self.state == "CAPTURE" and self.cap_buf:
            out.append(self._text_as_new_block(self.cap_buf))
            self.cap_buf = ""
            self.state = "PASS"
        elif self.scan_buf:
            out.append(self._text_delta(self.scan_buf))     # lazy open
            self.scan_buf = ""
        if self.cur_out is not None:                        # unconditionally close open block, mirror _on_block_stop
            out.append(self._emit("content_block_stop",
                      {"type": "content_block_stop", "index": self.cur_out}))
            self.cur_out = None
        return b"".join(out)

    # ---- synthesis helpers ----
    def _text_delta(self, text):
        if self.cur_out is None:
            head = self._open_text_block()
        else:
            head = b""
        return head + self._emit("content_block_delta", {"type": "content_block_delta",
                      "index": self.cur_out, "delta": {"type": "text_delta", "text": text}})

    def _open_text_block(self):
        self.cur_out = self.next_out
        self.next_out += 1
        self.cur_type = "text"
        return self._emit("content_block_start", {"type": "content_block_start",
                      "index": self.cur_out, "content_block": {"type": "text", "text": ""}})

    def _text_as_new_block(self, text):
        return self._text_delta(text) + self._emit("content_block_stop",
                      {"type": "content_block_stop", "index": self.cur_out}) + self._close_cur()

    def _close_cur(self):
        self.cur_out = None
        return b""

    def _tool_use_events(self, call):
        idx = self.next_out
        self.next_out += 1
        self.tool_n += 1
        tid = f"toolu_dsml_{self.nonce}_{self.tool_n}"
        start = self._emit("content_block_start", {"type": "content_block_start", "index": idx,
                    "content_block": {"type": "tool_use", "id": tid, "name": call["name"], "input": {}}})
        delta = self._emit("content_block_delta", {"type": "content_block_delta", "index": idx,
                    "delta": {"type": "input_json_delta",
                              "partial_json": json.dumps(call["input"], ensure_ascii=False)}})
        stop = self._emit("content_block_stop", {"type": "content_block_stop", "index": idx})
        return start + delta + stop

    def _on_message_delta(self, obj):
        if self.synthesized:
            d = dict(obj.get("delta") or {})
            if d.get("stop_reason") in ("end_turn", "stop", None):
                d["stop_reason"] = "tool_use"
            obj = {**obj, "delta": d}
        return self._emit("message_delta", obj)

    # ---- utilities ----
    @staticmethod
    def _parse_frame(frame):
        event, data_lines = None, []
        for line in frame.split("\n"):
            line = line.rstrip("\r")
            if line.startswith("event:"):
                event = line[6:].strip()
            elif line.startswith("data:"):
                data_lines.append(line[5:].lstrip())
        if not data_lines:
            return event, None
        try:
            return event, json.loads("\n".join(data_lines))
        except (ValueError, json.JSONDecodeError):
            return event, None

    @staticmethod
    def _emit(event, obj):
        return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n".encode("utf-8")

    @staticmethod
    def _raw(frame):
        return (frame + "\n\n").encode("utf-8")


def rewrite_nonstream_body(body_bytes, known_tools, nonce=""):
    """Non-streaming response body: expand DSML segments in text content blocks into ordered
    text/tool_use blocks. Conservative: return original bytes on bad JSON."""
    nonce = nonce or "x"
    try:
        obj = json.loads(body_bytes)
    except (ValueError, json.JSONDecodeError):
        return body_bytes
    if not isinstance(obj, dict) or not isinstance(obj.get("content"), list):
        return body_bytes
    new_content = []
    n = 0
    changed = False
    for blk in obj["content"]:
        if isinstance(blk, dict) and blk.get("type") == "text" and isinstance(blk.get("text"), str):
            segs = segment_dsml_text(blk["text"], known_tools)
            if any(s["type"] == "tool_use" for s in segs):
                changed = True
                for s in segs:
                    if s["type"] == "text":
                        new_content.append({"type": "text", "text": s["text"]})
                    else:
                        n += 1
                        new_content.append({"type": "tool_use", "id": f"toolu_dsml_{nonce}_{n}",
                                            "name": s["name"], "input": s["input"]})
                continue
        new_content.append(blk)
    if not changed:
        # No leak: return upstream bytes verbatim. Preserves byte-for-byte fidelity (no JSON round-trip,
        # no touching opaque upstream fields like thinking.signature) and keeps upper-layer
        # "byte diff → rewrite happened" telemetry accurate (otherwise clean responses would be
        # falsely reported as rewritten due to compact↔spaced re-serialization).
        return body_bytes
    obj["content"] = new_content
    if obj.get("stop_reason") in ("end_turn", "stop", None):
        obj["stop_reason"] = "tool_use"
    return json.dumps(obj, ensure_ascii=False).encode("utf-8")
