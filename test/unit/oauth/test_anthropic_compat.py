import json
import os
import sys
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from proxy.core import csp_proxy as cs
from proxy.policy import provider_policy as pp
from proxy.compat import anthropic_compat as ac
from proxy.registry import model_registry as mr

P2 = "｜｜"   # fullwidth double pipe U+FF5C (issue #8 observed leak shape)
# DSML where model meant web_search but leaked as plain text.
DSML_TEXT = (
    "<" + P2 + "DSML" + P2 + "tool_calls> "
    "<" + P2 + "DSML" + P2 + 'invoke name="web_search">'
    "<" + P2 + "DSML" + P2 + 'parameter name="query" string="true">GSE207177</' + P2 + "DSML" + P2 + "parameter>"
    "</" + P2 + "DSML" + P2 + "invoke> "
    "</" + P2 + "DSML" + P2 + "tool_calls>")
WEB_SEARCH = {"web_search": {"type": "object", "properties": {"query": {"type": "string"}}}}


def _state(prov, prov_name, nonce="n", **over):
    return pp.ProviderState(
        policy=pp.policy_from_prov(prov), prov_name=prov_name,
        relay_force_model=over.get("relay_force_model"),
        relay_models=over.get("relay_models", []),
        relay_thinking=over.get("relay_thinking"),
        shim_mode=over.get("shim_mode", "off"),
        nonce_factory=lambda: nonce,
        model_registry=over.get("model_registry"),
    )


def _ctx(**over):
    base = dict(src_model="claude-opus-4-8", target_model="deepseek-v4-pro",
                known_tools={}, nonce="fixed", shim_mode="off", provider="deepseek")
    base.update(over)
    return ac.Ctx(**base)


def _dsml_json_body():
    return json.dumps({
        "id": "msg", "type": "message", "role": "assistant", "model": "up",
        "content": [{"type": "text", "text": DSML_TEXT}],
        "stop_reason": "end_turn", "stop_sequence": None,
        "usage": {"input_tokens": 1, "output_tokens": 5},
    }, ensure_ascii=False).encode("utf-8")


