#!/usr/bin/env python3
"""Claude Science Proxy (CSP) gateway: forward Claude Science inference to third-party models.

Providers:
  deepseek (default): native Anthropic at api.deepseek.com — passthrough, rename model, swap auth,
                   clamp max_tokens, retry; thinking/tool_use stay native.
  qwen: DashScope compatible-mode — Anthropic↔OpenAI translation (streaming replays tool_use via SSE).
  openai-custom / openai-responses: arbitrary OpenAI-compatible roots (base + key + model).
  relay: arbitrary Anthropic-compatible relay (CSP_RELAY_BASE_URL + CSP_RELAY_KEY); passthrough model
         names; /v1/models fetched from upstream for the Science selector.

Security:
  - Strip inbound Science Authorization / x-api-key; never log or forward them.
  - Upstream uses provider keys from env only (memory-resident).
  - Listen on loopback only.

Usage:
  DEEPSEEK_API_KEY=... python3 csp_proxy.py --provider deepseek --port 18991
  DASHSCOPE_API_KEY=... python3 csp_proxy.py --provider qwen --port 18991
"""
import argparse
import json
import os
import re
import select
import socket
import sys
import time
import urllib.error
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import dsml_shim
import http_transport
import model_discovery
import model_registry
import openai_chat_compat
import provider_policy
import responses_compat
import anthropic_compat

# DSML shim mode: off (default, byte passthrough) / detect (passthrough + telemetry) / rewrite.
# Set in __main__ via shim_mode(PROV_NAME, PROV) from CSP_TOOLUSE_SHIM.
SHIM_MODE = "off"

# ---------- provider registry ----------
PROVIDERS = {
    "deepseek": {
        "mode": "anthropic",
        "dsml_capable": True,   # only DeepSeek enables DSML shim by default (relay needs explicit opt-in)
        "url": "https://api.deepseek.com/anthropic/v1/messages",
        "key_env": "DEEPSEEK_API_KEY",
        # Models shown in the Science selector. Science enforces two hard rules (binary s0/ZjO/XjO/hB_):
        #   1) id must start with claude-
        #   2) only claude-{opus|sonnet|haiku}-<digits...> land in the main list (one per family);
        #      others go to "More models" (overflow:true).
        # We borrow Science-approved main-list shell ids (opus/haiku), display DeepSeek names,
        # and map back via model_map so both models appear flat without opening More models.
        #   claude-opus-4-8  → "DeepSeek V4 Pro"  (tier0; Science default inference id)
        #   claude-haiku-4-5 → "DeepSeek V4 Flash" (tier2)
        "models": [
            ("claude-opus-4-8", "DeepSeek V4 Pro"),
            ("claude-haiku-4-5", "DeepSeek V4 Flash"),
        ],
        "model_map": {
            # Selector / Science hard-coded claude-* shell ids → real DeepSeek ids
            "claude-opus-4-8": "deepseek-v4-pro",
            "claude-sonnet-5": "deepseek-v4-flash",
            "claude-sonnet-4-6": "deepseek-v4-flash",
            "claude-haiku-4-5": "deepseek-v4-flash",
        },
        # Per-model output caps (provisional until verified against official model list).
        "model_caps": {
            "deepseek-v4-pro": 65536,
            "deepseek-v4-flash": 32768,
        },
        "default_cap": 8192,
        "default_model": "deepseek-v4-flash",
    },
    "qwen": {
        "mode": "openai",
        "url": "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        "key_env": "DASHSCOPE_API_KEY",
        "models": [
            ("qwen-max", "Qwen Max"),
            ("qwen-plus", "Qwen Plus"),
            ("qwen-turbo", "Qwen Turbo"),
        ],
        "model_map": {
            "claude-opus-4-8": "qwen-max",
            "claude-sonnet-5": "qwen-plus",
            "claude-sonnet-4-6": "qwen-plus",
            "claude-haiku-4-5": "qwen-turbo",
        },
        # provisional caps until verified against DashScope docs.
        "model_caps": {
            "qwen-max": 8192,
            "qwen-plus": 8192,
            "qwen-turbo": 8192,
        },
        "default_cap": 8192,
        "default_model": "qwen-plus",
    },
    "openai-custom": {
        "mode": "openai",
        "api_format": "openai_chat",
        "url": None,
        "models_url": None,
        "key_env": "CSP_OPENAI_KEY",
        "auth_style": "bearer",
        "force_model_override": True,
        "models": [],
        "model_map": {},
        "model_caps": {},
        "default_cap": None,
        "default_model": "",
    },
    "openai-responses": {
        "mode": "openai",
        "api_format": "openai_responses",
        "url": None,
        "models_url": None,
        "key_env": "CSP_OPENAI_KEY",
        "auth_style": "bearer",
        "force_model_override": True,
        "models": [],
        "model_map": {},
        "model_caps": {},
        # DashScope Responses rejects values above 65536; generic Responses-compatible
        # endpoints commonly accept this as a safe ceiling. Chat Completions custom
        # intentionally stays unclamped.
        "default_cap": 65536,
        "default_model": "",
    },
    "relay": {
        # Relay: arbitrary Anthropic-compatible base (CSP_RELAY_BASE_URL + token). Passthrough model names.
        # urls assembled in __main__: base + /v1/messages, base + /v1/models.
        "mode": "anthropic",       # reuse native anthropic handler (stream/non-stream/retry)
        "url": None,               # set in __main__
        "models_url": None,        # when set, /v1/models is fetched from upstream
        "key_env": "CSP_RELAY_KEY",
        "passthrough": True,       # resolve_model forwards model id unchanged
        "force_model_override": True,
        "auth_style": "both",      # x-api-key + Authorization: Bearer for relay compatibility
        "models": [],              # populated from upstream fetch
        "model_map": {},
        "model_caps": {},
        "default_cap": None,       # do not clamp max_tokens for relay
        # Fallback when model name empty: Science default inference shell id.
        "default_model": "claude-opus-4-8",
    },
}

