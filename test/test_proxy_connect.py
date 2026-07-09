"""#3 targeted fast-fail isolation test: hits proxy do_CONNECT only, no Science, no external network.

Covers:
  - CONNECT to Anthropic domains (api.anthropic.com / claude.ai / *.claude.com) → immediate 401 (not logged in);
  - CONNECT to non-Anthropic host (local echo service) → 200 tunnel established with bidirectional byte passthrough.
Local echo proves the passthrough path needs no external network.
"""
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
PORT = 18981  # S0 globally unique port: ProxyConnect
BASE = ("127.0.0.1", PORT)


def _http_get_status(path):
    s = socket.create_connection(BASE, timeout=5)
    try:
        s.sendall(f"GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n".encode())
        buf = s.recv(4096)
    finally:
        s.close()
    return buf.split(b"\r\n", 1)[0]


def _connect(target, timeout=5):
    """Send CONNECT to proxy; return (socket, status line bytes). Caller must close."""
    s = socket.create_connection(BASE, timeout=timeout)
    s.sendall(f"CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n".encode())
    buf = b""
    while b"\r\n\r\n" not in buf:
        chunk = s.recv(4096)
        if not chunk:
            break
        buf += chunk
    return s, buf.split(b"\r\n", 1)[0]


def _start_echo():
    """Start a local TCP echo server (echoes received bytes), return (port, server_socket)."""
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(1)
    port = srv.getsockname()[1]

    def serve():
        try:
            conn, _ = srv.accept()
            with conn:
                while True:
                    data = conn.recv(4096)
                    if not data:
                        break
                    conn.sendall(data)
        except Exception:
            pass

    threading.Thread(target=serve, daemon=True).start()
    return port, srv


@unittest.skipUnless(loopback_available(), "env-blocked: loopback bind/connect not permitted")
class ProxyConnect(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        env = dict(os.environ, DEEPSEEK_API_KEY="fake-key")
        cls.proc = subprocess.Popen(
            [sys.executable, PROXY, "--provider", "deepseek", "--port", str(PORT)],
            env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                if b"200" in _http_get_status("/health"):
                    break
            except Exception:
                pass
            time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls.proc.terminate()
        cls.proc.wait(timeout=5)

    def test_connect_anthropic_domains_fastfail_401(self):
        # 401 (not logged in) not 403 (forbidden): operon treats logged-out and skips org-switch retries.
        for target in ("api.anthropic.com:443", "claude.ai:443",
                       "platform.claude.com:443", "console.anthropic.com:443"):
            s, status = _connect(target)
            s.close()
            self.assertIn(b"401", status, f"{target} 应 fast-fail 401，实收：{status!r}")

    def test_connect_passthrough_tunnels_bytes(self):
        echo_port, srv = _start_echo()
        try:
            s, status = _connect(f"127.0.0.1:{echo_port}")
            try:
                self.assertIn(b"200", status, f"非 Anthropic 主机应建隧道 200，实收：{status!r}")
                # Tunnel up: bytes sent should echo back (via proxy to echo and back).
                s.sendall(b"ping-through-tunnel")
                got = s.recv(4096)
                self.assertEqual(got, b"ping-through-tunnel")
            finally:
                s.close()
        finally:
            srv.close()

    def test_connect_subdomain_of_blocked_is_blocked(self):
        # Subdomains blocked too (sub.anthropic.com), but lookalikes not (notanthropic.com).
        s, status = _connect("sub.anthropic.com:443")
        s.close()
        self.assertIn(b"401", status)


if __name__ == "__main__":
    unittest.main()
