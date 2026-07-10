import re
import unittest

import model_discovery as md


class ModelDiscoveryTests(unittest.TestCase):
    def test_force_shell_sanitizes_hyphen_display_names(self):
        _, body = md.force_shell_response("glm-5-turbo")
        self.assertEqual(body["data"][0]["display_name"], "glm-5.turbo")

        _, body = md.force_shell_response("glm-5")
        self.assertEqual(body["data"][0]["display_name"], "glm-5.0")

    def test_force_shell_keeps_safe_names(self):
        _, body = md.force_shell_response("glm-5.2")
        self.assertEqual(body["data"][0]["display_name"], "glm-5.2")

        _, body = md.force_shell_response("DeepSeek V4 Pro")
        self.assertEqual(body["data"][0]["display_name"], "DeepSeek V4 Pro")

    def test_normalize_models_response_sanitizes_display_names(self):
        raw = {
            "data": [
                {"id": "glm-5-turbo"},
                {"id": "glm-5.2", "display_name": "glm-5.2"},
            ]
        }
        out, ids = md.normalize_models_response(raw)
        by_id = {m["id"]: m for m in out}
        self.assertEqual(by_id["glm-5-turbo"]["display_name"], "glm-5.turbo")
        self.assertEqual(by_id["glm-5.2"]["display_name"], "glm-5.2")
        self.assertEqual(ids, ["glm-5.2", "glm-5-turbo"])

    def test_static_models_response_sanitizes_display_names(self):
        _, body = md.static_models_response([
            ("claude-opus-4-8", "glm-5"),
            ("claude-haiku-4-5", "DeepSeek V4 Flash"),
        ])
        names = [m["display_name"] for m in body["data"]]
        self.assertEqual(names[0], "glm-5.0")
        self.assertEqual(names[1], "DeepSeek V4 Flash")
        internal = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)+$")
        for name in names:
            self.assertIsNone(internal.match(name))


if __name__ == "__main__":
    unittest.main()
