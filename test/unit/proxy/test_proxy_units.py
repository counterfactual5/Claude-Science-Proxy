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


if __name__ == "__main__":
    unittest.main()
