import os
import sys
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from proxy.core import csp_proxy as cs
from proxy.registry import model_registry as mr


class ToolChoiceMapping(unittest.TestCase):
    def setUp(self):
        cs.PROV = cs.PROVIDERS["deepseek"]

    def test_tool_named_maps_to_function(self):
        out = cs.map_tool_choice({"type": "tool", "name": "grade"}, [{"name": "grade"}])
        self.assertEqual(out, {"type": "function", "function": {"name": "grade"}})

    def test_any_single_tool_names_that_function(self):
        out = cs.map_tool_choice({"type": "any"}, [{"name": "only"}])
        self.assertEqual(out, {"type": "function", "function": {"name": "only"}})

    def test_any_multi_tool_falls_back_to_required(self):
        out = cs.map_tool_choice({"type": "any"}, [{"name": "a"}, {"name": "b"}])
        self.assertEqual(out, "required")

    def test_auto_and_none_passthrough(self):
        self.assertEqual(cs.map_tool_choice({"type": "auto"}, []), "auto")
        self.assertEqual(cs.map_tool_choice({"type": "none"}, []), "none")

    def test_missing_returns_none(self):
        self.assertIsNone(cs.map_tool_choice(None, []))

    def test_translation_carries_tool_choice_stop_top_p(self):
        req = {
            "model": "claude-haiku-4-5",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"name": "grade", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "tool", "name": "grade"},
            "stop_sequences": ["STOP"],
            "top_p": 0.5,
        }
        out = cs.anthropic_to_openai(req)
        self.assertEqual(out["tool_choice"], {"type": "function", "function": {"name": "grade"}})
        self.assertEqual(out["stop"], ["STOP"])
        self.assertEqual(out["top_p"], 0.5)


class ResponsesMapping(unittest.TestCase):
    def setUp(self):
        cs.PROV = dict(cs.PROVIDERS["openai-responses"])
        cs.PROV_NAME = "openai-responses"
        cs.PROV["default_model"] = "gpt-5.2"
        cs.KEY = "sk-openai"
        cs.RELAY_FORCE_MODEL = "gpt-5.2"

    def tearDown(self):
        cs.RELAY_FORCE_MODEL = None

    def test_responses_request_maps_prompt_tools_and_limits(self):
        req = {
            "model": "claude-opus-4-8",
            "system": [{"type": "text", "text": "be brief"}],
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 999999,
            "temperature": 0.2,
            "top_p": 0.9,
            "tools": [{"name": "lookup", "description": "search", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "tool", "name": "lookup"},
        }
        out = cs.anthropic_to_openai_responses(req)
        self.assertEqual(out["model"], "gpt-5.2")
        self.assertEqual(out["instructions"], "be brief")
        self.assertEqual(out["input"], [{"role": "user", "content": "hi"}])
        self.assertEqual(out["max_output_tokens"], 65536)
        self.assertEqual(out["temperature"], 0.2)
        self.assertEqual(out["top_p"], 0.9)
        self.assertEqual(out["tools"][0]["type"], "function")
        self.assertEqual(out["tools"][0]["name"], "lookup")
        self.assertEqual(out["tool_choice"], "auto")

    def test_responses_forced_tool_choice_string_degrades_to_auto(self):
        out = cs.anthropic_to_openai_responses({
            "model": "claude-opus-4-8",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"name": "lookup", "input_schema": {"type": "object"}}],
            "tool_choice": "required",
        })
        self.assertEqual(out["tool_choice"], "auto")

    def test_responses_tool_choice_none_passthrough(self):
        out = cs.anthropic_to_openai_responses({
            "model": "claude-opus-4-8",
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "none"},
        })
        self.assertEqual(out["tool_choice"], "none")

    def test_responses_request_maps_tool_results(self):
        req = {
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "text", "text": "checking"},
                    {"type": "tool_use", "id": "call_1", "name": "lookup", "input": {"q": "x"}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": [{"type": "text", "text": "found"}]},
                ]},
            ],
        }
        out = cs.anthropic_to_openai_responses(req)
        self.assertEqual(out["input"][0], {"role": "assistant", "content": "checking"})
        self.assertEqual(out["input"][1]["type"], "function_call")
        self.assertEqual(out["input"][1]["call_id"], "call_1")
        self.assertEqual(json.loads(out["input"][1]["arguments"]), {"q": "x"})
        self.assertEqual(out["input"][2], {"type": "function_call_output", "call_id": "call_1", "output": "found"})

    def test_responses_response_maps_text_and_function_call(self):
        resp = {
            "id": "resp_1",
            "status": "completed",
            "output": [
                {"type": "message", "content": [{"type": "output_text", "text": "hello"}]},
                {"type": "function_call", "call_id": "call_2", "name": "lookup", "arguments": "{\"q\":\"y\"}"},
            ],
            "usage": {"input_tokens": 4, "output_tokens": 5},
        }
        out = cs.openai_responses_to_anthropic(resp, "claude-opus-4-8")
        self.assertEqual(out["content"][0], {"type": "text", "text": "hello"})
        self.assertEqual(out["content"][1], {"type": "tool_use", "id": "call_2", "name": "lookup", "input": {"q": "y"}})
        self.assertEqual(out["stop_reason"], "tool_use")
        self.assertEqual(out["usage"], {"input_tokens": 4, "output_tokens": 5})


