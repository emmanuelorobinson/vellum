#!/usr/bin/env bash
# Opt-in engines lane: records only a fixed TEST DATA prompt, never raw native stdout/stderr or auth metadata.
set -euo pipefail
export TURBO_TELEMETRY_DISABLED=1 NEXT_TELEMETRY_DISABLED=1
exec node "$(dirname "$0")/record-engine-fixtures.mjs" "$@"
