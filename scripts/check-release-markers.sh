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
# This proves agreement between the entry and the tree. It does NOT
# prove a tag exists for the entry, or that a tag points where the entry
# says: tags live in the remote, not the worktree, and CI clones without
# them. Tag discipline stays a human step.
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

# The newest entry: from the first release heading to the next one.
entry="$(awk '
    /^## \[/ { seen++; if (seen > 1) exit }
    seen == 1 { print }
' "$changelog")"

[ -n "$entry" ] || fail_closed "no release entry found in $changelog (fail-closed)"

version="$(printf '%s\n' "$entry" | awk 'NR == 1 { gsub(/^## \[|\].*$/, ""); print; exit }')"

# The value cell of a `| <label> | <value> |` row in the entry's table.
declared() {
    printf '%s\n' "$entry" | awk -F'|' -v want="$1" '
        {
            label = $2; value = $3
            gsub(/^[ \t]+|[ \t]+$/, "", label)
            gsub(/^[ \t]+|[ \t]+$/, "", value)
            gsub(/`/, "", value)
            if (label == want) { print value; exit }
        }
    '
}

# The tree's own answer for each of the five.
actual_abi="$(grep -oE '^pub const VERSION: u8 = [0-9]+' crates/indicate-instrument-state/src/abi/v6.rs | grep -oE '[0-9]+$')"
actual_scene="$(grep -oE '^pub const SCENE_FORMAT_VERSION: u8 = [0-9]+' crates/indicate-instrument-scene/src/lib.rs | grep -oE '[0-9]+$')"
actual_corpus="$(grep -oE '"corpusVersion": [0-9]+' crates/indicate-instrument-scene/corpus/scene-conformance-corpus.json | grep -oE '[0-9]+$')"
actual_digest="$(grep -A2 'BUILTIN_SCENE_DIGEST: &str' crates/indicate-instrument-panels/src/descriptors.rs \
    | grep -oE '[0-9a-f]{64}')"
# Panel ids in composition order, as the entry lists them.
actual_panels="$(awk '
    /^pub const BUILTIN_PANELS/ { collecting = 1 }
    collecting { line = line $0; if (/;/) exit }
    END {
        n = split(line, parts, /,/)
        for (i = 1; i <= n; i++) {
            if (match(parts[i], /[A-Z_]+_DESCRIPTOR/)) {
                name = tolower(substr(parts[i], RSTART, RLENGTH))
                sub(/_descriptor/, "", name)
                out = out (out == "" ? "" : ", ") name
            }
        }
        print out
    }
' crates/indicate-instrument-panels/src/descriptors.rs)"

compare() {
    local label="$1" actual="$2" found
    found="$(declared "$label")"
    if [ -z "$found" ]; then
        echo "MISSING: entry $version states no '$label' row" >&2
        status=1
        return
    fi
    if [ -z "$actual" ]; then
        echo "UNREADABLE: cannot read '$label' from the tree" >&2
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

if [ "$status" -ne 0 ]; then
    echo "check-release-markers: FAILED" >&2
    exit 1
fi

echo "check-release-markers: OK ($version; ABI $actual_abi, scene $actual_scene, corpus $actual_corpus, panels: $actual_panels)"
echo "check-release-markers: entry/tree agreement only; tag existence is not checked"