class RelayProvider(unittest.TestCase):
    """Relay provider: passthrough (no remap) + clamp + dual auth headers + no max_tokens clamp + /v1/models normalization."""

    def setUp(self):
        cs.PROV = dict(cs.PROVIDERS["relay"])
        cs.PROV_NAME = "relay"
        cs.PROV["url"] = "https://relay.test/claude/v1/messages"
        cs.PROV["models_url"] = "https://relay.test/claude/v1/models"
        cs.KEY = "cr_testkey"
        cs.RELAY_MODELS = []
        cs.RELAY_FORCE_MODEL = None

    def test_auth_headers_both(self):
        h = cs._upstream_auth_headers()
        self.assertEqual(h.get("x-api-key"), "cr_testkey")
        self.assertEqual(h.get("Authorization"), "Bearer cr_testkey")

    def test_deepseek_auth_headers_default_xapikey(self):
        # Regression: deepseek without auth_style → still x-api-key only (no mistaken Bearer).
        cs.PROV = cs.PROVIDERS["deepseek"]
        cs.KEY = "sk-ds"
        self.assertEqual(cs._upstream_auth_headers(), {"x-api-key": "sk-ds"})

    def test_models_normalized_and_cache_refreshed(self):
        # Fake http_get_json mimics relay OpenAI-style response; verify Anthropic normalization
        # + RELAY_MODELS cache refresh (offline, no network).
        orig = cs.http_get_json
        cs.http_get_json = lambda url, headers: {
            "data": [
                {"id": "claude-opus-4-8", "object": "model"},
                {"id": "claude-3-5-sonnet-20241022", "object": "model"},
                {"no_id": True},  # entries missing id should be skipped
            ]
        }
        try:
            out = cs.fetch_relay_models()
        finally:
            cs.http_get_json = orig
        self.assertEqual([m["id"] for m in out], ["claude-opus-4-8", "claude-3-5-sonnet-20241022"])
        self.assertTrue(all(m["type"] == "model" for m in out))
        self.assertEqual(out[0]["display_name"], "claude-opus-4-8")
        self.assertEqual(cs.RELAY_MODELS, ["claude-opus-4-8", "claude-3-5-sonnet-20241022"])

    def test_models_carry_supports_tools_from_supported_parameters(self):
        orig = cs.http_get_json
        cs.http_get_json = lambda url, headers: {"data": [
            {"id": "glm-4.6", "supported_parameters": ["tools", "temperature"]},  # has tools → True
            {"id": "glm-lite", "supported_parameters": ["temperature"]},          # field present, no tools → False
            {"id": "glm-x"},                                                        # no field → None (no guess)
        ]}
        try:
            out = cs.fetch_relay_models()
        finally:
            cs.http_get_json = orig
        got = {m["id"]: m["supports_tools"] for m in out}
        self.assertEqual(got, {"glm-4.6": True, "glm-lite": False, "glm-x": None})

    def test_build_models_response_success(self):
        orig = cs.http_get_json
        cs.http_get_json = lambda url, headers: {"data": [{"id": "glm-4.6"}]}
        try:
            code, body = cs.build_models_response()
        finally:
            cs.http_get_json = orig
        self.assertEqual(code, 200)
        self.assertEqual([m["id"] for m in body["data"]], ["glm-4.6"])

    def test_build_models_response_preserves_upstream_status(self):
        import urllib.error
        import io
        def boom(url, headers):
            raise urllib.error.HTTPError(url, 401, "Unauthorized", {}, io.BytesIO(b"nope"))
        orig = cs.http_get_json
        cs.http_get_json = boom
        try:
            code, body = cs.build_models_response()
        finally:
            cs.http_get_json = orig
        self.assertEqual(code, 401)                    # no longer swallowed as 200 + static list
        self.assertEqual(body["upstream_status"], 401)
        self.assertNotIn("data", body)                 # failure must not return static list

    def test_build_models_response_network_error_502(self):
        def boom(url, headers):
            raise ConnectionError("dns fail")
        orig = cs.http_get_json
        cs.http_get_json = boom
        try:
            code, body = cs.build_models_response()
        finally:
            cs.http_get_json = orig
        self.assertEqual(code, 502)
        self.assertEqual(body["error_kind"], "network")

    def test_build_models_response_non_relay_returns_static(self):
        # deepseek (no models_url) → still return static selector list, unchanged behavior.
        cs.PROV = cs.PROVIDERS["deepseek"]
        code, body = cs.build_models_response()
        self.assertEqual(code, 200)
        ids = [m["id"] for m in body["data"]]
        self.assertIn("claude-opus-4-8", ids)


