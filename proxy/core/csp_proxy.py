#!/usr/bin/env python3
"""Claude Science Proxy (CSP) gateway: forward Claude Science inference to third-party models.

Providers:
  deepseek (default): native Anthropic at api.deepseek.com — passthrough, rename model, swap auth,
                   clamp max_tokens, retry; thinking/tool_use stay native.
  openai-custom / openai-responses: arbitrary OpenAI-compatible roots (base + key + model).
  relay: arbitrary Anthropic-compatible relay (CSP_RELAY_BASE_URL + CSP_RELAY_KEY); passthrough model
         names; /v1/models fetched from upstream for the Science selector.

Security:
  - Strip inbound Science Authorization / x-api-key; never log or forward them.
  - Upstream uses provider keys from env only (memory-resident).
  - Listen on loopback only.

Usage:
  DEEPSEEK_API_KEY=... python3 csp_proxy.py --provider deepseek --port 18991
"""
import os
import sys
from pathlib import Path

# `python3 proxy/core/csp_proxy.py` must resolve the `proxy` package from repo/bundle root.
_REPO_ROOT = Path(__file__).resolve().parents[2]
if str(_REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(_REPO_ROOT))

import argparse
import http.client
import json
import os
import re
import select
import socket
import ssl
import sys
import time
import urllib.error
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

from proxy.dsml import dsml_shim
from proxy.core import http_transport
from proxy.registry import model_discovery, model_registry, model_sort
from proxy.compat import openai_chat_compat, responses_compat, anthropic_compat
from proxy.policy import provider_policy

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
        # Models shown in the Science selector. Science operon rules (k5W/qP_/V2_):
        #   1) id must start with claude-
        #   2) display_name must not match V2_ lowercase multi-hyphen pattern
        #   3) main list: one claude-{opus|sonnet|haiku} per family (max 3);
        #      rest go to "More models" (overflow:true, max 5; 8 shells total).
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
        # Generic Responses-compatible endpoints commonly accept 65536 as a safe ceiling.
        # Chat Completions custom intentionally stays unclamped.
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
# Per-profile upstream credentials for multi-provider platter (CSP_PROFILE_CREDENTIALS JSON).
PROFILE_CREDENTIALS: dict = {}

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


def _runtime_from_cred(cred: dict, base_runtime: RuntimeState) -> RuntimeState:
    adapter = (cred.get("adapter") or "relay").strip()
    prov = dict(PROVIDERS.get(adapter, PROVIDERS["relay"]))
    key = (cred.get("key") or "").strip()
    thinking = (cred.get("thinking_policy") or "").strip() or None
    if adapter == "relay":
        base_url = normalize_relay_base(cred.get("base_url") or "")
        if base_url:
            prov["url"] = base_url + "/v1/messages"
            prov["models_url"] = base_url + "/v1/models"
    elif adapter == "deepseek":
        # Fixed DeepSeek Anthropic endpoint (ignore empty/custom base).
        pass
    elif adapter in ("openai-custom", "openai-responses"):
        base_url = normalize_openai_base(cred.get("base_url") or "")
        suffix = "/responses" if prov.get("api_format") == "openai_responses" else "/chat/completions"
        if base_url:
            prov["url"] = openai_endpoint(base_url, suffix)
            prov["models_url"] = openai_endpoint(base_url, "/models")
    shim = base_runtime.shim_mode
    if adapter == "deepseek":
        try:
            shim = dsml_shim.shim_mode(adapter, prov)
        except Exception:
            shim = "off"
    elif adapter != "deepseek":
        shim = "off"
    return RuntimeState(
        prov=prov,
        prov_name=adapter,
        key=key,
        auth_secret=base_runtime.auth_secret,
        relay_models=base_runtime.relay_models,
        relay_force_model=None,
        relay_thinking=thinking,
        shim_mode=shim,
        model_registry=base_runtime.model_registry,
    )


