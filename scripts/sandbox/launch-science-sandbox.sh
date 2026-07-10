#!/bin/zsh
# Launch an [isolated, logged-out] Claude Science sandbox; inference routes through this project's translation proxy.
#
# Iron-rule safeguards (see AGENT.md):
#   - Separate HOME + data-dir + port; never modify/delete real ~/.claude-science; never use port 8765
#   - Read-only APFS clone of runtime assets from real ~/.claude-science (bin/conda/runtime/seed-assets); never copy login credentials
#     (.oauth-tokens / encryption.key / active-org.json / orgs / .key-backups)
#   - Sandbox therefore starts [logged out]. To infer, [log in fresh inside the sandbox] (zero impact on real login)
#
# Usage:
#   scripts/launch-science-sandbox.sh [--port 8990] [--proxy-url http://127.0.0.1:18991]
#   Start the proxy in another terminal first: CSP_RELAY_KEY=... python3 proxy/csp_proxy.py --provider relay --port 18991
set -euo pipefail

PROJ="$(cd "$(dirname "$0")/../.." && pwd)"
SANDBOX_HOME="$PROJ/.sandbox/home"
DATA_DIR="$SANDBOX_HOME/.claude-science"
REAL_DIR="$HOME/.claude-science"
APP="/Applications/Claude Science.app/Contents/MacOS/ClaudeScience"
BIN="/Applications/Claude Science.app/Contents/Resources/bin/claude-science"
PORT=8990
PROXY_URL="http://127.0.0.1:18991"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2;;
    --proxy-url) PROXY_URL="$2"; shift 2;;
    *) echo "未知参数: $1"; exit 1;;
  esac
done

# —— Iron-rule assertion: never use real directory / real port ——
_dd="${DATA_DIR:A}"; _rd="${REAL_DIR:A}"
if [[ "$_dd" == "$_rd" ]]; then echo "拒绝：data-dir 的真实路径指向真实目录"; exit 1; fi
[[ "$PORT" =~ ^[0-9]+$ ]] || { echo "拒绝：端口不是合法整数（$PORT）"; exit 1; }
if (( 10#${PORT} == 8765 )); then echo "拒绝：端口 8765 是真实实例保留端口"; exit 1; fi

# —— First run: clone runtime assets; never copy login credentials ——
if [[ ! -d "$DATA_DIR/bin" ]]; then
  echo "首次初始化沙箱运行时（APFS 克隆，只拷运行时、不拷登录）…"
  mkdir -p "$DATA_DIR"
  for asset in bin conda runtime seed-assets; do
    if [[ -d "$REAL_DIR/$asset" ]]; then
      cp -Rc "$REAL_DIR/$asset" "$DATA_DIR/$asset"
    fi
  done
  # Explicitly not copied: .oauth-tokens encryption.key active-org.json orgs .key-backups install-id
  echo "运行时就绪。沙箱为【未登录】状态。"
fi

# —— If login credentials leak into the sandbox, block startup (belt-and-suspenders) ——
for secret in .oauth-tokens encryption.key active-org.json orgs .key-backups; do
  if [[ -e "$DATA_DIR/$secret" ]]; then
    echo "拒绝启动：沙箱内出现登录凭证 '$secret'，违反铁律。请删除后重试。"; exit 1
  fi
done

echo "启动隔离沙箱 Science"
echo "  HOME     = $SANDBOX_HOME"
echo "  data-dir = $DATA_DIR"
echo "  端口     = $PORT   （真实实例 8765 不受影响）"
echo "  推理指向 = $PROXY_URL"
echo

HOME="$SANDBOX_HOME" \
ANTHROPIC_BASE_URL="$PROXY_URL" \
"$BIN" serve \
  --data-dir "$DATA_DIR" \
  --port "$PORT" \
  --no-browser --no-auto-update --detached

echo
echo "已后台启动。下一步（需你手动完成，Claude 不代做登录）:"
echo "  1) 取登录链接: HOME='$SANDBOX_HOME' '$BIN' url --data-dir '$DATA_DIR'"
echo "  2) 浏览器打开该链接，在沙箱里【全新登录】（另一套会话，真实登录不受影响）"
echo "  3) Log in inside the sandbox, then inference will route through the CSP proxy."
echo "停止: scripts/stop-science-sandbox.sh"
