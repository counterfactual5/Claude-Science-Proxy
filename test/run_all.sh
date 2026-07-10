#!/usr/bin/env bash
# Legacy entry point — delegates to layered runners (see test/runners/run_all.sh).
exec "$(cd "$(dirname "$0")" && pwd)/runners/run_all.sh" "$@"
