#!/usr/bin/env bash
# CSSwitch doctor：只读环境诊断。
#   - 不启动任何进程、不联网、绝不读写/删除真实 ~/.claude-science。
#   - 绝不打印任何 provider key 的值（只报 present/absent）。
#   - 端口命中真实实例保留端口 8765 直接失败（铁律）。
# 覆盖变量（便于测试与自定义）：
#   CSSWITCH_PROVIDER (deepseek|qwen)  CSSWITCH_PROXY_PORT  CSSWITCH_SANDBOX_PORT
#   CSSWITCH_CONFIG (config.json 路径)  SCIENCE_BIN
set -u

PROVIDER="${CSSWITCH_PROVIDER:-deepseek}"
PROXY_PORT="${CSSWITCH_PROXY_PORT:-18991}"
SANDBOX_PORT="${CSSWITCH_SANDBOX_PORT:-8990}"
CONFIG="${CSSWITCH_CONFIG:-$HOME/.csswitch/config.json}"
SCIENCE_BIN="${SCIENCE_BIN:-/Applications/Claude Science.app/Contents/Resources/bin/claude-science}"
REAL_DIR="$HOME/.claude-science"

WARN=0; FAIL=0
pass() { echo "  ✓ $1"; }
warn() { echo "  ⚠ $1"; WARN=$((WARN + 1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL + 1)); }

echo "CSSwitch doctor（只读诊断，不启动进程、不联网、绝不碰真实目录）"
echo "provider=$PROVIDER  代理端口=$PROXY_PORT  沙箱端口=$SANDBOX_PORT"

echo "[依赖]"
if command -v python3 >/dev/null 2>&1; then pass "python3 $(python3 --version 2>&1 | awk '{print $2}')"; else fail "缺 python3"; fi
if command -v node >/dev/null 2>&1; then pass "node $(node --version 2>&1)"; else fail "缺 node"; fi

echo "[Science 二进制]"
if [ -x "$SCIENCE_BIN" ]; then pass "找到 $SCIENCE_BIN"; else warn "未找到 Science 二进制（一键越登录需要）：$SCIENCE_BIN"; fi

echo "[Provider Key]"
case "$PROVIDER" in
  deepseek) KEY_ENV="DEEPSEEK_API_KEY"; KEY_VAL="${DEEPSEEK_API_KEY:-}";;
  qwen)     KEY_ENV="DASHSCOPE_API_KEY"; KEY_VAL="${DASHSCOPE_API_KEY:-}";;
  *)        KEY_ENV=""; KEY_VAL="";;
esac
if [ -z "$KEY_ENV" ]; then
  fail "未知 provider：${PROVIDER}（应为 deepseek 或 qwen）"
elif [ -n "$KEY_VAL" ]; then
  pass "$KEY_ENV 已设置（值不显示）"
else
  warn "$KEY_ENV 未在环境中设置（可改用 config.json 或代理 --env-file 提供）"
fi

echo "[端口]"
for p in "$PROXY_PORT" "$SANDBOX_PORT"; do
  if ! [[ "$p" =~ ^[0-9]+$ ]]; then fail "端口非法整数：$p"; continue; fi
  if [ "$((10#$p))" -eq 8765 ]; then fail "端口 $p 命中真实实例保留端口 8765（铁律禁用）"; continue; fi
  if lsof -nP -iTCP:"$p" -sTCP:LISTEN >/dev/null 2>&1; then
    warn "端口 $p 已被占用（可能上次没退干净，或别的程序占了）"
  else
    pass "端口 $p 空闲"
  fi
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
if [ -d "$REAL_DIR" ]; then pass "真实目录存在（本工具只读诊断，绝不写/删）：$REAL_DIR"; else warn "未见真实 Science 目录：$REAL_DIR"; fi

echo "----"
echo "诊断完成：警告 ${WARN}，失败 ${FAIL}"
[ "$FAIL" -eq 0 ] || exit 1
exit 0
