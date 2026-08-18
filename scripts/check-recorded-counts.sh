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

self="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
root_dir="$(cd "$(dirname "$self")/.." && pwd)"
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
#
# The recorded command is run with `--locked --quiet` appended. That is
# a variant of what the record names, not the record's command verbatim:
# `--locked` only asserts the lockfile is current and `--quiet` keeps the
# `test result:` lines, so neither changes which tests run or how many
# pass. A recorded command carrying its own `--` would receive them as
# harness arguments, which no recorded command does today.
run_command() {
    local output
    # shellcheck disable=SC2086
    if ! output="$(${1} --locked --quiet 2>&1)"; then
        printf '%s' "$output"
        return 1
    fi
    printf '%s' "$output" | sed -n 's/^\(test result:.*\)$/\1/p' | sed 's/; finished in .*//'
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
    local output
    if ! output="$(run_command "$command")"; then
        echo "UNRUNNABLE: $artifact records '$command', which does not run here:" >&2
        echo "$output" | tail -n 5 >&2
        status=1
        return
    fi
    current="$output"
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

    # Artifacts live one directory deep, under a scope; fixtures must
    # too, or every case would exit at the empty-directory guard without
    # reaching the comparison it claims to prove.
    mkdir -p "$dir/scope"

    # A record whose summary does not match its command.
    printf 'command: echo\nsummary:\ntest result: ok. 1 passed\n' >"$dir/scope/drift.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$self" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a drifted count was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    # A record with no command at all. The empty-command branch refuses
    # it; so does the unrunnable branch if that check ever moves, which
    # is why this case pins the refusal rather than the branch.
    rm -f "$dir/scope/drift.run.txt"
    printf 'summary:\ntest result: ok. 1 passed\n' >"$dir/scope/nocmd.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$self" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a record with no command was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    # A record with no summary at all.
    rm -f "$dir/scope/nocmd.run.txt"
    printf 'command: echo\n' >"$dir/scope/nosum.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$self" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: a record with no summary was accepted" >&2
        return 1
    fi
    count=$((count + 1))

    # A record naming a command that cannot run must be reported as
    # unrunnable, not pass and not abort in silence.
    rm -f "$dir/scope/nosum.run.txt"
    printf 'command: cargo test -p no-such-crate-here\nsummary:\ntest result: ok. 1 passed\n' \
        >"$dir/scope/unrunnable.run.txt"
    if INDICATE_EVIDENCE_ARTIFACTS="$dir" "$self" >/dev/null 2>&1; then
        echo "recorded-counts-selftest: an unrunnable command was accepted" >&2
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
