#!/usr/bin/env bash
# Claude Science Proxy (CSP) doctor: read-only environment diagnostics.
#   - Does not start any process, does not use the network, never reads/writes/deletes real ~/.claude-science.
#   - Never prints provider key values (only reports present/absent).
#   - Fails immediately if a port hits the real-instance reserved port 8765 (iron rule).
# Override variables (for testing and customization):
#   CSP_PROVIDER (active template_id, e.g. deepseek/glm/relay/…)
#   CSP_ADAPTER  (deepseek|relay)   CSP_KEY_PRESENT (0|1)
#   CSP_PROXY_PORT  CSP_SANDBOX_PORT  CSP_CONFIG (path to CSP.json)  SCIENCE_BIN
#   CSP_DOCTOR_CHECK_REAL_HOME=1  check whether $HOME/.claude-science exists only after explicit opt-in
set -u

PROVIDER="${CSP_PROVIDER:-}"
ADAPTER="${CSP_ADAPTER:-}"
KEY_PRESENT="${CSP_KEY_PRESENT:-0}"
PROXY_PORT="${CSP_PROXY_PORT:-18991}"
SANDBOX_PORT="${CSP_SANDBOX_PORT:-8990}"
CONFIG="${CSP_CONFIG:-$HOME/.csp/CSP.json}"
SCIENCE_BIN="${SCIENCE_BIN:-/Applications/Claude Science.app/Contents/Resources/bin/claude-science}"
CHECK_REAL_HOME="${CSP_DOCTOR_CHECK_REAL_HOME:-0}"

WARN=0; FAIL=0
pass() { echo "  ✓ $1"; }
warn() { echo "  ⚠ $1"; WARN=$((WARN + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }

echo "CSP doctor（Claude Science Proxy 只读诊断，不启动进程、不联网、绝不碰真实目录）"
echo "生效来源=${PROVIDER:-（无）}  适配器=${ADAPTER:-（无）}  代理端口=$PROXY_PORT  沙箱端口=$SANDBOX_PORT"

echo "[依赖]"
if command -v python3 >/dev/null 2>&1; then pass "python3 $(python3 --version 2>&1 | awk '{print $2}')"; else fail "缺 python3（起翻译代理需要）"; fi
# Since v0.1.4, node is NOT required for the app: virtual login is forged in-process by Rust (no node).
# node is only needed when running scripts/make-virtual-oauth.mjs standalone (dev/parity); missing node is a warning, not a failure.
if command -v node >/dev/null 2>&1; then pass "node $(node --version 2>&1)（app 已不需要，仅 dev 脚本用）"; else warn "无 node（app 无需；仅独立跑 make-virtual-oauth.mjs 时才需要）"; fi

echo "[Science 二进制]"
if [ -x "$SCIENCE_BIN" ]; then pass "找到 $SCIENCE_BIN"; else warn "未找到 Science 二进制（一键越登录需要）：$SCIENCE_BIN"; fi

echo "[生效配置]"
# Multi-profile: keys live in CSP.json (not shell env vars). The app passes template_id + adapter +
# key presence (KEY_PRESENT). No template should fail here with "unknown provider".
if [ -z "$PROVIDER" ]; then
  warn "当前没有「生效」配置（在面板点击一条配置切换为当前生效）"
elif [ "$KEY_PRESENT" = "1" ]; then
  pass "生效来源：${PROVIDER}（${ADAPTER:-?} 适配器）· key 已配置在 CSP.json（值不显示）"
else
  warn "生效来源：${PROVIDER}（${ADAPTER:-?} 适配器）· 尚未填 key（在面板该配置里粘贴）"
fi

echo "[端口]"
classify_port() {  # $1=port; prints occupancy classification
  local p="$1" cmd
  if [ "$((10#$p))" -eq 8765 ]; then echo "保留端口 8765（真实实例，绝不占用/干预）"; return; fi
  cmd="$(lsof -nP -iTCP:"$p" -sTCP:LISTEN 2>/dev/null | awk 'NR==2{print $1" (pid "$2")"}')"
  if [ -z "$cmd" ]; then echo "空闲"; return; fi
  local lc
  lc="$(printf '%s' "$cmd" | tr '[:upper:]' '[:lower:]')"
  case "$lc" in
    *python*|*csp*|*claude-science*) echo "疑似 CSP 旧进程：${cmd}（可 stop_all 或手动清）" ;;
    *) echo "未知占用：$cmd" ;;
  esac
}
for p in "$PROXY_PORT" "$SANDBOX_PORT"; do
  if ! [[ "$p" =~ ^[0-9]+$ ]]; then fail "端口非法整数：$p"; continue; fi
  if [ "$((10#$p))" -eq 8765 ]; then fail "端口 $p 命中真实实例保留端口 8765（铁律禁用）"; continue; fi
  detail="$(classify_port "$p")"
  if [ "$detail" = "空闲" ]; then pass "端口 $p 空闲"; else warn "端口 $p 占用分型：$detail"; fi
done

echo "[本地配置]"
if [ -L "$CONFIG" ]; then
  fail "config 是符号链接（拒绝，防跟随写到别处）：$CONFIG"
elif [ -e "$CONFIG" ]; then
  perm="$(stat -f '%Lp' "$CONFIG" 2>/dev/null || stat -c '%a' "$CONFIG" 2>/dev/null)"
  if [ "$perm" = "600" ]; then pass "config 存在且权限 600：$CONFIG"; else warn "config 权限为 ${perm}，应为 600：$CONFIG"; fi
else
  warn "config 不存在（首次运行 GUI 会创建）：$CONFIG"
fi

echo "[铁律]"
if [ "$CHECK_REAL_HOME" = "1" ]; then
  REAL_DIR="$HOME/.claude-science"
  if [ -d "$REAL_DIR" ]; then pass "真实目录存在（显式 opt-in 只读检查，绝不写/删）：$REAL_DIR"; else warn "未见真实 Science 目录（显式 opt-in 只读检查）：$REAL_DIR"; fi
else
  pass "真实 HOME 检查默认跳过（未设置 CSP_DOCTOR_CHECK_REAL_HOME=1，不读取 ~/.claude-science）"
fi

echo "----"
echo "诊断完成：警告 ${WARN}，失败 ${FAIL}"
[ "$FAIL" -eq 0 ] || exit 1
exit 0
