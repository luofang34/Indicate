#!/usr/bin/env bash
# Guards the release markers (#8): the newest CHANGELOG entry must state
# the contract versions the tree actually carries.
#
# A consumer pins a revision and reads the changelog to learn what it
# contains. A changelog that drifts from the code is worse than none —
# it answers the question wrongly instead of sending the reader to the
# source. So the five values are extracted from the newest entry and
# compared against the definitions they name.
#
# Two things this deliberately does NOT prove:
#
#   - That a tag exists for the entry, or points where the entry says.
#     CI does fetch tags, so this is not a visibility problem; it is a
#     timing one. A release tags the merge commit, which does not exist
#     while the pull request that creates the entry is open, so the
#     check would fail every release on the one run that matters. Tag
#     discipline is a human step, written down in CONTRIBUTING.md.
#   - That a release was cut when a value moved. The comparison is
#     newest-entry-against-tree, so editing a contract and its entry
#     together passes. That an entry is *added* rather than rewritten is
#     review's job, not this script's.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

changelog="${INDICATE_CHANGELOG:-CHANGELOG.md}"
status=0

fail_closed() {
    echo "$1" >&2
    echo "check-release-markers: FAILED" >&2
    exit 1
}

[ -f "$changelog" ] || fail_closed "MISSING CHANGELOG: $changelog does not exist (fail-closed)"

# The newest release entry: from the first version heading to the next.
# A heading whose bracket holds something other than a version — the
# conventional `[Unreleased]`, say — is not a release, and skipping it
# rather than reading it keeps an unreleased section from shadowing the
# entry this check exists to verify.
entry="$(awk '
    /^## \[/ {
        heading = $0
        sub(/^## \[/, "", heading)
        sub(/\].*$/, "", heading)
        is_release = (heading ~ /^[0-9]+\.[0-9]+\.[0-9]+$/)
        if (is_release) { seen++ }
        if (seen > 1) { exit }
    }
    seen == 1 { print }
' "$changelog")"

[ -n "$entry" ] || fail_closed "no versioned release entry found in $changelog (fail-closed)"

version="$(printf '%s\n' "$entry" | awk 'NR == 1 { gsub(/^## \[|\].*$/, ""); print; exit }')"

# The value cell of a `| <label> | <value> |` row in the entry's table.
# Rows inside a fenced block or an HTML comment are illustration, not
# claim, so a table that exists only as an example cannot stand in for
# the real one.
declared() {
    printf '%s\n' "$entry" | awk -F'|' -v want="$1" '
        /^[ \t]*```/ { fenced = !fenced; next }
        /<!--/ { commented = 1 }
        /-->/  { commented = 0; next }
        fenced || commented { next }
        {
            label = $2; value = $3
            gsub(/^[ \t]+|[ \t]+$/, "", label)
            gsub(/^[ \t]+|[ \t]+$/, "", value)
            gsub(/`/, "", value)
            if (label == want) { print value; exit }
        }
    '
}

# The tree's own answer for each of the five. Each reader is allowed to
# come back empty so `compare` can report an unreadable source, rather
# than `set -e` killing the run with a bare grep error.
read_or_empty() { "$@" 2>/dev/null || true; }

# The ABI in force is the highest version module the crate declares, so
# adding `v9` moves what this validates instead of leaving it pinned to
# the file that no longer answers.
abi_module="$(read_or_empty grep -oE '^pub mod v[0-9]+;' crates/indicate-instrument-state/src/abi.rs \
    | grep -oE 'v[0-9]+' | sort -V | tail -1)"
actual_abi=""
if [ -n "$abi_module" ]; then
    actual_abi="$(read_or_empty grep -oE '^pub const VERSION: u8 = [0-9]+' \
        "crates/indicate-instrument-state/src/abi/$abi_module.rs" | grep -oE '[0-9]+$')"
fi
actual_scene="$(read_or_empty grep -oE '^pub const SCENE_FORMAT_VERSION: u8 = [0-9]+' \
    crates/indicate-instrument-scene/src/lib.rs | grep -oE '[0-9]+$')"
actual_corpus="$(read_or_empty grep -oE '"corpusVersion": [0-9]+' \
    crates/indicate-instrument-scene/corpus/scene-conformance-corpus.json | grep -oE '[0-9]+$')"
actual_digest="$(read_or_empty grep -A2 'BUILTIN_SCENE_DIGEST: &str' \
    sets/indicate-instrument-panels/src/descriptors.rs | grep -oE '[0-9a-f]{64}')"

# The panel ids of BUILTIN_PANELS, in composition order. The slice names
# descriptor constants, so each is resolved to the `id` its descriptor
# declares: the id is what a shell selects and what a consumer reads,
# and a constant renamed without its id is not a panel-set change.
#
# Every descriptor file is read, not just the one holding the slice: a
# descriptor may live beside the panels it describes, and a resolution
# that only searched one file would report a moved descriptor as a
# vanished panel. An id that still cannot be resolved is refused rather
# than skipped.
actual_panels="$(read_or_empty awk '
    /^pub const [A-Z0-9_]+_DESCRIPTOR: PanelDescriptor/ {
        match($0, /[A-Z0-9_]+_DESCRIPTOR/)
        current = substr($0, RSTART, RLENGTH)
        next
    }
    current != "" && /^[ \t]*id:[ \t]*"/ {
        value = $0
        sub(/^[ \t]*id:[ \t]*"/, "", value)
        sub(/".*$/, "", value)
        id[current] = value
        current = ""
    }
    /^pub const BUILTIN_PANELS/ { collecting = 1 }
    collecting { slice = slice $0; if (/;/) { collecting = 0 } }
    END {
        n = split(slice, parts, /,/)
        for (i = 1; i <= n; i++) {
            if (match(parts[i], /[A-Z0-9_]+_DESCRIPTOR/)) {
                name = substr(parts[i], RSTART, RLENGTH)
                out = out (out == "" ? "" : ", ") (name in id ? id[name] : "<unresolved:" name ">")
            }
        }
        print out
    }
' $(find sets/indicate-instrument-panels/src -name '*.rs' | LC_ALL=C sort))"

compare() {
    local label="$1" actual="$2" found
    found="$(declared "$label")"
    if [ -z "$actual" ]; then
        echo "UNREADABLE: cannot read '$label' from the tree; the check cannot vouch for it" >&2
        status=1
        return
    fi
    if [ -z "$found" ]; then
        echo "MISSING: entry $version states no '$label' row" >&2
        status=1
        return
    fi
    if [ "$found" != "$actual" ]; then
        echo "DRIFT: entry $version says '$label' is '$found'; the tree says '$actual'" >&2
        status=1
    fi
}

compare "State ABI" "$actual_abi"
compare "Scene format" "$actual_scene"
compare "Corpus" "$actual_corpus"
compare "Composition digest" "$actual_digest"
compare "Panel set" "$actual_panels"

case "$actual_panels" in
    *'<unresolved:'*)
        echo "UNRESOLVED: a BUILTIN_PANELS entry has no readable id: $actual_panels" >&2
        status=1
        ;;
esac

if [ "$status" -ne 0 ]; then
    echo "check-release-markers: FAILED" >&2
    exit 1
fi

echo "check-release-markers: OK ($version; ABI $actual_abi, scene $actual_scene, corpus $actual_corpus, panels: $actual_panels)"
echo "check-release-markers: entry/tree agreement only; tag existence is a human step (CONTRIBUTING.md)"
