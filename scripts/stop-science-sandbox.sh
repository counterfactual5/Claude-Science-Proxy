#!/bin/zsh
# 停止隔离沙箱 Science（只停沙箱 data-dir 的守护进程，绝不影响真实实例 8765）。
set -euo pipefail
PROJ="${0:A:h:h}"
SANDBOX_HOME="$PROJ/.sandbox/home"
DATA_DIR="$SANDBOX_HOME/.claude-science"
BIN="/Applications/Claude Science.app/Contents/Resources/bin/claude-science"

if [[ ! -d "$DATA_DIR" ]]; then echo "沙箱不存在，无需停止。"; exit 0; fi

HOME="$SANDBOX_HOME" "$BIN" stop --data-dir "$DATA_DIR" 2>&1 | tail -2 || true
echo "沙箱已停。真实实例 8765 未受影响。"
