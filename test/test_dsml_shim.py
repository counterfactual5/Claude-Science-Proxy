import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "proxy"))
import dsml_shim as ds

P2 = "｜｜"   # 双全角竖线（issue #8 实测）
P1 = "｜"         # 单全角竖线（vLLM 文档）

WS = {"web_search": {"type": "object", "properties": {"query": {"type": "string"}}}}


def wrapper(pipe, name, params):
    # params: list[(pname, pval)]，均按 string="true" 编码
    ps = "".join(
        f'<{pipe}DSML{pipe}parameter name="{pn}" string="true">{pv}</{pipe}DSML{pipe}parameter>'
        for pn, pv in params)
    return f'<{pipe}DSML{pipe}invoke name="{name}">{ps}</{pipe}DSML{pipe}invoke>'


NUM = {"calc": {"type": "object", "properties": {
    "n": {"type": "integer"}, "f": {"type": "number"},
    "b": {"type": "boolean"}, "o": {"type": "object"}, "arr": {"type": "array"}}}}
# 工具的真实字段名恰好叫 arguments（reserved 名冲突场景）
ARG_FIELD = {"run": {"type": "object", "properties": {"arguments": {"type": "string"}}}}
# 工具期望一个对象 input，模型用单个 arguments 包装
WRAP_TOOL = {"do": {"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "string"}}}}


def wrap_typed(pipe, tool, params):
    ps = "".join(
        f'<{pipe}DSML{pipe}parameter name="{pn}"{sa}>{pv}</{pipe}DSML{pipe}parameter>'
        for pn, sa, pv in params)
    return (f'<{pipe}DSML{pipe}tool_calls> <{pipe}DSML{pipe}invoke name="{tool}">'
            f'{ps}</{pipe}DSML{pipe}invoke> </{pipe}DSML{pipe}tool_calls>')


class SegmentCore(unittest.TestCase):
    def test_issue8_two_wrappers_three_calls(self):
        # issue #8：第一段两个 invoke、第二段一个 invoke（两段之间无文本）
        q1 = 'site:https://www.ncbi.nlm.nih.gov/geo/ "GSE207177"'
        q2 = '"GSE207177" AND ("sepsis" OR "heart")'
        q3 = 'https://www.ncbi.nlm.nih.gov/geo/query/acc.cgi?acc=GSE207177'
        blk1 = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query",q1)])} {wrapper(P2,"web_search",[("query",q2)])} </{P2}DSML{P2}tool_calls>'
        blk2 = f'<{P2}DSML{P2}tool_calls> {wrapper(P2,"web_search",[("query",q3)])} </{P2}DSML{P2}tool_calls>'
        segs = ds.segment_dsml_text(blk1 + blk2, WS)
        tools = [s for s in segs if s["type"] == "tool_use"]
        self.assertEqual([t["input"]["query"] for t in tools], [q1, q2, q3])
        # 分段不含 id
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

    def test_missing_string_attr_falls_back_json_then_str(self):
        blk = wrap_typed(P2, "calc", [("f", "", "3.5")])
        # f 是 number → 3.5；无 schema 属性的 json 兜底另测
        self.assertEqual(ds.parse_dsml_tool_calls(blk, NUM)[0]["input"]["f"], 3.5)

    def test_wrapper_unwrap_when_not_real_field(self):
        # 单个 arguments 参数、且工具 schema 无 arguments 字段 → 解包成整个 input
        blk = wrap_typed(P2, "do", [("arguments", ' string="false"', '{"x":1,"y":"hi"}')])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, WRAP_TOOL)[0]["input"], {"x": 1, "y": "hi"})

    def test_no_unwrap_when_arguments_is_real_field(self):
        # 工具真的有 arguments 字段 → 不解包，保留为 {"arguments": "..."}（string=true）
        blk = wrap_typed(P2, "run", [("arguments", ' string="true"', "hello")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, ARG_FIELD)[0]["input"], {"arguments": "hello"})

    def test_missing_required_field_whole_block_stays_text(self):
        # 第三轮 P2：缺 required 字段 → 整段按文本放行
        req_tool = {"do": {"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]}}
        blk = wrap_typed(P2, "do", [("y", ' string="false"', "1")])   # 只给 y，缺 required x
        self.assertEqual(ds.parse_dsml_tool_calls(blk, req_tool), [])

    def test_type_mismatch_whole_block_stays_text(self):
        # 第三轮 P2：值与 schema 基础类型硬冲突 → 整段按文本放行
        req_tool = {"do": {"type": "object", "properties": {"x": {"type": "integer"}}, "required": ["x"]}}
        blk = wrap_typed(P2, "do", [("x", ' string="true"', "not-an-int")])
        self.assertEqual(ds.parse_dsml_tool_calls(blk, req_tool), [])


class ShimMode(unittest.TestCase):
    def setUp(self):
        os.environ.pop("CSSWITCH_TOOLUSE_SHIM", None)

    def tearDown(self):
        self.setUp()

    def test_off_when_not_capable(self):
        os.environ["CSSWITCH_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("qwen", {"dsml_capable": False}), "off")

    def test_deepseek_reads_env(self):
        prov = {"dsml_capable": True}
        os.environ["CSSWITCH_TOOLUSE_SHIM"] = "detect"
        self.assertEqual(ds.shim_mode("deepseek", prov), "detect")
        os.environ["CSSWITCH_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("deepseek", prov), "rewrite")

    def test_default_off_when_env_unset(self):
        self.assertEqual(ds.shim_mode("deepseek", {"dsml_capable": True}), "off")

    def test_relay_always_off_this_round(self):
        # 本轮 relay 永远关闭（deepseek-only）：即便 capable 或设了 env 也 off
        os.environ["CSSWITCH_TOOLUSE_SHIM"] = "rewrite"
        self.assertEqual(ds.shim_mode("relay", {"dsml_capable": False}), "off")
        self.assertEqual(ds.shim_mode("relay", {"dsml_capable": True}), "off")

    def test_marker_bytes_are_utf8_fullwidth(self):
        self.assertTrue(all(isinstance(b, bytes) for b in ds.DSML_MARKER_BYTES))
        self.assertIn("｜DSML｜".encode("utf-8"), ds.DSML_MARKER_BYTES)


def sse(event, obj):
    return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n"


def parse_sse(raw_bytes):
    """把下游 SSE 字节解析成 [(event, data_obj_or_rawstr)]，供语义断言。"""
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
    """一段无 DSML 的正常文本流（含 thinking 块），返回 SSE 字符串。"""
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
        # 事件类型序列一致
        self.assertEqual([e for e, _ in got], [e for e, _ in want])
        # 文本拼接一致
        def text_of(evs):
            return "".join(d["delta"]["text"] for e, d in evs
                           if e == "content_block_delta" and isinstance(d, dict)
                           and d.get("delta", {}).get("type") == "text_delta")
        self.assertEqual(text_of(got), "Hello world")
        # thinking 内容保真
        thinks = [d["delta"]["thinking"] for e, d in got if e == "content_block_delta"
                  and d.get("delta", {}).get("type") == "thinking_delta"]
        self.assertEqual(thinks, ["let me think"])
        # stop_reason 语义不变
        md = [d for e, d in got if e == "message_delta"][0]
        self.assertEqual(md["delta"]["stop_reason"], "end_turn")

    def test_empty_feed_returns_empty_bytes(self):
        rw = ds.DsmlStreamRewriter({}, nonce="t")
        # 只喂半个 SSE 帧（无空行结束）→ 不应产出任何字节
        self.assertEqual(rw.feed(b"event: ping\n"), b"")

    def test_utf8_split_across_chunks(self):
        # 把含 U+FF5C（3字节）的文本按 1 字节切，解码不崩、文本保真
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


if __name__ == "__main__":
    unittest.main()
