#!/usr/bin/env bash
# Dependency rules (guide Prompt 2; ARCHITECTURE §2.1). Each mode is one check:
#   rust  the pure crates depend only on scripts/rust-deps-allowlist.txt, and no vendor crate is in their
#         dependency closure (cargo tree)
#   deny  cargo-deny's bans, licences and sources (deny.toml). Advisories need the network: pnpm cargo:deny
#   ts    dependency-cruiser's rules (.dependency-cruiser.cjs) over packages/ and apps/
#   all   every check above; this is what `pnpm deps:check` runs
#
# Usage: scripts/check-deps.sh [all|rust|deny|ts] [--root <dir>]
#   --root  the workspace to check; defaults to this repository (the tests pass fixture workspaces)
set -euo pipefail

tools="$(cd "$(dirname "$0")/.." && pwd)"
root="$tools"
mode="all"
while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="$2"; shift 2 ;;
    all | rust | deny | ts) mode="$1"; shift ;;
    -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "check-deps: unknown argument: $1" >&2; exit 2 ;;
  esac
done

check_rust() {
  local allowlist="$root/scripts/rust-deps-allowlist.txt"
  local manifest="$root/Cargo.toml"
  local failures=0 vendors="" line kind crate allowed tree closure names direct dep vendor err

  if [ ! -f "$allowlist" ]; then
    echo "check-deps: $allowlist is missing" >&2
    return 1
  fi
  err="$(mktemp)"

  while IFS= read -r line; do
    case "$line" in
      vendor\ *) vendors="$vendors ${line#vendor }" ;;
    esac
  done < "$allowlist"

  while IFS= read -r line; do
    case "$line" in
      pure\ *) ;;
      *) continue ;;
    esac
    read -r kind crate allowed <<< "$line"

    if ! tree="$(cargo tree --locked --manifest-path "$manifest" -p "$crate" --depth 1 \
        --edges normal,build,dev --target all --prefix none 2>"$err")"; then
      echo "check-deps: cannot read the dependencies of $crate:" >&2
      cat "$err" >&2
      failures=$((failures + 1))
      continue
    fi
    direct="$(printf '%s\n' "$tree" | awk 'NR > 1 && NF { print $1 }' | sort -u)"
    for dep in $direct; do
      case " $allowed " in
        *" $dep "*) ;;
        *)
          echo "check-deps: $crate depends directly on $dep, which is not on its allowlist"
          failures=$((failures + 1))
          ;;
      esac
    done

    if ! closure="$(cargo tree --locked --manifest-path "$manifest" -p "$crate" \
        --edges normal,build,dev --target all --prefix none 2>"$err")"; then
      echo "check-deps: cannot read the dependency closure of $crate:" >&2
      cat "$err" >&2
      failures=$((failures + 1))
      continue
    fi
    names="$(printf '%s\n' "$closure" | awk 'NF { print $1 }' | sort -u)"
    for vendor in $vendors; do
      if grep -qx "$vendor" <<< "$names"; then
        echo "check-deps: $crate reaches $vendor, a vendor crate, through its dependencies (ARCHITECTURE §2.1)"
        failures=$((failures + 1))
      fi
    done
  done < "$allowlist"

  rm -f "$err"
  if [ "$failures" -ne 0 ]; then
    return 1
  fi
  echo "check-deps: rust: pure crates keep to their allowlist and no vendor crate is in their closure"
}

check_deny() {
  if ! cargo deny --version >/dev/null 2>&1; then
    echo "check-deps: cargo-deny is not installed (cargo install cargo-deny --locked --version 0.20.2)" >&2
    return 1
  fi
  cargo deny --manifest-path "$root/Cargo.toml" --config "$root/deny.toml" check bans licenses sources
  echo "check-deps: deny: bans, licences and sources pass"
}

check_ts() {
  local depcruise="$tools/node_modules/.bin/depcruise"
  local config="$root/.dependency-cruiser.cjs"
  local targets="" dir

  if [ ! -x "$depcruise" ]; then
    echo "check-deps: $depcruise is missing; run pnpm install" >&2
    return 1
  fi
  if [ ! -f "$config" ]; then
    echo "check-deps: $config is missing" >&2
    return 1
  fi
  for dir in packages apps; do
    if [ -d "$root/$dir" ]; then
      targets="$targets $dir"
    fi
  done
  if [ -z "$targets" ]; then
    echo "check-deps: no packages/ or apps/ under $root" >&2
    return 1
  fi
  # A run that cruised no modules, or that lacks a TypeScript compiler it can use, checked nothing: fail.
  local out status=0
  out="$(cd "$root" && "$depcruise" --config "$config" --output-type err $targets 2>&1)" || status=$?
  echo "$out"
  if [ "$status" -ne 0 ]; then
    return 1
  fi
  if grep -q "missing-typescript-transpiler\|(0 modules" <<< "$out"; then
    echo "check-deps: dependency-cruiser read no TypeScript; its rules were not checked" >&2
    return 1
  fi
  echo "check-deps: ts: packages/ and apps/ keep to the dependency rules"
}

status=0
case "$mode" in
  all)
    check_rust || status=1
    check_deny || status=1
    check_ts || status=1
    ;;
  rust) check_rust || status=1 ;;
  deny) check_deny || status=1 ;;
  ts) check_ts || status=1 ;;
esac

if [ "$status" -ne 0 ]; then
  echo "check-deps: FAILED ($mode)" >&2
  exit 1
fi
