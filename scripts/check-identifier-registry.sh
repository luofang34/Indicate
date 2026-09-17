#!/usr/bin/env bash
# Every work-package or decision identifier a Rust source cites has a
# row in docs/instruments/identifier-registry.md.
#
# Comments cite identifiers such as `ALR-01` or `ADR-0029` as the name
# of the behaviour they implement. A citation a clone cannot resolve is
# a broken reference, not documentation, so this check walks every
# `.rs` file under crates/, sets/, and tools/, collects the identifiers
# of those two shapes, and requires each to appear as a table row in the
# registry. Requirement (`AIR-*`) and failure-condition (`FC-*`) ids have
# their own registries and checks and are outside this one.
#
# Set INDICATE_IDENTIFIER_REGISTRY and INDICATE_IDENTIFIER_SOURCES to
# check a copy of the registry against another source tree, which is
# what the selftest does.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

registry="${INDICATE_IDENTIFIER_REGISTRY:-docs/instruments/identifier-registry.md}"
sources="${INDICATE_IDENTIFIER_SOURCES:-crates sets tools}"
status=0

fail_closed() {
    echo "$1" >&2
    echo "check-identifier-registry: FAILED" >&2
    exit 1
}

[ -f "$registry" ] || fail_closed "MISSING: $registry does not exist (fail-closed)"

# Rows are `| \`ID\` | ...`; the first cell is the identifier. The
# backticks are Markdown, not command substitution, and a registry with
# no rows yields an empty list rather than aborting the pipeline.
# shellcheck disable=SC2016
defined="$({ grep -oE '^\| `(ADR-[0-9]{4}|[A-Z]{2,7}-[0-9]{2})` ' "$registry" || true; } | sed -E 's/^\| `//; s/` $//' | LC_ALL=C sort -u)"
[ -n "$defined" ] || fail_closed "EMPTY: $registry defines no identifier rows (fail-closed)"

# Every citation in the sources. The identifier must not be preceded by
# a letter, digit, or hyphen (so `FC-ATT-06` is a failure condition, not
# `ATT-06`) and must not be followed by a digit (so `AIR-IN-008` is a
# requirement, not `IN-00`).
cited_file="$(mktemp)"
trap 'rm -f "$cited_file"' EXIT
# shellcheck disable=SC2086
find $sources \
    -type d \( -name target -o -name .worktrees -o -name worktrees \) -prune -o \
    -type f -name '*.rs' -print \
    | LC_ALL=C sort \
    | while IFS= read -r file; do
        grep -oE '(^|[^A-Za-z0-9-])(ADR-[0-9]{4}|[A-Z]{2,7}-[0-9]{2})([^0-9A-Za-z]|$)' "$file" \
            | sed -E 's/^[^A-Z]*//; s/[^0-9A-Za-z]$//' \
            | sed "s|$|\t$file|" || true
    done > "$cited_file"

cited_ids="$(cut -f1 "$cited_file" | LC_ALL=C sort -u)"
[ -n "$cited_ids" ] || fail_closed "EMPTY: no identifier citations found under $sources; the scan has gone blind (fail-closed)"

while IFS= read -r id; do
    [ -n "$id" ] || continue
    if ! printf '%s\n' "$defined" | grep -Fxq "$id"; then
        where="$(awk -F'\t' -v id="$id" '$1 == id { print $2 }' "$cited_file" | LC_ALL=C sort -u | head -3 | tr '\n' ' ')"
        echo "UNDEFINED IDENTIFIER: $id is cited in $where but has no row in $registry" >&2
        status=1
    fi
done <<< "$cited_ids"

if [ "$status" -ne 0 ]; then
    echo "check-identifier-registry: FAILED" >&2
    exit 1
fi
echo "check-identifier-registry: OK ($(printf '%s\n' "$cited_ids" | wc -l | tr -d ' ') cited identifiers resolve)"
