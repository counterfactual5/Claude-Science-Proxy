"""Unit tests for Kimi / relay history soft-repair + fail-closed validation."""
import json
import unittest

from proxy.compat import kimi_history as kh
from proxy.policy import provider_policy as pp


class StripOrphans(unittest.TestCase):
    def test_orphan_tool_result_downgraded_to_text(self):
        msgs = [
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "call_x", "content": "stale"},
            ]},
        ]
        out = kh.strip_orphan_tool_blocks(msgs)
        self.assertEqual(out[0]["content"][0]["type"], "text")
        self.assertIn("call_x", out[0]["content"][0]["text"])

    def test_trailing_tool_use_stripped(self):
        msgs = [
            {"role": "assistant", "content": [
                {"type": "text", "text": "hi"},
                {"type": "tool_use", "id": "call_1", "name": "bash", "input": {}},
            ]},
        ]
        out = kh.strip_orphan_tool_blocks(msgs)
        types = [b["type"] for b in out[0]["content"]]
        self.assertEqual(types, ["text"])

    def test_unresolved_mid_history_tool_use_stripped(self):
        msgs = [
            {"role": "assistant", "content": [
                {"type": "tool_use", "id": "call_1", "name": "bash", "input": {"cmd": "ls"}},
            ]},
            {"role": "user", "content": "never mind, do something else"},
            {"role": "user", "content": "continue"},
        ]
        out = kh.strip_orphan_tool_blocks(msgs)
        self.assertEqual(out[0]["content"], [{"type": "text", "text": ""}])


class KimiThinking(unittest.TestCase):
    def test_failed_tail_drops_empty_thinking_and_placeholder(self):
        msgs = [
            {"role": "user", "content": "hi"},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "", "signature": ""},
                {"type": "server_tool_use", "id": "s1", "name": "web_search", "input": {}},
            ]},
            {"role": "user", "content": "retry"},
        ]
        rules = []
        out = kh.normalize_kimi_failed_history_tail(msgs, rules)
        # empty assistant between users removed
        self.assertEqual([m["role"] for m in out], ["user", "user"])
        self.assertIn(pp.RULE_HISTORY_KIMI_FAILED_TAIL_NORMALIZE, rules)

    def test_nonempty_unsigned_thinking_fail_closed(self):
        msgs = [
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "plan", "signature": ""},
            ]},
            {"role": "user", "content": "ok"},
        ]
        with self.assertRaises(kh.RelayHistoryError) as ctx:
            kh.validate_kimi_thinking_in_history(msgs)
        self.assertIn("no valid signature", str(ctx.exception))

    def test_whitespace_signature_rejected(self):
        self.assertFalse(kh.kimi_signature_structurally_valid("bad sig"))
        self.assertTrue(kh.kimi_signature_structurally_valid("opaque-sig"))


class PrepareRelay(unittest.TestCase):
    def test_complete_tool_round_preserved(self):
        msgs = [
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "inspect", "signature": "opaque"},
                {"type": "tool_use", "id": "toolu_1", "name": "bash", "input": {"cmd": "ls"}},
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "found"},
            ]},
            {"role": "user", "content": "thanks"},
        ]
        out = kh.prepare_relay_messages(msgs, "kimi-k3")
        self.assertEqual(len(out), 3)
        self.assertEqual(out[0]["content"][0]["signature"], "opaque")
        self.assertEqual(out[0]["content"][1]["id"], "toolu_1")

    def test_tampered_thinking_refuses_upstream(self):
        msgs = [
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "secret", "signature": ""},
                {"type": "text", "text": "hi"},
            ]},
            {"role": "user", "content": "go"},
        ]
        with self.assertRaises(kh.RelayHistoryError):
            kh.prepare_relay_messages(msgs, "kimi-k3")


class NonstreamFilter(unittest.TestCase):
    def test_drops_empty_keeps_signed(self):
        body = json.dumps({
            "content": [
                {"type": "thinking", "thinking": "", "signature": ""},
                {"type": "thinking", "thinking": "plan", "signature": "sig"},
                {"type": "text", "text": "ok"},
            ]
        }).encode()
        out = json.loads(kh.filter_kimi_nonstream_response(body))
        self.assertEqual(
            [b["type"] for b in out["content"]],
            ["thinking", "text"],
        )

    def test_rejects_unsigned_nonempty(self):
        body = json.dumps({
            "content": [{"type": "thinking", "thinking": "x", "signature": ""}]
        }).encode()
        with self.assertRaises(kh.RelayHistoryError):
            kh.filter_kimi_nonstream_response(body)


if __name__ == "__main__":
    unittest.main()
