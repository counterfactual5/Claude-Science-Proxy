#!/usr/bin/env bash
# Claude Science Proxy (CSP) real-machine acceptance guard.
#
# Manages isolated test HOME and test ports only; never reads, copies, modifies, or deletes real
# ~/.claude-science. Real instance is tracked via lsof on port 8765 listener PIDs and compared at each
# stage to ensure they stay unchanged.
set -euo pipefail

PROJ="$(cd "$(dirname "$0")/.." && pwd)"
TEST_ROOT="${CSP_REAL_TEST_ROOT:-${TMPDIR:-/tmp}/csp-real-machine-${UID}}"
TEST_HOME="$TEST_ROOT/home"
STATE_DIR="$TEST_ROOT/state"
BASELINE="$STATE_DIR/port-8765.pids"
PROXY_PORT="${CSP_TEST_PROXY_PORT:-18991}"
SANDBOX_PORT="${CSP_TEST_SANDBOX_PORT:-8990}"
SCIENCE_BIN="${SCIENCE_BIN:-/Applications/Claude Science.app/Contents/Resources/bin/claude-science}"

die() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "PASS: $*"; }

# Isolation guard (GPT round-3 P1): reject pre-placed symlinks on directories we create. Prevents TEST_HOME
# symlinked to real HOME so prepare-legacy would overwrite real ~/.csp/CSP.json via symlink (or chmod real dir).
# Only checks directories we control, not system parents like /var (macOS TMPDIR defaults under /var/folders).
reject_symlinks() {
  local p
  for p in "$@"; do
    if [ -L "$p" ]; then
      die "隔离路径是符号链接，拒绝（防指向真实目录）：$p"
    fi
  done
}

