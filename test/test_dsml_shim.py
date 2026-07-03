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


if __name__ == "__main__":
    unittest.main()
