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

HERE = os.path.dirname(__file__)
sys.path.insert(0, HERE)
from mock_upstream import start_mock

PROXY = os.path.join(HERE, "..", "proxy", "csswitch_proxy.py")
SEC = "s3cr3t-test-token"


def _req(url, method="GET", body=None):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(url, data=data, method=method,
                              headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(r, timeout=5) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


class ProxyAuth(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.hits, cls.stop_up = start_mock("json")
        port = 18990
        cls.base = f"http://127.0.0.1:{port}"
        cls.logf = os.path.join(tempfile.gettempdir(), f"csswitch-auth-{port}.log")
        open(cls.logf, "w").close()
        env = dict(os.environ, DEEPSEEK_API_KEY="fake-key",
                   CSSWITCH_UPSTREAM_URL=cls.up_url)
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
        # /health 分支不调用 log()，只测它无法覆盖「secret 不落日志」这条不变量。
        # 改用 POST /v1/messages（会走 log()）之后再断言，才是对该不变量的真实覆盖。
        s, _b = _req(f"{self.base}/{SEC}/v1/messages", "POST",
                     {"model": "claude-opus-4-8", "max_tokens": 10,
                      "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(s, 200)
        with open(self.logf) as f:
            self.assertNotIn(SEC, f.read())

    def test_unauth_post_closes_connection_no_leak_on_reuse(self):
        # 回归：鉴权失败的 POST 在读走请求体之前就返回 403。若连接保持 keep-alive，
        # 服务端下一轮会从残留 body 中间开始解析下一个请求，产出的畸形 400 错误页
        # 会把残留字节和下一条请求行拼在一起回显给客户端，可能带出路径里的 secret。
        # 用同一条 http.client.HTTPConnection 连发两个请求来复现/验证修复。
        body = json.dumps({"model": "claude-opus-4-8", "max_tokens": 10,
                           "messages": [{"role": "user", "content": "hi"}]}).encode()
        conn = http.client.HTTPConnection("127.0.0.1", 18990, timeout=5)
        received = b""
        try:
            # 第一次请求：不带 secret 前缀，触发 403，其请求体故意不被服务端读走。
            conn.request("POST", "/v1/messages", body=body,
                         headers={"Content-Type": "application/json"})
            resp = conn.getresponse()
            received += resp.read()
            self.assertEqual(resp.status, 403)
            # 核心断言：修复后 403 响应显式声明 Connection: close。
            self.assertEqual(resp.getheader("Connection"), "close")

            # 第二次请求：带 secret。若服务端未关连接（未修复），会在残留 body 上
            # 错位解析，产出的畸形 400 会把这条请求行（含 secret）回显给客户端，
            # received 里就会出现 secret 明文，下面的 assertNotIn 会抓到。
            # 已修复时，http.client 见到上一响应带 Connection: close 会自动断开
            # 重连（不会复用被污染的旧 socket），第二次请求要么在一条新连接上
            # 干净地成功，要么因服务端已关闭而抛异常；两者都不泄露 secret。
            try:
                conn.request("POST", f"/{SEC}/v1/messages", body=body,
                             headers={"Content-Type": "application/json"})
                resp2 = conn.getresponse()
                received += resp2.read()
            except Exception:
                pass
        finally:
            conn.close()
        # 核心不变量：不论第二次请求成功、以新连接重试成功还是失败，全程客户端
        # 收到的字节都不能含 secret 明文。
        self.assertNotIn(SEC.encode(), received)


if __name__ == "__main__":
    unittest.main()