class OpenAICustomProvider(unittest.TestCase):
    def setUp(self):
        cs.PROV = dict(cs.PROVIDERS["openai-custom"])
        cs.PROV_NAME = "openai-custom"
        cs.PROV["url"] = "https://open.bigmodel.cn/api/paas/v4/chat/completions"
        cs.PROV["models_url"] = "https://open.bigmodel.cn/api/paas/v4/models"
        cs.PROV["default_model"] = "glm-4.5"
        cs.KEY = "sk-openai"
        cs.RELAY_FORCE_MODEL = "glm-4.5"

    def tearDown(self):
        cs.RELAY_FORCE_MODEL = None

    def test_openai_base_root_normalization(self):
        root = "https://open.bigmodel.cn/api/paas/v4"
        self.assertEqual(cs.normalize_openai_base(root + "/chat/completions"), root)
        self.assertEqual(cs.normalize_openai_base(root + "/responses"), root)
        self.assertEqual(cs.normalize_openai_base(root + "/models"), root)
        self.assertEqual(cs.openai_endpoint(root + "/chat/completions", "/models"), root + "/models")
        self.assertEqual(
            cs.openai_endpoint(root + "/models", "/chat/completions"),
            root + "/chat/completions",
        )
        self.assertEqual(
            cs.openai_endpoint(root + "/responses", "/responses"),
            root + "/responses",
        )
        self.assertEqual(
            cs.openai_endpoint("https://api.example.com", "/chat/completions"),
            "https://api.example.com/v1/chat/completions",
        )
        self.assertEqual(
            cs.openai_endpoint("https://api.example.com/v1/models", "/chat/completions"),
            "https://api.example.com/v1/chat/completions",
        )

    def test_relay_base_root_normalization(self):
        root = "https://open.bigmodel.cn/api/anthropic"
        self.assertEqual(cs.normalize_relay_base(root + "/v1/messages"), root)
        self.assertEqual(cs.normalize_relay_base(root + "/v1/models"), root)

    def test_force_model_enters_openai_translation(self):
        req = {
            "model": "claude-opus-4-8",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1000000,
        }
        out = cs.anthropic_to_openai(req)
        self.assertEqual(out["model"], "glm-4.5")
        self.assertEqual(out["max_tokens"], 1000000, "custom OpenAI 不做通用 token 夹取")

    def test_auth_headers_bearer_only(self):
        self.assertEqual(cs._upstream_auth_headers(), {"Authorization": "Bearer sk-openai"})

    def test_models_uses_bearer_without_anthropic_version(self):
        seen = {}
        orig = cs.http_get_json

        def fake(url, headers):
            seen["url"] = url
            seen["headers"] = headers
            return {"data": [{"id": "glm-4.5"}]}

        cs.http_get_json = fake
        try:
            out = cs.fetch_relay_models()
        finally:
            cs.http_get_json = orig
        self.assertEqual(seen["url"], "https://open.bigmodel.cn/api/paas/v4/models")
        self.assertEqual(seen["headers"].get("Authorization"), "Bearer sk-openai")
        self.assertNotIn("anthropic-version", seen["headers"])
        self.assertEqual([m["id"] for m in out], ["glm-4.5"])

    def test_models_discovery_works_without_forced_model(self):
        cs.RELAY_FORCE_MODEL = None
        orig = cs.http_get_json
        cs.http_get_json = lambda url, headers: {"data": [{"id": "glm-4.5"}]}
        try:
            code, body = cs.build_models_response()
        finally:
            cs.http_get_json = orig
        self.assertEqual(code, 200)
        self.assertEqual([m["id"] for m in body["data"]], ["glm-4.5"])


