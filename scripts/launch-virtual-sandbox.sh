#!/bin/zsh
# Launch an [isolated + virtual-login] Claude Science sandbox:
#   Uses locally forged virtual OAuth so Science thinks it is logged in (virtual@localhost.invalid);
#   inference is routed via ANTHROPIC_BASE_URL to this project's translation proxy → Qwen.
#   Inference uses zero Anthropic and zero real credentials; however, during startup Science may still
#   try its hard-coded profile/account endpoints (api.anthropic.com). Those failures do not block use,
#   so we do not claim absolute "zero Anthropic contact" (consistent with README disclaimer).
#
# Iron-rule safeguards (see CLAUDE.md):
#   - Separate HOME + data-dir + port; never modify/delete real ~/.claude-science; never use port 8765
#   - Read-only APFS clone of runtime assets from real ~/.claude-science (bin/conda/runtime/seed-assets); never copy real login credentials or user data
#   - Sandbox gets [forged fake credentials] (make-virtual-oauth.mjs), unrelated to real OAuth
#   - encryption.key keychain mirror account is derived by [path hash]; sandbox and real are naturally isolated
#
# Usage:
#   Start proxy first: DEEPSEEK_API_KEY=... python3 proxy/csp_proxy.py --provider deepseek --port 18991
#   Then sandbox: scripts/launch-virtual-sandbox.sh [--port 8990] [--proxy-url http://127.0.0.1:18991]
set -euo pipefail

PROJ="${0:A:h:h}"
SANDBOX_HOME="${SANDBOX_HOME:-$PROJ/.sandbox/home}"
DATA_DIR="$SANDBOX_HOME/.claude-science"   # = auth_dir (Science derives from HOME)
REAL_DIR="$HOME/.claude-science"
APP_BIN="/Applications/Claude Science.app/Contents/Resources/bin/claude-science"
BIN="${SCIENCE_BIN:-}"
PORT=8990
PROXY_URL="http://127.0.0.1:18991"
EMAIL="virtual@localhost.invalid"
DRY_RUN=0
SKIP_FORGE=0   # set to 1 when called from app: OAuth forged in-process by Rust; this script skips node

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2;;
    --proxy-url) PROXY_URL="$2"; shift 2;;
    --email) EMAIL="$2"; shift 2;;
    --dry-run) DRY_RUN=1; shift;;
    --skip-oauth-forge) SKIP_FORGE=1; shift;;
    *) echo "未知参数: $1"; exit 1;;
  esac
done