PROV = None      # active provider dict (runtime)
KEY = None       # provider key in memory only
LOG = None
PROV_NAME = None  # runtime; defined at import time for unit tests
AUTH_SECRET = None  # unset → no path-secret auth (legacy behavior)
# Relay: last upstream model ids from /v1/models; used to align bare Science ids with dated upstream ids.
RELAY_MODELS = []
# Force-model override from panel (CSP_RELAY_MODEL / CSP_OPENAI_MODEL); None when unset.
RELAY_FORCE_MODEL = None
# Relay thinking policy from template (CSP_RELAY_THINKING): None/adaptive vs enabled (e.g. Kimi).
RELAY_THINKING = None
# Multi-model virtual registry (CSP_MODEL_REGISTRY JSON); routes by shell id instead of force override.
MODEL_REGISTRY = None

@dataclass(frozen=True)
class RuntimeState:
    prov: dict
    prov_name: str
    key: str
    auth_secret: str
    relay_models: list
    relay_force_model: str
    relay_thinking: str
    shim_mode: str
    model_registry: object


def current_runtime():
    """Snapshot mutable module runtime into one object for request handling.

    Tests still patch the legacy globals directly; this thin boundary keeps those tests
    working while reducing how far global state leaks through the request path.
    """
    return RuntimeState(
        prov=PROV,
        prov_name=PROV_NAME,
        key=KEY,
        auth_secret=AUTH_SECRET,
        relay_models=RELAY_MODELS,
        relay_force_model=RELAY_FORCE_MODEL,
        relay_thinking=RELAY_THINKING,
        shim_mode=SHIM_MODE,
        model_registry=MODEL_REGISTRY,
    )

# ---------- CONNECT fast-fail for sandbox "Switching organization" hang ----------
# Sandbox Science blocks on claude.ai/api/oauth/profile when the network cannot reach claude.ai.
# launch-virtual-sandbox.sh points http(s)_proxy at this proxy; do_CONNECT short-circuits
# Anthropic domains below; other hosts tunnel normally (package installs, MCP, etc.).
# Inference still uses 127.0.0.1 via no_proxy.
#
# Why 401 not 403: operon's claudeAiFetch treats CONNECT status as login state —
#   401 → logged-out, passes quickly (`treating as logged-out`);
#   403 → permission/org problem → retries → stuck on "Switching organization".
# Virtual login should look logged-out, so we return 401.
_BLOCKED_SUFFIXES = ("anthropic.com", "claude.ai", "claude.com")


def _is_blocked_host(host):
    h = host.lower().rstrip(".")
    return any(h == s or h.endswith("." + s) for s in _BLOCKED_SUFFIXES)


def log(msg):
    line = f"[{time.strftime('%H:%M:%S')}] {msg}"
    print(line, flush=True)
    if LOG:
        with open(LOG, "a") as f:
            f.write(line + "\n")


def load_key(prov, args):
    env = prov["key_env"]
    if os.environ.get(env):
        return os.environ[env].strip()
    if args.env_file and os.path.isfile(args.env_file):
        for raw in open(args.env_file):
            raw = raw.strip()
            if not raw or raw.startswith("#") or "=" not in raw:
                continue
            k, v = raw.split("=", 1)
            if k.strip() == env:
                return v.strip().strip('"').strip("'")
    return None


