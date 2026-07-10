import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "proxy"))
import dsml_shim as ds

P2 = "｜｜"   # fullwidth double pipe (issue #8 observed)
P1 = "｜"         # single fullwidth pipe (vLLM docs)

WS = {"web_search": {"type": "object", "properties": {"query": {"type": "string"}}}}


def wrapper(pipe, name, params):
    # params: list[(pname, pval)], all encoded with string="true"
    ps = "".join(
        f'<{pipe}DSML{pipe}parameter name="{pn}" string="true">{pv}</{pipe}DSML{pipe}parameter>'
        for pn, pv in params)
    return f'<{pipe}DSML{pipe}invoke name="{name}">{ps}</{pipe}DSML{pipe}invoke>'


NUM = {"calc": {"type": "object", "properties": {
    "n": {"type": "integer"}, "f": {"type": "number"},
    "b": {"type": "boolean"}, "o": {"type": "object"}, "arr": {"type": "array"}}}}
# Tool's real field name is literally arguments (reserved-name collision scenario)
ARG_FIELD = {"run": {"type": "object", "properties": {"arguments": {"type": "string"}}}}
# Tool expects object input; model wraps with single arguments parameter
WRAP_TOOL = {"do": {"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "string"}}}}


def wrap_typed(pipe, tool, params):
    ps = "".join(
        f'<{pipe}DSML{pipe}parameter name="{pn}"{sa}>{pv}</{pipe}DSML{pipe}parameter>'
        for pn, sa, pv in params)
    return (f'<{pipe}DSML{pipe}tool_calls> <{pipe}DSML{pipe}invoke name="{tool}">'
            f'{ps}</{pipe}DSML{pipe}invoke> </{pipe}DSML{pipe}tool_calls>')


class SegmentCore(unittest.TestCase):
    def test_issue8_two_wrappers_three_calls(self):
        # issue #8: first segment two invokes, second one invoke (no text between segments)
        q1 = 'site:https://www.ncbi.nlm.nih.gov/geo/ "GSE207177"'
        q2 = '"GSE207177" AND ("sepsis" OR "heart")'
        q3 = 'https://www.ncbi.nlm.nih.gov/geo/query/acc.cgi?acc=GSE207177'
        blk1 = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query",q1)])} {wrapper(P2,"web_search",[("query",q2)])} </{P2}DSML{P2}tool_calls>'
        blk2 = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query",q3)])} </{P2}DSML{P2}tool_calls>'
        segs = ds.segment_dsml_text(blk1 + blk2, WS)
        tools = [s for s in segs if s["type"] == "tool_use"]
        self.assertEqual([t["input"]["query"] for t in tools], [q1, q2, q3])
        # segments must not include id
        self.assertNotIn("id", tools[0])

    def test_single_pipe_and_function_calls_alias(self):
        blk = f'<{P1}DSML{P1}function_calls> {wrapper(P1,"web_search",[("query","x")])} </{P1}DSML{P1}function_calls>'
        tools = [s for s in ds.segment_dsml_text(blk, WS) if s["type"] == "tool_use"]
        self.assertEqual(len(tools), 1)
        self.assertEqual(tools[0]["name"], "web_search")

    def test_unknown_tool_whole_block_stays_text(self):
        blk = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"evil_exec",[("cmd","rm -rf /")])} </{P2}DSML{P2}tool_calls>'
        segs = ds.segment_dsml_text(blk, WS)
        self.assertTrue(all(s["type"] == "text" for s in segs))
        self.assertEqual("".join(s["text"] for s in segs), blk)

    def test_mixed_known_unknown_whole_block_stays_text(self):
        blk = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query","x")])} {wrapper(P2,"evil",[("a","b")])} </{P2}DSML{P2}tool_calls>'
        segs = ds.segment_dsml_text(blk, WS)
        self.assertTrue(all(s["type"] == "text" for s in segs))

    def test_interleaving_preserved(self):
        b = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query","q")])} </{P2}DSML{P2}tool_calls>'
        text = "A" + b + "B" + b + "C"
        segs = ds.segment_dsml_text(text, WS)
        kinds = [(s["type"], s.get("text")) for s in segs]
        self.assertEqual(kinds[0], ("text", "A"))
        self.assertEqual(segs[1]["type"], "tool_use")
        self.assertEqual(kinds[2], ("text", "B"))
        self.assertEqual(segs[3]["type"], "tool_use")
        self.assertEqual(kinds[4], ("text", "C"))

    def test_param_value_with_specials(self):
        val = 'a">b<c:(d) https://x.y?z=1'
        blk = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query",val)])} </{P2}DSML{P2}tool_calls>'
        tools = [s for s in ds.segment_dsml_text(blk, WS) if s["type"] == "tool_use"]
        self.assertEqual(tools[0]["input"]["query"], val)

    def test_no_dsml_is_single_text(self):
        self.assertEqual(ds.segment_dsml_text("hello world", WS),
                         [{"type": "text", "text": "hello world"}])

    def test_parse_wrapper_helper(self):
        blk = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query","q")])} </{P2}DSML{P2}tool_calls>'
        calls = ds.parse_dsml_tool_calls(blk, WS)
        self.assertEqual(calls, [{"name": "web_search", "input": {"query": "q"}}])


