#!/usr/bin/env bash
# Clean-room checks (guide Prompt 2, Invariant 14). Two rules, applied to every file git sees in the
# repository (tracked or not yet tracked; ignored files such as refs/ and target/ are skipped):
#   1. No file contains a string listed in scripts/clean-room-denylist.txt (human-maintained).
#   2. A file whose comment header reads `ported-from: <owner>/<repo> <path> <40-character commit>` has a
#      THIRD_PARTY_NOTICES.md row for that repository and commit, with an MIT or Apache-2.0 licence.
#      Penpot (MPL-2.0) and tldraw (commercial) are never accepted as sources.
#
# Usage: scripts/check-clean-room.sh [--root <dir>]
#   --root  the repository to check; defaults to this one (the tests pass fixture directories)
set -euo pipefail

tools="$(cd "$(dirname "$0")/.." && pwd)"
root="$tools"
while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="$2"; shift 2 ;;
    -h | --help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "check-clean-room: unknown argument: $1" >&2; exit 2 ;;
  esac
done

denylist="$root/scripts/clean-room-denylist.txt"
notices="$root/THIRD_PARTY_NOTICES.md"
denylist_rel="scripts/clean-room-denylist.txt"
header_re='^[[:space:]]*(//|#|/\*|\*|--|;|<!--)?[[:space:]]*ported-from:[[:space:]]*(.*)$'
source_re='^([A-Za-z0-9._-]+/[A-Za-z0-9._-]+)[[:space:]]+([^[:space:]]+)[[:space:]]+([0-9a-f]{40})([[:space:]]|\*/|-->|$)'

if [ ! -f "$denylist" ] || [ ! -f "$notices" ]; then
  echo "check-clean-room: $denylist and $notices must both exist" >&2
  exit 1
fi

# Files git sees, or (outside a repository, as in the tests) every file except the usual build output.
list_files() {
  local top
  top="$(git -C "$root" rev-parse --show-toplevel 2>/dev/null || true)"
  if [ -n "$top" ] && [ "$top" = "$(cd "$root" && pwd -P)" ]; then
    git -C "$root" ls-files --cached --others --exclude-standard
  else
    (cd "$root" && find . -type f \
      -not -path '*/node_modules/*' -not -path './.git/*' -not -path './target/*' \
      -not -path './refs/*' -not -path './.design-cache/*' -not -path '*/.turbo/*' |
      sed 's|^\./||')
  fi
}

patterns="$(mktemp)"
trap 'rm -f "$patterns"' EXIT
sed -e '/^[[:space:]]*#/d' -e '/^[[:space:]]*$/d' "$denylist" > "$patterns"

failures=0
checked=0
headers=0
while IFS= read -r file; do
  [ "$file" = "$denylist_rel" ] && continue
  case "$file" in
    *.pen) continue ;; # the design file is encrypted; it is read only through the Pencil MCP (guide §2.1)
  esac
  [ -f "$root/$file" ] || continue
  checked=$((checked + 1))

  if [ -s "$patterns" ] && grep -I -q -F -f "$patterns" "$root/$file" 2>/dev/null; then
    echo "check-clean-room: $file contains a string from $denylist_rel"
    failures=$((failures + 1))
  fi

  while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    headers=$((headers + 1))
    line="${hit#*:}"
    if ! [[ "$line" =~ $header_re ]]; then
      continue
    fi
    rest="${BASH_REMATCH[2]}"
    if ! [[ "$rest" =~ $source_re ]]; then
      echo "check-clean-room: $file: ported-from header must read: ported-from: <owner>/<repo> <path> <40-character commit>"
      failures=$((failures + 1))
      continue
    fi
    repo="${BASH_REMATCH[1]}"
    commit="${BASH_REMATCH[3]}"
    case "$repo" in
      penpot/* | tldraw/*)
        echo "check-clean-room: $file is ported from $repo. Penpot (MPL-2.0) and tldraw (commercial) are never ported."
        failures=$((failures + 1))
        continue
        ;;
    esac
    row="$(grep -F -- "$commit" "$notices" 2>/dev/null | grep -F -- "| $repo |" 2>/dev/null || true)"
    if [ -z "$row" ]; then
      echo "check-clean-room: $file is ported from $repo at $commit, but THIRD_PARTY_NOTICES.md has no row for it"
      failures=$((failures + 1))
    elif ! grep -E -q 'MIT|Apache-2\.0' <<< "$row" || grep -E -q 'MPL|commercial' <<< "$row"; then
      echo "check-clean-room: $file is ported from $repo, but its THIRD_PARTY_NOTICES.md row is not MIT or Apache-2.0"
      failures=$((failures + 1))
    fi
  done < <(grep -I -n -E "$header_re" "$root/$file" 2>/dev/null || true)
done < <(list_files)

if [ "$failures" -ne 0 ]; then
  echo "check-clean-room: FAILED with $failures problem(s) in $checked file(s)" >&2
  exit 1
fi
echo "check-clean-room: $checked files checked, $headers ported-from header line(s) with notices, no denylisted strings"
