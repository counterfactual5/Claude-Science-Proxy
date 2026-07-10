#!/usr/bin/env bash
# Ops trio regression: doctor (read-only diagnostics) / verify-proxy (running proxy check) / self-test (offline suite wrapper).
# Never touches real ~/.claude-science, never starts Science, never hits network upstream. verify-proxy only hits /health and
# /v1/models (answered locally by proxy, no upstream calls, zero cost).
set -u
FAILS=0
ok() { echo "ok - $1"; }
no() { echo "NOT ok - $1"; FAILS=$((FAILS+1)); }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOCTOR="$ROOT/scripts/doctor.sh"
VERIFY="$ROOT/scripts/verify-proxy.sh"
SELFTEST="$ROOT/scripts/self-test.sh"
CLEAN="$ROOT/scripts/clean-bundle-resources.sh"
PROXY="$ROOT/proxy/csp_proxy.py"
T="$(mktemp -d)"
cleanup_bundle_test_artifacts() {
  rm -f "$ROOT/proxy/__pycache__/csp-clean-test.pyc" "$ROOT/scripts/csp-clean-test.pyc"
  rm -f "$ROOT/proxy/csp-clean-test.pyo" "$ROOT/scripts/csp-clean-test.pyo"
  rmdir "$ROOT/proxy/__pycache__" 2>/dev/null || true
}
trap cleanup_bundle_test_artifacts EXIT

# ---------- doctor ----------
# Normal: deps present (python3/node on host), config points at nonexistent temp path → exit 0
out="$(CSP_CONFIG="$T/nope.json" SCIENCE_BIN="$T/no-bin" "$DOCTOR" 2>&1)"; rc=$?
if [ $rc -eq 0 ]; then ok "doctor exits 0 when deps present"; else no "doctor failed with deps present (rc=$rc): $out"; fi
if echo "$out" | grep -q "$HOME/.claude-science"; then no "doctor default probed real HOME path"; else ok "doctor skips real HOME check by default"; fi
REAL_HOME_TMP="$T/real-home-optin"; mkdir -p "$REAL_HOME_TMP/.claude-science"
out="$(HOME="$REAL_HOME_TMP" CSP_DOCTOR_CHECK_REAL_HOME=1 CSP_CONFIG="$T/nope.json" SCIENCE_BIN="$T/no-bin" "$DOCTOR" 2>&1)"; rc=$?
if [ $rc -eq 0 ] && echo "$out" | grep -q "显式 opt-in"; then ok "doctor real HOME check is explicit opt-in"; else no "doctor opt-in real HOME check drifted (rc=$rc): $out"; fi

# Iron rule: proxy port 8765 → must fail-closed, output mentions 8765
out="$(CSP_PROXY_PORT=8765 CSP_CONFIG="$T/nope.json" "$DOCTOR" 2>&1)"; rc=$?
if [ $rc -ne 0 ] && echo "$out" | grep -q "8765"; then ok "doctor fails on reserved port 8765"; else no "doctor did not reject 8765 (rc=$rc): $out"; fi
if [ "$(python3 "$ROOT/test/_capability.py")" != "1" ]; then
  echo "skip - doctor occupied-port classification env-blocked（loopback 被禁）"
else
  PORT_FILE="$T/doctor-listener-port"
  python3 - "$PORT_FILE" <<'PY' &
import socket, sys, time
s = socket.socket()
s.bind(("127.0.0.1", 0))
s.listen(1)
with open(sys.argv[1], "w", encoding="utf-8") as f:
    f.write(str(s.getsockname()[1]))
try:
    time.sleep(10)
finally:
    s.close()
PY
  LISTENER_PID=$!
  for _ in $(seq 1 50); do [ -s "$PORT_FILE" ] && break; sleep 0.1; done
  OCC_PORT="$(cat "$PORT_FILE" 2>/dev/null || true)"
  out="$(CSP_PROXY_PORT="$OCC_PORT" CSP_CONFIG="$T/nope.json" "$DOCTOR" 2>&1)"; rc=$?
  kill "$LISTENER_PID" 2>/dev/null; wait "$LISTENER_PID" 2>/dev/null
  if echo "$out" | grep -q "unbound variable"; then no "doctor occupied-port diagnostic hit set -u error"; else ok "doctor occupied-port diagnostic avoids set -u error"; fi
  if [ $rc -eq 0 ] && echo "$out" | grep -q "疑似 CSP 旧进程"; then ok "doctor classifies python listener as CSP-like occupied port"; else no "doctor occupied-port classification drifted (rc=$rc): $out"; fi
fi

# key present contract: app passes CSP_KEY_PRESENT=1 + provider/adapter; doctor reports configured and never prints key value
SECRETVAL="DUMMY-KEY-abc123XYZ-should-never-print"
out="$(DEEPSEEK_API_KEY="$SECRETVAL" CSP_PROVIDER=deepseek CSP_ADAPTER=deepseek CSP_KEY_PRESENT=1 CSP_CONFIG="$T/nope.json" "$DOCTOR" 2>&1)"; rc=$?
if echo "$out" | grep -q "$SECRETVAL"; then no "doctor LEAKED key value"; else ok "doctor never prints key value"; fi
if echo "$out" | grep -q "已配置"; then ok "doctor reports key present (已配置)"; else no "doctor did not report key present: $out"; fi
# Negative: without KEY_PRESENT → should report key absent, not configured
out2="$(CSP_PROVIDER=deepseek CSP_ADAPTER=deepseek CSP_CONFIG="$T/nope.json" "$DOCTOR" 2>&1)"
if echo "$out2" | grep -q "尚未填 key"; then ok "doctor reports key absent when KEY_PRESENT unset"; else no "doctor absent-key wording drift: $out2"; fi

