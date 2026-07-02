import os
import socket
import subprocess
import sys
import threading
import time
import unittest

HERE = os.path.dirname(__file__)
PROXY = os.path.join(HERE, "..", "proxy", "csswitch_proxy.py")
SEC = "streamtok"

# 代理两处 upstream 读都用 r.read(4096)（见 csswitch_proxy.py open_stream/_handle_anthropic）。
# http.client 对「Content-Length 声明过大、提前断连」这种截断走的是 readinto 静默 EOF
# 路径（不抛异常，见 CPython http/client.py HTTPResponse.readinto），无法用来逼出
# 「头已发出后流中途异常」这个场景；chunked 传输编码则不同：分块框架一旦被截断，
# http.client 的 _readinto_chunked/_safe_readinto 会确定性抛 IncompleteRead
# （或底层 socket 层的 ConnectionResetError，取决于 close 时机，两者都是普通
# Exception，代理侧都会被同一个 except 分支捕获）。
# 因此这里改用 chunked：先发一个恰好 4096 字节（与两处 read(4096) 的缓冲区大小
# 对齐）的合法首块，让代理的第一次 read 干净拿到整块、正常发出 200 响应头；
# 再发一个声明了长度却只给一小段就断连的第二块，逼代理循环里的第二次
# read(4096) 在「头已发出」之后抛异常，从而命中被测的「流中断」收尾分支。


def start_dropping_upstream():
    """假上游：chunked 传输，首块恰好填满一次 read(4096)（让代理先正常发出 200
    响应头），随后声明第二块但提前断连，逼代理在头已发出后的下一次 read 抛异常。"""
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
            pad = ":" + "p" * (4096 - len(payload) - 2) + "\n"  # SSE 注释行，纯 padding
            payload += pad
            assert len(payload) == 4096, len(payload)
            head = ("HTTP/1.1 200 OK\r\n"
                    "Content-Type: text/event-stream\r\n"
                    "Transfer-Encoding: chunked\r\n\r\n")
            chunk1 = hex(len(payload))[2:] + "\r\n" + payload + "\r\n"
            chunk2_partial = "1f4\r\n" + "0123456789"  # 声明 500 字节，只发 10 字节就断
            c.sendall((head + chunk1 + chunk2_partial).encode())
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


class StreamInterruption(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.up_url, cls.up_sock = start_dropping_upstream()
        cls.port = 18992
        env = dict(os.environ, DEEPSEEK_API_KEY="fake",
                   CSSWITCH_UPSTREAM_URL=cls.up_url)
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
        self.assertIn(b"200", head.split(b"\r\n")[0])          # 头已发 200
        self.assertIn(b"event: error", tail)                   # 干净的 SSE 错误帧
        self.assertTrue(raw.rstrip().endswith(b"0"))           # chunked 终止块
        self.assertNotIn(b"HTTP/1.1 502", tail)                # 未把 502 拼进流


if __name__ == "__main__":
    unittest.main()
