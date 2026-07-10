import http.client
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from mock_upstream import start_mock
from _capability import loopback_available

HERE = os.path.dirname(__file__)
sys.path.insert(0, HERE)

PROXY = os.path.join(HERE, "..", "proxy", "csp_proxy.py")
SEC = "s3cr3t-test-token"


def _start_capture_upstream():
    bodies = []

    class Capture(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            pass

        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            raw = self.rfile.read(n)
            try:
                bodies.append(json.loads(raw or b"{}"))
            except Exception:
                bodies.append(raw.decode("utf-8", "replace"))
            body = json.dumps({
                "id": "msg_mock", "type": "message", "role": "assistant",
                "model": "mock", "content": [{"type": "text", "text": "ok"}],
                "stop_reason": "end_turn", "stop_sequence": None,
                "usage": {"input_tokens": 1, "output_tokens": 1},
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            self.send_response(404)
            self.end_headers()

    srv = ThreadingHTTPServer(("127.0.0.1", 0), Capture)
    port = srv.server_address[1]
    import threading
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{port}", bodies, srv.shutdown


def _req(url, method="GET", body=None):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method,
                              headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(r, timeout=5) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyAuth(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("json")
        port = 18970  # S0 globally unique port: ProxyAuth
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-auth-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, DEEPSEEK_API_KEY="fake-key",
                   CSP_UPSTREAM_URL=cls.up_url)
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_health_with_secret_ok(self):
        s, _b = _req(f"{self.base}/{SEC}/health")
        self.assertEqual(s, 200)

    def test_health_without_secret_forbidden(self):
        s, _b = _req(f"{self.base}/health")
        self.assertEqual(s, 403)

    def test_messages_without_secret_forbidden_and_upstream_untouched(self):
        before = len(self.hits)
        s, _b = _req(f"{self.base}/v1/messages", "POST",
                     {"model": "claude-opus-4-8", "max_tokens": 10,
                      "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(s, 403)
        self.assertEqual(len(self.hits), before)

    def test_messages_with_secret_forwarded(self):
        before = len(self.hits)
        s, b = _req(f"{self.base}/{SEC}/v1/messages", "POST",
                    {"model": "claude-opus-4-8", "max_tokens": 10,
                     "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(s, 200)
        self.assertEqual(len(self.hits), before + 1)
        self.assertEqual(json.loads(b)["content"][0]["text"], "ok")

    def test_secret_not_in_log(self):
        # /health path does not call log(), so it cannot cover the secret-not-in-log invariant alone.
        # Use POST /v1/messages (hits log()) then assert for real coverage of that invariant.
        s, _b = _req(f"{self.base}/{SEC}/v1/messages", "POST",
                     {"model": "claude-opus-4-8", "max_tokens": 10,
                      "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(s, 200)
        with open(self.logf) as f:
            self.assertNotIn(SEC, f.read())

    def test_unauth_post_closes_connection_no_leak_on_reuse(self):
        # Regression: unauthenticated POST returns 403 before request body is read. With keep-alive,
        # server would resume parsing from leftover body bytes, producing malformed 400 that splices
        # leftover bytes with the next request line back to client, possibly leaking path secret.
        # Reuse one http.client.HTTPConnection for two requests to reproduce/verify fix.
        body = json.dumps({"model": "claude-opus-4-8", "max_tokens": 10,
                           "messages": [{"role": "user", "content": "hi"}]}).encode()
        conn = http.client.HTTPConnection("127.0.0.1", 18970, timeout=5)  # S0 globally unique port: same as ProxyAuth
        received = b""
        try:
            # First request: no secret prefix → 403; body intentionally not read by server.
            conn.request("POST", "/v1/messages", body=body,
                         headers={"Content-Type": "application/json"})
            resp = conn.getresponse()
            received += resp.read()
            self.assertEqual(resp.status, 403)
            # Core assertion: fixed 403 response explicitly declares Connection: close.
            self.assertEqual(resp.getheader("Connection"), "close")

            # Second request: with secret. If server kept connection (unfixed), parses from leftover body,
            # malformed 400 echoes this request line (with secret) to client; assertNotIn catches it in received.
            # When fixed, http.client sees Connection: close on prior response, reconnects cleanly;
            # second request succeeds on new connection or fails closed—neither leaks secret.
            try:
                conn.request("POST", f"/{SEC}/v1/messages", body=body,
                             headers={"Content-Type": "application/json"})
                resp2 = conn.getresponse()
                received += resp2.read()
            except Exception:
                pass
        finally:
            conn.close()
        # Core invariant: regardless of second request outcome, client-received bytes must not contain secret plaintext.
        self.assertNotIn(SEC.encode(), received)

    def test_malformed_content_length_returns_400(self):
        # Regression: malformed Content-Length (non-integer) used to raise ValueError in int() and kill handler;
        # client got empty response/reset. Fixed → proper 400 (invalid_request_error).
        import socket
        payload = (
            f"POST /{SEC}/v1/messages HTTP/1.1\r\n"
            "Host: 127.0.0.1\r\n"
            "Content-Type: application/json\r\n"
            "Content-Length: oops\r\n"
            "Connection: close\r\n"
            "\r\n"
        ).encode()
        s = socket.create_connection(("127.0.0.1", 18970), timeout=5)  # S0 globally unique port: same as ProxyAuth
        try:
            s.sendall(payload)
            resp = b""
            while True:
                chunk = s.recv(4096)
                if not chunk:
                    break
                resp += chunk
        finally:
            s.close()
        status_line = resp.split(b"\r\n", 1)[0]
        self.assertIn(b"400", status_line, f"应为 400，实收：{resp[:120]!r}")
        self.assertIn(b"invalid_request_error", resp)

    def test_malformed_request_structure_returns_400(self):
        # Regression (GPT P1 review): JSON parses but structure wrong (non-object top / messages not array)
        # used to raise AttributeError/TypeError downstream and kill thread with empty client response.
        # Fixed → proper 400 always, upstream never hit.
        before = len(self.hits)
        for bad in ([], "hello", 42, {"messages": None}, {"model": "x"}):
            s, b = _req(f"{self.base}/{SEC}/v1/messages", "POST", bad)
            self.assertEqual(s, 400, f"{bad!r} 应回 400，实收 {s} {b[:120]!r}")
            self.assertIn(b"invalid_request_error", b)
        self.assertEqual(len(self.hits), before, "畸形请求不应打到上游")


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyUpstreamErrorPassthrough(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("status:401")
        port = 18971  # S0 globally unique port: ProxyUpstreamErrorPassthrough
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-401-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, DEEPSEEK_API_KEY="fake-key",
                   CSP_UPSTREAM_URL=cls.up_url)
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_upstream_401_preserved_not_502(self):
        # Fix P3 (GPT review): upstream 401 passthrough (no longer normalized to 502) so verify_key can detect invalid key.
        s, b = _req(f"{self.base}/{SEC}/v1/messages", "POST",
                    {"model": "claude-opus-4-8", "max_tokens": 1,
                     "messages": [{"role": "user", "content": "ping"}]})
        self.assertEqual(s, 401, f"应保留上游 401，实收 {s} {b[:160]!r}")





@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyRelayToolSchemaNormalization(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_base, cls.bodies, cls.stop_up = _start_capture_upstream()
        port = 18997  # S0 globally unique port: ProxyRelayToolSchemaNormalization
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-relay-tools-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, CSP_RELAY_KEY="fake-relay-key",
                   CSP_RELAY_BASE_URL=cls.up_base,
                   CSP_RELAY_MODEL="MiniMax-M2")
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "relay",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_relay_outbound_tools_are_normalized_before_provider(self):
        s, b = _req(f"{self.base}/{SEC}/v1/messages", "POST", {
            "model": "claude-opus-4-8",
            "max_tokens": 10,
            "messages": [{"role": "user", "content": "hi"}],
            "tool_choice": {"type": "tool", "name": "removed"},
            "tools": [
                {"name": "empty", "input_schema": {}},
                {"name": "bad_required", "input_schema": {
                    "type": "object", "properties": [], "required": "q",
                }},
                {"name": "", "input_schema": {"type": "object"}},
            ],
        })
        self.assertEqual(s, 200, b[:160])
        out = self.bodies[-1]
        self.assertEqual(out["model"], "MiniMax-M2")
        self.assertEqual([t["name"] for t in out["tools"]], ["empty", "bad_required"])
        self.assertEqual(out["tools"][0]["input_schema"], {"type": "object", "properties": {}})
        self.assertEqual(out["tools"][1]["input_schema"], {"type": "object", "properties": {}})
        self.assertEqual(out["tool_choice"], {"type": "auto"})


class ProxyOpenAICustomModelsDiscovery(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("openai_models")
        port = 18995
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-openai-models-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, CSP_OPENAI_KEY="fake-openai-key",
                   CSP_OPENAI_BASE_URL=cls.up_url)
        env.pop("CSP_OPENAI_MODEL", None)
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "openai-custom",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_models_fetch_starts_without_model_env(self):
        # Regression v0.3.3 F1: model-fetch scratch runs without CSP_OPENAI_MODEL.
        # Proxy should start and only hit upstream for /models; formal inference still requires model on Rust side.
        s, b = _req(f"{self.base}/{SEC}/v1/models")
        self.assertEqual(s, 200, b[:160])
        body = json.loads(b)
        self.assertEqual([m["id"] for m in body["data"]], ["glm-4.5"])
        self.assertEqual(self.hits, ["/up/v1/models"])


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyOpenAICustomForcedModelList(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("openai_models")
        port = 18998
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-openai-forced-models-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, CSP_OPENAI_KEY="fake-openai-key",
                   CSP_OPENAI_BASE_URL=cls.up_url,
                   CSP_OPENAI_MODEL="glm-4.5")
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "openai-custom",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_formal_proxy_returns_claude_shell_for_science_selector(self):
        # Regression #26: when formal proxy has custom OpenAI model selected, Science may only show
        # claude-* shell ids; real model name goes in display_name, outbound uses force override.
        s, b = _req(f"{self.base}/{SEC}/v1/models")
        self.assertEqual(s, 200, b[:160])
        body = json.loads(b)
        self.assertEqual(body["data"], [{
            "type": "model",
            "id": "claude-opus-4-8",
            "display_name": "glm-4.5",
            "supports_tools": None,
            "created_at": "2026-01-01T00:00:00Z",
        }])
        self.assertEqual(body["first_id"], "claude-opus-4-8")
        self.assertEqual(body["last_id"], "claude-opus-4-8")
        self.assertEqual(self.hits, [], "formal proxy should not expose raw non-claude IDs")
        log = _read(self.logf)
        self.assertIn("GET /v1/models -> openai-custom(force 借壳): glm-4.5", log)
        self.assertNotIn("fake-openai-key", log)


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyOpenAIResponses(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("openai_responses")
        port = 18996
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csp-openai-responses-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, CSP_OPENAI_KEY="fake-openai-key",
                   CSP_OPENAI_BASE_URL=cls.up_url,
                   CSP_OPENAI_MODEL="gpt-5.2")
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "openai-responses",
             "--port", str(port), "--auth-token", SEC, "--log", cls.logf],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                s, _b = _req(f"{cls.base}/{SEC}/health")
                if s == 200:
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)
        cls.stop_up()

    def test_messages_use_responses_endpoint_and_convert_back(self):
        s, b = _req(f"{self.base}/{SEC}/v1/messages", "POST",
                    {"model": "claude-opus-4-8", "max_tokens": 10,
                     "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(s, 200, b[:160])
        body = json.loads(b)
        self.assertEqual(body["content"], [{"type": "text", "text": "ok-responses"}])
        self.assertEqual(body["usage"], {"input_tokens": 2, "output_tokens": 3})
        self.assertIn("/up/v1/responses", self.hits)
        self.assertNotIn("/up/v1/chat/completions", self.hits)

    def test_models_returns_claude_shell_for_science_selector(self):
        self.hits.clear()
        s, b = _req(f"{self.base}/{SEC}/v1/models")
        self.assertEqual(s, 200, b[:160])
        body = json.loads(b)
        self.assertEqual(body["data"], [{
            "type": "model",
            "id": "claude-opus-4-8",
            "display_name": "gpt-5.2",
            "supports_tools": None,
            "created_at": "2026-01-01T00:00:00Z",
        }])
        self.assertEqual(self.hits, [], "formal proxy should not expose raw non-claude IDs")
        log = _read(self.logf)
        self.assertIn("GET /v1/models -> openai-responses(force 借壳): gpt-5.2", log)
        self.assertNotIn("fake-openai-key", log)


if __name__ == "__main__":
    unittest.main()