def _provider_state(areq, runtime=None):
    """从模块全局一次性组装 ProviderState（骨架侧），传给 compat / policy。
    nonce_factory 捕获 areq，保留旧 id(areq) 派生（字节级等价）。"""
    runtime = runtime or current_runtime()
    return provider_policy.ProviderState(
        policy=provider_policy.policy_from_prov(runtime.prov),
        prov_name=runtime.prov_name,
        relay_force_model=runtime.relay_force_model,
        relay_models=runtime.relay_models,
        relay_thinking=runtime.relay_thinking,
        shim_mode=runtime.shim_mode,
        nonce_factory=lambda: f"{id(areq) & 0xffffff:x}",
        model_registry=runtime.model_registry,
    )


def http_post(url, data, headers, attempts=4, timeout=300):
    """POST 上游；重试覆盖【连接 + 完整读体】（含 SSL EOF、握手超时、对端断开、IncompleteRead），
    对服务端明确响应（HTTPError，如 400）不重试。返回 (body_bytes, content_type)。"""
    return http_transport.post(url, data, headers, log, attempts, timeout)


def open_stream(url, data, headers, attempts=4, timeout=300):
    """打开上游流式连接并预读首行（把「200 但立刻空体」这种抖动也纳入重试）。
    返回 (resp, first_chunk, content_type)；首字节到手后不再重试。"""
    return http_transport.open_stream(url, data, headers, log, attempts, timeout)


def _open_stream_with_keepalive(write_chunk, url, data, headers):
    """等待上游首帧时持续给下游发 SSE 注释心跳。

    下游主请求可能带大量工具定义；部分上游首帧 TTFT 较长时，如果下游在这段
    时间完全收不到 body 字节，会先断开并重试。注释帧是合法 SSE，客户端应忽略内容，
    但能证明连接仍活着。"""
    return http_transport.open_stream_with_keepalive(write_chunk, url, data, headers, log)


def http_get_json(url, headers, attempts=3, timeout=30):
    """GET 上游并解析 JSON（relay 回源拉 /v1/models 用）。连接抖动重试，服务端明确响应不重试。"""
    return http_transport.get_json(url, headers, log, attempts, timeout)


def normalize_openai_base(base):
    """OpenAI 兼容端点存 base root。用户若误填到 /chat/completions 或 /models，
    这里收敛回 root，避免后续双拼。"""
    b = (base or "").strip().rstrip("/")
    for suffix in ("/v1/chat/completions", "/chat/completions",
                   "/v1/responses", "/responses", "/v1/models", "/models"):
        if b.endswith(suffix):
            b = b[: -len(suffix)].rstrip("/")
    return b


def normalize_relay_base(base):
    """Anthropic relay base root。剥掉误填的 /v1/messages、/v1/models 等后缀。"""
    b = (base or "").strip().rstrip("/")
    for suffix in ("/v1/messages", "/v1/models"):
        if b.endswith(suffix):
            b = b[: -len(suffix)].rstrip("/")
    return b


def _ends_with_version_segment(base):
    return bool(re.search(r"/v\d+(?:\.\d+)?$", base))


def openai_endpoint(base, suffix):
    root = normalize_openai_base(base)
    if not _ends_with_version_segment(root):
        root = root + "/v1"
    return root + suffix


def _upstream_auth_headers(runtime=None):
    """上游鉴权头：按当前 provider 的 auth_style 装 x-api-key / bearer / both。
    deepseek 未设 → 默认 x-api-key（保持原状）；relay = both。"""
    runtime = runtime or current_runtime()
    style = runtime.prov.get("auth_style", "x-api-key")
    h = {}
    if style in ("x-api-key", "both"):
        h["x-api-key"] = runtime.key
    if style in ("bearer", "both"):
        h["Authorization"] = f"Bearer {runtime.key}"
    return h


def fetch_relay_models(runtime=None):
    """回源拉上游模型列表，归一化成 Science 可消费的模型列表。relay 会刷新
    RELAY_MODELS 缓存（供 resolve_model 贴合）；openai-custom 仅用于模型发现。返回 list（可空）。"""
    global RELAY_MODELS
    runtime = runtime or current_runtime()
    murl = runtime.prov.get("models_url")
    if not murl:
        return []
    headers = dict(_upstream_auth_headers(runtime))
    if runtime.prov_name == "relay":
        headers["anthropic-version"] = "2023-06-01"
    raw = http_get_json(murl, headers)
    out, ids = model_discovery.normalize_models_response(raw)
    if ids and runtime.prov_name == "relay":
        RELAY_MODELS = ids
    return out