class BuildModelsResponse(unittest.TestCase):
    def setUp(self):
        self._saved = (cs.PROV, cs.PROV_NAME, cs.RELAY_FORCE_MODEL, cs.MODEL_REGISTRY)

    def tearDown(self):
        cs.PROV, cs.PROV_NAME, cs.RELAY_FORCE_MODEL, cs.MODEL_REGISTRY = self._saved

    def test_registry_returns_eight_shells_with_safe_names(self):
        models = [
            "glm-5.2", "glm-5.1", "glm-5-turbo", "glm-5",
            "glm-4.7", "glm-4.6", "glm-4.5-air", "glm-4.5",
        ]
        cs.PROV = {"models_url": "https://r/v1/models"}
        cs.PROV_NAME = "relay"
        cs.RELAY_FORCE_MODEL = "glm-4.5"
        cs.MODEL_REGISTRY = mr.ModelRegistry.from_models(
            models, default_model="glm-5.2", fast_model="glm-4.5",
        )
        code, body = cs.build_models_response()
        self.assertEqual(code, 200)
        self.assertEqual(len(body["data"]), 8)
        names = {m["display_name"] for m in body["data"]}
        self.assertIn("glm-5.turbo", names)
        self.assertIn("glm-5.0", names)
        self.assertTrue(all(m["id"].startswith("claude-") for m in body["data"]))

    def test_force_returns_single_shell(self):
        # force (Science resident proxy): /v1/models returns one shell, display_name=real model name.
        cs.PROV = {"models_url": "https://r/v1/models"}
        cs.PROV_NAME = "relay"
        cs.RELAY_FORCE_MODEL = "glm-5.2"
        code, body = cs.build_models_response()
        self.assertEqual(code, 200)
        self.assertEqual(
            body["data"],
            [{
                "type": "model", "id": "claude-opus-4-8",
                "display_name": "glm-5.2", "supports_tools": None,
                "created_at": "2026-01-01T00:00:00Z",
            }],
        )
        self.assertEqual(body["first_id"], "claude-opus-4-8")
        self.assertEqual(body["last_id"], "claude-opus-4-8")
        self.assertFalse(body["has_more"])

    def test_force_sanitizes_hyphen_only_display_name(self):
        cs.PROV = {"models_url": "https://r/v1/models"}
        cs.PROV_NAME = "relay"
        cs.RELAY_FORCE_MODEL = "glm-5-turbo"
        code, body = cs.build_models_response()
        self.assertEqual(code, 200)
        self.assertEqual(body["data"][0]["display_name"], "glm-5.turbo")

    def test_not_forced_falls_back_to_source(self):
        # not forced (app scratch proxy for model pick): still fetch real ids for user selection (two consumers split).
        cs.PROV = {"models_url": "https://r/v1/models"}
        cs.PROV_NAME = "relay"
        cs.RELAY_FORCE_MODEL = None
        saved_fetch = cs.fetch_relay_models
        cs.fetch_relay_models = lambda runtime=None: [{"type": "model", "id": "glm-4.6"}]
        try:
            code, body = cs.build_models_response()
        finally:
            cs.fetch_relay_models = saved_fetch
        self.assertEqual(code, 200)
        self.assertEqual(body["data"][0]["id"], "glm-4.6")


