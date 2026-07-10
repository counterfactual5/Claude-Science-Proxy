import re
import unittest

from proxy.registry import model_registry as mr
from proxy.registry import model_sort
from functools import cmp_to_key


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
            '{"models":["glm-5.2","glm-4.5"],"default_model":"glm-4.5","fast_model":"glm-4.5"}'
        )
        self.assertEqual(reg.resolve("claude-opus-4-8"), "glm-5.2")
        self.assertEqual(reg.resolve("claude-haiku-4-5"), "glm-4.5")

    def test_display_keeps_version_sort_not_stale_default(self):
        reg = mr.ModelRegistry.from_models(
            ["glm-5.2", "glm-4.7", "glm-4.5"],
            default_model="glm-4.5",
            fast_model="glm-4.5",
        )
        names = [m["display_name"] for m in reg.models_response()[1]["data"]]
        self.assertEqual(names[0], "glm-5.2")
        self.assertEqual(names[-1], "glm-4.5")

    def test_single_haiku_keeps_flagship_on_main(self):
        """A second haiku shell makes Science park glm-4.5 in the main Fast slot."""
        models = [
            "glm-5.2",
            "glm-5.1",
            "glm-5-turbo",
            "glm-5",
            "glm-4.7",
            "glm-4.6",
            "glm-4.5-air",
            "glm-4.5",
        ]
        reg = mr.ModelRegistry.from_payload(
            {"models": models, "default_model": "glm-5.2", "fast_model": "glm-4.5"}
        )
        haiku_shells = [e.shell_id for e in reg.entries if "haiku" in e.shell_id]
        self.assertEqual(haiku_shells, ["claude-haiku-4-5"])
        self.assertEqual(reg.routes["claude-haiku-4-5"], "glm-5-turbo")
        self.assertEqual(
            mr.science_main_display_names(reg),
            ["glm-5.2", "glm-5.1", "glm-5.turbo"],
        )

    def test_science_safe_display_name_avoids_internal_filter(self):
        self.assertEqual(mr.science_safe_display_name("glm-5.2"), "glm-5.2")
        self.assertEqual(mr.science_safe_display_name("glm-5"), "glm-5.0")
        self.assertEqual(mr.science_safe_display_name("glm-5-turbo"), "glm-5.turbo")
        self.assertEqual(mr.science_safe_display_name("glm-4.5-air"), "glm-4.5-air")

    def test_eight_models_all_have_safe_display_names(self):
        models = [
            "glm-5.2",
            "glm-5.1",
            "glm-5-turbo",
            "glm-5",
            "glm-4.7",
            "glm-4.6",
            "glm-4.5-air",
            "glm-4.5",
        ]
        reg = mr.ModelRegistry.from_payload(
            {"models": models, "default_model": "glm-5.2", "fast_model": "glm-4.5"}
        )
        names = [m["display_name"] for m in reg.models_response()[1]["data"]]
        self.assertEqual(len(names), 8)
        internal = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)+$")
        for name in names:
            self.assertIsNone(internal.match(name), msg=f"{name} would be hidden by Science")

    def test_overflow_more_models_version_order(self):
        models = [
            "glm-5.2",
            "glm-5.1",
            "glm-5-turbo",
            "glm-5",
            "glm-4.7",
            "glm-4.6",
            "glm-4.5-air",
            "glm-4.5",
        ]
        reg = mr.ModelRegistry.from_payload(
            {"models": models, "default_model": "glm-5.2", "fast_model": "glm-4.5"}
        )
        overflow = [e for e in reg.entries if e.tier == "overflow"]
        overflow_by_shell = sorted(
            overflow,
            key=lambda e: cmp_to_key(model_sort.compare_models_desc)(e.shell_id),
        )
        self.assertEqual(
            [e.display_name for e in overflow_by_shell],
            ["glm-5.0", "glm-4.7", "glm-4.6", "glm-4.5-air", "glm-4.5"],
        )
        self.assertEqual(reg.resolve("claude-opus-4-5"), "glm-4.5")
        self.assertNotIn("claude-haiku-4-4", reg.routes)


if __name__ == "__main__":
    unittest.main()
