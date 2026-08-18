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

# A graph that parses to no baseline at all. The value has to be
# unparsable rather than merely short: the pattern takes 7 to 40
# characters, so a 39-character digest is a form it accepts.
echo "attr config-digest not-a-digest" > "$work/unparsable.evg"
INDICATE_EVIDENCE_GRAPH="$work/unparsable.evg" expect 1 "a graph with no parsable baseline is refused"

# A graph with no `config-digest` line at all is the case that counts
# zero, and a counter that treats an empty count as an error kills the
# script before its message runs.
echo "node RESULT-PROBE verification-result" > "$work/no-attr.evg"
INDICATE_EVIDENCE_GRAPH="$work/no-attr.evg" expect 1 "a graph declaring no baseline is refused"

said="$(INDICATE_EVIDENCE_GRAPH="$work/no-attr.evg" "$check" 2>&1 || true)"
if [ -n "$said" ]; then
    echo "ok: a graph declaring no baseline says why before it exits"
    passed=$((passed + 1))
else
    echo "REGRESSION: the no-baseline path exits silently" >&2
    failed=$((failed + 1))
fi

# And it must say so rather than dying quietly: an exit status with no
# message is indistinguishable from a crash, which is the shape the
# script's own header argues against.
# Captured rather than piped: this script runs under `pipefail`, so a
# pipeline would carry the check's own non-zero status and say nothing
# about whether it spoke.
said="$(INDICATE_EVIDENCE_GRAPH="$work/unparsable.evg" "$check" 2>&1 || true)"
if [ -n "$said" ]; then
    echo "ok: an unparsable graph says why before it exits"
    passed=$((passed + 1))
else
    echo "REGRESSION: the unparsable-graph path exits silently" >&2
    failed=$((failed + 1))
fi

# One placeable baseline beside one the pattern cannot read. The loop
# would place the first and report the all-clear, so the count of
# distinct declarations against distinct parses is what refuses it.
{
    echo "node RESULT-PROBE verification-result"
    echo "attr config-digest $on_base"
    echo "attr config-digest not-a-digest"
} > "$work/partly-parsable.evg"
INDICATE_EVIDENCE_GRAPH="$work/partly-parsable.evg" \
    expect 1 "a baseline the pattern cannot read refuses the whole run"

# A graph that cannot be read at all.
INDICATE_EVIDENCE_GRAPH="$work/absent.evg" expect 1 "an unreadable graph is refused"

# Every form the gate accepts must be parsed here. The gate hands the
# attribute to git after a trim, and git resolves abbreviated and
# upper-case object ids, so a parser that took only 40 lowercase
# characters would skip those in silence — and a graph mixing one form
# it parses with one it does not would check the first and print the
# all-clear.
short_on_base="$(git rev-parse --short=12 "${INDICATE_MERGE_BASE:-origin/main}")"
graph_with "$work/short-form.evg" "$short_on_base"
INDICATE_EVIDENCE_GRAPH="$work/short-form.evg" expect 0 "an abbreviated baseline is placed, not skipped"

upper_on_base="$(git rev-parse "${INDICATE_MERGE_BASE:-origin/main}" | tr 'a-f' 'A-F')"
graph_with "$work/upper-form.evg" "$upper_on_base"
INDICATE_EVIDENCE_GRAPH="$work/upper-form.evg" expect 0 "an upper-case baseline is placed, not skipped"

# The mixed case is the one that fails open rather than closed: one form
# parses and passes, so a narrower parser reports the all-clear while
# never having looked at the other.
{
    echo "node RESULT-PROBE verification-result"
    echo "attr config-digest $on_base"
    echo "attr config-digest 0123456789ab"
} > "$work/mixed.evg"
INDICATE_EVIDENCE_GRAPH="$work/mixed.evg" expect 1 "a graph mixing a placeable and an unplaceable baseline is refused"

# A trailing space is a form the gate accepts, because it trims.
printf 'node RESULT-PROBE verification-result\nattr config-digest %s \n' "$on_base" \
    > "$work/trailing.evg"
INDICATE_EVIDENCE_GRAPH="$work/trailing.evg" expect 0 "a trailing space does not hide a baseline"

# A base ref that is not fetched: nothing can be placed against it.
INDICATE_EVIDENCE_GRAPH="$work/on-base.evg" INDICATE_MERGE_BASE="refs/heads/no-such-base" \
    expect 1 "an unfetched base is refused"

if [ "$failed" -ne 0 ]; then
    echo "baseline-survives-merge-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "baseline-survives-merge-selftest: OK ($passed cases)"
