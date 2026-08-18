#!/usr/bin/env bash
# Selftest for check-baseline-survives-merge.sh.
#
# The check is advisory, which is exactly why it needs this: an advisory
# step that exits 0 on every path looks identical to an advisory step
# that found nothing, and nobody investigates a green step. Each case
# below drives one path and asserts on the exit status, not on the text.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

check="$root_dir/scripts/check-baseline-survives-merge.sh"
passed=0
failed=0
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# `expect <want> <label>` runs the check against the environment already
# exported by the caller and compares its exit status to `want`, where 0
# means "checked, and every baseline is on the base".
expect() {
    local want="$1" label="$2" got=0
    "$check" >/dev/null 2>&1 || got=$?
    if [ "$got" -eq "$want" ]; then
        echo "ok: $label"
        passed=$((passed + 1))
    else
        echo "REGRESSION: $label — wanted exit $want, got $got" >&2
        failed=$((failed + 1))
    fi
}

graph_with() {
    local path="$1"
    shift
    {
        echo "node RESULT-PROBE verification-result"
        for digest in "$@"; do
            echo "attr config-digest $digest"
        done
    } > "$path"
}

on_base="$(git rev-parse "${INDICATE_MERGE_BASE:-origin/main}")"
head_commit="$(git rev-parse HEAD)"

# A baseline already on the base branch survives any merge button.
graph_with "$work/on-base.evg" "$on_base"
INDICATE_EVIDENCE_GRAPH="$work/on-base.evg" expect 0 "a baseline on the base is accepted"

# A commit that exists but is not on the base is the case the check is
# for. It must be refused, or the advisory step is green on exactly the
# branch it exists to warn about.
if [ "$head_commit" != "$on_base" ] && ! git merge-base --is-ancestor "$head_commit" "$on_base"; then
    graph_with "$work/branch-local.evg" "$head_commit"
    INDICATE_EVIDENCE_GRAPH="$work/branch-local.evg" expect 1 "a branch-local baseline is refused"
else
    echo "ok: skipped the branch-local case — this checkout is on the base"
    passed=$((passed + 1))
fi

# A well-formed digest naming no object in this clone cannot be placed.
# Counting it and then printing the all-clear is the fail-open shape.
graph_with "$work/unknown.evg" "0123456789abcdef0123456789abcdef01234567"
INDICATE_EVIDENCE_GRAPH="$work/unknown.evg" expect 1 "an unplaceable baseline is refused"

# A graph that parses to no baseline at all: the digest is one character
# short, so the pattern matches nothing and the loop never runs.
graph_with "$work/short.evg" >/dev/null
echo "attr config-digest 0123456789abcdef0123456789abcdef0123456" > "$work/short.evg"
INDICATE_EVIDENCE_GRAPH="$work/short.evg" expect 1 "a graph with no parsable baseline is refused"

# A graph that cannot be read at all.
INDICATE_EVIDENCE_GRAPH="$work/absent.evg" expect 1 "an unreadable graph is refused"

# A base ref that is not fetched: nothing can be placed against it.
INDICATE_EVIDENCE_GRAPH="$work/on-base.evg" INDICATE_MERGE_BASE="refs/heads/no-such-base" \
    expect 1 "an unfetched base is refused"

if [ "$failed" -ne 0 ]; then
    echo "baseline-survives-merge-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "baseline-survives-merge-selftest: OK ($passed cases)"
