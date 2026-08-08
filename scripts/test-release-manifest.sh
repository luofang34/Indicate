#!/usr/bin/env bash
# Selftest for the release-manifest guard.
#
# The guard's whole claim is that a pinned value edited without
# regenerating the manifest turns CI red, so this drives it with a
# manifest whose pins were hand-edited, one with a value deleted, an
# absent one, an empty one, and a generator that cannot run — and
# requires a refusal, by name, from each. A guard that only passes on
# good input proves nothing.
#
# Every case points the guard at a copy under a temporary directory; the
# committed manifest is never written to.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

checker="$root_dir/scripts/check-release-manifest.sh"
committed="$root_dir/release-manifest.json"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

passed=0
failed=0

run_checker() {
    env \
        "INDICATE_RELEASE_MANIFEST=$1" \
        "INDICATE_MANIFEST_GENERATOR_PACKAGE=${2:-xtask}" \
        INDICATE_RELEASE_MANIFEST_SELFTEST_CHILD=1 \
        bash "$checker"
}

show_failure() {
    echo "release-manifest-selftest: $1" >&2
    sed 's/^/    /' "$output_file" >&2
    failed=$((failed + 1))
}

# $1 = case name, $2 = manifest path, $3 = generator package
expect_success() {
    output_file="$tmp_dir/$1.output"
    if run_checker "$2" "${3:-}" > "$output_file" 2>&1; then
        echo "ok: $1 accepted"
        passed=$((passed + 1))
    else
        show_failure "$1 unexpectedly failed"
    fi
}

# $1 = case name, $2 = manifest path, $3 = expected message, $4 = generator package
expect_refusal() {
    output_file="$tmp_dir/$1.output"
    if run_checker "$2" "${4:-}" > "$output_file" 2>&1; then
        show_failure "$1 was accepted; the guard did not notice"
        return
    fi
    if ! grep -Fq "$3" "$output_file"; then
        show_failure "$1 was refused, but not for '$3'"
        return
    fi
    echo "ok: $1 refused"
    passed=$((passed + 1))
}

# The committed manifest must be the generator's own output.
expect_success live "$committed"

# $1 = case name, $2 = the sed expression that plants the drift. An
# expression that changes nothing is a failed case, not a passed one:
# the fixture would otherwise quietly stop testing anything.
expect_drift() {
    local name="$1" path="$tmp_dir/$1.json"
    sed "$2" "$committed" > "$path"
    if cmp -s "$committed" "$path"; then
        output_file="$tmp_dir/$name.output"
        : > "$output_file"
        show_failure "$name edited nothing, so it proves nothing about the guard"
        return
    fi
    expect_refusal "$name" "$path" "DRIFT"
}

# A hand-edited pin with no regeneration: the failure mode this exists
# for. One case per shape of pinned value — a hash, a version number, a
# measured rectangle — so a guard that compared only part of the
# document cannot pass. Each edit is matched by shape rather than by the
# value it expects, so a pin that legitimately moves does not send an
# author here to re-edit a fixture.
expect_drift hash-drift \
    's|"compositionDigest": "[0-9a-f]\{8\}|"compositionDigest": "deadbeef|'
expect_drift version-drift \
    's|^    "version": [0-9][0-9]*|    "version": 0|'
expect_drift band-drift \
    's|"band": { "x": [0-9][0-9.]*|"band": { "x": 999|'

# The moved key is named, not only diffed: an author who has to read a
# JSON diff to learn which pin moved has been told less than the guard knows.
if ! grep -Fq "compositionDigest" "$tmp_dir/hash-drift.output"; then
    output_file="$tmp_dir/hash-drift.output"
    show_failure "hash-drift did not name the key that moved"
fi

# A manifest missing a whole value cannot be repaired by reading it.
grep -v '"glyphPackHash"' "$committed" > "$tmp_dir/truncated.json"
expect_refusal truncated "$tmp_dir/truncated.json" "DRIFT"

# An absent manifest must fail closed, never report green on no data.
expect_refusal absent "$tmp_dir/does-not-exist.json" "MISSING MANIFEST"

# An empty one too: a zero-byte file is not a statement of no pins.
: > "$tmp_dir/empty.json"
expect_refusal empty "$tmp_dir/empty.json" "EMPTY MANIFEST"

# A generator that cannot run leaves the manifest unvouched-for, which is
# a refusal and not a pass. The package name is the honest lever: cargo
# fails for real rather than a stubbed command failing on command.
expect_refusal generator-failure "$committed" "GENERATOR FAILED" "xtask-no-such-package"

if [ "$failed" -ne 0 ]; then
    echo "release-manifest-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "release-manifest-selftest: OK ($passed cases)"
