#!/usr/bin/env bash
# Chrome layout oracle (guide Prompt 6, ADR-002): lays out fixtures/layout-oracle/*.html in headless
# Chrome and writes the expected boxes to fixtures/layout-oracle/expected/. --help lists the options.
set -euo pipefail
exec node "$(dirname "$0")/gen-layout-oracle.mjs" "$@"
