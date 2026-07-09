#!/usr/bin/env python3
"""Isolated E2E translation proxy: Claude Science (Anthropic Messages) -> Alibaba DashScope (OpenAI compatible).

Design constraints (security):
  - Strip inbound Authorization (Claude OAuth Bearer from Science); never log or forward.
  - Upstream uses only the local .env DashScope key; key stays in memory only, never printed or logged.
  - Listen on loopback only; no outbound connections except DashScope.

Routes:
  GET  /v1/models     -> mapped model list (Anthropic style) for Science model panel
  POST /v1/messages   -> translate to DashScope chat/completions; streaming/non-streaming + basic tool round-trip

Usage:
  DASHSCOPE_API_KEY=... python3 qwen_proxy.py --port 18991
  or   python3 qwen_proxy.py --port 18991 --env-file /path/to/.env --key-name DASHSCOPE_API_KEY
"""
import argparse
import json
import os
import re
import sys
import time
import urllib.request
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DASHSCOPE_URL = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"

# Science requests claude-* model names; map them to Qwen.
# Observed Science sends two kinds: claude-haiku-4-5-20251001 (session title) and claude-opus-4-8 (inference).
MODEL_MAP = {
    "claude-opus-4-8": "qwen-max",
    "claude-sonnet-5": "qwen-plus",
    "claude-sonnet-4-6": "qwen-plus",
    "claude-haiku-4-5-20251001": "qwen-turbo",
    "claude-haiku-4-5": "qwen-turbo",
}
DEFAULT_UPSTREAM = "qwen-plus"
# Qwen tiers have different max_tokens ceilings (turbo 16384, max/plus ~8192). Science agents send
# large max_tokens; over-limit values get 400 from DashScope. Clamp to a safe value all tiers accept.
UPSTREAM_MAX_TOKENS = 8192
_DATE_SUFFIX = re.compile(r"-\d{8}$")


def map_model(name):
    """Map Science claude-* names to upstream Qwen models.
    Tolerates date suffixes (claude-haiku-4-5-20251001) and prefix match; falls back to default."""
    if not name:
        return DEFAULT_UPSTREAM
    if name in MODEL_MAP:
        return MODEL_MAP[name]
    stripped = _DATE_SUFFIX.sub("", name)
    if stripped in MODEL_MAP:
        return MODEL_MAP[stripped]
    for cid, up in MODEL_MAP.items():
        base = _DATE_SUFFIX.sub("", cid)
        if name.startswith(base) or stripped.startswith(base):
            return up
    return DEFAULT_UPSTREAM

LOG = None  # runtime log file path; route/status only, never key or Authorization


def log(msg):
    line = f"[{time.strftime('%H:%M:%S')}] {msg}"
    print(line, flush=True)
    if LOG:
        with open(LOG, "a") as f:
            f.write(line + "\n")


def load_key(args):
    if os.environ.get("DASHSCOPE_API_KEY"):
        return os.environ["DASHSCOPE_API_KEY"].strip()
    if args.env_file and os.path.isfile(args.env_file):
        for raw in open(args.env_file):
            raw = raw.strip()
            if not raw or raw.startswith("#") or "=" not in raw:
                continue
            k, v = raw.split("=", 1)
            if k.strip() == args.key_name:
                return v.strip().strip('"').strip("'")
    return None


# ---------- Anthropic -> OpenAI request translation ----------
def anthropic_to_openai(req):
    msgs = []
    sys_prompt = req.get("system")
    if isinstance(sys_prompt, list):  # system may be a block array
        sys_prompt = "\n".join(b.get("text", "") for b in sys_prompt if isinstance(b, dict))
    if sys_prompt:
        msgs.append({"role": "system", "content": sys_prompt})

    for m in req.get("messages", []):
        role = m.get("role")
        content = m.get("content")
        if isinstance(content, str):
            msgs.append({"role": role, "content": content})
            continue
        # content is a block array
        text_parts, tool_calls, tool_results = [], [], []
        for blk in content or []:
            t = blk.get("type")
            if t == "text":
                text_parts.append(blk.get("text", ""))
            elif t == "tool_use":
                tool_calls.append({
                    "id": blk.get("id"),
                    "type": "function",
                    "function": {"name": blk.get("name"),
                                 "arguments": json.dumps(blk.get("input", {}), ensure_ascii=False)},
                })
            elif t == "tool_result":
                c = blk.get("content")
                if isinstance(c, list):
                    c = "".join(x.get("text", "") for x in c if isinstance(x, dict))
                tool_results.append({"role": "tool", "tool_call_id": blk.get("tool_use_id"),
                                     "content": c if isinstance(c, str) else json.dumps(c, ensure_ascii=False)})
        if role == "assistant" and tool_calls:
            msgs.append({"role": "assistant", "content": "".join(text_parts) or None, "tool_calls": tool_calls})
        elif tool_results:
            msgs.extend(tool_results)
            if text_parts:
                msgs.append({"role": role, "content": "".join(text_parts)})
        else:
            msgs.append({"role": role, "content": "".join(text_parts)})

    out = {
        "model": map_model(req.get("model")),
        "messages": msgs,
        "stream": bool(req.get("stream")),
    }
    if req.get("max_tokens"):
        out["max_tokens"] = min(int(req["max_tokens"]), UPSTREAM_MAX_TOKENS)
    if req.get("temperature") is not None:
        out["temperature"] = req["temperature"]
    if req.get("tools"):
        out["tools"] = [{"type": "function",
                         "function": {"name": t["name"], "description": t.get("description", ""),
                                      "parameters": t.get("input_schema", {})}}
                        for t in req["tools"] if t.get("name")]
    return out