class ParamTyping(unittest.TestCase):
    def test_string_false_coerced_by_schema(self):
        blk = wrap_typed(P2, "calc", [
            ("n", ' string="false"', "42"),
            ("f", ' string="false"', "3.5"),
            ("b", ' string="false"', "true"),
            ("o", ' string="false"', '{"k":1}'),
            ("arr", ' string="false"', "[1,2]")])
        t = ds.parse_dsml_tool_calls(blk, NUM)[0]["input"]
        self.assertEqual(t, {"n": 42, "f": 3.5, "b": True, "o": {"k": 1}, "arr": [1, 2]})

    def test_string_true_stays_string_even_if_numeric(self):
        blk = wrap_typed(P2, "calc", [("n", ' string="true"', "42")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, NUM)[0]["input"]["n"], "42")

    def test_schema_string_stays_string_without_string_attr(self):
        # DeepSeek can omit string="true" on string parameters. Schema type string
        # must win over the JSON fallback, or numeric-looking text is coerced and
        # the whole tool call gets discarded by validation.
        blk = wrap_typed(P2, "web_search", [("query", "", "42")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, WS),
                         [{"name": "web_search", "input": {"query": "42"}}])

    def test_missing_string_attr_falls_back_json_then_str(self):
        blk = wrap_typed(P2, "calc", [("f", "", "3.5")])
        # f is number → 3.5; JSON fallback without schema attrs tested separately
        self.assertEqual(ds.parse_dsml_tool_calls(blk, NUM)[0]["input"]["f"], 3.5)

    def test_wrapper_unwrap_when_not_real_field(self):
        # Single arguments param and tool schema has no arguments field → unwrap to whole input
        blk = wrap_typed(P2, "do", [("arguments", ' string="false"', '{"x":1,"y":"hi"}')])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, WRAP_TOOL)[0]["input"], {"x": 1, "y": "hi"})

    def test_no_unwrap_when_arguments_is_real_field(self):
        # Tool really has arguments field → no unwrap, keep {"arguments": "..."} (string=true)
        blk = wrap_typed(P2, "run", [("arguments", ' string="true"', "hello")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, ARG_FIELD)[0]["input"], {"arguments": "hello"})

    def test_missing_required_field_whole_block_stays_text(self):
        # Round-3 P2: missing required field → whole block kept as text
        req_tool = {"do": {"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]}}
        blk = wrap_typed(P2, "do", [("y", ' string="false"', "1")])   # only y given, required x missing
        self.assertEqual(ds.parse_dsml_tool_calls(blk, req_tool), [])

    def test_type_mismatch_whole_block_stays_text(self):
        # Round-3 P2: value hard-conflicts with schema base type → whole block kept as text
        req_tool = {"do": {"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]}}
        blk = wrap_typed(P2, "do", [("x", ' string="true"', "not-an-int")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, req_tool), [])

    def test_illegal_boolean_voids_block_not_silent_false(self):
        # Codex P1: illegal boolean literals (e.g. maybe/garbage) must not be guessed as False and pass validation,
        # which would synthesize a real tool call with wrong args. Whole block voided (conservative text passthrough).
        bt = {"setflag": {"type": "object", "properties": {"flag": {"type": "boolean"}}, "required": ["flag"]}}
        for bad in ("maybe", "garbage", "2", "TrueFalseMaybe"):
            self.assertEqual(ds.parse_dsml_tool_calls(wrap_typed(P2, "setflag", [("flag", "", bad)]), bt), [],
                             f"非法布尔 {bad!r} 应作废整块")

    def test_legal_boolean_literals_coerce(self):
        # Legal boolean literals still coerce correctly (case-insensitive, 1/0, yes/no).
        bt = {"setflag": {"type": "object", "properties": {"flag": {"type": "boolean"}}, "required": ["flag"]}}
        for raw, want in [("true", True), ("TRUE", True), ("1", True), ("yes", True),
                          ("false", False), ("False", False), ("0", False), ("no", False)]:
            got = ds.parse_dsml_tool_calls(wrap_typed(P2, "setflag", [("flag", "", raw)]), bt)
            self.assertEqual(got, [{"name": "setflag", "input": {"flag": want}}], f"{raw!r} 应转 {want}")


class ShimMode(unittest.TestCase):
    def setUp(self):
        os.environ.pop("CSP_TOOLUSE_SHIM", None)

    def tearDown(self):
        self.setUp()

    def test_off_when_not_capable(self):
        os.environ["CSP_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("deepseek", {"dsml_capable": False}), "off")

    def test_deepseek_reads_env(self):
        prov = {"dsml_capable": True}
        os.environ["CSP_TOOLUSE_SHIM"] = "detect"
        self.assertEqual(ds.shim_mode("deepseek", prov), "detect")
        os.environ["CSP_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("deepseek", prov), "rewrite")

    def test_default_off_when_env_unset(self):
        self.assertEqual(ds.shim_mode("deepseek", {"dsml_capable": True}), "off")

    def test_relay_always_off_this_round(self):
        # Relay always off this round (deepseek-only): off even when capable or env set
        os.environ["CSP_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("relay", {"dsml_capable": False}), "off")
        self.assertEqual(ds.shim_mode("relay", {"dsml_capable": True}), "off")

    def test_marker_bytes_are_utf8_fullwidth(self):
        self.assertTrue(all(isinstance(b, bytes) for b in ds.DSML_MARKER_BYTES))
        self.assertIn("｜DSML｜".encode("utf-8"), ds.DSML_MARKER_BYTES)


def sse(event, obj):
    return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n"


def parse_sse(raw_bytes):
    """Parse downstream SSE bytes into [(event, data_obj_or_rawstr)] for semantic assertions."""
    text = raw_bytes.decode("utf-8")
    out = []
    ev, data_lines = None, []
    for line in text.split("\n"):
        line = line.rstrip("\r")
        if line.startswith("event:"):
            ev = line[6:].strip()
        elif line.startswith("data:"):
            data_lines.append(line[5:].lstrip())
        elif line == "":
            if ev is not None:
                joined = "\n".join(data_lines)
                try:
                    out.append((ev, json.loads(joined)))
                except Exception:
                    out.append((ev, joined))
            ev, data_lines = None, []
    return out


def normal_text_stream():
    """A normal text stream without DSML (includes thinking block); returns SSE string."""
    parts = [
        sse("message_start", {"type": "message_start", "message": {"id": "m1", "type": "message",
            "role": "assistant", "model": "deepseek-v4-pro", "content": [], "stop_reason": None,
            "usage": {"input_tokens": 1, "output_tokens": 0}}}),
        sse("content_block_start", {"type": "content_block_start", "index": 0,
            "content_block": {"type": "thinking", "thinking": ""}}),
        sse("content_block_delta", {"type": "content_block_delta", "index": 0,
            "delta": {"type": "thinking_delta", "thinking": "let me think"}}),
        sse("content_block_delta", {"type": "content_block_delta", "index": 0,
            "delta": {"type": "signature_delta", "signature": "abc"}}),
        sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
        sse("content_block_start", {"type": "content_block_start", "index": 1,
            "content_block": {"type": "text", "text": ""}}),
        sse("content_block_delta", {"type": "content_block_delta", "index": 1,
            "delta": {"type": "text_delta", "text": "Hello world"}}),
        sse("content_block_stop", {"type": "content_block_stop", "index": 1}),
        sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 5}}),
        sse("message_stop", {"type": "message_stop"}),
    ]
    return "".join(parts)


class RewriterPassthrough(unittest.TestCase):
    def _run(self, raw_str, chunk=7):
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        data = raw_str.encode("utf-8")
        out = b""
        for i in range(0, len(data), chunk):
            out += rw.feed(data[i:i + chunk])
        out += rw.finalize()
        return out

    def test_semantic_equivalence_no_dsml(self):
        src = normal_text_stream()
        got = parse_sse(self._run(src, chunk=5))
        want = parse_sse(src.encode("utf-8"))
        # Event type sequence matches
        self.assertEqual([e for e, _ in got], [e for e, _ in want])
        # Concatenated text matches
        def text_of(evs):
            return "".join(d["delta"]["text"] for e, d in evs
                           if e == "content_block_delta" and isinstance(d, dict)
                           and d.get("delta", {}).get("type") == "text_delta")
        self.assertEqual(text_of(got), "Hello world")
        # thinking content preserved
        thinks = [d["delta"]["thinking"] for e, d in got if e == "content_block_delta"
                  and d.get("delta", {}).get("type") == "thinking_delta"]
        self.assertEqual(thinks, ["let me think"])
        # stop_reason semantics unchanged
        md = [d for e, d in got if e == "message_delta"][0]
        self.assertEqual(md["delta"]["stop_reason"], "end_turn")

    def test_empty_feed_returns_empty_bytes(self):
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        # Feed half an SSE frame (no blank line terminator) → must emit no bytes
        self.assertEqual(rw.feed(b"event: ping\n"), b"")

    def test_utf8_split_across_chunks(self):
        # Split text containing U+FF5C (3 bytes) at 1-byte chunks; decode must not break, text preserved
        text_val = "a｜b｜｜c"
        src = sse("content_block_start", {"type": "content_block_start", "index": 0,
                  "content_block": {"type": "text", "text": ""}}) + \
              sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                  "delta": {"type": "text_delta", "text": text_val}}) + \
              sse("content_block_stop", {"type": "content_block_stop", "index": 0})
        got = parse_sse(self._run(src, chunk=1))
        deltas = [d["delta"]["text"] for e, d in got if e == "content_block_delta"]
        self.assertEqual("".join(deltas), text_val)

    def test_crlf_and_unknown_fields_survive(self):
        src = ("event: ping\r\ndata: {\"type\":\"ping\"}\r\n\r\n"
               ": this is a comment\n\n"
               + sse("message_stop", {"type": "message_stop"}))
        got = parse_sse(self._run(src, chunk=4))
        self.assertIn("ping", [e for e, _ in got])
        self.assertIn("message_stop", [e for e, _ in got])

    def test_finalize_flushes_unterminated_tail_frame(self):
        # Codex P1: when upstream's last frame has no trailing blank line (sudden EOF), flush_tail must emit it,
        # or entire message_stop (or trailing delta) is silently dropped.
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        out = rw.feed(b'event: message_stop\ndata: {"type":"message_stop"}\n')  # note: only one \n
        out += rw.finalize()
        self.assertIn("message_stop", [e for e, _ in parse_sse(out)])

    def test_finalize_flushes_unterminated_message_delta(self):
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        out = rw.feed(b'event: message_delta\ndata: {"type":"message_delta",'
                      b'"delta":{"stop_reason":"end_turn"}}')  # no terminator at all
        out += rw.finalize()
        got = parse_sse(out)
        self.assertIn("message_delta", [e for e, _ in got])


class Detector(unittest.TestCase):
    def test_flags_response_containing_dsml(self):
        d = ds.DsmlDetector()
        d.feed(b"some text ")
        d.feed(("<" + P2 + "DSML" + P2 + "tool_calls>").encode("utf-8"))
        self.assertTrue(d.found)

    def test_clean_response_not_flagged(self):
        d = ds.DsmlDetector()
        d.feed(b"just a normal answer, no tool markers at all")
        self.assertFalse(d.found)

    def test_marker_split_across_chunks_still_detected(self):
        marker = ("｜DSML｜").encode("utf-8")
        d = ds.DsmlDetector()
        # Feed marker byte by byte across chunks
        for i in range(len(marker)):
            d.feed(marker[i:i + 1])
        self.assertTrue(d.found)

    def test_mixed_crlf_lf_frames_no_event_loss(self):
        # CRLF-ended content_block_start then LF-ended delta and stop: events must not drop, indices paired
        f1 = ('event: content_block_start\r\n'
              'data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}\r\n\r\n')
        f2 = sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                 "delta": {"type": "text_delta", "text": "Hello"}})
        f3 = sse("content_block_stop", {"type": "content_block_stop", "index": 0})
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        out = rw.feed((f1 + f2 + f3).encode("utf-8")) + rw.finalize()
        evs = parse_sse(out)
        kinds = [e for e, _ in evs]
        self.assertIn("content_block_start", kinds)     # start block must not be merged away
        self.assertIn("content_block_stop", kinds)      # stop block must not be silently dropped
        texts = "".join(d["delta"]["text"] for e, d in evs
                        if e == "content_block_delta" and isinstance(d, dict)
                        and d.get("delta", {}).get("type") == "text_delta")
        self.assertEqual(texts, "Hello")
        starts = [d["index"] for e, d in evs if e == "content_block_start" and isinstance(d, dict)]
        stops = [d["index"] for e, d in evs if e == "content_block_stop" and isinstance(d, dict)]
        self.assertEqual(sorted(stops), sorted(starts))  # each start has exactly one stop


