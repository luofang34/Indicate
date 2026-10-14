#!/usr/bin/env bash
# Selftest for check-identifier-registry.sh.
#
# Every case drives one refusal against a copy of the registry or a
# synthetic source tree and asserts both the exit status and the
# reason, because a check that reports resolved citations when it
# scanned nothing is the failure worth catching.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

check="$root_dir/scripts/check-identifier-registry.sh"
work="$(mktemp -d)"
registry="$work/identifier-registry.md"
sources="$work/src"
output_file="$work/output"
passed=0
failed=0
trap 'rm -rf "$work"' EXIT

restore() {
    cp docs/instruments/identifier-registry.md "$registry"
    rm -rf "$sources"
    mkdir -p "$sources/crate/src"
    printf '//! Cites ALR-01 and ADR-0029; FC-ATT-06 and AIR-IN-008 belong to other registries.\n' >"$sources/crate/src/lib.rs"
    export INDICATE_IDENTIFIER_REGISTRY="$registry"
    export INDICATE_IDENTIFIER_SOURCES="$sources"
}

run_check() {
    "$check" >"$output_file" 2>&1
}

accepts() {
    if run_check; then
        echo "ok: $1"
        passed=$((passed + 1))
    else
        echo "REGRESSION: $1 — refused, saying: $(head -1 "$output_file")" >&2
        failed=$((failed + 1))
    fi
}

refuses() {
    if run_check; then
        echo "REGRESSION: $1 was accepted; the guard did not notice" >&2
        failed=$((failed + 1))
        return
    fi
    if ! grep -Fq "$2" "$output_file"; then
        echo "REGRESSION: $1 was refused, but not for '$2'" >&2
        echo "  it said: $(head -1 "$output_file")" >&2
        failed=$((failed + 1))
        return
    fi
    echo "ok: $1"
    passed=$((passed + 1))
}

restore
accepts "citations of defined identifiers, with other families ignored"

restore
unset INDICATE_IDENTIFIER_SOURCES
accepts "the real tree against the committed registry"

restore
printf '// Implements ZZZ-99.\n' >>"$sources/crate/src/lib.rs"
refuses "a citation with no row" "UNDEFINED IDENTIFIER: ZZZ-99"

restore
# shellcheck disable=SC2016
sed -i.bak '/^| `ALR-01` /d' "$registry" && rm -f "$registry.bak"
refuses "a row removed from the registry" "UNDEFINED IDENTIFIER: ALR-01"

restore
printf '//! nothing cited here\n' >"$sources/crate/src/lib.rs"
refuses "a source tree with no citations at all" "EMPTY: no identifier citations"

restore
printf '# empty\n' >"$registry"
refuses "an empty registry" "EMPTY:"

restore
rm -f "$registry"
refuses "a missing registry" "MISSING:"

echo "identifier-registry-selftest: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