# ---------- OpenAI -> Anthropic response translation (non-streaming) ----------
def openai_to_anthropic(resp, model_id):
    choice = (resp.get("choices") or [{}])[0]
    msg = choice.get("message", {})
    blocks = []
    if msg.get("content"):
        blocks.append({"type": "text", "text": msg["content"]})
    for tc in msg.get("tool_calls") or []:
        fn = tc.get("function", {})
        try:
            args = json.loads(fn.get("arguments") or "{}")
        except Exception:
            args = {}
        blocks.append({"type": "tool_use", "id": tc.get("id"), "name": fn.get("name"), "input": args})
    fr = choice.get("finish_reason")
    stop = {"stop": "end_turn", "length": "max_tokens", "tool_calls": "tool_use"}.get(fr, "end_turn")
    usage = resp.get("usage", {})
    return {
        "id": resp.get("id", "msg_proxy"),
        "type": "message", "role": "assistant", "model": model_id,
        "content": blocks or [{"type": "text", "text": ""}],
        "stop_reason": stop, "stop_sequence": None,
        "usage": {"input_tokens": usage.get("prompt_tokens", 0),
                  "output_tokens": usage.get("completion_tokens", 0)},
    }


def dashscope_call(oreq, attempts=4):
    """Call DashScope (non-streaming). Retry connection-level jitter (SSL EOF, handshake timeout,
    peer disconnect); do not retry explicit server responses (e.g. 400 max_tokens) — re-raise."""
    data = json.dumps(oreq).encode()
    headers = {"Authorization": f"Bearer {KEY}", "Content-Type": "application/json"}
    for i in range(attempts):
        req = urllib.request.Request(DASHSCOPE_URL, data=data, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=180) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError:
            raise  # explicit server response; let caller handle
        except Exception as e:
            if i < attempts - 1:
                log(f"  ~ 上游连接抖动，重试 {i + 1}/{attempts - 1}: {e}")
                time.sleep(0.8 * (i + 1))
                continue
            raise


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "qwen-proxy"

    def log_message(self, *a):
        pass

    def _send_json(self, code, obj):
        body = json.dumps(obj, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _sse(self, event, data):
        chunk = f"event: {event}\ndata: {json.dumps(data, ensure_ascii=False)}\n\n".encode()
        self.wfile.write(hex(len(chunk))[2:].encode() + b"\r\n" + chunk + b"\r\n")

    def do_GET(self):
        # Strip and ignore inbound Authorization (not logged)
        if self.path.startswith("/v1/models"):
            data = [{"type": "model", "id": cid, "display_name": f"{cid} → {up} (Qwen 代理)",
                     "created_at": "2026-01-01T00:00:00Z"} for cid, up in MODEL_MAP.items()]
            log("GET /v1/models -> 返回映射模型列表")
            self._send_json(200, {"data": data, "has_more": False,
                                  "first_id": data[0]["id"], "last_id": data[-1]["id"]})
        elif self.path.startswith("/health"):
            self._send_json(200, {"status": "ok"})
        else:
            self._send_json(404, {"type": "error", "error": {"type": "not_found_error", "message": self.path}})

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n) if n else b"{}"
        if not self.path.startswith("/v1/messages"):
            self._send_json(404, {"type": "error", "error": {"type": "not_found_error", "message": self.path}})
            return
        try:
            areq = json.loads(raw)
        except Exception as e:
            self._send_json(400, {"type": "error", "error": {"type": "invalid_request_error", "message": str(e)}})
            return

        model_id = areq.get("model", "claude-sonnet-5")
        stream = bool(areq.get("stream"))
        # Debug: dump full upstream tools array to disk (named by tool count for main-agent captures)
        dump_dir = os.environ.get("PROXY_DUMP_TOOLS")
        if dump_dir and areq.get("tools"):
            try:
                with open(os.path.join(dump_dir, f"tools_{len(areq['tools'])}_{model_id}.json"), "w") as _f:
                    json.dump(areq["tools"], _f, ensure_ascii=False, indent=2)
            except Exception:
                pass
        oreq = anthropic_to_openai(areq)
        oreq["stream"] = False  # always non-stream upstream: get full tool_calls before replying to Science
        n_tools = len(oreq.get("tools", []))
        log(f"POST /v1/messages  model={model_id}->{oreq['model']} stream={stream} tools={n_tools} "
            f"msgs={len(oreq['messages'])}  (入站 Authorization 已剥离)")

        try:
            oresp = dashscope_call(oreq)
            aresp = openai_to_anthropic(oresp, model_id)
            n_blocks = len(aresp.get("content", []))
            n_tooluse = sum(1 for b in aresp.get("content", []) if b.get("type") == "tool_use")
            if stream:
                self._replay_as_sse(aresp)
                log(f"  <- DashScope OK → 以 SSE 回放（blocks={n_blocks} tool_use={n_tooluse} stop={aresp['stop_reason']}）")
            else:
                self._send_json(200, aresp)
                log(f"  <- DashScope 非流式 OK（blocks={n_blocks} tool_use={n_tooluse} stop={aresp['stop_reason']}）")
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")[:400]
            log(f"  !! DashScope HTTP {e.code}: {detail}")
            self._send_json(502, {"type": "error", "error": {"type": "api_error",
                                  "message": f"upstream {e.code}: {detail}"}})
        except Exception as e:
            log(f"  !! 代理异常: {e}")
            self._send_json(502, {"type": "error", "error": {"type": "api_error", "message": str(e)}})

    def _replay_as_sse(self, aresp):
        """Replay a fully translated Anthropic message to Science as SSE events.
        Text and tool_use blocks restored in full (tool_use params via one input_json_delta)."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        blocks = aresp.get("content") or [{"type": "text", "text": ""}]
        self._sse("message_start", {"type": "message_start", "message": {
            "id": aresp.get("id", "msg_proxy"), "type": "message", "role": "assistant",
            "model": aresp.get("model"), "content": [], "stop_reason": None, "stop_sequence": None,
            "usage": aresp.get("usage", {"input_tokens": 0, "output_tokens": 0})}})
        self._sse("ping", {"type": "ping"})
        for idx, blk in enumerate(blocks):
            if blk.get("type") == "tool_use":
                self._sse("content_block_start", {"type": "content_block_start", "index": idx,
                          "content_block": {"type": "tool_use", "id": blk.get("id"),
                                            "name": blk.get("name"), "input": {}}})
                self._sse("content_block_delta", {"type": "content_block_delta", "index": idx,
                          "delta": {"type": "input_json_delta",
                                    "partial_json": json.dumps(blk.get("input", {}), ensure_ascii=False)}})
            else:
                self._sse("content_block_start", {"type": "content_block_start", "index": idx,
                          "content_block": {"type": "text", "text": ""}})
                self._sse("content_block_delta", {"type": "content_block_delta", "index": idx,
                          "delta": {"type": "text_delta", "text": blk.get("text", "")}})
            self._sse("content_block_stop", {"type": "content_block_stop", "index": idx})
        self._sse("message_delta", {"type": "message_delta",
                  "delta": {"stop_reason": aresp.get("stop_reason", "end_turn"), "stop_sequence": None},
                  "usage": {"output_tokens": aresp.get("usage", {}).get("output_tokens", 0)}})
        self._sse("message_stop", {"type": "message_stop"})
        self.wfile.write(b"0\r\n\r\n")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=18991)
    ap.add_argument("--env-file", default=None)
    ap.add_argument("--key-name", default="DASHSCOPE_API_KEY")
    ap.add_argument("--log", default=None)
    args = ap.parse_args()
    LOG = args.log
    KEY = load_key(args)
    if not KEY:
        print("找不到 DashScope key。用 DASHSCOPE_API_KEY 环境变量，或 --env-file <路径> --key-name <变量名>", file=sys.stderr)
        sys.exit(1)
    log(f"翻译代理启动 127.0.0.1:{args.port}  key=已加载(未显示)  上游=DashScope compatible-mode")
    ThreadingHTTPServer(("127.0.0.1", args.port), H).serve_forever()
