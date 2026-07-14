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

    def test_ddg_lite_is_challenge_detects_anomaly_js(self):
        self.assertTrue(
            self.ws._ddg_lite_is_challenge(
                202,
                '<form action="//duckduckgo.com/anomaly.js?sv=lite&cc=botnet"></form>',
            )
        )
        # Valid Lite markup should not be flagged even with HTTP 202.
        self.assertFalse(
            self.ws._ddg_lite_is_challenge(
                200,
                '<a class="result-link" href="https://example.com">ex</a>',
            )
        )
        # Bare "challenge" in a title/snippet must not trip the detector.
        self.assertFalse(
            self.ws._ddg_lite_is_challenge(
                200,
                '<a class="result-link" href="https://example.com/challenge">x</a>',
            )
        )

    def test_empty_general_hint_forbids_wikipedia_fallback_story(self):
        hint = self.ws._EMPTY_GENERAL_HINT
        self.assertIn("does NOT fall back to Wikipedia", hint)
        self.assertIn("NOT required", hint)
        self.assertIn("fell back to", hint)

    def test_general_skips_wikipedia_only_ia_when_lite_available(self):
        """Wiki-only Instant Answer must not short-circuit GENERAL before Lite."""
        calls = []

        def fake_ia(query, max_results, env):
            calls.append("ia")
            return [{
                "title": "AI slop",
                "url": "https://en.wikipedia.org/wiki/AI_slop",
                "snippet": "…",
                "source": "duckduckgo_ia",
            }]

        def fake_lite(query, max_results, env):
            calls.append("lite")
            return [{
                "title": "BBC",
                "url": "https://www.bbc.com/news/ai-slop",
                "snippet": "…",
                "source": "duckduckgo_lite",
            }]

        orig = dict(self.ws.PROVIDERS)
        try:
            self.ws.PROVIDERS["duckduckgo_ia"] = fake_ia
            self.ws.PROVIDERS["duckduckgo_lite"] = fake_lite
            out = self.ws.do_web_search(
                {"query": "AI slop", "max_results": 5}, {}, lane="general",
            )
        finally:
            self.ws.PROVIDERS.clear()
            self.ws.PROVIDERS.update(orig)
        self.assertEqual(calls, ["ia", "lite"])
        self.assertEqual(out["provider"], "duckduckgo_lite")
        self.assertEqual(out["results"][0]["url"], "https://www.bbc.com/news/ai-slop")
        self.assertTrue(
            any("Wikipedia-only" in w for w in out.get("warnings") or [])
        )

    def test_general_keeps_wiki_ia_if_lite_fails(self):
        def fake_ia(query, max_results, env):
            return [{
                "title": "AI slop",
                "url": "https://en.wikipedia.org/wiki/AI_slop",
                "snippet": "…",
                "source": "duckduckgo_ia",
            }]

        def fake_lite(query, max_results, env):
            raise self.ws.HttpError("duckduckgo_lite anti-bot challenge")

        orig = dict(self.ws.PROVIDERS)
        try:
            self.ws.PROVIDERS["duckduckgo_ia"] = fake_ia
            self.ws.PROVIDERS["duckduckgo_lite"] = fake_lite
            out = self.ws.do_web_search(
                {"query": "AI slop", "max_results": 5}, {}, lane="general",
            )
        finally:
            self.ws.PROVIDERS.clear()
            self.ws.PROVIDERS.update(orig)
        self.assertEqual(out["provider"], "duckduckgo_ia")
        self.assertIn("wikipedia.org", out["results"][0]["url"])


if __name__ == "__main__":
    unittest.main()
