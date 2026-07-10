import unittest

import model_sort as ms


class ModelSortTests(unittest.TestCase):
    def test_glm_versions_newest_first(self):
        self.assertEqual(
            ms.sort_model_ids(["glm-4.5", "glm-4.7", "glm-5.2"]),
            ["glm-5.2", "glm-4.7", "glm-4.5"],
        )

    def test_glm_minor_versions(self):
        self.assertEqual(
            ms.sort_model_ids(["glm-5.1", "glm-5.2"]),
            ["glm-5.2", "glm-5.1"],
        )

    def test_glm_45_air_before_47(self):
        self.assertEqual(
            ms.sort_model_ids(["glm-4.5-air", "glm-4.7", "glm-4.5"]),
            ["glm-4.7", "glm-4.5-air", "glm-4.5"],
        )

    def test_compare_models_desc(self):
        self.assertLess(ms.compare_models_desc("glm-5.2", "glm-4.5"), 0)
        self.assertGreater(ms.compare_models_desc("glm-4.5", "glm-5.2"), 0)
        self.assertEqual(ms.compare_models_desc("glm-5.2", "glm-5.2"), 0)


if __name__ == "__main__":
    unittest.main()