class RegistryDatedShellResolve(unittest.TestCase):
    def test_dated_haiku_shell_resolves_like_undated(self):
        reg = mr.ModelRegistry.from_models(
            ["glm-5.2", "glm-5.1", "glm-5-turbo", "glm-5", "glm-4.7", "glm-4.6", "glm-4.5-air", "glm-4.5"],
            default_model="glm-5.2",
            fast_model="glm-4.5",
        )
        self.assertEqual(
            reg.resolve("claude-haiku-4-5-20251001"),
            reg.resolve("claude-haiku-4-5"),
        )
        self.assertTrue(reg.resolve("claude-haiku-4-5-20251001"))

    def test_openai_tool_call_content_is_empty_string(self):
        cs.PROV = dict(cs.PROVIDERS["openai-custom"])
        cs.PROV_NAME = "openai-custom"
        cs.PROV["default_model"] = "glm-4.7"
        cs.RELAY_FORCE_MODEL = "glm-4.7"
        cs.MODEL_REGISTRY = None
        out = cs.anthropic_to_openai({
            "model": "claude-sonnet-4-6",
            "messages": [{
                "role": "assistant",
                "content": [{"type": "tool_use", "id": "t1", "name": "lookup", "input": {"q": "x"}}],
            }],
        })
        asst = next(m for m in out["messages"] if m.get("tool_calls"))
        self.assertEqual(asst["content"], "")


class OpenAIChatCompatBoundaryCases(unittest.TestCase):
    """GLM `code 1210` boundary cases for the Anthropic -> OpenAI Chat message
    translation (proxy/compat/openai_chat_compat.py). Exercised through the
    custom-openai provider (GLM Coding Plan's actual translation path)."""

    def setUp(self):
        cs.PROV = dict(cs.PROVIDERS["openai-custom"])
        cs.PROV_NAME = "openai-custom"
        cs.PROV["default_model"] = "glm-4.6"
        cs.KEY = "sk-openai"
        cs.RELAY_FORCE_MODEL = "glm-4.6"
        cs.MODEL_REGISTRY = None

    def _msgs(self, req):
        return cs.anthropic_to_openai(req)["messages"]

    def test_tool_result_list_content_flattened_to_plain_string(self):
        # tool_result.content as a list of text blocks must collapse to a bare
        # string — GLM rejects list/dict `tool` message content with 1210.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "lookup", "input": {"q": "x"}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": [
                        {"type": "text", "text": "part1 "},
                        {"type": "text", "text": "part2"},
                    ]},
                ]},
            ],
        })
        tool_msg = next(m for m in out if m.get("role") == "tool")
        self.assertIsInstance(tool_msg["content"], str)
        self.assertEqual(tool_msg["content"], "part1 part2")

    def test_tool_result_non_text_nested_block_gets_placeholder_not_dropped(self):
        # A tool_result containing only an image block must not collapse to "" —
        # a placeholder preserves that something was there.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "screenshot", "input": {}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": [
                        {"type": "image", "source": {"type": "base64", "data": "AAAA"}},
                    ]},
                ]},
            ],
        })
        tool_msg = next(m for m in out if m.get("role") == "tool")
        self.assertIn("image omitted", tool_msg["content"])

    def test_orphan_tool_result_not_emitted_as_dangling_tool_message(self):
        # tool_use for call_1 is NOT present in this replayed history (as happens
        # after Science compacts/summarizes a long session) — the tool_result
        # must never surface as a `role: tool` message with an unmatched id.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_orphan", "content": "stale result"},
                ]},
            ],
        })
        self.assertFalse(any(m.get("role") == "tool" for m in out))
        user_msg = next(m for m in out if m.get("role") == "user")
        self.assertIn("call_orphan", user_msg["content"])
        self.assertIn("stale result", user_msg["content"])

    def test_tool_results_reordered_to_match_assistant_call_order(self):
        # Two parallel tool_calls; tool_results arrive content-block-order
        # reversed relative to the assistant's tool_calls[] — must be
        # re-sorted to match, since GLM can be strict about call/result order.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "a", "input": {}},
                    {"type": "tool_use", "id": "call_2", "name": "b", "input": {}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_2", "content": "second"},
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "first"},
                ]},
            ],
        })
        tool_msgs = [m for m in out if m.get("role") == "tool"]
        self.assertEqual([m["tool_call_id"] for m in tool_msgs], ["call_1", "call_2"])

    def test_tool_use_arguments_always_json_string_even_when_input_missing(self):
        # `function.arguments` must always be a JSON string (never a bare dict,
        # and never the literal "null" from a missing/None `input`).
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "noop"},
                ]},
            ],
        })
        asst = next(m for m in out if m.get("tool_calls"))
        args = asst["tool_calls"][0]["function"]["arguments"]
        self.assertIsInstance(args, str)
        self.assertEqual(json.loads(args), {})

    def test_unknown_block_type_keeps_message_non_empty(self):
        # An assistant turn whose only content is a `server_tool_use` block
        # (no text, no ordinary tool_use) must not become a fully empty
        # message with nothing distinguishing it from a blank turn.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "server_tool_use", "id": "srv_1", "name": "web_search", "input": {}},
                ]},
            ],
        })
        asst = out[-1]
        self.assertEqual(asst["role"], "assistant")
        self.assertNotIn("tool_calls", asst)
        self.assertIn("omitted", asst["content"])

    def test_thinking_block_silently_dropped_without_placeholder(self):
        # Extended-thinking blocks have no OpenAI-compat slot and never own an
        # id used for pairing, so they should vanish cleanly (no placeholder
        # noise) rather than surface as visible dialogue text.
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "internal reasoning..."},
                    {"type": "text", "text": "final answer"},
                ]},
            ],
        })
        asst = out[-1]
        self.assertEqual(asst["content"], "final answer")

    def test_tool_result_is_error_flag_prefixed(self):
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "risky", "input": {}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "boom", "is_error": True},
                ]},
            ],
        })
        tool_msg = next(m for m in out if m.get("role") == "tool")
        self.assertTrue(tool_msg["content"].startswith("[ERROR]"))

    def test_oversized_tool_result_is_truncated(self):
        from proxy.compat import openai_chat_compat as occ
        huge = "x" * (occ._MAX_SINGLE_TEXT_CHARS + 500)
        out = self._msgs({
            "model": "claude-opus-4-8",
            "messages": [
                {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "call_1", "name": "dump", "input": {}},
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": huge},
                ]},
            ],
        })
        tool_msg = next(m for m in out if m.get("role") == "tool")
        self.assertLess(len(tool_msg["content"]), len(huge))
        self.assertIn("truncated", tool_msg["content"])


