#!/usr/bin/env bash
# S0 offline pure unit layer: no loopback / no network / no upstream.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
if ! command -v python3 >/dev/null 2>&1; then
  echo "S0_LAYER offline env-blocked (no python3)"; exit 0
fi
export PYTHONPATH="$ROOT/proxy${PYTHONPATH:+:$PYTHONPATH}"
if python3 -m unittest test.test_proxy_units test.test_provider_policy test.test_anthropic_compat test.test_dsml_shim test.test_capability test.test_capability_catalog test.test_proxy_packaging test.test_model_registry test.test_model_sort -v; then
  echo "S0_LAYER offline pass"; exit 0
else
  echo "S0_LAYER offline fail"; exit 1
fi
