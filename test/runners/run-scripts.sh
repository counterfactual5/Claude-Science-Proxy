#!/usr/bin/env bash
# S0 scripts layer: bash script tests + node OAuth parity (env-blocked without node) + ops test_ops.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
fail=0
echo "== bash scripts =="
bash test/scripts/test_scripts.sh || fail=1
echo "== node oauth parity =="
if command -v node >/dev/null 2>&1; then
  node --test test/unit/oauth/test_make_virtual_oauth.mjs || fail=1
else
  echo "skip - node oauth parity env-blocked (no node)"
fi
echo "== ops (doctor contract + verify-proxy self-test) =="
bash test/scripts/test_ops_scripts.sh || fail=1
if [ "$fail" -eq 0 ]; then echo "S0_LAYER scripts pass"; exit 0; else echo "S0_LAYER scripts fail"; exit 1; fi