# Canonicalize isolation dir: after resolving parent symlinks, test HOME must not equal or sit inside real HOME,
# or test config would be written into the user's real home. Default TEST_ROOT under TMPDIR passes naturally.
assert_isolated_from_real_home() {
  local dir="$1" real_home canon
  real_home="$(cd "$HOME" 2>/dev/null && pwd -P)" || die "无法解析真实 HOME"
  canon="$(cd "$dir" 2>/dev/null && pwd -P)" || die "无法解析隔离目录：$dir"
  case "$canon/" in
    "$real_home/" | "$real_home"/*)
      die "隔离目录解析后落在真实 HOME 内，拒绝：${canon}（真实 HOME=${real_home}）。请把 CSP_REAL_TEST_ROOT 指向 HOME 之外（默认 TMPDIR 即可）。"
      ;;
  esac
}

# Safe write for state/config files (GPT round-4 P1): remove any pre-placed symlink at target (`rm -f` removes link,
# does not follow), then write a fresh regular file from stdin. Prevents `>` following a pre-placed symlink into
# a real file (e.g. real ~/.csp/CSP.json or port-8765.now symlinked in STATE_DIR). **Note: eliminates
# follow-preplaced-symlink writes, not atomicity or check-then-write races** (local test helper, not adversarial threat model).
write_fresh() {
  local target="$1"
  if [ -d "$target" ] && [ ! -L "$target" ]; then
    die "目标是目录，拒绝当文件覆盖：$target"
  fi
  rm -f "$target"
  cat >"$target"
}

validate_ports() {
  case "$PROXY_PORT:$SANDBOX_PORT" in
    *[!0-9:]*|:*) die "测试端口必须是整数" ;;
  esac
  [ "$PROXY_PORT" -ne 8765 ] || die "代理端口命中真实实例保留端口 8765"
  [ "$SANDBOX_PORT" -ne 8765 ] || die "沙箱端口命中真实实例保留端口 8765"
  [ "$PROXY_PORT" -ne "$SANDBOX_PORT" ] || die "代理端口与沙箱端口不能相同"
}

listener_pids() {
  lsof -nP -t -iTCP:"$1" -sTCP:LISTEN 2>/dev/null | sort -u || true
}

assert_real_unchanged() {
  [ -f "$BASELINE" ] || die "缺少 8765 基线；先运行 preflight"
  local now="$STATE_DIR/port-8765.now"
  listener_pids 8765 | write_fresh "$now" # remove pre-placed symlinks then write fresh file, never via symlink to real files
  if ! cmp -s "$BASELINE" "$now"; then
    echo "基线 PID: $(tr '\n' ' ' <"$BASELINE")" >&2
    echo "当前 PID: $(tr '\n' ' ' <"$now")" >&2
    die "真实 Science 8765 监听发生变化，立即停止验收"
  fi
  pass "真实 Science 8765 监听 PID 保持不变"
}

assert_port_free() {
  local p="$1"
  [ -z "$(listener_pids "$p")" ] || die "测试端口 $p 已被占用"
}

preflight() {
  validate_ports
  reject_symlinks "$TEST_ROOT" "$TEST_HOME" "$STATE_DIR" # before mkdir/chmod: reject pre-placed symlinks
  umask 077
  mkdir -p "$TEST_HOME" "$STATE_DIR"
  chmod 700 "$TEST_ROOT" "$TEST_HOME" "$STATE_DIR"
  assert_isolated_from_real_home "$TEST_HOME" # after mkdir: resolve parent chain, confirm not inside real HOME
  listener_pids 8765 | write_fresh "$BASELINE" # same: remove symlink then write fresh baseline file
  assert_port_free "$PROXY_PORT"
  assert_port_free "$SANDBOX_PORT"
  [ -x "$SCIENCE_BIN" ] || die "未找到可执行 Science：$SCIENCE_BIN"
  [ -x "$PROJ/desktop/src-tauri/target/release/desktop" ] || \
    echo "WARN: release 测试二进制尚未构建"
  pass "测试 HOME 已隔离：$TEST_HOME"
  pass "测试端口空闲：$PROXY_PORT / $SANDBOX_PORT"
  assert_real_unchanged
}

prepare_legacy() {
  validate_ports
  [ -f "$BASELINE" ] || die "先运行 preflight"
  [ -n "${DEEPSEEK_API_KEY:-}" ] || die "DEEPSEEK_API_KEY 未设置"
  command -v jq >/dev/null 2>&1 || die "prepare-legacy 需要 jq"
  # Re-verify isolation dirs (incl. STATE_DIR) are not symlinks before write: narrows window after preflight,
  # gives clear early fail; real write safety is write_fresh below (remove symlink then write), not race-free.
  reject_symlinks "$TEST_ROOT" "$TEST_HOME" "$STATE_DIR" "$TEST_HOME/.csp"
  assert_isolated_from_real_home "$TEST_HOME"
  local cfg_dir="$TEST_HOME/.csp"
  umask 077
  mkdir -p "$cfg_dir"
  chmod 700 "$cfg_dir"
  # If CSP.json was pre-placed as symlink (to real ~/.csp/CSP.json), write_fresh removes it first
  # then writes a fresh regular file, never overwriting real config via symlink.
  jq -n \
    --arg deepseek "$DEEPSEEK_API_KEY" \
    --argjson proxy_port "$PROXY_PORT" \
    --argjson sandbox_port "$SANDBOX_PORT" \
    '{provider:"deepseek",proxy_port:$proxy_port,sandbox_port:$sandbox_port,secret:"",mode:"proxy",providers:{deepseek:{key:$deepseek,base_url:"https://api.deepseek.com/anthropic",model:""}}}' \
    | write_fresh "$cfg_dir/CSP.json"
  chmod 600 "$cfg_dir/CSP.json"
  pass "已在独立测试 HOME 写入 v1 schema 样本（key 未回显）"
}

assert_running() {
  assert_real_unchanged
  [ -n "$(listener_pids "$PROXY_PORT")" ] || die "代理端口 $PROXY_PORT 未监听"
  [ -n "$(listener_pids "$SANDBOX_PORT")" ] || die "沙箱端口 $SANDBOX_PORT 未监听"
  local sbx_home="$TEST_HOME/.csp/sandbox/home"
  local data_dir="$sbx_home/.claude-science"
  local out
  out="$(HOME="$sbx_home" "$SCIENCE_BIN" status --data-dir "$data_dir" 2>/dev/null || true)"
  case "$out" in
    *'"running":true'*|*'"running": true'*) pass "独立 data-dir 的沙箱身份确认通过" ;;
    *) die "测试端口虽监听，但独立 data-dir 未报告 running=true" ;;
  esac
  pass "代理与沙箱测试端口均在监听"
}

assert_stopped() {
  assert_real_unchanged
  assert_port_free "$PROXY_PORT"
  assert_port_free "$SANDBOX_PORT"
  pass "测试代理与沙箱均已停止"
}

show_env() {
  cat <<EOF
CSP_REAL_TEST_ROOT=$TEST_ROOT
HOME=$TEST_HOME
CSP_REPO=$PROJ
CSP_TEST_PROXY_PORT=$PROXY_PORT
CSP_TEST_SANDBOX_PORT=$SANDBOX_PORT
EOF
}

case "${1:-}" in
  preflight) preflight ;;
  prepare-legacy) prepare_legacy ;;
  guard) assert_real_unchanged ;;
  assert-running) assert_running ;;
  assert-stopped) assert_stopped ;;
  env) show_env ;;
  *)
    echo "usage: $0 {preflight|prepare-legacy|guard|assert-running|assert-stopped|env}" >&2
    exit 2
    ;;
esac
