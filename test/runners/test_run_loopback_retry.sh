# Bounded retry logic for test/run-loopback.sh (offline, deterministic; does not run real 4 loopback modules).
# Note: not wired into any layer runner yet (run-scripts.sh etc.); wiring deferred to Task 10 / final review.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fails=0

# (a) Always-fail stub → should exhaust 3 attempts, final line fail, rc=1
out_a="$(cd "$ROOT" && CSP_LOOPBACK_TEST_CMD='false' bash test/run-loopback.sh 2>&1)"; rc_a=$?
attempts_a="$(echo "$out_a" | grep -c '^loopback 尝试 ')"
[ "$rc_a" -eq 1 ] || { echo "NOT ok - 恒失败桩 rc 应为 1, 实际 $rc_a"; fails=1; }
echo "$out_a" | grep -qE '^S0_LAYER loopback fail$' || { echo "NOT ok - 恒失败桩缺 S0_LAYER loopback fail"; fails=1; }
[ "$attempts_a" -eq 2 ] || { echo "NOT ok - 恒失败桩应有 2 条重试提示, 实际 $attempts_a"; fails=1; }

# (b) Pass on 3rd attempt stub → two failure lines + final pass, rc=0
CNT="$(mktemp)"; printf 0 > "$CNT"
out_b="$(cd "$ROOT" && CSP_LOOPBACK_TEST_CMD='n=$(cat '"$CNT"'); n=$((n+1)); printf %s "$n" > '"$CNT"'; [ "$n" -ge 3 ]' bash test/run-loopback.sh 2>&1)"; rc_b=$?
rm -f "$CNT"
attempts_b="$(echo "$out_b" | grep -c '^loopback 尝试 ')"
[ "$rc_b" -eq 0 ] || { echo "NOT ok - 第3次通过桩 rc 应为 0, 实际 $rc_b"; fails=1; }
echo "$out_b" | grep -qE '^S0_LAYER loopback pass$' || { echo "NOT ok - 第3次通过桩缺 S0_LAYER loopback pass"; fails=1; }
[ "$attempts_b" -eq 2 ] || { echo "NOT ok - 第3次通过桩应有 2 条重试提示, 实际 $attempts_b"; fails=1; }
echo "$out_b" | grep -q '第 3 次尝试通过' || { echo "NOT ok - 第3次通过桩缺「通过」提示行"; fails=1; }

[ "$fails" -eq 0 ] && echo "ALL PASS" || { echo "$fails FAILED"; exit 1; }
