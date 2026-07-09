"""Proxy-level e2e: proves DSML fallback shim is wired into csp_proxy (Codex P0).

Starts a real proxy subprocess with CSP_UPSTREAM_URL pointing at a fake upstream that emits DSML as plain-text
tool calls, then sends real HTTP to the proxy. Asserts:
  - rewrite mode: DSML leaked as text in stream/non-stream is restored to real tool_use blocks,
    stop_reason rewritten to tool_use;
  - off mode (default): byte-level passthrough, DSML stays text, no tool_use (zero regression).

Isolation-layer test: proxy ↔ fake upstream only, never touches Science (iron rule 4).
"""
import json
import os
import socket
import subprocess
import sys
import threading
import time
import unittest

sys.path.insert(0, os.path.dirname(__file__))
from _capability import loopback_available

HERE = os.path.dirname(__file__)
PROXY = os.path.join(HERE, "..", "proxy", "csp_proxy.py")
SEC = "dsmltok"
P2 = "｜｜"   # fullwidth double pipe (issue #8 observed leak shape)

# DSML where model meant web_search but leaked as plain text (same shape as test_dsml_shim wrap_typed).
DSML_TEXT = (
    "<" + P2 + "DSML" + P2 + "tool_calls> "
    "<" + P2 + "DSML" + P2 + 'invoke name="web_search">'
    "<" + P2 + "DSML" + P2 + 'parameter name="query" string="true">GSE207177</' + P2 + "DSML" + P2 + "parameter>"
    "</" + P2 + "DSML" + P2 + "invoke> "
    "</" + P2 + "DSML" + P2 + "tool_calls>")


def _sse(event, obj):
    return f"event: {event}\ndata: {json.dumps(obj, ensure_ascii=False)}\n\n"


def _build_sse():
    frames = "".join([
        _sse("message_start", {"type": "message_start", "message": {
            "id": "msg_up", "type": "message", "role": "assistant", "model": "up",
            "content": [], "stop_reason": None, "stop_sequence": None,
            "usage": {"input_tokens": 1, "output_tokens": 1}}}),
        _sse("content_block_start", {"type": "content_block_start", "index": 0,
             "content_block": {"type": "text", "text": ""}}),
        _sse("content_block_delta", {"type": "content_block_delta", "index": 0,
             "delta": {"type": "text_delta", "text": DSML_TEXT}}),
        _sse("content_block_stop", {"type": "content_block_stop", "index": 0}),
        _sse("message_delta", {"type": "message_delta",
             "delta": {"stop_reason": "end_turn", "stop_sequence": None},
             "usage": {"output_tokens": 5}}),
        _sse("message_stop", {"type": "message_stop"}),
    ])
    return frames.encode("utf-8")


def _build_json():
    return json.dumps({
        "id": "msg_up", "type": "message", "role": "assistant", "model": "up",
        "content": [{"type": "text", "text": DSML_TEXT}],
        "stop_reason": "end_turn", "stop_sequence": None,
        "usage": {"input_tokens": 1, "output_tokens": 5},
    }, ensure_ascii=False).encode("utf-8")


def start_dsml_upstream():
    """Fake upstream: by stream flag in request body, returns DSML text in SSE or non-stream JSON."""
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(5)
    port = srv.getsockname()[1]

    def serve():
        while True:
            try:
                c, _ = srv.accept()
            except OSError:
                return
            req = _read_http_request(c)
            is_stream = b'"stream": true' in req or b'"stream":true' in req
            if is_stream:
                sse = _build_sse()
                head = ("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n"
                        f"Content-Length: {len(sse)}\r\nConnection: close\r\n\r\n").encode()
                c.sendall(head + sse)
            else:
                j = _build_json()
                head = ("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n"
                        f"Content-Length: {len(j)}\r\nConnection: close\r\n\r\n").encode()
                c.sendall(head + j)
            c.close()

    threading.Thread(target=serve, daemon=True).start()
    return f"http://127.0.0.1:{port}/up", srv


def _read_http_request(sock):
    data = b""
    while b"\r\n\r\n" not in data:
        chunk = sock.recv(65536)
        if not chunk:
            return data
        data += chunk

    head, sep, body = data.partition(b"\r\n\r\n")
    content_length = 0
    for line in head.split(b"\r\n"):
        name, colon, value = line.partition(b":")
        if colon and name.strip().lower() == b"content-length":
            try:
                content_length = int(value.strip())
            except ValueError:
                content_length = 0
            break

    while len(body) < content_length:
        chunk = sock.recv(65536)
        if not chunk:
            break
        body += chunk
    return head + sep + body


class _ChunkedSocket:
    def __init__(self, chunks):
        self._chunks = list(chunks)

    def recv(self, _size):
        if not self._chunks:
            return b""
        return self._chunks.pop(0)


