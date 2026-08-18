#!/usr/bin/env bash
# Selftest for the tier law in check-structure.sh.
#
# A guard is only worth its line count if it fails on the things it
# claims to catch, so this plants a crate in `sets/` that reaches the
# verification tier, once per way a manifest can spell a dependency, and
# requires check-structure.sh to refuse each one. An earlier text-scanner
# implementation passed six of these while cargo built them happily; the
# case list is that set plus the plain form.
#
# Each case mutates the worktree and restores it, so the trap is
# load-bearing: an aborted run must not leave a planted crate behind.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

probe_dir="sets/indicate-tier-probe"
probe_doc="docs/instruments/zz-structure-probe.md"
lock_backup="$(mktemp)"
cp Cargo.lock "$lock_backup"
# Every tracked file this script edits is restored by the trap, not by
# the case that edited it. A case that cleans up after itself leaves the
# repository dirty the moment it aborts, and `CONTRIBUTING.md` carrying
# an invented citation makes `check-structure.sh` fail against a
# document the contributor never wrote.
citation_backup="$(mktemp)"
cp CONTRIBUTING.md "$citation_backup"

cleanup() {
    rm -rf "$probe_dir"
    rm -f "$probe_doc"
    cp "$lock_backup" Cargo.lock
    rm -f "$lock_backup"
    cp "$citation_backup" CONTRIBUTING.md
    rm -f "$citation_backup"
}
trap cleanup EXIT

passed=0
failed=0

# $1 = case name, $2 = the dependency stanza under test.
expect_refusal() {
    local name="$1" stanza="$2"
    mkdir -p "$probe_dir/src"
    printf '//! Tier-law probe.\n' > "$probe_dir/src/lib.rs"
    cat > "$probe_dir/Cargo.toml" <<EOF
[package]
name = "indicate-tier-probe"
version = "0.1.0"
edition.workspace = true

$stanza

[lints]
workspace = true
EOF
    if INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh >/dev/null 2>&1; then
        echo "REGRESSION: $name was accepted; a set reached the verification tier unseen" >&2
        failed=$((failed + 1))
    else
        echo "ok: $name refused"
        passed=$((passed + 1))
    fi
    rm -rf "$probe_dir"
    cp "$lock_backup" Cargo.lock
}

expect_refusal "plain inline table" \
'[dependencies]
indicate-instrument-raster = { workspace = true }'

expect_refusal 'dotted key (the repo own idiom)' \
'[dependencies]
indicate-instrument-raster.workspace = true'

expect_refusal "quoted key" \
'[dependencies]
"indicate-instrument-raster" = { workspace = true }'

expect_refusal "section-style dependency" \
'[dependencies.indicate-instrument-raster]
workspace = true'

expect_refusal "target-conditional dependency" \
"[target.'cfg(unix)'.dependencies]
indicate-instrument-raster = { workspace = true }"

expect_refusal "renamed via package =" \
'[dependencies]
judge = { package = "indicate-instrument-raster", path = "../../crates/indicate-instrument-raster" }'

expect_refusal "table header trailed by a comment" \
'[dependencies] # the shipping dependencies
indicate-instrument-raster = { workspace = true }'

# The guard must also accept what the law permits, or it would pass this
# selftest by refusing everything.
expect_acceptance() {
    local name="$1" stanza="$2"
    mkdir -p "$probe_dir/src"
    printf '//! Tier-law probe.\n' > "$probe_dir/src/lib.rs"
    cat > "$probe_dir/Cargo.toml" <<EOF
[package]
name = "indicate-tier-probe"
version = "0.1.0"
edition.workspace = true

$stanza

[lints]
workspace = true
EOF
    # The crate map cannot know a probe crate, so that finding is
    # expected here and filtered; a tier finding is not.
    if INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh 2>&1 | grep -q "may not reach"; then
        echo "REGRESSION: $name was refused; the law forbids something it permits" >&2
        failed=$((failed + 1))
    else
        echo "ok: $name permitted"
        passed=$((passed + 1))
    fi
    rm -rf "$probe_dir"
    cp "$lock_backup" Cargo.lock
}

expect_acceptance "kernel dependency" \
'[dependencies]
indicate-instrument-scene = { workspace = true }'

expect_acceptance "registry as a DEV dependency" \
'[dev-dependencies]
indicate-instrument-registry = { workspace = true }'

# The retired Apple-backend terms get the same treatment as the tier law:
# plant each one in a contract document and require the guard to refuse.
expect_term_refusal() {
    local name="$1" term="$2"
    printf 'A probe sentence naming %s.\n' "$term" > "$probe_doc"
    if INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh >/dev/null 2>&1; then
        echo "REGRESSION: $name was accepted; a retired Apple-backend term passed unseen" >&2
        failed=$((failed + 1))
    else
        echo "ok: $name refused"
        passed=$((passed + 1))
    fi
    rm -f "$probe_doc"
}

expect_term_refusal "InstrumentSceneKit" "InstrumentSceneKit"
expect_term_refusal "IndicateAppleDisplay" "IndicateAppleDisplay"
expect_term_refusal "Swift SceneKit backend" "Swift SceneKit backend"

# A citation the clone cannot resolve must be refused wherever it sits
# on the line. The first version of this check read only the last
# citation per line, so a document that named a missing file before a
# present one passed — which is the spelling a contributor would most
# easily write by accident.
for line in 'It cites `GHOST.md` and then `AGENTS.md`.' \
    'It cites `AGENTS.md` and then `GHOST.md`.'; do
    printf '%s\n' "$line" >> CONTRIBUTING.md
    if INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh >/dev/null 2>&1; then
        echo "REGRESSION: a citation of a missing document was accepted: $line" >&2
        failed=$((failed + 1))
    else
        echo "ok: a citation of a missing document refused"
        passed=$((passed + 1))
    fi
    cp "$citation_backup" CONTRIBUTING.md
done

if [ "$failed" -ne 0 ]; then
    echo "structure-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "structure-selftest: OK ($passed cases)"
