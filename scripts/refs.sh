#!/usr/bin/env bash
# Clones the reference repositories pinned in scripts/refs.tsv into refs/ (guide §3, Prompt 2).
# Each clone is blob-less and shallow, checked out at its pinned commit. Re-running is safe: a clone
# already at its pin is left alone, and a changed pin moves the clone to the new commit.
# refs/ is read-only reference material. It is gitignored, and nothing in the repository writes to it.
#
# Usage: scripts/refs.sh [--root <dir>]
#   --root  repository root; defaults to this repository (the tests pass a scratch directory)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="$2"; shift 2 ;;
    -h | --help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "refs.sh: unknown argument: $1" >&2; exit 2 ;;
  esac
done

manifest="$root/scripts/refs.tsv"
refs="$root/refs"
[ -f "$manifest" ] || { echo "refs.sh: $manifest is missing" >&2; exit 1; }
mkdir -p "$refs"

# Brings refs/<name> to <sha>. Returns non-zero (after saying why) when it cannot.
sync_one() {
  local name="$1" url="$2" sha="$3"
  local dir="$refs/$name" head

  if [ -d "$dir/.git" ]; then
    head="$(git -C "$dir" rev-parse --verify --quiet HEAD || true)"
    if [ "$head" = "$sha" ]; then
      echo "refs: $name is already at $sha"
      return 0
    fi
  else
    if ! git init -q "$dir"; then
      echo "refs: $name: cannot create $dir" >&2
      return 1
    fi
  fi
  if ! git -C "$dir" remote get-url origin >/dev/null 2>&1; then
    git -C "$dir" remote add origin "$url"
  fi

  # One commit, no blobs: checkout fetches only the files of that commit that are actually read.
  if ! git -C "$dir" fetch --quiet --depth 1 --filter=blob:none origin "$sha"; then
    echo "refs: $name: could not fetch $sha from $url" >&2
    return 1
  fi
  if ! git -C "$dir" -c advice.detachedHead=false checkout --quiet --force --detach FETCH_HEAD; then
    echo "refs: $name: checkout of $sha failed" >&2
    return 1
  fi

  head="$(git -C "$dir" rev-parse HEAD)"
  if [ "$head" != "$sha" ]; then
    echo "refs: $name: HEAD is $head, expected $sha" >&2
    return 1
  fi
  echo "refs: $name checked out at $sha"
}

failed=0
count=0
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    '' | '#'*) continue ;;
  esac
  name="$(printf '%s\n' "$line" | cut -f1)"
  url="$(printf '%s\n' "$line" | cut -f2)"
  sha="$(printf '%s\n' "$line" | cut -f3)"
  [ "$name" = "name" ] && continue # the header row

  if ! [[ "$sha" =~ ^[0-9a-f]{40}$ ]]; then
    echo "refs: $name: pin \"$sha\" is not a full 40-character lowercase SHA" >&2
    failed=1
    continue
  fi
  count=$((count + 1))
  if ! sync_one "$name" "$url" "$sha"; then
    failed=1
  fi
done < "$manifest"

if [ "$failed" -ne 0 ]; then
  echo "refs.sh: some repositories could not be cloned (see above)" >&2
  exit 1
fi
echo "refs.sh: $count repositories are at their pins in $refs"