# config perms: 0644 → warning should say 600 (exit code unchanged, still 0)
CFG644="$T/cfg644.json"; echo '{}' > "$CFG644"; chmod 644 "$CFG644"
out="$(CSP_CONFIG="$CFG644" "$DOCTOR" 2>&1)"; rc=$?
if echo "$out" | grep -q "600"; then ok "doctor warns on non-600 config perms"; else no "doctor missed bad config perms: $out"; fi

# config is symlink → reject (fail-closed)
CFGLINK="$T/cfglink.json"; ln -s "$CFG644" "$CFGLINK"
out="$(CSP_CONFIG="$CFGLINK" "$DOCTOR" 2>&1)"; rc=$?
if [ $rc -ne 0 ] && echo "$out" | grep -q "符号链接"; then ok "doctor rejects symlinked config"; else no "doctor accepted symlinked config (rc=$rc): $out"; fi

# ---------- verify-proxy ----------
out="$("$VERIFY" --port 8765 --secret anything 2>&1)"; rc=$?
if [ $rc -ne 0 ] && echo "$out" | grep -q "8765"; then ok "verify-proxy rejects reserved port 8765"; else no "verify-proxy did not reject 8765 (rc=$rc): $out"; fi

if [ "$(python3 "$ROOT/test/_capability.py")" != "1" ]; then
  echo "skip - verify-proxy 段 env-blocked（loopback 被禁，无法起临时代理）"
else
# Find a free port, start a real proxy (fake key; upstream URL is dummy but /health and /v1/models never reach it)
P="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')"
SEC="verify-test-secret"
DEEPSEEK_API_KEY=fake CSP_UPSTREAM_URL="http://127.0.0.1:1/never" \
  python3 "$PROXY" --provider deepseek --port "$P" --auth-token "$SEC" \
  >/dev/null 2>&1 &
PROXY_PID=$!
# Wait for healthy
up=0
for _ in $(seq 1 50); do
  if curl -s -m 2 "http://127.0.0.1:$P/$SEC/health" 2>/dev/null | grep -q '"ok"'; then up=1; break; fi
  sleep 0.1
done
if [ "$up" = "1" ]; then ok "test proxy came up"; else no "test proxy did not start"; fi

out="$("$VERIFY" --port "$P" --secret "$SEC" 2>&1)"; rc=$?
if [ $rc -eq 0 ] && echo "$out" | grep -q "代理校验通过"; then ok "verify-proxy passes on healthy proxy"; else no "verify-proxy failed on healthy proxy (rc=$rc): $out"; fi

out="$("$VERIFY" --port "$P" 2>&1)"; rc=$?
if [ $rc -ne 0 ]; then ok "verify-proxy fails without required secret (403)"; else no "verify-proxy passed without secret (rc=$rc): $out"; fi

kill "$PROXY_PID" 2>/dev/null; wait "$PROXY_PID" 2>/dev/null
out="$("$VERIFY" --port "$P" --secret "$SEC" 2>&1)"; rc=$?
if [ $rc -ne 0 ] && echo "$out" | grep -q "✗"; then ok "verify-proxy fails when proxy is down"; else no "verify-proxy passed with proxy down (rc=$rc): $out"; fi
fi   # end verify-proxy loopback-gate

# ---------- self-test ----------
# Static checks only (no real run, avoid run_all recursion): executable + delegates to run_all.sh
if [ -x "$SELFTEST" ]; then ok "self-test.sh is executable"; else no "self-test.sh not executable"; fi
if grep -q "run_all.sh" "$SELFTEST"; then ok "self-test delegates to run_all.sh"; else no "self-test does not delegate to run_all.sh"; fi

# ---------- bundle resource cleanup ----------
mkdir -p "$ROOT/proxy/__pycache__"
: > "$ROOT/proxy/__pycache__/csp-clean-test.pyc"
: > "$ROOT/scripts/csp-clean-test.pyc"
: > "$ROOT/proxy/csp-clean-test.pyo"
: > "$ROOT/scripts/csp-clean-test.pyo"
out="$("$CLEAN" 2>&1)"; rc=$?
if [ $rc -eq 0 ]; then ok "clean-bundle-resources exits 0"; else no "clean-bundle-resources failed (rc=$rc): $out"; fi
if [ ! -e "$ROOT/proxy/__pycache__/csp-clean-test.pyc" ] \
  && [ ! -e "$ROOT/scripts/csp-clean-test.pyc" ] \
  && [ ! -e "$ROOT/proxy/csp-clean-test.pyo" ] \
  && [ ! -e "$ROOT/scripts/csp-clean-test.pyo" ]; then
  ok "clean-bundle-resources removes pycache artifacts from bundled dirs"
else
  no "clean-bundle-resources left pycache artifacts behind"
fi

echo "----"
if [ $FAILS -eq 0 ]; then echo "ALL PASS"; exit 0; else echo "$FAILS FAILED"; exit 1; fi