def build_models_response(runtime=None):
    """装配 /v1/models 响应，返回 (状态码, body dict)。协议锁定（修评审 P2-2）：
      - relay/openai-custom 回源成功 → (200, {data:[…含 supports_tools…]})。
      - 回源 HTTPError → (上游同状态码, {error_kind:"upstream", upstream_status, message})，
        绝不吞成 200+静态（否则掩盖坏 key）。builtin 兜底交 Rust 命令决定。
      - 网络异常 → (502, {error_kind:"network", upstream_status:None, message})。
      - 非 relay（无 models_url，deepseek/qwen）→ (200, {静态选择器列表})，行为不变。"""
    runtime = runtime or current_runtime()
    if runtime.model_registry is not None:
        log(f"GET /v1/models -> {runtime.prov_name}(registry): "
            f"{len(runtime.model_registry.entries)} 个模型")
        return runtime.model_registry.models_response()
    if runtime.prov.get("models_url"):
        if runtime.relay_force_model:
            # force（Science 常驻代理）：只返回一个壳，Science 主列表显示真实模型名。
            # 出站由 resolve_model 的 force 分支覆盖，无需 model_map。app 的 fetch_models
            # 不设 RELAY_FORCE_MODEL，故仍走下面回源拿真实 id 供用户选（两个消费者切分）。
            log(f"GET /v1/models -> {runtime.prov_name}(force 借壳): {runtime.relay_force_model}")
            return model_discovery.force_shell_response(runtime.relay_force_model)
        try:
            data = fetch_relay_models(runtime)
            log(f"GET /v1/models -> {runtime.prov_name}(回源): {len(data)} 个模型")
            return model_discovery.live_models_response(data)
        except urllib.error.HTTPError as e:
            detail = ""
            try:
                detail = e.read().decode("utf-8", "replace")[:200]
            except Exception:
                pass
            log(f"GET /v1/models -> {runtime.prov_name} 回源 HTTP {e.code}（保留状态码，不回静态）")
            return e.code, {"error_kind": "upstream", "upstream_status": e.code,
                            "message": f"upstream {e.code}: {detail}"}
        except Exception as e:
            log(f"GET /v1/models -> {runtime.prov_name} 回源网络异常，本地回 502: {e}")
            return 502, {"error_kind": "network", "upstream_status": None, "message": str(e)}
    # 非 relay：静态选择器列表（deepseek/qwen）。
    return model_discovery.static_models_response(runtime.prov["models"])


# ---------- Anthropic -> OpenAI 翻译（qwen 路径） ----------
def anthropic_to_openai(req):
    return openai_chat_compat.anthropic_to_openai(req, _provider_state(req))


def map_tool_choice(tc, tools):
    return openai_chat_compat.map_tool_choice(tc, tools)


def map_responses_tool_choice(tc, tools):
    return responses_compat.map_tool_choice(tc, tools)


def responses_max_output_tokens(req, model, state, has_tools):
    return responses_compat.max_output_tokens(req, model, state, has_tools, _is_dashscope_responses())


def _is_dashscope_responses():
    return responses_compat.is_dashscope_responses(current_runtime().prov)


def normalize_responses_tool_parameters(schema):
    return responses_compat.normalize_tool_parameters(schema)


def map_responses_tools(tools):
    return responses_compat.map_tools(tools, _is_dashscope_responses())


def anthropic_to_openai_responses(req):
    out, _metadata = anthropic_to_openai_responses_with_metadata(req)
    return out


def anthropic_to_openai_responses_with_metadata(req):
    return responses_compat.anthropic_to_openai_with_metadata(
        req,
        _provider_state(req),
        _is_dashscope_responses(),
    )


def openai_to_anthropic(resp, model_id):
    return openai_chat_compat.openai_to_anthropic(resp, model_id)


def _responses_output_text(item):
    return responses_compat.output_text(item)