class OpenAIStreamPlaceholderReplay(unittest.TestCase):
    """openai-custom opens a counted SSE preamble before the buffered upstream
    completion returns; replay must continue that open text block instead of
    emitting a second message_start."""

    def _capture_replay(self, aresp, placeholder_text_open):
        import io

        class _H(cs.H):
            def __init__(self):
                self.wfile = io.BytesIO()
                self._headers = []

            def send_response(self, *a, **k):
                pass

            def send_header(self, k, v):
                self._headers.append((k, v))

            def end_headers(self):
                pass

        h = _H.__new__(_H)
        h.wfile = io.BytesIO()
        h._headers = []
        h._replay_as_sse(aresp, headers_already_sent=True,
                         placeholder_text_open=placeholder_text_open)
        return h.wfile.getvalue()

    def test_placeholder_text_merges_into_open_block(self):
        raw = self._capture_replay({
            "id": "msg_real",
            "model": "claude-sonnet-4-6",
            "content": [{"type": "text", "text": "hello"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
        }, placeholder_text_open=True)
        self.assertNotIn(b"event: message_start", raw)
        self.assertIn(b'"text": "hello"', raw)
        self.assertIn(b"event: message_stop", raw)

    def test_placeholder_then_tool_use_closes_empty_text_first(self):
        raw = self._capture_replay({
            "id": "msg_real",
            "model": "claude-sonnet-4-6",
            "content": [{"type": "tool_use", "id": "t1", "name": "lookup", "input": {"q": "x"}}],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 1, "output_tokens": 1},
        }, placeholder_text_open=True)
        self.assertNotIn(b"event: message_start", raw)
        # Empty placeholder closed at index 0, tool_use starts at index 1.
        self.assertIn(b'"index": 0', raw)
        self.assertIn(b'"index": 1', raw)
        self.assertIn(b'"name": "lookup"', raw)

    def test_counted_keepalive_is_empty_text_delta(self):
        from proxy.core import http_transport as ht
        self.assertIn(b"content_block_delta", ht._COUNTED_TEXT_DELTA_KEEPALIVE)
        self.assertIn(b'"text": ""', ht._COUNTED_TEXT_DELTA_KEEPALIVE)


class PlatterCredentialsFailClosed(unittest.TestCase):
    """Platter must never fall back to host process key/url for a routed shell."""

    def setUp(self):
        self._prev_creds = cs.PROFILE_CREDENTIALS
        self._prev_reg = cs.MODEL_REGISTRY
        reg = mr.ModelRegistry.from_payload({
            "platter": True,
            "default_model": "glm-5.2",
            "fast_model": "deepseek-v4-flash",
            "entries": [
                {"profile_id": "p-glm", "model": "glm-5.2", "display_prefix": "GLM"},
                {"profile_id": "p-ds", "model": "deepseek-v4-pro", "display_prefix": "DS"},
            ],
        })
        self.base = cs.RuntimeState(
            prov=dict(cs.PROVIDERS["relay"]),
            prov_name="relay",
            key="host-key-must-not-leak",
            auth_secret="s",
            relay_models=[{"id": "host-model"}],
            relay_force_model=None,
            relay_thinking=None,
            shim_mode="off",
            model_registry=reg,
        )
        # First platter entry maps to main shell claude-opus-4-8.
        self.shell_glm = next(
            e.shell_id for e in reg.entries if e.profile_id == "p-glm"
        )

    def tearDown(self):
        cs.PROFILE_CREDENTIALS = self._prev_creds
        cs.MODEL_REGISTRY = self._prev_reg

    def test_switches_to_profile_credentials(self):
        cs.PROFILE_CREDENTIALS = {
            "p-glm": {
                "adapter": "relay",
                "key": "gk-glm",
                "base_url": "https://open.bigmodel.cn/api/anthropic",
            },
        }
        rt = cs.runtime_for_model(self.base, self.shell_glm)
        self.assertEqual(rt.key, "gk-glm")
        self.assertEqual(rt.prov_name, "relay")
        self.assertIn("open.bigmodel.cn", rt.prov["url"])
        self.assertEqual(rt.relay_models, [])  # no shared host catalog

    def test_missing_profile_credentials_raises(self):
        cs.PROFILE_CREDENTIALS = {
            "p-other": {"adapter": "relay", "key": "x", "base_url": "https://example.com"},
        }
        with self.assertRaises(cs.PlatterCredentialError) as ctx:
            cs.runtime_for_model(self.base, self.shell_glm)
        self.assertIn("p-glm", str(ctx.exception))

    def test_empty_key_raises(self):
        cs.PROFILE_CREDENTIALS = {
            "p-glm": {
                "adapter": "relay",
                "key": "",
                "base_url": "https://open.bigmodel.cn/api/anthropic",
            },
        }
        with self.assertRaises(cs.PlatterCredentialError) as ctx:
            cs.runtime_for_model(self.base, self.shell_glm)
        self.assertIn("empty API key", str(ctx.exception))

    def test_unknown_adapter_raises(self):
        cs.PROFILE_CREDENTIALS = {
            "p-glm": {
                "adapter": "not-a-provider",
                "key": "k",
                "base_url": "https://example.com",
            },
        }
        with self.assertRaises(cs.PlatterCredentialError) as ctx:
            cs.runtime_for_model(self.base, self.shell_glm)
        self.assertIn("unknown adapter", str(ctx.exception))

    def test_no_platter_credentials_returns_host(self):
        cs.PROFILE_CREDENTIALS = {}
        rt = cs.runtime_for_model(self.base, self.shell_glm)
        self.assertIs(rt, self.base)
        self.assertEqual(rt.key, "host-key-must-not-leak")


if __name__ == "__main__":
    unittest.main()
