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
worktree_probe_dir=".worktrees/zz-structure-probe"
lock_backup="$(mktemp)"
cp Cargo.lock "$lock_backup"

cleanup() {
    rm -rf "$probe_dir"
    rm -rf "$worktree_probe_dir"
    rm -f "$probe_doc"
    cp "$lock_backup" Cargo.lock
    rm -f "$lock_backup"
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

# The display-reason completeness check: a variant outside `ALL` takes a
# code no Rust test can see, and a duplicated slot then collides with an
# existing reason's identity. Probe it by adding exactly that variant.
condition_file="crates/indicate-alerts/src/condition.rs"
condition_backup="$(mktemp)"
cp "$condition_file" "$condition_backup"
python3 - "$condition_file" <<'PROBE'
import sys
p = sys.argv[1]
s = open(p).read()
s = s.replace("    RetainedImage,\n}", "    RetainedImage,\n    /// Probe.\n    ProbeReason,\n}", 1)
s = s.replace("            Self::RetainedImage => 4,", "            Self::RetainedImage => 4,\n            Self::ProbeReason => 2,", 1)
open(p, "w").write(s)
PROBE
# Matched on the message, not on the exit status: the gate refuses for
# many reasons, so a case that accepted any refusal would pass on a file
# that merely grew past its line limit — and then report that a check it
# never ran is working.
# Captured, not piped: this script runs under `pipefail`, so a pipeline
# would carry the gate's own refusal status and say nothing about which
# refusal it was.
probe_status=0
probe_output="$(INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh 2>&1)" \
    || probe_status=$?
# Both halves, because either alone passes a broken gate: a message with
# a zero exit leaves CI green on a violation the gate printed out loud,
# and a non-zero exit without the message is any other refusal.
if [ "$probe_status" -ne 0 ] \
    && printf '%s' "$probe_output" | grep -q "takes a code no test can see"; then
    echo "ok: a reason outside DisplayFault::ALL refused"
    passed=$((passed + 1))
else
    echo "REGRESSION: a reason outside ALL was accepted; it can take a code no test sees" >&2
    failed=$((failed + 1))
fi
cp "$condition_backup" "$condition_file"
rm -f "$condition_backup"

# A linked worktree is a checkout, not content, and the gate walks the
# tree with `find`, which reads no ignore file. Without the prune the
# walks reach a worktree's own manifests and sources: the workspace-only
# root manifest has no `[package]` name, so the gate reports findings
# against a checkout of itself. Plant both a manifest and a source file,
# because the manifest walks and the source walk are pruned separately.
mkdir -p "$worktree_probe_dir/probe/src"
cat > "$worktree_probe_dir/probe/Cargo.toml" <<'EOF'
[workspace]
members = []
EOF
printf '//! Structure probe inside a linked worktree.\n' \
    > "$worktree_probe_dir/probe/src/mod.rs"
if INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash scripts/check-structure.sh >/dev/null 2>&1; then
    echo "ok: content under .worktrees is not walked"
    passed=$((passed + 1))
else
    echo "REGRESSION: the gate walked into a linked worktree and reported on a checkout" >&2
    failed=$((failed + 1))
fi
rm -rf "$worktree_probe_dir"

if [ "$failed" -ne 0 ]; then
    echo "structure-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "structure-selftest: OK ($passed cases)"
