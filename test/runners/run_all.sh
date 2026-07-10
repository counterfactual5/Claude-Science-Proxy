#!/usr/bin/env bash
# S0 layered acceptance gate aggregator (keeps legacy entry name; summarizes only, no test details).
# Usage: run_all.sh [--require-release-ready]
#   Two overall verdicts:
#     current-env clean = no fail in this environment (env-blocked / needs-real-machine allowed)
#     release-ready green = all 5 layers pass with no env-blocked
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
REQUIRE_RELEASE=0; [ "${1:-}" = "--require-release-ready" ] && REQUIRE_RELEASE=1
LAYERS="offline loopback scripts rust frontend"
# Note: /bin/bash on macOS is 3.2 (no declare -A). Use STATUS_<layer> plain variables instead.
any_fail=0; not_release=0
echo "== S0 layered acceptance gate =="
for L in $LAYERS; do
  line="$(bash "test/runners/run-$L.sh" 2>&1 | tee /dev/stderr | grep -E '^S0_LAYER ' | tail -1)"
  st="$(echo "$line" | awk '{print $3}')"; [ -z "$st" ] && st="fail"   # missing marker line = treat as fail (not silent)
  eval "STATUS_$L=\"$st\""
  case "$st" in
    fail) any_fail=1; not_release=1 ;;
    pass) : ;;
    *) not_release=1 ;;   # env-blocked / skipped / needs-real-machine do not satisfy release-ready
  esac
done
echo "---- summary ----"
for L in $LAYERS; do
  eval "st=\$STATUS_$L"
  printf '  %-9s %s\n' "$L" "$st"
done
echo "----"
if [ "$any_fail" -eq 0 ]; then echo "current-env clean: YES (no fail in this env)"; else echo "current-env clean: NO (has fail)"; fi