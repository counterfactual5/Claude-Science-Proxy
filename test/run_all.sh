#!/usr/bin/env bash
# S0 layered acceptance gate aggregator (keeps legacy entry name; summarizes only, no test details).
# Usage: run_all.sh [--require-release-ready]
#   Two overall verdicts:
#     current-env clean = no fail in this environment (env-blocked / needs-real-machine allowed)
#     release-ready green = all 5 layers pass with no env-blocked
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
REQUIRE_RELEASE=0; [ "${1:-}" = "--require-release-ready" ] && REQUIRE_RELEASE=1
LAYERS="offline loopback scripts rust frontend"
# Note: /bin/bash on macOS is 3.2 (no declare -A). Use STATUS_<layer> plain variables instead.
any_fail=0; not_release=0
echo "== S0 分层验收门 =="
for L in $LAYERS; do
  line="$(bash "test/run-$L.sh" 2>&1 | tee /dev/stderr | grep -E '^S0_LAYER ' | tail -1)"
  st="$(echo "$line" | awk '{print $3}')"; [ -z "$st" ] && st="fail"   # missing marker line = treat as fail (not silent)
  eval "STATUS_$L=\"\$st\""
  case "$st" in
    fail) any_fail=1; not_release=1 ;;
    pass) : ;;
    *) not_release=1 ;;   # env-blocked / skipped / needs-real-machine do not satisfy release-ready
  esac
done
echo "---- 汇总 ----"
for L in $LAYERS; do
  eval "st=\"\$STATUS_$L\""
  printf '  %-9s %s\n' "$L" "$st"
done
echo "----"
if [ "$any_fail" -eq 0 ]; then echo "current-env clean: YES（本环境无 fail）"; else echo "current-env clean: NO（有 fail）"; fi
if [ "$not_release" -eq 0 ]; then echo "release-ready green: YES（5 层均 pass、无 env-blocked）"; else echo "release-ready green: NO（有 env-blocked / fail，须在具备全部能力的机器复跑）"; fi
if [ "$any_fail" -ne 0 ]; then exit 1; fi
if [ "$REQUIRE_RELEASE" -eq 1 ] && [ "$not_release" -ne 0 ]; then exit 2; fi
exit 0
