import unittest

import model_registry as mr


class ModelRegistryTests(unittest.TestCase):
    def test_allocates_unique_shells(self):
        reg = mr.ModelRegistry.from_models(["glm-5.2", "glm-4.7"])
        self.assertEqual(reg.routes["claude-opus-4-8"], "glm-5.2")
        self.assertEqual(reg.routes["claude-sonnet-5"], "glm-4.7")
        self.assertEqual(reg.resolve("claude-opus-4-8"), "glm-5.2")
        self.assertEqual(reg.resolve("claude-sonnet-5"), "glm-4.7")

    def test_fallback_routes_background_shells(self):
        reg = mr.ModelRegistry.from_models(
            ["glm-5.2", "glm-4.7"],
            default_model="glm-5.2",
            fast_model="glm-4.7",
        )
        self.assertEqual(reg.resolve("claude-haiku-4-5"), "glm-4.7")
        self.assertEqual(reg.resolve("claude-opus-4-8"), "glm-5.2")

    def test_models_response_uses_shell_ids(self):
        reg = mr.ModelRegistry.from_models(["glm-5.2", "glm-4.7"])
        code, body = reg.models_response()
        self.assertEqual(code, 200)
        ids = [m["id"] for m in body["data"]]
        self.assertEqual(ids, ["claude-opus-4-8", "claude-sonnet-5"])
        self.assertEqual(body["data"][0]["display_name"], "glm-5.2")

    def test_from_json_payload(self):
        reg = mr.ModelRegistry.from_json(
            '{"models":["a","b"],"default_model":"a","fast_model":"b"}'
        )
        self.assertEqual(reg.resolve("claude-opus-4-8"), "a")
        self.assertEqual(reg.resolve("claude-haiku-4-5"), "b")


if __name__ == "__main__":
    unittest.main()
