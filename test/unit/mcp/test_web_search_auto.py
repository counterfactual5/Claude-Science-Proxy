"""Unit tests for web-search GENERAL vs LITERATURE auto provider chains."""
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
_SERVER = (
    ROOT
    / "desktop"
    / "src-tauri"
    / "src"
    / "mcp_manager"
    / "web_search_server.py"
)


def _load_web_search():
    spec = importlib.util.spec_from_file_location("csp_web_search_server", _SERVER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


class GeneralAutoChain(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ws = _load_web_search()

    def test_general_free_fallbacks_exclude_wikipedia(self):
        self.assertEqual(
            self.ws.GENERAL_FREE_FALLBACKS,
            ["duckduckgo_ia", "duckduckgo_lite"],
        )
        self.assertNotIn("wikipedia", self.ws.GENERAL_FREE_FALLBACKS)

    def test_literature_auto_still_starts_with_wikipedia(self):
        self.assertEqual(
            self.ws.LITERATURE_FREE_FALLBACKS[0],
            "wikipedia",
        )
        self.assertEqual(
            self.ws.LITERATURE_FREE_FALLBACKS,
            ["wikipedia", "crossref", "arxiv", "pubmed"],
        )

    def test_auto_order_general_no_wikipedia_without_keys(self):
        order = self.ws.auto_order({}, lane="general")
        self.assertEqual(order, ["duckduckgo_ia", "duckduckgo_lite"])
        self.assertNotIn("wikipedia", order)

    def test_auto_order_general_with_keyed_providers(self):
        env = {"BRAVE_SEARCH_API_KEY": "x", "SERPER_API_KEY": "y"}
        order = self.ws.auto_order(env, lane="general")
        self.assertEqual(
            order,
            ["brave", "serper", "duckduckgo_ia", "duckduckgo_lite"],
        )
        self.assertNotIn("wikipedia", order)

    def test_auto_order_literature_keeps_wikipedia(self):
        order = self.ws.auto_order({}, lane="literature")
        self.assertEqual(
            order,
            ["wikipedia", "crossref", "arxiv", "pubmed"],
        )


if __name__ == "__main__":
    unittest.main()
