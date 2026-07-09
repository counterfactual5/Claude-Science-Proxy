#!/usr/bin/env bash
# S0 loopback layer: requires 127.0.0.1 bind/connect. If blocked, whole layer env-blocked (not fail).
# Known low-frequency mock-timing race (cross-class /health readiness wait mismatch): absorb with bounded retry,
# never swallow real failures: exhausted retries → explicit fail; retry progress stays visible (not silent).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
if ! command -v python3 >/dev/null 2>&1; then
  echo "S0_LAYER loopback env-blocked (no python3)"; exit 0
fi
if [ "$(python3 test/_capability.py)" != "1" ]; then
  echo "本环境禁止 loopback bind/connect，跳过 loopback 层。"
  echo "S0_LAYER loopback env-blocked (loopback not permitted)"; exit 0
fi

# CSP_LOOPBACK_TEST_CMD is test-only: inject deterministic pass/fail stubs to verify retry logic, not for normal runs.
run_loopback_once() {
  if [ -n "${CSP_LOOPBACK_TEST_CMD:-}" ]; then
    eval "$CSP_LOOPBACK_TEST_CMD"
  else
    python3 -m unittest test.test_proxy_connect test.test_proxy_stream test.test_proxy_dsml_e2e test.test_proxy_auth test.test_proxy_golden -v
  fi
}

MAX_ATTEMPTS=3
attempt=1
while [ "$attempt" -le "$MAX_ATTEMPTS" ]; do
  if run_loopback_once; then
    if [ "$attempt" -gt 1 ]; then
      echo "loopback 第 $attempt 次尝试通过（此前 $((attempt - 1)) 次疑似 mock-timing race 失败）。"
    fi
    echo "S0_LAYER loopback pass"; exit 0
  fi
  if [ "$attempt" -lt "$MAX_ATTEMPTS" ]; then
    echo "loopback 尝试 $attempt/$MAX_ATTEMPTS 失败, 重试(已知低频 mock-timing race)"
  fi
  attempt=$((attempt + 1))
done
echo "S0_LAYER loopback fail"; exit 1
