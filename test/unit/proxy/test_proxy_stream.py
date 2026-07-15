import os
import socket
import subprocess
import sys
import threading
import time
import unittest

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from test.fixtures._paths import PROXY_SCRIPT
from test.fixtures._capability import loopback_available

HERE = os.path.dirname(__file__)
PROXY = str(PROXY_SCRIPT)
SEC = "streamtok"

# Covers two streaming proxy edge cases:
#   1) When upstream sends a tiny first frame and stays open, proxy must forward SSE lines promptly, not wait for EOF/buffer fill;
#   2) When upstream drops after response headers are sent, proxy must end with clean SSE error + chunked terminator.
#
# http.client on truncation (Content-Length too large, early disconnect) takes the readinto silent-EOF path
# (no exception; see CPython http/client.py HTTPResponse.readinto), so it cannot force
# "mid-stream error after headers sent". Chunked encoding differs: once the chunk framing is truncated,
# http.client _readinto_chunked/_safe_readinto deterministically raises IncompleteRead
# (or underlying socket ConnectionResetError depending on close timing; both are plain
# Exception and hit the same except branch on the proxy side).
# So interruption tests use chunked: send one complete valid first chunk so proxy emits 200 headers normally;
# then send a second chunk that declares a length but disconnects after a short prefix, forcing the proxy loop's
# second readline to raise after headers are out, hitting the tested mid-stream cleanup branch.


def start_dropping_upstream():
    """Fake upstream: chunked transport, first chunk complete (proxy emits 200 headers),
    then declares second chunk but disconnects early, forcing readline after headers to raise."""
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
            c.recv(65536)
            payload = ("event: content_block_delta\n"
                       "data: {\"type\":\"content_block_delta\"}\n\n")
            pad = ":" + "p" * (4096 - len(payload) - 2) + "\n"  # SSE comment line, padding only
            payload += pad
            assert len(payload) == 4096, len(payload)
            head = ("HTTP/1.1 200 OK\r\n"
                    "Content-Type: text/event-stream\r\n"
                    "Transfer-Encoding: chunked\r\n\r\n")
            chunk1 = hex(len(payload))[2:] + "\r\n" + payload + "\r\n"
            chunk2_partial = "1f4\r\n" + "0123456789"  # declares 500 bytes, sends 10 then disconnects
            c.sendall((head + chunk1 + chunk2_partial).encode())
            c.close()

    threading.Thread(target=serve, daemon=True).start()
    return f"http://127.0.0.1:{port}/up", srv


def start_slow_first_frame_upstream():
    """Fake upstream: sends a tiny SSE line immediately, then delays tail.
    Old read(4096) waits for EOF/more bytes; line-based read should forward first line at once."""
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
            c.recv(65536)
            head = ("HTTP/1.1 200 OK\r\n"
                    "Content-Type: text/event-stream\r\n"
                    "Transfer-Encoding: chunked\r\n\r\n")
            first = b"event: message_start\n"
            c.sendall(head.encode() + hex(len(first))[2:].encode() + b"\r\n" + first + b"\r\n")
            time.sleep(1.2)
            last = b"data: {\"type\":\"message_start\"}\n\n"
            c.sendall(hex(len(last))[2:].encode() + b"\r\n" + last + b"\r\n0\r\n\r\n")
            c.close()

    threading.Thread(target=serve, daemon=True).start()
    return f"http://127.0.0.1:{port}/up", srv


def start_delayed_response_upstream():
    """Fake upstream: delays response headers and first frame after accepting request.
    Proxy should open downstream SSE to Science first, avoiding client timeout on long upstream TTFT."""
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
            c.recv(65536)
            time.sleep(1.2)
            head = ("HTTP/1.1 200 OK\r\n"
                    "Content-Type: text/event-stream\r\n"
                    "Transfer-Encoding: chunked\r\n\r\n")
            first = b"event: message_start\n"
            c.sendall(head.encode() + hex(len(first))[2:].encode() + b"\r\n" + first + b"\r\n0\r\n\r\n")
            c.close()

    threading.Thread(target=serve, daemon=True).start()
    return f"http://127.0.0.1:{port}/up", srv


def start_status_upstream(status, body=b'{"error":"bad key"}'):
    """Fake upstream that immediately returns an HTTP error status."""
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
            c.recv(65536)
            head = (f"HTTP/1.1 {status} Upstream Error\r\n"
                    "Content-Type: application/json\r\n"
                    f"Content-Length: {len(body)}\r\n\r\n").encode()
            c.sendall(head + body)
            c.close()

    threading.Thread(target=serve, daemon=True).start()
    return f"http://127.0.0.1:{port}/up", srv


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