def openai_responses_to_anthropic(resp, model_id):
    return responses_compat.openai_to_anthropic(resp, model_id)


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "csp-proxy"

    def log_message(self, *a):
        pass

    def _send_json(self, code, obj):
        body = json.dumps(obj, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        if self.close_connection:
            # 主动关闭连接时显式告知客户端，避免其在已关闭的 socket 上复用连接。
            self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def _sse(self, event, data):
        chunk = f"event: {event}\ndata: {json.dumps(data, ensure_ascii=False)}\n\n".encode()
        self.wfile.write(hex(len(chunk))[2:].encode() + b"\r\n" + chunk + b"\r\n")

    def _sse_error_and_terminate(self, msg):
        frame = ("event: error\ndata: " + json.dumps(
            {"type": "error", "error": {"type": "api_error", "message": msg}},
            ensure_ascii=False) + "\n\n").encode()
        self.wfile.write(hex(len(frame))[2:].encode() + b"\r\n" + frame + b"\r\n")
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def _auth_ok(self):
        runtime = current_runtime()
        if not runtime.auth_secret:
            return True
        prefix = "/" + runtime.auth_secret
        if self.path == prefix or self.path.startswith(prefix + "/"):
            self.path = self.path[len(prefix):] or "/"
            return True
        # 鉴权失败时请求体（POST）尚未读取，若保持长连接，服务端下一轮会从残留
        # body 中间开始解析下一个请求，产出的畸形 400 错误页会把残留字节和下一条
        # 请求行拼在一起回显给客户端，可能带出路径里的 secret。这里主动关连接
        # 阻断该复用路径；_send_json 会据 close_connection 追加 Connection: close。
        self.close_connection = True
        self._send_json(403, {"type": "error", "error": {
            "type": "permission_error", "message": "forbidden"}})
        return False

    def do_GET(self):
        if not self._auth_ok():
            return
        runtime = current_runtime()
        if self.path.startswith("/v1/models"):
            code, body = build_models_response(runtime)
            self._send_json(code, body)
        elif self.path.startswith("/health"):
            self._send_json(200, {"status": "ok", "provider": runtime.prov_name})
        else:
            self._send_json(404, {"type": "error", "error": {"type": "not_found_error", "message": self.path}})

    def do_POST(self):
        if not self._auth_ok():
            return
        # Content-Length 解析放在保护内：畸形头（如 "oops" / 负数）应回规范 400，
        # 不能让 int() 抛 ValueError 击穿 handler、给客户端一个空响应。
        try:
            n = int(self.headers.get("Content-Length") or 0)
            if n < 0:
                raise ValueError("negative length")
        except (ValueError, TypeError):
            self._send_json(400, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "invalid Content-Length"}})
            return
        raw = self.rfile.read(n) if n else b"{}"
        if not self.path.startswith("/v1/messages"):
            self._send_json(404, {"type": "error", "error": {"type": "not_found_error", "message": self.path}})
            return
        try:
            areq = json.loads(raw)
        except Exception as e:
            self._send_json(400, {"type": "error", "error": {"type": "invalid_request_error", "message": str(e)}})
            return
        # 结构校验（修 P1 GPT 复审）：顶层必须是对象且 messages 是数组，否则回规范 400。
        # 否则 []/"hello"/{"messages":null} 会在下游 .get / 迭代处抛 AttributeError/TypeError，
        # 击穿线程 → 客户端拿到空响应而非 400。
        if not isinstance(areq, dict) or not isinstance(areq.get("messages"), list):
            self._send_json(400, {"type": "error", "error": {
                "type": "invalid_request_error",
                "message": "request body must be a JSON object with a 'messages' array"}})
            return
        _dd = os.environ.get("PROXY_DUMP_REQ")
        if _dd:
            try:
                with open(os.path.join(_dd, f"req_{areq.get('model','x')}_{len(raw)}.json"), "w") as _f:
                    json.dump({"model": areq.get("model"), "thinking": areq.get("thinking"),
                               "tool_choice": areq.get("tool_choice"),
                               "n_tools": len(areq.get("tools") or [])}, _f, ensure_ascii=False, indent=2)
            except Exception:
                pass
        runtime = current_runtime()
        if runtime.prov["mode"] == "anthropic":
            self._handle_anthropic(areq, runtime)
        else:
            self._handle_openai(areq, runtime)

    # ---- HTTP CONNECT 隧道：Anthropic 域名 fast-fail、其余透传（修 #3） ----
    def do_CONNECT(self):
        # operon 用 https_proxy 走到这里；self.path 形如 "host:port"。
        # 【为何不走 _auth_ok】CONNECT 把目标放在请求行、没有可嵌 path-secret 的位置，
        # operon 的 https_proxy 也带不上 secret。此处不鉴权的实际风险面很小：
        #   - 只监听回环（127.0.0.1），本机进程本就能自行外连，隧道不给它任何新能力；
        #   - 隧道是裸 TCP 转发，不注入上游 key、不经推理端点（那两条仍受 secret 保护）。
        #   即 path-secret 真正守护的边界（第三方 key + 推理端点）未被削弱。
        # 进一步收紧可让 launch 把 secret 放进 https_proxy 的 userinfo 再校验
        # Proxy-Authorization，但需先实测 operon 是否会带该头（否则误伤透传），留待整链联调。
        target = self.path
        host = target.rsplit(":", 1)[0].strip("[]").lower()
        if _is_blocked_host(host):
            # 401（未登录）而非 403（禁止）：让 operon 判 logged-out 秒过，而非当组织问题反复重试。
            log(f"CONNECT {target} -> 401 未登录（Anthropic 域名 fast-fail）")
            self._connect_reply(401)
            return
        try:
            port = int(target.rsplit(":", 1)[1])
        except (ValueError, IndexError):
            self._connect_reply(400)
            return
        try:
            upstream = socket.create_connection((host, port), timeout=10)
        except Exception as e:
            log(f"CONNECT {target} -> 502 上游连不上: {e}")
            self._connect_reply(502)
            return
        self.send_response(200, "Connection Established")
        self.end_headers()
        try:
            self.wfile.flush()
        except Exception:
            pass
        log(f"CONNECT {target} -> 隧道建立，透传")
        try:
            self._tunnel(self.connection, upstream)
        finally:
            try:
                upstream.close()
            except Exception:
                pass
        self.close_connection = True

    def _connect_reply(self, code):
        """CONNECT 的短响应（拒绝/错误）：空体 + 主动关连接。"""
        self.send_response(code)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    @staticmethod
    def _tunnel(client, upstream):
        """在两个已连接 socket 间双向搬字节，直到任一侧 EOF / 出错。"""
        socks = [client, upstream]
        while True:
            try:
                r, _, _ = select.select(socks, [], [])
            except Exception:
                return
            for s in r:
                other = upstream if s is client else client
                try:
                    data = s.recv(65536)
                except Exception:
                    return
                if not data:  # 对端 EOF
                    return
                try:
                    other.sendall(data)
                except Exception:
                    return

    # ---- DeepSeek：Anthropic 原生透传（改模型名+换鉴权+夹 max_tokens+重试） ----
    def _handle_anthropic(self, areq, runtime=None):
        runtime = runtime or current_runtime()
        state = _provider_state(areq, runtime)
        upstream_body, ctx = anthropic_compat.transform_request(areq, state)
        stream = bool(upstream_body.get("stream"))
        n_tools = len(upstream_body.get("tools") or [])
        rules = ",".join(ctx.rule_ids) if ctx.rule_ids else "-"
        log(f"POST /v1/messages  {ctx.src_model}->{ctx.target_model} stream={stream} "
            f"tools={n_tools} msgs={len(upstream_body.get('messages') or [])}  "
            f"rules={rules}  (入站鉴权已剥离, 直连 {runtime.prov_name})")
        # 鉴权头按 provider 的 auth_style（deepseek x-api-key / relay both）。KEY 只驻内存、不入日志。
        headers = {"content-type": "application/json", "anthropic-version": "2023-06-01"}
        headers.update(_upstream_auth_headers(runtime))
        data = json.dumps(upstream_body).encode()
        headers_sent = False
        try:
            if stream:
                # Deliberate stream tradeoff: open the downstream SSE response before
                # upstream TTFT, then send comment keepalives while waiting. This keeps
                # the client from retrying long tool-heavy requests, but upstream
                # HTTP errors after this point must be represented as SSE error frames
                # because the HTTP status line is already committed.
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                self.wfile.flush()
                headers_sent = True

                def _wc(b):
                    if b:
                        self.wfile.write(hex(len(b))[2:].encode() + b"\r\n" + b + b"\r\n")
                        self.wfile.flush()

                r, first, _ct = _open_stream_with_keepalive(_wc, runtime.prov["url"], data, headers)
                with r:
                    # off / 无工具 → None（骨架直接透传，零开销）；detect / rewrite → 统一 filter。
                    f = anthropic_compat.make_stream_rewriter(ctx)
                    # 第一帧同样要过 filter（状态机 / 检测器必须从第 0 字节按序看到全部上游数据）。
                    if f is not None:
                        _wc(f.feed(first))
                    else:
                        _wc(first)
                    while True:
                        try:
                            chunk = r.readline(65536)
                        except Exception as e:
                            log(f"  !! 流中断（头已发），SSE error 收尾: {e}")
                            self._sse_error_and_terminate(str(e))
                            return
                        if not chunk:
                            break
                        if f is not None:
                            _wc(f.feed(chunk))
                        else:
                            _wc(chunk)
                    if f is not None:
                        _wc(f.finalize())
                    self.wfile.write(b"0\r\n\r\n")
                    self.wfile.flush()
                st = f.stats() if f is not None else {}
                if st.get("synthesized"):
                    log(f"  <- {runtime.prov_name} 流式 DSML 改写 OK（合成 tool_use×{st['tool_n']}）")
                elif st.get("found"):
                    log(f"  <- {runtime.prov_name} 流式透传 OK（!! detect：本响应含 DSML 泄漏，未改写）")
                else:
                    log(f"  <- {runtime.prov_name} 流式透传 OK")
            else:
                body_bytes, ct = http_post(runtime.prov["url"], data, headers)
                body_bytes, stats = anthropic_compat.rewrite_nonstream(body_bytes, ctx)
                if stats.get("rewritten"):
                    log(f"  <- {runtime.prov_name} 非流式 DSML 改写 OK（展开 tool_use）")
                elif stats.get("found"):
                    log(f"  !! detect：非流式响应含 DSML 泄漏，未改写")
                self.send_response(200)
                self.send_header("Content-Type", ct)
                self.send_header("Content-Length", str(len(body_bytes)))
                self.end_headers()
                headers_sent = True
                self.wfile.write(body_bytes)
                if "rewritten" not in stats:
                    log(f"  <- {runtime.prov_name} 非流式透传 OK")
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")[:400]
            log(f"  !! 上游 HTTP {e.code}: {detail}")
            if not headers_sent:
                # 上游鉴权 / 额度类状态码原样透传（401/403/429），其余归一化 502。
                code = e.code if e.code in (401, 403, 429) else 502
                self._send_json(code, {"type": "error", "error": {
                    "type": "api_error", "message": f"upstream {e.code}: {detail}"}})
            else:
                try:
                    self._sse_error_and_terminate(f"upstream {e.code}: {detail}")
                except Exception:
                    pass
        except Exception as e:
            log(f"  !! 代理异常: {e}")
            if headers_sent:
                try:
                    self._sse_error_and_terminate(str(e))
                except Exception:
                    pass
            else:
                self._send_json(502, {"type": "error", "error": {
                    "type": "api_error", "message": str(e)}})

    # ---- Qwen：翻译到 OpenAI，非流式取全再按需 SSE 回放 ----
    def _handle_openai(self, areq, runtime=None):
        runtime = runtime or current_runtime()
        model_id = areq.get("model", "claude-sonnet-5")
        stream = bool(areq.get("stream"))
        metadata = {"rule_ids": ()}
        if runtime.prov.get("api_format") == "openai_responses":
            oreq, metadata = anthropic_to_openai_responses_with_metadata(areq)
            msg_count = len(oreq.get("input") or [])
        else:
            oreq = anthropic_to_openai(areq)
            msg_count = len(oreq.get("messages") or [])
        n_tools = len(oreq.get("tools", []))
        rules = ",".join(metadata.get("rule_ids") or ()) or "-"
        log(f"POST /v1/messages  {model_id}->{oreq['model']} stream={stream} tools={n_tools} "
            f"msgs={msg_count} rules={rules}  (入站鉴权已剥离, {runtime.prov_name})")
        headers = {"Authorization": f"Bearer {runtime.key}", "Content-Type": "application/json"}
        data = json.dumps(oreq).encode()
        try:
            raw, _ct = http_post(runtime.prov["url"], data, headers)
            oresp = json.loads(raw)
            if runtime.prov.get("api_format") == "openai_responses":
                aresp = openai_responses_to_anthropic(oresp, model_id)
            else:
                aresp = openai_to_anthropic(oresp, model_id)
            if stream:
                self._replay_as_sse(aresp)
            else:
                self._send_json(200, aresp)
            log(f"  <- {runtime.prov_name} OK (blocks={len(aresp['content'])} stop={aresp['stop_reason']})")
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")[:400]
            log(f"  !! 上游 HTTP {e.code}: {detail}")
            # 修 P2（GPT 复审）：OpenAI 翻译路径（qwen 等）同样保留上游 401/403/429，
            # 别一律归一化 502——否则 verify_key 无法准确提示「key 无效」。其余仍归 502。
            code = e.code if e.code in (401, 403, 429) else 502
            self._send_json(code, {"type": "error", "error": {"type": "api_error",
                                   "message": f"upstream {e.code}: {detail}"}})
        except Exception as e:
            log(f"  !! 代理异常: {e}")
            self._send_json(502, {"type": "error", "error": {"type": "api_error", "message": str(e)}})

    def _replay_as_sse(self, aresp):
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
    ap.add_argument("--provider", default=os.environ.get("CSP_PROVIDER", "deepseek"),
                    choices=list(PROVIDERS.keys()))
    ap.add_argument("--port", type=int, default=18991)
    ap.add_argument("--env-file", default=None)
    ap.add_argument("--log", default=None)
    ap.add_argument("--auth-token", default=None)
    ap.add_argument("--relay-base", default=None,
                    help="relay provider 的中转站 base_url（也可用环境变量 CSP_RELAY_BASE_URL）")
    ap.add_argument("--openai-base", default=None,
                    help="openai-custom provider 的 OpenAI base root（也可用环境变量 CSP_OPENAI_BASE_URL）")
    args = ap.parse_args()
    reg_raw = (os.environ.get("CSP_MODEL_REGISTRY") or "").strip()
    if reg_raw:
        MODEL_REGISTRY = model_registry.ModelRegistry.from_json(reg_raw)
    PROV_NAME = args.provider
    PROV = PROVIDERS[PROV_NAME]
    LOG = args.log
    KEY = load_key(PROV, args)
    AUTH_SECRET = os.environ.get("CSP_AUTH_TOKEN") or args.auth_token
    # relay：按中转站 base_url 装配上游端点（base + /v1/messages、base + /v1/models）。
    if PROV_NAME == "relay":
        base = normalize_relay_base(
            os.environ.get("CSP_RELAY_BASE_URL") or args.relay_base or "")
        if not base or not re.match(r"^https?://", base):
            print("relay 需要中转站 base_url（http(s)://…）。用 --relay-base 或环境变量 "
                  "CSP_RELAY_BASE_URL 提供。", file=sys.stderr)
            sys.exit(1)
        PROV = dict(PROV)
        PROV["url"] = base + "/v1/messages"
        PROV["models_url"] = base + "/v1/models"
        forced = (os.environ.get("CSP_RELAY_MODEL") or "").strip()
        if forced and MODEL_REGISTRY is None:
            RELAY_FORCE_MODEL = forced
        RELAY_THINKING = (os.environ.get("CSP_RELAY_THINKING") or "").strip() or None
    elif PROV_NAME in ("openai-custom", "openai-responses"):
        base = normalize_openai_base(os.environ.get("CSP_OPENAI_BASE_URL") or args.openai_base or "")
        if not base or not re.match(r"^https?://", base):
            print(f"{PROV_NAME} 需要 OpenAI base root（http(s)://…）。用 --openai-base 或环境变量 "
                  "CSP_OPENAI_BASE_URL 提供。", file=sys.stderr)
            sys.exit(1)
        forced = (os.environ.get("CSP_OPENAI_MODEL") or "").strip()
        PROV = dict(PROV)
        suffix = "/responses" if PROV.get("api_format") == "openai_responses" else "/chat/completions"
        PROV["url"] = openai_endpoint(base, suffix)
        PROV["models_url"] = openai_endpoint(base, "/models")
        # 模型发现 scratch 只需要 /models，不能要求 CSP_OPENAI_MODEL；
        # 正式推理由 Rust 侧 relay_missing_model + Message 探针保证 model 必填。
        if forced and MODEL_REGISTRY is None:
            PROV["default_model"] = forced
            RELAY_FORCE_MODEL = forced
    _up = os.environ.get("CSP_UPSTREAM_URL")
    if _up:
        PROV = dict(PROV)
        PROV["url"] = _up
    if not KEY:
        print(f"找不到 {PROV['key_env']}。用环境变量或 --env-file <路径> 提供。", file=sys.stderr)
        sys.exit(1)
    # DSML 兜底 shim 模式（默认 off；relay 恒 off；deepseek 且 dsml_capable 才读环境变量）。
    SHIM_MODE = dsml_shim.shim_mode(PROV_NAME, PROV)
    log(f"CSP proxy listening 127.0.0.1:{args.port}  provider={PROV_NAME}  "
        f"key=已加载(未显示)  上游={PROV['url']}  dsml_shim={SHIM_MODE}  "
        f"registry={'on' if MODEL_REGISTRY else 'off'}")
    # 绑定重试：上次会话遗留的孤儿代理可能还占着端口（app 侧会主动清，但退干净需一点时间）。
    # 重试 ~3s 等端口释放，避免一次绑不上就直接失败（Errno 48）。
    srv = None
    for attempt in range(10):
        try:
            srv = ThreadingHTTPServer(("127.0.0.1", args.port), H)
            break
        except OSError as e:
            if attempt == 9:
                print(f"[csp] port {args.port} bind failed: {e}. "
                      f"Port may be in use (stop the other process, or pick another port in Advanced settings).",
                      file=sys.stderr, flush=True)
                sys.exit(2)
            time.sleep(0.3)
    srv.serve_forever()
