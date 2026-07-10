#!/usr/bin/env bash
# Claude Science Proxy (CSP) self-test: run offline regression suite (test/run_all.sh).
#   Isolated environment; exercises proxy, forger, and script units only; never touches real ~/.claude-science or upstream network.
#   Use after install for self-check, or after changes for regression.
set -u
PROJ="$(cd "$(dirname "$0")/../.." && pwd)"
echo "CSP self-test → 离线回归套件（隔离，不碰 Science、不联网）"
exec bash "$PROJ/test/run_all.sh"
