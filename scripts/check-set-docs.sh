#!/usr/bin/env bash
# Every set crate has a set document, and every set document's generated
# block is what the descriptors declare now.
#
# A set document is the per-set half of the documentation: intended
# function, inputs, failure presentation, requirements, and the block
# `cargo xtask gen-set-docs` writes from the descriptors. This check
# fails closed on a set crate with no document, on a document with no
# generated block, and on a block that no longer matches the code. It
# does not judge the prose; review does.
#
# Set INDICATE_SET_DOCS_DIR to check a copy of the documents, which is
# what the selftest does.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

docs_dir="${INDICATE_SET_DOCS_DIR:-docs/instruments}"
status=0

fail_closed() {
    echo "$1" >&2
    echo "check-set-docs: FAILED" >&2
    exit 1
}

[ -d "$docs_dir" ] || fail_closed "MISSING: $docs_dir does not exist (fail-closed)"

package_name() {
    awk '
        /^\[package\]/ { in_package = 1; next }
        /^\[/ { in_package = 0 }
        in_package && /^name[[:space:]]*=/ {
            sub(/^name[[:space:]]*=[[:space:]]*"/, "")
            sub(/".*$/, "")
            print
            exit
        }
    ' "$1"
}

found_any=0
for manifest in sets/*/Cargo.toml; do
    [ -f "$manifest" ] || continue
    found_any=1
    name="$(package_name "$manifest")"
    [ -n "$name" ] || fail_closed "MALFORMED: $manifest declares no package name"
    if ! grep -lF "Crate: \`$name\`" "$docs_dir"/set-*.md >/dev/null 2>&1; then
        echo "MISSING SET DOCUMENT: no $docs_dir/set-*.md names crate $name in its generated block" >&2
        status=1
    fi
done
[ "$found_any" -eq 1 ] || fail_closed "EMPTY: no set crates found under sets/ (fail-closed)"

for doc in "$docs_dir"/set-*.md; do
    [ -f "$doc" ] || continue
    if ! grep -qF 'descriptor facts: begin' "$doc" || ! grep -qF 'descriptor facts: end' "$doc"; then
        echo "NO GENERATED BLOCK: $doc lacks the descriptor-facts markers" >&2
        status=1
    fi
    if ! grep -qE '^## Intended function' "$doc"; then
        echo "NO INTENDED FUNCTION: $doc has no '## Intended function' section" >&2
        status=1
    fi
    if ! grep -qE 'AIR-[A-Z]+-[0-9]{3}' "$doc"; then
        echo "NO REQUIREMENT CITED: $doc cites no AIR-* requirement (AIR-BAS-005)" >&2
        status=1
    fi
done

if ! INDICATE_SET_DOCS_DIR="$docs_dir" cargo run --locked -q -p xtask -- gen-set-docs --check 2>&1 | sed 's/^/  /'; then
    echo "STALE: a set document's generated block differs from the descriptors; run 'cargo xtask gen-set-docs'" >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    echo "check-set-docs: FAILED" >&2
    exit 1
fi
echo "check-set-docs: OK"
