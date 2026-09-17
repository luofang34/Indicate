#!/usr/bin/env bash
# Selftest for check-ci-closure.sh.
#
# Every case drives one refusal against a copy of the workflow or of
# CONTRIBUTING.md and asserts both the exit status and the reason,
# because a check that reports clean lists when it read nothing is the
# failure worth catching.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

check="$root_dir/scripts/check-ci-closure.sh"
work="$(mktemp -d)"
workflow="$work/ci.yml"
contributing="$work/CONTRIBUTING.md"
output_file="$work/output"
export INDICATE_CI_WORKFLOW="$workflow"
export INDICATE_CONTRIBUTING="$contributing"
passed=0
failed=0
trap 'rm -rf "$work"' EXIT

restore() {
    cp .github/workflows/ci.yml "$workflow"
    cp CONTRIBUTING.md "$contributing"
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
accepts "the committed lists are accepted"

restore
sed -i.bak 's/ indicate-instrument-template / /' "$workflow" && rm -f "$workflow.bak"
refuses "a set dropped from the allowlist" "MISSING FROM ALLOWLIST: indicate-instrument-template"

restore
sed -i.bak '/^ *-p indicate-instrument-template$/d' "$workflow" && rm -f "$workflow.bak"
refuses "a no_std set dropped from the closure" "MISSING FROM CLOSURE: indicate-instrument-template"

restore
# Duplicates the registry line with the host-side harness in its place;
# awk rather than sed because BSD sed cannot spell a newline in a
# replacement.
awk '{ print } /^ *-p indicate-instrument-registry$/ { sub(/-p .*/, "-p indicate-instrument-conformance"); print }' "$workflow" >"$workflow.tmp" && mv "$workflow.tmp" "$workflow"
refuses "a host-side crate added to the closure" "NOT NO_STD: indicate-instrument-conformance"

restore
sed -i.bak 's/-p indicate-instrument-template -p indicate-instrument-hmd/-p indicate-instrument-template/' "$contributing" && rm -f "$contributing.bak"
refuses "CONTRIBUTING drifting from the CI closure" "DRIFT: the cargo check closure"

restore
sed -i.bak '/known="/,/"$/d' "$workflow" && rm -f "$workflow.bak"
refuses "a workflow with no allowlist at all" "EMPTY: no known= allowlist"

rm -f "$workflow"
refuses "a missing workflow file" "MISSING:"

echo "ci-closure-selftest: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
