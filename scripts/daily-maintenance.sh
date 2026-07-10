#!/bin/bash
# Claude Science Proxy (CSP) daily maintenance patrol — triggered by launchd at 09:00 / 21:00 daily (Asia/Shanghai).
# Runs an unattended, restricted `claude -p` session: read-only repo + fetch public web pages + write patrol report to
# findings/auto-maint/. Never changes code, commits, pushes, touches ~/.claude-science, or starts any Science instance.
# See scripts/daily-maintenance.prompt.md for details.
#
# Manual test: bash scripts/daily-maintenance.sh
# Install/uninstall schedule: scripts/install-maintenance.sh {install|uninstall|status}
set -euo pipefail

REPO="/Users/superjj/ccproj/CSswitch"
PROMPT_FILE="$REPO/scripts/daily-maintenance.prompt.md"
OUT_DIR="$REPO/findings/auto-maint"
LOG_DIR="$OUT_DIR/logs"
LOCK="$OUT_DIR/.run.lock"

# launchd provides a minimal environment; extend PATH ourselves (claude is in ~/.local/bin)
export PATH="/Users/superjj/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export HOME="/Users/superjj"

mkdir -p "$LOG_DIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$LOG_DIR/run-$STAMP.log"

# Prevent overlapping runs (e.g. manual test colliding with scheduled run)
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "[$STAMP] 已有一次巡检在跑（$LOCK 存在），本次跳过。" >>"$LOG"
  exit 0
fi
trap 'rmdir "$LOCK" 2>/dev/null || true' EXIT

cd "$REPO"

CLAUDE_BIN="$(command -v claude || echo /Users/superjj/.local/bin/claude)"
PROMPT="$(cat "$PROMPT_FILE")"

{
  echo "===== CSP daily-maintenance @ $STAMP ====="
  echo "repo=$REPO branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?') claude=$CLAUDE_BIN"
  echo "-----------------------------------------------"
} >>"$LOG"

# Timeout guard: if stuck on permission dialogs etc. while unattended, kill after TIMEOUT seconds to avoid hanging forever.
TIMEOUT="${MAINT_TIMEOUT:-900}"
run_with_timeout() { perl -e 'alarm shift @ARGV; exec @ARGV or exit 127' "$@"; }

# Restricted headless run:
#  - acceptEdits: allow file writes (report to disk) without prompts
#  - allowedTools: read-only commands + WebFetch/WebSearch + file writes only; no arbitrary Bash
#  - disallowedTools: hard-block dangerous git ops, rm, and read/write to ~/.claude-science (deny wins)
set +e
run_with_timeout "$TIMEOUT" "$CLAUDE_BIN" -p "$PROMPT" \
  --permission-mode acceptEdits \
  --allowedTools \
    "Read Grep Glob Write Edit WebFetch WebSearch \
     Bash(git status:*) Bash(git log:*) Bash(git branch:*) Bash(git diff:*) Bash(git rev-parse:*) \
     Bash(plutil:*) Bash(date:*) Bash(ls:*) Bash(mkdir:*) Bash(grep:*) Bash(cat:*) \
     Bash(head:*) Bash(tail:*) Bash(wc:*) Bash(find:*) Bash(sed:*)" \
  --disallowedTools \
    "Bash(rm:*) Bash(git commit:*) Bash(git push:*) Bash(git add:*) Bash(git checkout:*) \
     Bash(git switch:*) Bash(git reset:*) Bash(git stash:*) Bash(git branch -D:*) \
     Read(/Users/superjj/.claude-science/**) Write(/Users/superjj/.claude-science/**) Edit(/Users/superjj/.claude-science/**)" \
  >>"$LOG" 2>&1
RC=$?
set -e

echo "----- claude exit=$RC @ $(date +%Y%m%d-%H%M%S) -----" >>"$LOG"
exit "$RC"
