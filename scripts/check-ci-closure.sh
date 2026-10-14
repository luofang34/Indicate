#!/usr/bin/env bash
# Holds the two hand-written crate lists in the CI workflow to the tree.
#
# The downstream-agnostic step keeps an allowlist of family crates, and
# the bare-metal step keeps the list of crates it compiles for
# thumbv7em-none-eabihf. Both are prose a person edits when a crate is
# added, and both fail in different directions when forgotten: a family
# crate absent from the allowlist reads as a boundary leak, and a
# `no_std` crate absent from the closure is a set CI never compiles
# bare-metal. This check derives both expectations from the tree:
#
#   - every workspace package whose name the allowlist's own prefix
#     filter would match must be in the allowlist;
#   - every library crate under crates/ or sets/ that declares
#     `#![no_std]` must be in the closure, and nothing else may be;
#   - the local copy of the closure command in CONTRIBUTING.md must
#     name the same crates as CI.
#
# Set INDICATE_CI_WORKFLOW and INDICATE_CONTRIBUTING to check copies of
# the files, which is what the selftest does.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

workflow="${INDICATE_CI_WORKFLOW:-.github/workflows/ci.yml}"
contributing="${INDICATE_CONTRIBUTING:-CONTRIBUTING.md}"
status=0

fail_closed() {
    echo "$1" >&2
    echo "check-ci-closure: FAILED" >&2
    exit 1
}

[ -f "$workflow" ] || fail_closed "MISSING: $workflow does not exist (fail-closed)"
[ -f "$contributing" ] || fail_closed "MISSING: $contributing does not exist (fail-closed)"

# The package name from a manifest's [package] table only.
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

# The allowlist, read from the `known="..."` assignment: every
# whitespace-separated token between the opening and closing quote.
allowlist() {
    awk '
        /known="/ { capturing = 1; sub(/.*known="/, "") }
        capturing {
            line = $0
            done = 0
            if (index(line, "\"") > 0) { sub(/".*$/, "", line); done = 1 }
            gsub(/\\/, " ", line)
            n = split(line, tokens, /[[:space:]]+/)
            for (i = 1; i <= n; i++) if (tokens[i] != "") print tokens[i]
            if (done) exit
        }
    ' "$workflow"
}

# The closure, read from the step that compiles for the bare-metal
# target: every `-p <crate>` between the step name and `--target`.
closure_from_workflow() {
    awk '
        /name: instrument closure is no_std, standalone/ { in_step = 1; next }
        in_step && /--target thumbv7em-none-eabihf/ { exit }
        in_step && /^[[:space:]]*-p / { sub(/^[[:space:]]*-p /, ""); print }
    ' "$workflow"
}

# The same list from CONTRIBUTING.md's local battery: the `cargo check`
# command up to its `--target` line, however the `-p` flags are wrapped.
closure_from_contributing() {
    awk '
        /^cargo check --locked/ { in_command = 1 }
        in_command {
            line = $0
            while (match(line, /-p [A-Za-z0-9_-]+/)) {
                print substr(line, RSTART + 3, RLENGTH - 3)
                line = substr(line, RSTART + RLENGTH)
            }
            if ($0 ~ /--target thumbv7em-none-eabihf/) exit
        }
    ' "$contributing"
}

known="$(allowlist | sort -u)"
[ -n "$known" ] || fail_closed "EMPTY: no known= allowlist found in $workflow (fail-closed)"
closure="$(closure_from_workflow | sort -u)"
[ -n "$closure" ] || fail_closed "EMPTY: no bare-metal closure step found in $workflow (fail-closed)"
local_closure="$(closure_from_contributing | sort -u)"
[ -n "$local_closure" ] || fail_closed "EMPTY: no cargo check closure found in $contributing (fail-closed)"

in_list() {
    printf '%s\n' "$2" | grep -Fxq "$1"
}

# Every package the allowlist's own prefix filter would match.
for manifest in crates/*/Cargo.toml sets/*/Cargo.toml tools/*/Cargo.toml; do
    name="$(package_name "$manifest")"
    [ -n "$name" ] || continue
    case "$name" in
        indicate*|pilotage*|avionics*|navigate*|aviate*|instrument*)
            if ! in_list "$name" "$known"; then
                echo "MISSING FROM ALLOWLIST: $name ($manifest) matches the downstream-agnostic filter but is not in known=; CI would report a boundary leak" >&2
                status=1
            fi
            ;;
    esac
done

# Every no_std library crate is in the closure, and only those are.
expected_closure=""
for manifest in crates/*/Cargo.toml sets/*/Cargo.toml; do
    dir="$(dirname "$manifest")"
    name="$(package_name "$manifest")"
    [ -n "$name" ] || continue
    [ -f "$dir/src/lib.rs" ] || continue
    if grep -qE '^#!\[no_std\]' "$dir/src/lib.rs"; then
        expected_closure="$expected_closure$name"$'\n'
        if ! in_list "$name" "$closure"; then
            echo "MISSING FROM CLOSURE: $name ($dir) is no_std but the bare-metal step never compiles it" >&2
            status=1
        fi
    fi
done
while IFS= read -r name; do
    [ -n "$name" ] || continue
    if ! printf '%s' "$expected_closure" | grep -Fxq "$name"; then
        echo "NOT NO_STD: $name is in the bare-metal closure step but is not a no_std library crate under crates/ or sets/" >&2
        status=1
    fi
done <<< "$closure"

# CONTRIBUTING.md's copy names the same crates.
if [ "$closure" != "$local_closure" ]; then
    echo "DRIFT: the cargo check closure in $contributing differs from the CI step" >&2
    echo "  CI:           $(echo "$closure" | tr '\n' ' ')" >&2
    echo "  CONTRIBUTING: $(echo "$local_closure" | tr '\n' ' ')" >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    echo "check-ci-closure: FAILED" >&2
    exit 1
fi
echo "check-ci-closure: OK ($(echo "$known" | wc -l | tr -d ' ') allowlisted, $(echo "$closure" | wc -l | tr -d ' ') in the bare-metal closure)"
