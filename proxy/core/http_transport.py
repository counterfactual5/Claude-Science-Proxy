"""HTTP transport helpers for the Claude Science Proxy (CSP) Python gateway.

This module owns outbound urllib retry/stream mechanics. The main proxy keeps
thin wrappers so older tests can still monkeypatch ``csp_proxy.http_post``
and friends.
"""
import json
import queue
import threading
import time
import urllib.error
import urllib.request


UPSTREAM_UA = "Claude-Science-Proxy/1.0 (+https://github.com/counterfactual5/Claude-Science-Proxy)"


def _with_user_agent(headers):
    return {"User-Agent": UPSTREAM_UA, **headers}


def post(url, data, headers, log_fn, attempts=4, timeout=300):
    """POST upstream and return ``(body_bytes, content_type)``.

    Retries cover connection setup and full body reads; explicit upstream HTTP
    status responses are surfaced without retry so callers can preserve status.
    """
    headers = _with_user_agent(headers)
    for i in range(attempts):
        req = urllib.request.Request(url, data=data, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return r.read(), r.headers.get("Content-Type", "application/json")
        except urllib.error.HTTPError:
            raise
        except Exception as e:
            if i < attempts - 1:
                log_fn(f"  ~ 上游连接抖动，重试 {i + 1}/{attempts - 1}: {e}")
                time.sleep(0.8 * (i + 1))
                continue
            raise


def open_stream(url, data, headers, log_fn, attempts=4, timeout=300):
    """Open upstream stream and pre-read the first line.

    A 200 with immediate empty body is treated as a retryable transport wobble.
    Once the first byte is received, the caller owns the stream and no retry is
    attempted.
    """
    headers = _with_user_agent(headers)
    for i in range(attempts):
        req = urllib.request.Request(url, data=data, headers=headers)
        try:
            r = urllib.request.urlopen(req, timeout=timeout)
            first = r.readline(65536)
            if not first:
                r.close()
                raise ConnectionError("上游 200 但立刻空体")
            return r, first, r.headers.get("Content-Type", "application/json")
        except urllib.error.HTTPError:
            raise
        except Exception as e:
            if i < attempts - 1:
                log_fn(f"  ~ 上游连接抖动，重试 {i + 1}/{attempts - 1}: {e}")
                time.sleep(0.8 * (i + 1))
                continue
            raise


# Science's stream-idle watchdog resets only on yielded protocol events
# (message_start / content_block_* / message_delta). SSE comments are ignored
# at the wire-parser layer, and `event: ping` is explicitly `continue`d in
# Science's Anthropic SSE client — neither resets the 120s idle timer.
#
# For the Anthropic passthrough path we still emit a comment while waiting for
# the first upstream frame (keeps TCP/proxies awake; real message_start usually
# arrives quickly from a native stream). For the OpenAI-compat buffered path the
# caller must open message_start + a text block first, then pass a counted
# keepalive such as an empty content_block_delta text_delta.
_WIRE_KEEPALIVE_COMMENT = b": csp-keepalive\n\n"
_COUNTED_TEXT_DELTA_KEEPALIVE = (
    b"event: content_block_delta\n"
    b'data: {"type": "content_block_delta", "index": 0, '
    b'"delta": {"type": "text_delta", "text": ""}}\n\n'
)


def open_stream_with_keepalive(write_chunk, url, data, headers, log_fn):
    """Wait for upstream first frame while sending downstream SSE wire keepalives."""
    q = queue.Queue(maxsize=1)

    def _open():
        try:
            q.put(("ok", open_stream(url, data, headers, log_fn)))
        except BaseException as e:
            q.put(("err", e))

    threading.Thread(target=_open, daemon=True).start()
    while True:
        try:
            kind, payload = q.get(timeout=1.0)
            if kind == "err":
                raise payload
            return payload
        except queue.Empty:
            write_chunk(_WIRE_KEEPALIVE_COMMENT)


def post_with_keepalive(write_chunk, url, data, headers, log_fn, attempts=4, timeout=300,
                        keepalive=None):
    """Buffered POST while emitting downstream SSE keepalives (OpenAI-compat path).

    openai-custom forces ``stream: false`` upstream then replays as SSE. Default
    keepalive is an empty ``content_block_delta`` that Science counts toward its
    idle watchdog — the caller must already have emitted ``message_start`` and
    opened text block index 0.
    """
    if keepalive is None:
        keepalive = _COUNTED_TEXT_DELTA_KEEPALIVE
    q = queue.Queue(maxsize=1)

    def _post():
        try:
            q.put(("ok", post(url, data, headers, log_fn, attempts, timeout)))
        except BaseException as e:
            q.put(("err", e))

    threading.Thread(target=_post, daemon=True).start()
    while True:
        try:
            kind, payload = q.get(timeout=1.0)
            if kind == "err":
                raise payload
            return payload
        except queue.Empty:
            write_chunk(keepalive)


def get_json(url, headers, log_fn, attempts=3, timeout=30):
    """GET upstream JSON with connection-level retry."""
    headers = _with_user_agent(headers)
    for i in range(attempts):
        req = urllib.request.Request(url, headers=headers, method="GET")
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError:
            raise
        except Exception as e:
            if i < attempts - 1:
                log_fn(f"  ~ 上游连接抖动，重试 {i + 1}/{attempts - 1}: {e}")
                time.sleep(0.6 * (i + 1))
                continue
            raise