def dsml_text_stream(query, pipe="｜｜", pre="", post=""):
    """SSE stream that leaks web_search as DSML text."""
    leak = (pre + f'<{pipe}DSML{pipe}tool_calls> <{pipe}DSML{pipe}invoke name="web_search">'
            f'<{pipe}DSML{pipe}parameter name="query" string="true">{query}'
            f'</{pipe}DSML{pipe}parameter> </{pipe}DSML{pipe}invoke> </{pipe}DSML{pipe}tool_calls>' + post)
    return "".join([
        sse("message_start", {"type": "message_start", "message": {"id": "m", "type": "message",
            "role": "assistant", "model": "deepseek-v4-pro", "content": [], "stop_reason": None,
            "usage": {"input_tokens": 1, "output_tokens": 0}}}),
        sse("content_block_start", {"type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": ""}}),
        sse("content_block_delta", {"type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": leak}}),
        sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
        sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 9}}),
        sse("message_stop", {"type": "message_stop"}),
    ])


class RewriterDsml(unittest.TestCase):
    def _run(self, raw_str, chunk):
        rw = ds.DsmlStreamRewriter(WS, nonce="t")
        data = raw_str.encode("utf-8")
        out = b""
        for i in range(0, len(data), chunk):
            out += rw.feed(data[i:i + chunk])
        out += rw.finalize()
        return parse_sse(out)

    def _tool_uses(self, evs):
        return [d["content_block"] for e, d in evs
                if e == "content_block_start" and isinstance(d, dict)
                and d.get("content_block", {}).get("type") == "tool_use"]

    def _stop_reason(self, evs):
        return [d for e, d in evs if e == "message_delta"][0]["delta"]["stop_reason"]

    def test_leak_becomes_tool_use_various_chunks(self):
        for chunk in (1, 3, 7, 4096):
            evs = self._run(dsml_text_stream("GSE207177"), chunk)
            tus = self._tool_uses(evs)
            self.assertEqual(len(tus), 1, f"chunk={chunk}")
            self.assertEqual(tus[0]["name"], "web_search")
            self.assertEqual(self._stop_reason(evs), "tool_use", f"chunk={chunk}")

    def test_tool_use_input_carried_via_input_json_delta(self):
        evs = self._run(dsml_text_stream("hello"), 5)
        ijd = [d for e, d in evs if e == "content_block_delta"
               and d.get("delta", {}).get("type") == "input_json_delta"]
        self.assertTrue(ijd)
        merged = "".join(x["delta"]["partial_json"] for x in ijd)
        self.assertEqual(json.loads(merged), {"query": "hello"})

    def test_no_double_stop_and_indices_unique(self):
        evs = self._run(dsml_text_stream("q"), 6)
        stops = [d["index"] for e, d in evs if e == "content_block_stop"]
        starts = [d["index"] for e, d in evs if e == "content_block_start"]
        self.assertEqual(len(stops), len(set(stops)))     # no duplicate closes
        self.assertEqual(len(starts), len(set(starts)))   # unique indices
        self.assertEqual(sorted(stops), sorted(starts))   # each start has exactly one stop

    def test_pre_and_post_text_preserved(self):
        evs = self._run(dsml_text_stream("q", pre="before ", post=" after"), 3)
        texts = "".join(d["delta"]["text"] for e, d in evs if e == "content_block_delta"
                        and d.get("delta", {}).get("type") == "text_delta")
        self.assertIn("before ", texts)
        self.assertIn(" after", texts)
        self.assertNotIn("DSML", texts)     # DSML markers must not remain in visible text

    def test_incomplete_dsml_flushed_as_text(self):
        # OPEN without CLOSE: EOF should flush held text verbatim, not rewrite stop_reason
        src = "".join([
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta",
                          "text": '<｜｜DSML｜｜tool_calls> <｜｜DSML｜｜invoke name="web_search">'}}),
            sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
            sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
        evs = self._run(src, 5)
        texts = "".join(d["delta"]["text"] for e, d in evs if e == "content_block_delta"
                        and d.get("delta", {}).get("type") == "text_delta")
        self.assertIn("DSML", texts)                      # held content flushed, not swallowed
        self.assertEqual(self._stop_reason(evs), "end_turn")

    def test_unknown_tool_stream_stays_text(self):
        evs = self._run(dsml_text_stream("q").replace("web_search", "evil_exec"), 5)
        self.assertEqual(self._tool_uses(evs), [])        # undeclared tool not synthesized
        self.assertEqual(self._stop_reason(evs), "end_turn")

    def test_stream_matches_segment_invariant(self):
        # Consistency invariant: state machine text/tool order == segment_dsml_text(full text)
        query = "q"
        leak_text = (f'A<｜｜DSML｜｜tool_calls> '
                     f'<｜｜DSML｜｜invoke name="web_search">'
                     f'<｜｜DSML｜｜parameter name="query" string="true">{query}'
                     f'</｜｜DSML｜｜parameter> </｜｜DSML｜｜invoke> '
                     f'</｜｜DSML｜｜tool_calls>B')
        want = [s["type"] for s in ds.segment_dsml_text(leak_text, WS)]
        src = "".join([
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": leak_text}}),
            sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
            sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
        evs = self._run(src, 3)
        got = []
        for e, d in evs:
            if e == "content_block_start" and isinstance(d, dict):
                ct = d.get("content_block", {}).get("type")
                if ct == "tool_use":
                    got.append("tool_use")
                elif ct == "text":
                    got.append("text")
        # Empty text blocks may appear; filter no-text segments with relaxed invariant: type subsequence matches
        self.assertEqual([g for g in got], want)

    def test_normal_text_ending_with_half_marker_flushed(self):
        # Round-3 P0: normal answer ending with "2 <" ("<" is possible marker prefix) → boundary must flush, no dropped chars
        src = "".join([
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": "the answer is 2 <"}}),
            sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
            sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
        evs = self._run(src, 3)
        texts = "".join(d["delta"]["text"] for e, d in evs if e == "content_block_delta"
                        and d.get("delta", {}).get("type") == "text_delta")
        self.assertEqual(texts, "the answer is 2 <")     # half marker must not be dropped
        self.assertEqual(self._stop_reason(evs), "end_turn")

    def test_close_tag_at_delta_end_then_more_text(self):
        # Round-3 P0: close tag ends one delta, next delta continues plain text → no index=None
        blk = ('<｜｜DSML｜｜tool_calls> <｜｜DSML｜｜invoke name="web_search">'
               '<｜｜DSML｜｜parameter name="query" string="true">q</｜｜DSML｜｜parameter>'
               ' </｜｜DSML｜｜invoke> </｜｜DSML｜｜tool_calls>')
        src = "".join([
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": blk}}),        # ends with close tag
            sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                "delta": {"type": "text_delta", "text": "done searching"}}),  # continues with plain text
            sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
            sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
        evs = self._run(src, 4096)     # each delta fed separately
        self.assertTrue(self._tool_uses(evs))
        for e, d in evs:                # all content_block_* index must be non-None
            if e.startswith("content_block") and isinstance(d, dict):
                self.assertIsNotNone(d.get("index"))
        texts = "".join(d["delta"]["text"] for e, d in evs if e == "content_block_delta"
                        and d.get("delta", {}).get("type") == "text_delta")
        self.assertIn("done searching", texts)
        self.assertEqual(self._stop_reason(evs), "tool_use")

    def test_open_block_closed_before_message_stop_when_upstream_omits_its_stop(self):
        # Upstream opened text block but skipped its content_block_stop before message_delta/stop:
        # fallback close must happen before message_stop, never across message boundary
        src = "".join([
            sse("content_block_start", {"type": "content_block_start", "index": 0,
                "content_block": {"type": "text", "text": ""}}),
            sse("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {}}),
            sse("message_stop", {"type": "message_stop"}),
        ])
        evs = self._run(src, 4096)
        kinds = [e for e, _ in evs]
        self.assertIn("content_block_stop", kinds)
        self.assertIn("message_stop", kinds)
        self.assertLess(kinds.index("content_block_stop"), kinds.index("message_stop"))


class NonStream(unittest.TestCase):
    def _resp(self, text):
        return json.dumps({"id": "m", "type": "message", "role": "assistant",
            "model": "deepseek-v4-pro", "content": [{"type": "text", "text": text}],
            "stop_reason": "end_turn", "usage": {"input_tokens": 1, "output_tokens": 2}},
            ensure_ascii=False).encode("utf-8")

    def test_leak_rewritten_to_tool_use_and_stop_reason(self):
        leak = ('<｜｜DSML｜｜tool_calls> <｜｜DSML｜｜invoke name="web_search">'
                '<｜｜DSML｜｜parameter name="query" string="true">q</｜｜DSML｜｜parameter>'
                ' </｜｜DSML｜｜invoke> </｜｜DSML｜｜tool_calls>')
        out = json.loads(ds.rewrite_nonstream_body(self._resp("A" + leak + "B"), WS, nonce="t"))
        types = [b["type"] for b in out["content"]]
        self.assertEqual(types, ["text", "tool_use", "text"])
        self.assertEqual(out["content"][1]["name"], "web_search")
        self.assertEqual(out["content"][1]["input"], {"query": "q"})
        self.assertEqual(out["stop_reason"], "tool_use")

    def test_no_dsml_unchanged_semantics(self):
        raw = self._resp("plain answer")
        out = json.loads(ds.rewrite_nonstream_body(raw, WS, nonce="t"))
        self.assertEqual(out["content"], [{"type": "text", "text": "plain answer"}])
        self.assertEqual(out["stop_reason"], "end_turn")

    def test_no_dsml_returns_verbatim_bytes(self):
        # Real-machine finding: with no leak, must not JSON round-trip—or bytes change (opaque upstream fields like
        # thinking.signature) and byte-diff telemetry falsely reports rewritten. Use compact separators + \\uXXXX escapes
        # in raw bytes (different shape from json.dumps(ensure_ascii=False)) to pin verbatim return.
        raw = (b'{"id":"m","type":"message","content":'
               b'[{"type":"text","text":"caf\\u00e9 \\u4f60\\u597d"}],"stop_reason":"end_turn"}')
        self.assertEqual(ds.rewrite_nonstream_body(raw, WS, nonce="t"), raw)

    def test_unknown_tool_unchanged(self):
        leak = ('<｜｜DSML｜｜tool_calls> <｜｜DSML｜｜invoke name="evil">'
                '<｜｜DSML｜｜parameter name="c" string="true">x</｜｜DSML｜｜parameter>'
                ' </｜｜DSML｜｜invoke> </｜｜DSML｜｜tool_calls>')
        out = json.loads(ds.rewrite_nonstream_body(self._resp(leak), WS, nonce="t"))
        self.assertEqual([b["type"] for b in out["content"]], ["text"])
        self.assertEqual(out["stop_reason"], "end_turn")

    def test_non_json_returned_as_is(self):
        self.assertEqual(ds.rewrite_nonstream_body(b"not json", WS), b"not json")


if __name__ == "__main__":
    unittest.main()
