#!/usr/bin/env bash
# S0 rust layer: cargo fmt+clippy+test. No cargo → env-blocked. No loopback → skip port bind tests and mark layer env-blocked.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT/desktop/src-tauri"

if ! command -v cargo >/dev/null 2>&1; then
  [ -x "$HOME/.cargo/bin/cargo" ] && export PATH="$HOME/.cargo/bin:$PATH"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "S0_LAYER rust env-blocked (no cargo)"; exit 0
fi

fail=0
blocked=0
cargo fmt --check || fail=1
cargo clippy --all-targets -- -D warnings || fail=1
# Port bind test list (Step 1 lives in scratch.rs; skip and mark env-blocked when no loopback).
PORT_TESTS="pick_scratch_port_returns_usable_nonreserved_port two_picks_are_bindable"
if [ "$(python3 "$ROOT/test/fixtures/_capability.py")" = "1" ]; then
  cargo test || fail=1
else
  blocked=1
  echo "loopback disabled -> skipping port bind tests, marking rust layer env-blocked: $PORT_TESTS"
  skip_args=""; for t in $PORT_TESTS; do skip_args="$skip_args --skip $t"; done
  cargo test -- $skip_args || fail=1
fi

if [ "$fail" -ne 0 ]; then echo "S0_LAYER rust fail"; exit 1; fi
if [ "$blocked" -ne 0 ]; then echo "S0_LAYER rust env-blocked (loopback bind tests skipped)"; exit 0; fi
echo "S0_LAYER rust pass"; exit 0