class HttpRequestReader(unittest.TestCase):
    def test_reads_body_split_after_headers(self):
        body = b'{"stream": true, "messages":["large enough to split"]}'
        head = (b"POST /up HTTP/1.1\r\nHost: 127.0.0.1\r\n"
                b"Content-Type: application/json\r\n"
                + f"Content-Length: {len(body)}\r\n\r\n".encode("ascii"))
        sock = _ChunkedSocket([head[:17], head[17:], body[:9], body[9:]])
        req = _read_http_request(sock)
        self.assertEqual(req, head + body)


def raw_post(host, port, path, body):
    s = socket.create_connection((host, port), timeout=5)
    req = (f"POST {path} HTTP/1.1\r\nHost: {host}\r\n"
           f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
           f"Connection: close\r\n\r\n").encode() + body
    s.sendall(req)
    chunks = []
    while True:
        d = s.recv(65536)
        if not d:
            break
        chunks.append(d)
    s.close()
    return b"".join(chunks)


def dechunk(raw):
    """Decode proxy chunked response body to raw body; non-chunked returns body as-is."""
    head, _, rest = raw.partition(b"\r\n\r\n")
    if b"chunked" not in head.lower():
        return rest
    out, buf = b"", rest
    while buf:
        line, _, buf = buf.partition(b"\r\n")
        try:
            size = int(line.strip() or b"0", 16)
        except ValueError:
            break
        if size == 0:
            break
        out += buf[:size]
        buf = buf[size + 2:]      # skip chunk trailing \r\n
    return out


def _req_body(stream):
    return json.dumps({
        "model": "claude-opus-4-8", "max_tokens": 100, "stream": stream,
        "tools": [{"name": "web_search",
                   "input_schema": {"type": "object", "properties": {"query": {"type": "string"}}}}],
        "messages": [{"role": "user", "content": "find GSE207177"}],
    }).encode("utf-8")


def launch_proxy(port, shim_mode, upstream):
    env = dict(os.environ, DEEPSEEK_API_KEY="fake", CSP_UPSTREAM_URL=upstream)
    if shim_mode is None:
        env.pop("CSP_TOOLUSE_SHIM", None)
    else:
        env["CSP_TOOLUSE_SHIM"] = shim_mode
    proc = subprocess.Popen(
        [sys.executable, PROXY, "--provider", "deepseek", "--port", str(port), "--auth-token", SEC],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    # Probe: wait for /health to come up
    for _ in range(50):
        try:
            r = raw_post("127.0.0.1", port, f"/{SEC}/v1/messages", b'{"messages":[]}')
            if r:
                break
        except OSError:
            time.sleep(0.1)
    time.sleep(0.3)
    return proc


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class DsmlRewriteWired(unittest.TestCase):
    """rewrite mode: DSML leak restored to real tool_use (core P0 wiring proof)."""

    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_dsml_upstream()
        cls.port = 18979  # S0 globally unique port: DsmlRewriteWired
        cls.proc = launch_proxy(cls.port, "rewrite", cls.up_url)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_streaming_dsml_becomes_tool_use(self):
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", _req_body(True))
        body = dechunk(raw).decode("utf-8", "replace")
        self.assertIn("200", raw.split(b"\r\n", 1)[0].decode())
        self.assertIn('"type": "tool_use"', body)          # synthesized tool_use block
        self.assertIn('"name": "web_search"', body)
        self.assertIn("GSE207177", body)                    # args preserved
        self.assertIn('"stop_reason": "tool_use"', body)    # stop_reason rewritten
        self.assertNotIn("DSML", body)                      # original leak marker digested

    def test_nonstreaming_dsml_becomes_tool_use(self):
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", _req_body(False))
        body = dechunk(raw)
        obj = json.loads(body)
        kinds = [b.get("type") for b in obj["content"]]
        self.assertIn("tool_use", kinds)
        tu = next(b for b in obj["content"] if b["type"] == "tool_use")
        self.assertEqual(tu["name"], "web_search")
        self.assertEqual(tu["input"], {"query": "GSE207177"})
        self.assertEqual(obj["stop_reason"], "tool_use")


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class DsmlOffIsVerbatim(unittest.TestCase):
    """off mode (default): byte-level passthrough, DSML stays text, no tool_use (zero-regression guard)."""

    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_dsml_upstream()
        cls.port = 18980  # S0 globally unique port: DsmlOffIsVerbatim
        cls.proc = launch_proxy(cls.port, None, cls.up_url)   # no env → default off

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_streaming_passes_dsml_verbatim(self):
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", _req_body(True))
        body = dechunk(raw).decode("utf-8", "replace")
        self.assertIn("DSML", body)                          # leak text preserved verbatim
        self.assertNotIn('"type": "tool_use"', body)         # not rewritten

    def test_nonstreaming_passes_dsml_verbatim(self):
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", _req_body(False))
        obj = json.loads(dechunk(raw))
        self.assertEqual([b["type"] for b in obj["content"]], ["text"])
        self.assertIn("DSML", obj["content"][0]["text"])


if __name__ == "__main__":
    unittest.main()