def raw_post_until(host, port, path, body, needle, timeout=0.7):
    s = socket.create_connection((host, port), timeout=5)
    s.settimeout(timeout)
    req = (f"POST {path} HTTP/1.1\r\nHost: {host}\r\n"
           f"Content-Type: application/json\r\nContent-Length: {len(body)}\r\n"
           f"Connection: close\r\n\r\n").encode() + body
    t0 = time.monotonic()
    s.sendall(req)
    chunks = []
    try:
        while needle not in b"".join(chunks):
            d = s.recv(65536)
            if not d:
                break
            chunks.append(d)
    except socket.timeout:
        pass
    finally:
        elapsed = time.monotonic() - t0
        s.close()
    return b"".join(chunks), elapsed


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class StreamInterruption(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_dropping_upstream()
        cls.port = 18973  # S0 globally unique port: StreamInterruption
        env = dict(os.environ, DEEPSEEK_API_KEY="fake",
                   CSP_UPSTREAM_URL=cls.up_url,
                   PYTHONPATH=str(ROOT))
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(cls.port), "--auth-token", SEC],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_midstream_error_yields_clean_sse_not_spliced_json(self):
        body = b'{"model":"claude-opus-4-8","max_tokens":10,"stream":true,' \
               b'"messages":[{"role":"user","content":"hi"}]}'
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", body)
        head, _, tail = raw.partition(b"\r\n\r\n")
        self.assertIn(b"200", head.split(b"\r\n")[0])          # headers sent with 200
        self.assertIn(b"event: error", tail)                   # clean SSE error frame
        self.assertTrue(raw.rstrip().endswith(b"0"))           # chunked terminator
        self.assertNotIn(b"HTTP/1.1 502", tail)                # no 502 spliced into stream


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class StreamFlushesFirstLine(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_slow_first_frame_upstream()
        cls.port = 18974  # S0 globally unique port: StreamFlushesFirstLine
        env = dict(os.environ, DEEPSEEK_API_KEY="fake",
                   CSP_UPSTREAM_URL=cls.up_url,
                   PYTHONPATH=str(ROOT))
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(cls.port), "--auth-token", SEC],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_first_sse_line_is_forwarded_before_upstream_eof(self):
        body = b'{"model":"claude-opus-4-8","max_tokens":10,"stream":true,' \
               b'"messages":[{"role":"user","content":"hi"}]}'
        raw, elapsed = raw_post_until(
            "127.0.0.1", self.port, f"/{SEC}/v1/messages", body,
            b"event: message_start", timeout=0.7)
        self.assertIn(b"200", raw.split(b"\r\n", 1)[0])
        self.assertIn(b"event: message_start", raw)
        self.assertLess(elapsed, 0.7)


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class StreamHeadersOpenBeforeUpstreamTtft(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_delayed_response_upstream()
        cls.port = 18975  # S0 globally unique port: StreamHeadersOpenBeforeUpstreamTtft
        env = dict(os.environ, DEEPSEEK_API_KEY="fake",
                   CSP_UPSTREAM_URL=cls.up_url,
                   PYTHONPATH=str(ROOT))
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(cls.port), "--auth-token", SEC],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_downstream_sse_keepalive_does_not_wait_for_upstream_first_byte(self):
        # Anthropic passthrough path emits a wire comment while waiting for the
        # first upstream frame (TCP keepalive). Science's idle watchdog needs
        # counted protocol events; that is handled on the openai-custom buffered
        # path via message_start + empty text_delta keepalives.
        body = b'{"model":"claude-opus-4-8","max_tokens":10,"stream":true,' \
               b'"messages":[{"role":"user","content":"hi"}]}'
        raw, elapsed = raw_post_until(
            "127.0.0.1", self.port, f"/{SEC}/v1/messages", body,
            b": csp-keepalive", timeout=1.1)
        self.assertIn(b"HTTP/1.1 200", raw)
        self.assertIn(b"Content-Type: text/event-stream", raw)
        self.assertIn(b": csp-keepalive", raw)
        self.assertLess(elapsed, 1.1)


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class StreamUpstreamStatusAfterHeaders(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_status_upstream(401)
        cls.port = 18976  # S0 globally unique port: StreamUpstreamStatusAfterHeaders
        env = dict(os.environ, DEEPSEEK_API_KEY="fake",
                   CSP_UPSTREAM_URL=cls.up_url,
                   PYTHONPATH=str(ROOT))
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(cls.port), "--auth-token", SEC],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.up_sock.close()

    def test_stream_upstream_http_error_is_sse_error_after_early_200(self):
        body = b'{"model":"claude-opus-4-8","max_tokens":10,"stream":true,' \
               b'"messages":[{"role":"user","content":"hi"}]}'
        raw = raw_post("127.0.0.1", self.port, f"/{SEC}/v1/messages", body)
        head, _, tail = raw.partition(b"\r\n\r\n")
        self.assertIn(b"HTTP/1.1 200", head)
        self.assertIn(b"Content-Type: text/event-stream", head)
        self.assertIn(b"event: error", tail)
        self.assertIn(b"upstream 401", tail)
        self.assertNotIn(b"HTTP/1.1 401", raw)


if __name__ == "__main__":
    unittest.main()
