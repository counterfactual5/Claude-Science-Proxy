#!/bin/bash
# Install/uninstall/status/run the CSP daily maintenance launchd agent.
#   scripts/install-maintenance.sh install
#   scripts/install-maintenance.sh uninstall
#   scripts/install-maintenance.sh status
#   scripts/install-maintenance.sh run
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
LABEL="com.csp.maintenance"
LEGACY_LABEL="com.csswitch.maintenance"
SRC="$REPO/scripts/com.csp.maintenance.plist"
DST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"

render_plist() {
  sed "s|__REPO__|$REPO|g" "$SRC"
}

cmd="${1:-status}"
case "$cmd" in
  install)
    mkdir -p "$HOME/Library/LaunchAgents" "$REPO/findings/auto-maint/logs"
    render_plist > "$DST"
    launchctl bootout "$DOMAIN/$LEGACY_LABEL" 2>/dev/null || true
    launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
    launchctl bootstrap "$DOMAIN" "$DST"
    launchctl enable "$DOMAIN/$LABEL" 2>/dev/null || true
    echo "Installed and loaded: ${LABEL} (09:00 / 21:00 Asia/Shanghai)"
    launchctl print "$DOMAIN/$LABEL" 2>/dev/null | grep -E "state|program|run" | head || true
    ;;
  uninstall)
    launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
    launchctl bootout "$DOMAIN/$LEGACY_LABEL" 2>/dev/null || true
    rm -f "$DST" "$HOME/Library/LaunchAgents/$LEGACY_LABEL.plist"
    echo "Uninstalled: $LABEL (and legacy $LEGACY_LABEL if present)"
    ;;
  status)
    if launchctl print "$DOMAIN/$LABEL" >/dev/null 2>&1; then
      echo "== loaded ($LABEL) =="
      launchctl print "$DOMAIN/$LABEL" 2>/dev/null | grep -E "state|program arguments|run at load|next fire" -A2 | head -20 || true
    elif launchctl print "$DOMAIN/$LEGACY_LABEL" >/dev/null 2>&1; then
      echo "== legacy loaded ($LEGACY_LABEL) — run install to migrate =="
      launchctl print "$DOMAIN/$LEGACY_LABEL" 2>/dev/null | grep -E "state|program arguments" -A2 | head -10 || true
    else
      echo "== not loaded =="
    fi
    echo "== recent reports =="
    ls -t "$REPO/findings/auto-maint"/report-*.md 2>/dev/null | head -3 || echo "(none yet)"
    echo "== recent logs =="
    ls -t "$REPO/findings/auto-maint/logs"/run-*.log 2>/dev/null | head -3 || echo "(none yet)"
    ;;
  run)
    launchctl kickstart -k "$DOMAIN/$LABEL"
    echo "Triggered one run; check findings/auto-maint/logs/ and report-*.md"
    ;;
  *)
    echo "usage: $0 {install|uninstall|status|run}" >&2
    exit 2
    ;;
esac