def runtime_for_model(base_runtime: RuntimeState, model_name: str) -> RuntimeState:
    registry = base_runtime.model_registry
    if not registry or not PROFILE_CREDENTIALS:
        return base_runtime
    route = registry.resolve_route(model_name)
    if not route:
        return base_runtime
    _real_id, profile_id = route
    if not profile_id:
        return base_runtime
    cred = PROFILE_CREDENTIALS.get(profile_id)
    if not isinstance(cred, dict):
        return base_runtime
    return _runtime_from_cred(cred, base_runtime)


def load_profile_credentials():
    global PROFILE_CREDENTIALS
    raw = (os.environ.get("CSP_PROFILE_CREDENTIALS") or "").strip()
    if not raw:
        PROFILE_CREDENTIALS = {}
        return
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        PROFILE_CREDENTIALS = {}
        return
    PROFILE_CREDENTIALS = parsed if isinstance(parsed, dict) else {}

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

# Hop-by-hop headers (RFC 7230 §6.1) plus proxy-connection; never forwarded when
# CSP relays an absolute-form proxied request to an upstream and back.
_HOP_BY_HOP = frozenset({
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailers", "transfer-encoding", "upgrade", "proxy-connection",
})


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
    """Build ProviderState from module globals for compat/policy (nonce uses id(areq) for stability)."""
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
    """POST upstream with retries on connection/body read; no retry on explicit HTTPError responses."""
    return http_transport.post(url, data, headers, log, attempts, timeout)


def open_stream(url, data, headers, attempts=4, timeout=300):
    """Open streaming upstream connection and pre-read first line (retries early empty 200)."""
    return http_transport.open_stream(url, data, headers, log, attempts, timeout)


def _open_stream_with_keepalive(write_chunk, url, data, headers):
    """Emit SSE wire keepalives while waiting for upstream first frame."""
    return http_transport.open_stream_with_keepalive(write_chunk, url, data, headers, log)


def _post_with_keepalive(write_chunk, url, data, headers, keepalive=None):
    """Emit counted SSE keepalives while waiting for a buffered upstream POST."""
    return http_transport.post_with_keepalive(
        write_chunk, url, data, headers, log, keepalive=keepalive)


def http_get_json(url, headers, attempts=3, timeout=30):
    """GET upstream JSON (relay /v1/models fetch). Retry connection errors only."""
    return http_transport.get_json(url, headers, log, attempts, timeout)


def normalize_openai_base(base):
    """OpenAI-compatible roots only: strip accidental /chat/completions or /models suffixes."""
    b = (base or "").strip().rstrip("/")
    for suffix in ("/v1/chat/completions", "/chat/completions",
                   "/v1/responses", "/responses", "/v1/models", "/models"):
        if b.endswith(suffix):
            b = b[: -len(suffix)].rstrip("/")
    return b


def normalize_relay_base(base):
    """Anthropic relay base root; strip accidental /v1/messages or /v1/models suffixes."""
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
    """Upstream auth headers per provider auth_style (x-api-key / bearer / both)."""
    runtime = runtime or current_runtime()
    style = runtime.prov.get("auth_style", "x-api-key")
    h = {}
    if style in ("x-api-key", "both"):
        h["x-api-key"] = runtime.key
    if style in ("bearer", "both"):
        h["Authorization"] = f"Bearer {runtime.key}"
    return h


def fetch_relay_models(runtime=None):
    """Fetch upstream /v1/models and normalize for Science; relay refreshes RELAY_MODELS cache."""
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
    """Build /v1/models response (status, body). Relay/openai fetch live; static for deepseek."""
    runtime = runtime or current_runtime()
    if runtime.model_registry is not None:
        log(f"GET /v1/models -> {runtime.prov_name}(registry): "
            f"{len(runtime.model_registry.entries)} 个模型")
        return runtime.model_registry.models_response()
    if runtime.prov.get("models_url"):
        if runtime.relay_force_model:
            # Formal proxy force: return single shell; outbound resolve_model applies real id.
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
    # deepseek: static selector list
    return model_discovery.static_models_response(runtime.prov["models"])


# ---------- Anthropic -> OpenAI translation ----------
def anthropic_to_openai(req, runtime=None):
    return openai_chat_compat.anthropic_to_openai(req, _provider_state(req, runtime))


def map_tool_choice(tc, tools):
    return openai_chat_compat.map_tool_choice(tc, tools)