class TransformRequest(unittest.TestCase):
    def test_deepseek_maps_model_clamps_normalizes_thinking(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek")
        body = {"model": "claude-opus-4-8", "max_tokens": 100000,
                "thinking": {"type": "auto"},
                "messages": [{"role": "user", "content": "hi"}]}
        up, ctx = ac.transform_request(body, st)
        self.assertEqual(up["model"], "deepseek-v4-pro")       # mapped
        self.assertEqual(up["max_tokens"], 65536)               # clamped to pro cap
        self.assertEqual(up["thinking"]["type"], "adaptive")    # auto → adaptive
        self.assertEqual(ctx.src_model, "claude-opus-4-8")
        self.assertEqual(ctx.target_model, "deepseek-v4-pro")
        self.assertEqual(ctx.provider, "deepseek")
        # original body unchanged (copy semantics).
        self.assertEqual(body["model"], "claude-opus-4-8")

    def test_relay_passthrough_no_clamp(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay")
        up, ctx = ac.transform_request(
            {"model": "claude-opus-4-8", "max_tokens": 1000000, "messages": []}, st)
        self.assertEqual(up["model"], "claude-opus-4-8")        # passthrough, no map
        self.assertEqual(up["max_tokens"], 1000000)             # relay does not clamp

    def test_nonce_injected_from_factory(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek", nonce="deadbeef")
        _up, ctx = ac.transform_request({"model": "claude-opus-4-8", "messages": []}, st)
        self.assertEqual(ctx.nonce, "deadbeef")

    def test_known_tools_extracted(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tools": [{"name": "web_search", "input_schema": {"type": "object"}},
                          {"no_name": True}]}
        _up, ctx = ac.transform_request(body, st)
        self.assertEqual(list(ctx.known_tools), ["web_search"])

    def test_virtual_model_registry_advertises_catalog_rule(self):
        reg = mr.ModelRegistry.from_models(["glm-5.2", "glm-4.7"])
        st = _state(dict(cs.PROVIDERS["relay"]), "relay", model_registry=reg)
        _up, ctx = ac.transform_request(
            {"model": "claude-opus-4-8", "messages": [], "max_tokens": 100},
            st,
        )
        self.assertEqual(ctx.rule_ids[0], pp.RULE_PROVIDER_VIRTUAL_MODEL_REGISTRY)

    def test_kimi_relay_does_not_advertise_web_search_server_tool(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay",
                    relay_force_model="kimi-k2.7-code", relay_thinking="enabled")
        body = {"model": "claude-opus-4-8", "messages": [], "max_tokens": 1000,
                "tools": [{"name": "web_search", "input_schema": {"type": "object"}},
                          {"name": "bash", "input_schema": {"type": "object"}}]}
        up, ctx = ac.transform_request(body, st)
        self.assertEqual(up["model"], "kimi-k2.7-code")
        self.assertEqual([t["name"] for t in up["tools"]], ["bash"])
        self.assertEqual(set(ctx.known_tools), {"web_search", "bash"})
        self.assertEqual(ctx.rule_ids, (
            pp.RULE_PROVIDER_RELAY_FORCE_MODEL_SHELL,
            pp.RULE_PROVIDER_KIMI_RELAY_THINKING_ENABLED,
            pp.RULE_TOOL_RELAY_INPUT_SCHEMA_NORMALIZE,
            pp.RULE_TOOL_KIMI_WEB_SEARCH_SERVER_TOOL_FILTER,
        ))

    def test_non_kimi_relay_keeps_web_search_tool(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay",
                    relay_force_model="glm-5.2", relay_thinking="adaptive")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tools": [{"name": "web_search", "input_schema": {"type": "object"}}]}
        up, _ctx = ac.transform_request(body, st)
        self.assertEqual([t["name"] for t in up["tools"]], ["web_search"])

    def test_relay_normalizes_loose_tool_schemas(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay",
                    relay_force_model="MiniMax-M2")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tools": [
                    {"name": "empty", "input_schema": {}},
                    {"name": "props_only", "input_schema": {
                        "properties": {"q": {"type": "string"}},
                    }},
                    {"name": "bad_props", "input_schema": {
                        "type": "object", "properties": [], "required": "q",
                    }},
                    {"name": "not_dict", "input_schema": []},
                    {"name": "", "input_schema": {"type": "object"}},
                    "bad-tool",
                ]}
        up, ctx = ac.transform_request(body, st)
        schemas = {t["name"]: t["input_schema"] for t in up["tools"]}
        self.assertEqual(list(schemas), ["empty", "props_only", "bad_props", "not_dict"])
        self.assertEqual(schemas["empty"], {"type": "object", "properties": {}})
        self.assertEqual(schemas["props_only"]["type"], "object")
        self.assertEqual(schemas["props_only"]["properties"]["q"]["type"], "string")
        self.assertEqual(schemas["bad_props"], {"type": "object", "properties": {}})
        self.assertEqual(schemas["not_dict"], {"type": "object", "properties": {}})
        self.assertEqual(set(ctx.known_tools), {"empty", "props_only", "bad_props", "not_dict"})
        self.assertIn(pp.RULE_TOOL_RELAY_INPUT_SCHEMA_NORMALIZE, ctx.rule_ids)
        self.assertEqual(body["tools"][0]["input_schema"], {}, "不应原地改调用者请求体")

    def test_relay_tool_choice_for_removed_tool_degrades_to_auto(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay",
                    relay_force_model="MiniMax-M2")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tool_choice": {"type": "tool", "name": "removed"},
                "tools": [{"name": "", "input_schema": {"type": "object"}}]}
        up, _ctx = ac.transform_request(body, st)
        self.assertNotIn("tools", up)
        self.assertEqual(up["tool_choice"], {"type": "auto"})

    def test_kimi_web_search_tool_choice_degrades_to_auto_after_filter(self):
        st = _state(dict(cs.PROVIDERS["relay"]), "relay",
                    relay_force_model="kimi-k2.7-code")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tool_choice": {"type": "tool", "name": "web_search"},
                "tools": [{"name": "web_search", "input_schema": {"type": "object"}}]}
        up, _ctx = ac.transform_request(body, st)
        self.assertNotIn("tools", up)
        self.assertEqual(up["tool_choice"], {"type": "auto"})

    def test_non_relay_tool_schemas_are_not_normalized_here(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek")
        body = {"model": "claude-opus-4-8", "messages": [],
                "tools": [{"name": "loose", "input_schema": {}}]}
        up, _ctx = ac.transform_request(body, st)
        self.assertEqual(up["tools"][0]["input_schema"], {})

    def test_no_max_tokens_left_absent(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek")
        up, _ctx = ac.transform_request({"model": "claude-opus-4-8", "messages": []}, st)
        self.assertNotIn("max_tokens", up)


class RewriteNonstream(unittest.TestCase):
    def test_off_returns_verbatim_empty_stats(self):
        body = _dsml_json_body()
        out, stats = ac.rewrite_nonstream(body, _ctx(shim_mode="off", known_tools=WEB_SEARCH))
        self.assertEqual(out, body)
        self.assertEqual(stats, {})

    def test_no_tools_returns_verbatim(self):
        body = _dsml_json_body()
        out, stats = ac.rewrite_nonstream(body, _ctx(shim_mode="rewrite", known_tools={}))
        self.assertEqual(out, body)
        self.assertEqual(stats, {})

    def test_detect_reports_found_without_changing_bytes(self):
        body = _dsml_json_body()
        out, stats = ac.rewrite_nonstream(body, _ctx(shim_mode="detect", known_tools=WEB_SEARCH))
        self.assertEqual(out, body)
        self.assertTrue(stats["found"])

    def test_rewrite_synthesizes_tool_use_with_fixed_nonce(self):
        body = _dsml_json_body()
        out, stats = ac.rewrite_nonstream(
            body, _ctx(shim_mode="rewrite", known_tools=WEB_SEARCH, nonce="fixed"))
        self.assertTrue(stats["rewritten"])
        obj = json.loads(out)
        tu = next(b for b in obj["content"] if b["type"] == "tool_use")
        self.assertEqual(tu["id"], "toolu_dsml_fixed_1")       # fixed nonce → stable id
        self.assertEqual(tu["name"], "web_search")
        self.assertEqual(tu["input"], {"query": "GSE207177"})
        self.assertEqual(obj["stop_reason"], "tool_use")

    def test_rewrite_no_leak_reports_not_rewritten(self):
        clean = json.dumps({"content": [{"type": "text", "text": "hello"}],
                            "stop_reason": "end_turn"}).encode()
        out, stats = ac.rewrite_nonstream(
            clean, _ctx(shim_mode="rewrite", known_tools=WEB_SEARCH))
        self.assertEqual(out, clean)
        self.assertFalse(stats["rewritten"])


class MakeStreamRewriter(unittest.TestCase):
    def test_off_returns_none(self):
        self.assertIsNone(ac.make_stream_rewriter(_ctx(shim_mode="off", known_tools=WEB_SEARCH)))

    def test_no_tools_returns_none(self):
        self.assertIsNone(ac.make_stream_rewriter(_ctx(shim_mode="rewrite", known_tools={})))

    def test_detect_filter_passes_through_and_reports_found(self):
        f = ac.make_stream_rewriter(_ctx(shim_mode="detect", known_tools=WEB_SEARCH))
        chunk = DSML_TEXT.encode("utf-8")
        self.assertEqual(f.feed(chunk), chunk)                 # detect passes through unchanged
        self.assertEqual(f.finalize(), b"")
        self.assertTrue(f.stats()["found"])

    def test_kimi_filter_drops_server_tool_blocks_and_compacts_indexes(self):
        f = ac.make_stream_rewriter(_ctx(provider="relay", target_model="kimi-k2.7-code"))
        sse = b"".join([
            b'event: content_block_start\ndata: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}\n\n',
            b'event: content_block_stop\ndata: {"type":"content_block_stop","index":0}\n\n',
            b'event: content_block_start\ndata: {"type":"content_block_start","index":1,"content_block":{"type":"server_tool_use","name":"web_search"}}\n\n',
            b'event: content_block_delta\ndata: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta"}}\n\n',
            b'event: content_block_stop\ndata: {"type":"content_block_stop","index":1}\n\n',
            b'event: content_block_start\ndata: {"type":"content_block_start","index":2,"content_block":{"type":"web_search_tool_result","content":[]}}\n\n',
            b'event: content_block_stop\ndata: {"type":"content_block_stop","index":2}\n\n',
            b'event: content_block_start\ndata: {"type":"content_block_start","index":3,"content_block":{"type":"thinking","thinking":"","signature":""}}\n\n',
            b'event: content_block_stop\ndata: {"type":"content_block_stop","index":3}\n\n',
            b'event: content_block_start\ndata: {"type":"content_block_start","index":4,"content_block":{"type":"text","text":""}}\n\n',
            b'event: content_block_delta\ndata: {"type":"content_block_delta","index":4,"delta":{"type":"text_delta","text":"OK"}}\n\n',
        ])
        out = f.feed(sse) + f.finalize()
        self.assertNotIn(b"server_tool_use", out)
        self.assertNotIn(b"web_search_tool_result", out)
        self.assertIn(b'"type":"thinking","thinking":"","signature":""', out)
        self.assertIn(b'"index":1', out)
        self.assertIn(b'"index":2', out)
        self.assertIn(b'"text":"OK"', out)
        self.assertEqual(f.stats()["dropped_kimi_server_tool_blocks"], 2)
        self.assertEqual(f.stats()["rule_ids"], [pp.RULE_TOOL_KIMI_WEB_SEARCH_SERVER_TOOL_FILTER])

    def test_rewrite_filter_synthesizes_tool_use(self):
        f = ac.make_stream_rewriter(
            _ctx(shim_mode="rewrite", known_tools=WEB_SEARCH, nonce="fixed"))
        # Feed minimal SSE containing DSML frame by frame, then finalize.
        out = b""
        for frame in _min_sse_frames():
            out += f.feed(frame)
        out += f.finalize()
        text = out.decode("utf-8", "replace")
        self.assertIn('"type":"tool_use"', text.replace(" ", ""))
        self.assertTrue(f.stats()["synthesized"])
        self.assertGreaterEqual(f.stats()["tool_n"], 1)


def _min_sse_frames():
    def sse(event, obj):
        return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n".encode("utf-8")
    return [
        sse("message_start", {"type": "message_start", "message": {
            "id": "m", "type": "message", "role": "assistant", "model": "up",
            "content": [], "stop_reason": None, "stop_sequence": None,
            "usage": {"input_tokens": 1, "output_tokens": 1}}}),
        sse("content_block_start", {"type": "content_block_start", "index": 0,
            "content_block": {"type": "text", "text": ""}}),
        sse("content_block_delta", {"type": "content_block_delta", "index": 0,
            "delta": {"type": "text_delta", "text": DSML_TEXT}}),
        sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
        sse("message_delta", {"type": "message_delta",
            "delta": {"stop_reason": "end_turn", "stop_sequence": None},
            "usage": {"output_tokens": 5}}),
        sse("message_stop", {"type": "message_stop"}),
    ]


class InjectCspWebAccessGuidance(unittest.TestCase):
    def test_skips_when_no_system(self):
        body = {"model": "claude-opus-4-8", "messages": [{"role": "user", "content": "hi"}]}
        self.assertIs(ac.inject_csp_web_access_guidance(body), body)

    def test_appends_to_string_system(self):
        frozen = datetime(2026, 7, 14, 12, 46, tzinfo=timezone(timedelta(hours=8), "CST"))
        body = {"system": "You are helpful.", "messages": []}
        out = ac.inject_csp_web_access_guidance(body, now=frozen)
        self.assertIsNot(out, body)
        self.assertEqual(body["system"], "You are helpful.")  # original untouched
        self.assertIn(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL, out["system"])
        self.assertIn('host.mcp("web-search", "web_search"', out["system"])
        self.assertIn('host.mcp("web-search", "search_literature"', out["system"])
        self.assertIn('host.mcp("web-search", "fetch_url"', out["system"])
        self.assertIn('data["results"]', out["system"])
        self.assertIn('page["content"]', out["system"])
        self.assertIn("duckduckgo_ia", out["system"])
        self.assertIn("GENERAL", out["system"])
        self.assertIn("LITERATURE", out["system"])
        self.assertIn("Current local date/time:", out["system"])
        self.assertIn("2026-07-14", out["system"])
        self.assertIn("latest as of 2026-07-14", out["system"])
        self.assertIn("freshness", out["system"])
        self.assertTrue(out["system"].startswith("You are helpful."))

    def test_appends_block_to_list_system(self):
        body = {"system": [{"type": "text", "text": "base"}], "messages": []}
        out = ac.inject_csp_web_access_guidance(body)
        self.assertEqual(len(out["system"]), 2)
        self.assertEqual(out["system"][0], {"type": "text", "text": "base"})
        self.assertEqual(out["system"][1]["type"], "text")
        self.assertIn(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL, out["system"][1]["text"])
        self.assertIn("Current local date/time:", out["system"][1]["text"])

    def test_idempotent_on_string_and_list(self):
        frozen = datetime(2026, 7, 14, 12, 46, tzinfo=timezone(timedelta(hours=8), "CST"))
        once = ac.inject_csp_web_access_guidance(
            {"system": "a", "messages": []}, now=frozen)
        twice = ac.inject_csp_web_access_guidance(once, now=frozen)
        self.assertIs(twice, once)
        self.assertEqual(twice["system"].count(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL), 1)

        once_list = ac.inject_csp_web_access_guidance({
            "system": [{"type": "text", "text": "a"}], "messages": []}, now=frozen)
        twice_list = ac.inject_csp_web_access_guidance(once_list, now=frozen)
        self.assertIs(twice_list, once_list)
        texts = [b.get("text", "") for b in twice_list["system"] if isinstance(b, dict)]
        self.assertEqual("".join(texts).count(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL), 1)

    def test_refreshes_stale_date_without_duplicate_sentinel(self):
        day1 = datetime(2026, 7, 13, 9, 0, tzinfo=timezone(timedelta(hours=8), "CST"))
        day2 = datetime(2026, 7, 14, 12, 46, tzinfo=timezone(timedelta(hours=8), "CST"))
        once = ac.inject_csp_web_access_guidance(
            {"system": "operon", "messages": []}, now=day1)
        self.assertIn("2026-07-13", once["system"])
        refreshed = ac.inject_csp_web_access_guidance(once, now=day2)
        self.assertIsNot(refreshed, once)
        self.assertEqual(refreshed["system"].count(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL), 1)
        self.assertIn("2026-07-14", refreshed["system"])
        self.assertNotIn("2026-07-13", refreshed["system"])
        self.assertTrue(refreshed["system"].startswith("operon"))

    def test_transform_request_injects_for_anthropic_path(self):
        st = _state(cs.PROVIDERS["deepseek"], "deepseek")
        body = {"model": "claude-opus-4-8", "system": "operon",
                "messages": [{"role": "user", "content": "search"}], "max_tokens": 100}
        up, _ctx = ac.transform_request(body, st)
        self.assertIn(ac.CSP_WEB_ACCESS_GUIDANCE_SENTINEL, up["system"])
        self.assertIn("Current local date/time:", up["system"])
        self.assertEqual(body["system"], "operon")  # copy semantics


if __name__ == "__main__":
    unittest.main()
