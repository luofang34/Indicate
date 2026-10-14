#!/usr/bin/env bash
# Selftest for check-set-docs.sh.
#
# Every case works on a copy of the set documents and asserts both the
# exit status and the reason, because a check that reports current
# documents when it read nothing is the failure worth catching.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

check="$root_dir/scripts/check-set-docs.sh"
work="$(mktemp -d)"
docs="$work/docs"
output_file="$work/output"
export INDICATE_SET_DOCS_DIR="$docs"
passed=0
failed=0
trap 'rm -rf "$work"' EXIT

restore() {
    rm -rf "$docs"
    mkdir -p "$docs"
    cp docs/instruments/set-*.md "$docs/"
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
accepts "the committed set documents are current"

restore
rm -f "$docs/set-template.md"
refuses "a set crate with no document" "MISSING SET DOCUMENT"

restore
sed -i.bak 's/| Background | Opaque |/| Background | Cedeable |/' "$docs/set-template.md" && rm -f "$docs/set-template.md.bak"
refuses "a generated block that drifted from the descriptor" "STALE:"

restore
sed -i.bak '/descriptor facts: begin/,/descriptor facts: end/d' "$docs/set-template.md" && rm -f "$docs/set-template.md.bak"
refuses "a document with no generated block" "NO GENERATED BLOCK"

restore
sed -i.bak 's/^## Intended function$/## Purpose/' "$docs/set-template.md" && rm -f "$docs/set-template.md.bak"
refuses "a document with no intended-function section" "NO INTENDED FUNCTION"

restore
sed -i.bak -E 's/AIR-[A-Z]+-[0-9]{3}/REQ/g' "$docs/set-template.md" && rm -f "$docs/set-template.md.bak"
refuses "a document that cites no requirement" "NO REQUIREMENT CITED"

rm -rf "$docs"
refuses "a missing documents directory" "MISSING:"

echo "set-docs-selftest: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