def map_responses_tool_choice(tc, tools):
    return responses_compat.map_tool_choice(tc, tools)


def responses_max_output_tokens(req, model, state, has_tools):
    return responses_compat.max_output_tokens(req, model, state, has_tools)


def normalize_responses_tool_parameters(schema):
    return responses_compat.normalize_tool_parameters(schema)


def map_responses_tools(tools):
    return responses_compat.map_tools(tools)


def anthropic_to_openai_responses(req, runtime=None):
    out, _metadata = anthropic_to_openai_responses_with_metadata(req, runtime)
    return out


def anthropic_to_openai_responses_with_metadata(req, runtime=None):
    return responses_compat.anthropic_to_openai_with_metadata(
        req,
        _provider_state(req, runtime),
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
            # Tell client not to reuse connection after we close the socket.
            self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def _sse(self, event, data, flush=False):
        chunk = f"event: {event}\ndata: {json.dumps(data, ensure_ascii=False)}\n\n".encode()
        self.wfile.write(hex(len(chunk))[2:].encode() + b"\r\n" + chunk + b"\r\n")
        if flush:
            self.wfile.flush()

    def _sse_error_and_terminate(self, msg):
        frame = ("event: error\ndata: " + json.dumps(
            {"type": "error", "error": {"type": "api_error", "message": msg}},
            ensure_ascii=False) + "\n\n").encode()
        self.wfile.write(hex(len(frame))[2:].encode() + b"\r\n" + frame + b"\r\n")
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def _emit_openai_stream_preamble(self, model_id):
        """Open a counted SSE shell before the buffered upstream completion returns.

        Science ignores SSE comments and ``ping`` for its 120s idle watchdog; it
        only resets on yielded protocol events. Emit ``message_start`` plus an
        empty text block so subsequent empty ``text_delta`` keepalives count.
        """
        self._sse("message_start", {"type": "message_start", "message": {
            "id": "msg_proxy_pending", "type": "message", "role": "assistant",
            "model": model_id, "content": [], "stop_reason": None, "stop_sequence": None,
            "usage": {"input_tokens": 0, "output_tokens": 0}}}, flush=True)
        self._sse("content_block_start", {"type": "content_block_start", "index": 0,
                  "content_block": {"type": "text", "text": ""}}, flush=True)

    def _auth_ok(self):
        runtime = current_runtime()
        if not runtime.auth_secret:
            return True
        prefix = "/" + runtime.auth_secret
        if self.path == prefix or self.path.startswith(prefix + "/"):
            self.path = self.path[len(prefix):] or "/"
            return True
            # Close connection on auth failure so the client cannot reuse a socket with unread POST body
            # (would splice secret bytes into the next response).
        self.close_connection = True
        self._send_json(403, {"type": "error", "error": {
            "type": "permission_error", "message": "forbidden"}})
        return False

    def _maybe_forward_absolute(self):
        """Handle an absolute-form proxied request (`GET https://host/path …`).

        Some HTTP clients (notably axios, used by several stdio MCP servers) do
        not CONNECT-tunnel when `https_proxy` is set — they send the origin
        request in absolute form to the proxy and expect it to forward. The
        sandbox routes MCP egress through this proxy (direct DNS is blocked), so
        without forwarding those requests fail. We forward to non-Anthropic hosts
        and relay the response, mirroring the openness of `do_CONNECT` (loopback
        listener; we never inject provider keys here — the MCP's own auth headers
        pass through unchanged). Returns True when the request was absolute-form
        and has been fully handled.
        """
        low = self.path.lower()
        if not (low.startswith("http://") or low.startswith("https://")):
            return False
        u = urlsplit(self.path)
        host = (u.hostname or "").lower()
        if not host:
            self._send_json(400, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "absolute URL missing host"}})
            return True
        if _is_blocked_host(host):
            # Mirror CONNECT fast-fail: Anthropic domains look logged-out, no tunnel.
            log(f"FORWARD {self.command} {u.scheme}://{host} -> 401 fast-fail (Anthropic domain)")
            self.close_connection = True
            self._send_json(401, {"type": "error", "error": {
                "type": "authentication_error", "message": "logged out"}})
            return True
        try:
            n = int(self.headers.get("Content-Length") or 0)
            if n < 0:
                raise ValueError("negative length")
        except (ValueError, TypeError):
            self._send_json(400, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "invalid Content-Length"}})
            return True
        body = self.rfile.read(n) if n else None
        target = u.path or "/"
        if u.query:
            target += "?" + u.query
        out_headers = {k: v for k, v in self.headers.items()
                       if k.lower() not in _HOP_BY_HOP}
        out_headers["Host"] = u.netloc.rsplit("@", 1)[-1]
        out_headers["Connection"] = "close"
        port = u.port or (443 if u.scheme == "https" else 80)
        conn = None
        try:
            if u.scheme == "https":
                conn = http.client.HTTPSConnection(
                    host, port, timeout=60, context=ssl.create_default_context())
            else:
                conn = http.client.HTTPConnection(host, port, timeout=60)
            conn.request(self.command, target, body=body, headers=out_headers)
            resp = conn.getresponse()
            data = resp.read()
        except Exception as e:
            log(f"FORWARD {self.command} {u.scheme}://{host}{u.path} -> 502 {e}")
            self.close_connection = True
            self._send_json(502, {"type": "error", "error": {
                "type": "api_error", "message": f"forward proxy: {e}"}})
            if conn:
                try:
                    conn.close()
                except Exception:
                    pass
            return True
        log(f"FORWARD {self.command} {u.scheme}://{host}{u.path} -> {resp.status} ({len(data)}B)")
        self.send_response(resp.status)
        for k, v in resp.getheaders():
            kl = k.lower()
            if kl in _HOP_BY_HOP or kl == "content-length":
                continue
            self.send_header(k, v)
        # Body is fully buffered; advertise the real length and close.
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.close_connection = True
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(data)
        try:
            conn.close()
        except Exception:
            pass
        return True

    def do_GET(self):
        if self._maybe_forward_absolute():
            return
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

    def do_PUT(self):
        if not self._maybe_forward_absolute():
            self._send_json(405, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "method not allowed"}})

    def do_PATCH(self):
        if not self._maybe_forward_absolute():
            self._send_json(405, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "method not allowed"}})

    def do_DELETE(self):
        if not self._maybe_forward_absolute():
            self._send_json(405, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "method not allowed"}})

    def do_HEAD(self):
        if not self._maybe_forward_absolute():
            self._send_json(405, {"type": "error", "error": {
                "type": "invalid_request_error", "message": "method not allowed"}})

    def do_POST(self):
        if self._maybe_forward_absolute():
            return
        if not self._auth_ok():
            return
        # Parse Content-Length inside try so malformed values return 400, not empty response.
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
        # Require JSON object with messages array; avoid AttributeError on malformed body.
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
        # Platter: resolve shell → owning provider before picking Anthropic vs OpenAI path.
        runtime = runtime_for_model(runtime, areq.get("model", ""))
        if runtime.prov["mode"] == "anthropic":
            self._handle_anthropic(areq, runtime)
        else:
            self._handle_openai(areq, runtime)

    # ---- HTTP CONNECT: Anthropic domains fast-fail, others tunneled ----
    def do_CONNECT(self):
        # operon uses https_proxy; self.path is host:port. No path-secret on CONNECT line.
        # Risk is low: loopback-only listener; tunnel does not inject keys or hit /v1/messages.
        # Tightening could use Proxy-Authorization in https_proxy userinfo (needs operon verification).
        target = self.path
        host = target.rsplit(":", 1)[0].strip("[]").lower()
        if _is_blocked_host(host):
            # 401 logged-out (not 403 forbidden) so operon skips org-retry loop.
            log(f"CONNECT {target} -> 401 fast-fail (Anthropic domain)")
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
        """Short CONNECT rejection/ error: empty body + Connection: close."""
        self.send_response(code)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    @staticmethod
    def _tunnel(client, upstream):
        """Bidirectional byte relay between connected sockets until EOF/error."""
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
                if not data:  # peer EOF
                    return
                try:
                    other.sendall(data)
                except Exception:
                    return

    # ---- DeepSeek / relay: native Anthropic passthrough ----
    def _handle_anthropic(self, areq, runtime=None):
        runtime = runtime or current_runtime()
        # Already resolved in do_POST when platter credentials are present.
        if not PROFILE_CREDENTIALS:
            runtime = runtime_for_model(runtime, areq.get("model", ""))
        state = _provider_state(areq, runtime)
        upstream_body, ctx = anthropic_compat.transform_request(areq, state)
        stream = bool(upstream_body.get("stream"))
        n_tools = len(upstream_body.get("tools") or [])
        rules = ",".join(ctx.rule_ids) if ctx.rule_ids else "-"
        log(f"POST /v1/messages  {ctx.src_model}->{ctx.target_model} stream={stream} "
            f"tools={n_tools} msgs={len(upstream_body.get('messages') or [])}  "
            f"rules={rules}  (入站鉴权已剥离, 直连 {runtime.prov_name})")
        # Auth headers per auth_style; KEY stays in memory only.
        headers = {"content-type": "application/json", "anthropic-version": "2023-06-01"}
        headers.update(_upstream_auth_headers(runtime))
        data = json.dumps(upstream_body).encode()
        headers_sent = False
        try:
            if stream:
                # Deliberate stream tradeoff: open the downstream SSE response before
                # upstream TTFT, then send wire keepalives while waiting. Native streams
                # usually emit message_start quickly; openai-custom uses a counted
                # preamble instead (see _handle_openai). Upstream
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
                    # off / no tools → passthrough; detect/rewrite → DSML filter from byte 0.
                    f = anthropic_compat.make_stream_rewriter(ctx)
                    # First upstream bytes must go through the filter in order.
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
                # Pass through upstream 401/403/429; normalize other errors to 502.
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

    # ---- OpenAI compatible: translate to OpenAI; non-stream may replay as SSE ----
    def _handle_openai(self, areq, runtime=None):
        runtime = runtime or current_runtime()
        if not PROFILE_CREDENTIALS:
            runtime = runtime_for_model(runtime, areq.get("model", ""))
        # Same standing web-access guidance as Anthropic passthrough (Science
        # always sends Anthropic-shaped /v1/messages; OpenAI translation reads
        # `system` after this inject). Idempotent via sentinel.
        areq = anthropic_compat.inject_csp_web_access_guidance(areq)
        model_id = areq.get("model", "claude-sonnet-5")
        stream = bool(areq.get("stream"))
        metadata = {"rule_ids": ()}
        if runtime.prov.get("api_format") == "openai_responses":
            oreq, metadata = anthropic_to_openai_responses_with_metadata(areq, runtime)
            msg_count = len(oreq.get("input") or [])
        else:
            oreq = anthropic_to_openai(areq, runtime)
            msg_count = len(oreq.get("messages") or [])
        n_tools = len(oreq.get("tools", []))
        rules = ",".join(metadata.get("rule_ids") or ()) or "-"
        log(f"POST /v1/messages  {model_id}->{oreq['model']} stream={stream} tools={n_tools} "
            f"msgs={msg_count} rules={rules}  (入站鉴权已剥离, {runtime.prov_name})")
        headers = {"Authorization": f"Bearer {runtime.key}", "Content-Type": "application/json"}
        data = json.dumps(oreq).encode()
        headers_sent = False
        placeholder_text_open = False
        try:
            if stream:
                # Science asks for SSE; openai-custom still buffers upstream as a
                # single JSON completion. Open the downstream SSE first, emit a
                # counted preamble (message_start + text block), then empty
                # text_delta keepalives during the wait. Science ignores `ping`
                # and SSE comments for its 120s idle watchdog.
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                self.wfile.flush()
                headers_sent = True
                self._emit_openai_stream_preamble(model_id)
                placeholder_text_open = True

                def _wc(b):
                    if b:
                        self.wfile.write(hex(len(b))[2:].encode() + b"\r\n" + b + b"\r\n")
                        self.wfile.flush()

                raw, _ct = _post_with_keepalive(_wc, runtime.prov["url"], data, headers)
            else:
                raw, _ct = http_post(runtime.prov["url"], data, headers)
            oresp = json.loads(raw)
            if runtime.prov.get("api_format") == "openai_responses":
                aresp = openai_responses_to_anthropic(oresp, model_id)
            else:
                aresp = openai_to_anthropic(oresp, model_id)
            if stream:
                self._replay_as_sse(
                    aresp, headers_already_sent=True,
                    placeholder_text_open=placeholder_text_open)
            else:
                self._send_json(200, aresp)
            log(f"  <- {runtime.prov_name} OK (blocks={len(aresp['content'])} stop={aresp['stop_reason']})")
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")[:400]
            log(f"  !! 上游 HTTP {e.code}: {detail}")
            # Keep upstream 401/403/429 on OpenAI path so verify_key can surface bad keys.
            code = e.code if e.code in (401, 403, 429) else 502
            if headers_sent:
                try:
                    self._sse_error_and_terminate(f"upstream {e.code}: {detail}")
                except BrokenPipeError:
                    pass
            else:
                self._send_json(code, {"type": "error", "error": {"type": "api_error",
                                       "message": f"upstream {e.code}: {detail}"}})
        except BrokenPipeError:
            log("  !! 代理异常: [Errno 32] Broken pipe")
        except Exception as e:
            log(f"  !! 代理异常: {e}")
            if headers_sent:
                try:
                    self._sse_error_and_terminate(str(e))
                except BrokenPipeError:
                    pass
            else:
                try:
                    self._send_json(502, {"type": "error", "error": {"type": "api_error", "message": str(e)}})
                except BrokenPipeError:
                    pass

    def _replay_as_sse(self, aresp, headers_already_sent=False, placeholder_text_open=False):
        if not headers_already_sent:
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
        blocks = aresp.get("content") or [{"type": "text", "text": ""}]
        if placeholder_text_open:
            # message_start + text block 0 already open (keepalive preamble).
            first = blocks[0] if blocks else {"type": "text", "text": ""}
            if first.get("type") == "tool_use":
                self._sse("content_block_stop", {"type": "content_block_stop", "index": 0})
                emit_blocks = blocks
                start_idx = 1
            else:
                text = first.get("text", "")
                if text:
                    self._sse("content_block_delta", {"type": "content_block_delta", "index": 0,
                              "delta": {"type": "text_delta", "text": text}})
                self._sse("content_block_stop", {"type": "content_block_stop", "index": 0})
                emit_blocks = blocks[1:]
                start_idx = 1
        else:
            self._sse("message_start", {"type": "message_start", "message": {
                "id": aresp.get("id", "msg_proxy"), "type": "message", "role": "assistant",
                "model": aresp.get("model"), "content": [], "stop_reason": None, "stop_sequence": None,
                "usage": aresp.get("usage", {"input_tokens": 0, "output_tokens": 0})}})
            self._sse("ping", {"type": "ping"})
            emit_blocks = blocks
            start_idx = 0
        for offset, blk in enumerate(emit_blocks):
            idx = start_idx + offset
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
        self.wfile.flush()


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
    load_profile_credentials()
    PROV_NAME = args.provider
    PROV = PROVIDERS[PROV_NAME]
    LOG = args.log
    KEY = load_key(PROV, args)
    AUTH_SECRET = os.environ.get("CSP_AUTH_TOKEN") or args.auth_token
    # relay: assemble upstream urls from CSP_RELAY_BASE_URL in __main__.
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
        # Scratch model discovery only needs /models; formal inference requires model (Rust enforces).
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
    # DSML shim mode (default off; relay always off; deepseek reads CSP_TOOLUSE_SHIM when capable).
    SHIM_MODE = dsml_shim.shim_mode(PROV_NAME, PROV)
    log(f"CSP proxy listening 127.0.0.1:{args.port}  provider={PROV_NAME}  "
        f"key=已加载(未显示)  上游={PROV['url']}  dsml_shim={SHIM_MODE}  "
        f"registry={'on' if MODEL_REGISTRY else 'off'}")
    # Bind retry ~3s for orphaned proxy still holding the port (Errno 48).
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