# —— Iron-rule assertion: never use real directory / real port ——
[[ "$PORT" =~ ^[0-9]+$ ]] || { echo "拒绝：端口不是合法整数（$PORT）"; exit 1; }
if (( 10#${PORT} == 8765 )); then echo "拒绝：端口 8765 是真实实例保留端口"; exit 1; fi
_dd_real="${DATA_DIR:A}"; _real_real="${REAL_DIR:A}"
if [[ "$_dd_real" == "$_real_real" ]]; then echo "拒绝：data-dir 的真实路径指向真实目录"; exit 1; fi
if [[ "$DRY_RUN" == "1" ]]; then echo "DRY-RUN OK：护栏通过，未启动沙箱。"; exit 0; fi

# —— First run: clone runtime assets; never copy real login credentials ——
if [[ ! -d "$DATA_DIR/bin" ]]; then
  echo "首次初始化沙箱运行时（APFS 克隆，只拷运行时、不拷真实登录）…"
  mkdir -p "$DATA_DIR"
  for asset in bin conda runtime seed-assets; do
    if [[ -d "$REAL_DIR/$asset" ]]; then
      cp -Rc "$REAL_DIR/$asset" "$DATA_DIR/$asset"
    fi
  done
  echo "运行时就绪。"
fi

# During ~/.csswitch → ~/.csp migration, atomic_copy writes 0600 and drops +x; cp -Rc usually preserves source perms.
# Idempotently fix conda/bin and bin/claude-science on every start to avoid micromamba permission denied in Environments.
if [[ -f "$DATA_DIR/bin/claude-science" ]]; then
  chmod +x "$DATA_DIR/bin/claude-science" 2>/dev/null || true
fi
if [[ -d "$DATA_DIR/conda/bin" ]]; then
  chmod +x "$DATA_DIR/conda/bin"/* 2>/dev/null || true
fi

# Priority: explicit SCIENCE_BIN > cloned runtime inside sandbox > App-bundled binary.
# Prefer sandbox runtime; App-bundled binary is fallback only.
if [[ -z "$BIN" ]]; then
  if [[ -x "$DATA_DIR/bin/claude-science" ]]; then
    BIN="$DATA_DIR/bin/claude-science"
  else
    BIN="$APP_BIN"
  fi
fi
if [[ ! -x "$BIN" ]]; then echo "找不到 Science 二进制: $BIN"; exit 1; fi

# —— Sandbox-only keychain (eliminates "keychain not found" dialogs) ——
# Science mirrors encryption.key into the macOS keychain. Sandbox HOME has no keychain,
# so securityd reports "default keychain not found" → repeated "restore to default" dialogs.
# Here we create a separate, empty-password, non-auto-locking login.keychain-db inside [sandbox HOME]
# and set it as default only in the HOME=$SANDBOX_HOME context. Real login keychain
# (~/Library/Keychains) is never modified or touched.
SANDBOX_KC="$SANDBOX_HOME/Library/Keychains/login.keychain-db"
if [[ ! -f "$SANDBOX_KC" ]]; then
  echo "创建沙箱专属钥匙串（隔离，空密码，不自动锁）…"
  mkdir -p "$SANDBOX_HOME/Library/Keychains"
  HOME="$SANDBOX_HOME" security create-keychain -p "" "$SANDBOX_KC" || true
fi
# On every start: add to sandbox search list, set default, unlock, disable auto-lock (sandbox HOME only)
HOME="$SANDBOX_HOME" security list-keychains -d user -s "$SANDBOX_KC" >/dev/null 2>&1 || true
HOME="$SANDBOX_HOME" security default-keychain -d user -s "$SANDBOX_KC" >/dev/null 2>&1 || true
HOME="$SANDBOX_HOME" security unlock-keychain -p "" "$SANDBOX_KC" >/dev/null 2>&1 || true
HOME="$SANDBOX_HOME" security set-keychain-settings "$SANDBOX_KC" >/dev/null 2>&1 || true

# —— Write forged virtual OAuth (overwrite each time; keep single .enc; reuse existing encryption.key) ——
# Note: do not override HOME — forger uses [real] HOME to detect accidental writes to real credential dir (guardrail).
# App one-click flow uses --skip-oauth-forge: OAuth already forged in-process by Rust (src/oauth_forge.rs);
# packaged app needs zero node. Standalone/dev runs of this script (without that flag) still use .mjs forgery (requires node).
if [[ "$SKIP_FORGE" == "1" ]]; then
  echo "虚拟 OAuth 由 app 进程内写入（Rust 原生，已跳过 node 伪造）→ $DATA_DIR"
else
  echo "写入虚拟 OAuth 凭证（node）→ $DATA_DIR"
  node "$PROJ/scripts/make-virtual-oauth.mjs" --auth-dir "$DATA_DIR" --email "$EMAIL"
fi

echo
echo "启动隔离沙箱 Science（虚拟登录）"
echo "  HOME     = $SANDBOX_HOME"
echo "  data-dir = $DATA_DIR"
echo "  端口     = $PORT   （真实实例 8765 不受影响）"
echo "  二进制   = $BIN"
# Mask path secret in proxy-url (one-time auth token must not appear in logs)
_masked_proxy="$(printf '%s' "$PROXY_URL" | sed -E 's#(://[^/]+/).+#\1****#')"
echo "  推理指向 = $_masked_proxy"
echo "  账号     = $EMAIL （本地假账号，不用真实凭证）"

# Fix #3: outbound calls from sandbox to Anthropic domains (claude.ai / *.claude.com / *.anthropic.com)
# go through local proxy fast-fail. Without this, blocking requests to claude.ai/api/oauth/profile at startup
# hang and retry on networks that cannot reach claude.ai → UI stuck on "Switching organization".
# Approach: set **https_proxy only** (that stuck profile request is HTTPS → CONNECT; proxy
# do_CONNECT returns 401 immediately for those domains so the client quickly treats as logged out; other HTTPS tunnels pass through).
# **[Intentionally omit http_proxy]**: proxy does not implement plain HTTP forwarding (GET http://host/…); setting http_proxy
# would make plain HTTP MCP/downloads/package mirrors hit the proxy and get 404 (P2 fix); without it, plain HTTP goes direct or via the user's own
# http_proxy, and no Anthropic domain uses plain HTTP, so fast-fail is unaffected.
# no_proxy keeps local inference direct to 127.0.0.1 (operon recognizes lowercase https_proxy/no_proxy).
_PROXY_HOSTPORT="$(printf '%s' "$PROXY_URL" | sed -E 's#^[a-zA-Z][a-zA-Z0-9+.-]*://([^/]+).*#\1#')"
_FASTFAIL_PROXY="http://$_PROXY_HOSTPORT"
_NO_PROXY="127.0.0.1,localhost,::1"
echo "  外联防卡 = Anthropic HTTPS fast-fail（经 $_FASTFAIL_PROXY，no_proxy=$_NO_PROXY）"
echo

HOME="$SANDBOX_HOME" \
ANTHROPIC_BASE_URL="$PROXY_URL" \
https_proxy="$_FASTFAIL_PROXY" HTTPS_PROXY="$_FASTFAIL_PROXY" \
no_proxy="$_NO_PROXY" NO_PROXY="$_NO_PROXY" \
"$BIN" serve \
  --data-dir "$DATA_DIR" \
  --port "$PORT" \
  --no-browser --no-auto-update --detached

echo
echo "已后台启动。验证:"
echo "  健康:   curl -s http://127.0.0.1:$PORT/health || true"
echo "  状态:   HOME='$SANDBOX_HOME' '$BIN' status --data-dir '$DATA_DIR'"
echo "停止:     scripts/stop-science-sandbox.sh   （data-dir 已改为虚拟沙箱同一路径）"
