#!/usr/bin/env bash
# Guards the freshness of recorded run evidence (#44): each artifact's
# recorded `summary:` must be what its own recorded `command:` prints
# from this tree.
#
# The evidence gate proves a record's identity — the artifact hashes to
# its digest, its fields agree with the graph, its baseline is
# reachable, its bound sources match. It deliberately runs no build, so
# it cannot prove freshness: a test added to a suite without an edit to
# the bound source file moves no digest, and the recorded count drifts
# with every gate green.
#
# Freshness needs a build, which is why the gate does not check it. This
# script does, because CI has already paid for that build by the time
# it runs. The two are separate on purpose: the gate stays a program
# that reads files and runs git, and freshness is a step beside it.
#
# What this deliberately does NOT prove:
#
#   - That the recorded run happened. A record whose summary matches
#     this tree may still have been hand-written. Nothing here, and
#     nothing in the gate, distinguishes a genuine execution from a
#     consistent hand-edit; only the discipline in CONTRIBUTING.md
#     does.
#   - That the artifact is the right one for its result node. That is
#     the gate's `output-digest` check, not this one.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

artifact_dir="${INDICATE_EVIDENCE_ARTIFACTS:-docs/instruments/evidence-artifacts}"
status=0

fail_closed() {
    echo "$1" >&2
    echo "check-recorded-counts: FAILED" >&2
    exit 1
}

[ -d "$artifact_dir" ] ||
    fail_closed "MISSING ARTIFACTS: $artifact_dir does not exist (fail-closed)"

# The recorded command, verbatim from the artifact. A record with no
# command names no run, which is a malformed record rather than a
# passing one.
recorded_command() {
    sed -n 's/^command: //p' "$1"
}

# The recorded result lines, in order. `finished in` carries a duration
# that differs between runs of the same suite, so it is not part of what
# a record claims.
recorded_summary() {
    sed -n '/^summary:$/,$p' "$1" | tail -n +2 | sed 's/; finished in .*//'
}

# What the command prints from this tree now, normalized the same way.
current_summary() {
    # shellcheck disable=SC2086
    ${1} --quiet 2>&1 | sed -n 's/^\(test result:.*\)$/\1/p' | sed 's/; finished in .*//'
}

check_artifact() {
    local artifact="$1"
    local command recorded current
    command="$(recorded_command "$artifact")"
    if [ -z "$command" ]; then
        echo "MALFORMED: $artifact declares no command" >&2
        status=1
        return
    fi
    recorded="$(recorded_summary "$artifact")"
    if [ -z "$recorded" ]; then
        echo "MALFORMED: $artifact declares no summary" >&2
        status=1
        return
    fi
    current="$(current_summary "$command --locked")"
    if [ "$recorded" != "$current" ]; then
        echo "DRIFT: $artifact records a run of '$command' that this tree" >&2
        echo "       no longer produces." >&2
        echo "  recorded: $(echo "$recorded" | tr '\n' '|')" >&2
        echo "  this tree: $(echo "$current" | tr '\n' '|')" >&2
        status=1
        return
    fi
    echo "ok: $artifact matches '$command'"
}

# The guard ships with the proof that it refuses: a record whose count
# has drifted must fail, or a green result says nothing.
selftest() {
    local dir count
    dir="$(mktemp -d)"
    trap 'rm -rf "$dir"' RETURN
    count=0

    # A record whose summary does not match its command.
    printf 'command: echo\nsummary:\ntest result: ok. 1 passed\n' >"$dir/drift.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$0" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a drifted count was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    # A record with no command at all.
    rm -f "$dir/drift.run.txt"
    printf 'summary:\ntest result: ok. 1 passed\n' >"$dir/nocmd.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$0" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a record with no command was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    # A record with no summary at all.
    rm -f "$dir/nocmd.run.txt"
    printf 'command: echo\n' >"$dir/nosum.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$0" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a record with no summary was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    echo "recorded-counts-selftest: OK ($count cases)"
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi

shopt -s nullglob
artifacts=("$artifact_dir"/*/*.run.txt)
shopt -u nullglob

[ ${#artifacts[@]} -gt 0 ] ||
    fail_closed "NO ARTIFACTS: $artifact_dir holds no *.run.txt (fail-closed)"

for artifact in "${artifacts[@]}"; do
    check_artifact "$artifact"
done

if [ "$status" -ne 0 ]; then
    echo "check-recorded-counts: FAILED" >&2
    exit 1
fi

echo "check-recorded-counts: OK (${#artifacts[@]} records match this tree)"
echo "check-recorded-counts: freshness only; that a run happened is the gate's" \
    "identity checks plus the discipline in CONTRIBUTING.md"
