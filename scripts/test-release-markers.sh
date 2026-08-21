#!/usr/bin/env bash
# Selftest for check-release-markers.sh.
#
# The check reads the newest release entry and compares its five values
# against the tree. Every case here drives one refusal and asserts on
# the exit status, because a check that reports a clean tree when it
# examined nothing is the failure worth catching.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

check="$root_dir/scripts/check-release-markers.sh"
changelog="CHANGELOG.md"
passed=0
failed=0

backup="$(mktemp)"
cp "$changelog" "$backup"
cleanup() {
    cp "$backup" "$changelog"
    rm -f "$backup"
}
trap cleanup EXIT

# `expect <want> <label>` runs the check against the CHANGELOG as it
# currently stands and compares its exit status to `want`.
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

expect 0 "the committed CHANGELOG agrees with the tree"

# Two entries under one version: the check reads the newest only, so the
# older twin's values are never compared against anything. This is the
# shape a merge produces when two branches each add the next release.
# A complete duplicate of the newest entry, table and all, so the only
# thing wrong with the file is that one version appears twice. A twin
# with no table would be refused for a missing row instead, and the case
# would pass with the duplicate check deleted.
python3 - "$changelog" <<'PLANT'
import sys, pathlib, re
p = pathlib.Path(sys.argv[1])
s = p.read_text()
headings = [m.start() for m in re.finditer(r"^## \[\d+\.\d+\.\d+\]", s, re.M)]
start = headings[0]
end = headings[1] if len(headings) > 1 else len(s)
p.write_text(s[:start] + s[start:end] + s[start:])
PLANT
expect 1 "a version declared twice is refused"
cp "$backup" "$changelog"

# A value that disagrees with the tree.
python3 - "$changelog" <<'DRIFT'
import sys, pathlib, re
p = pathlib.Path(sys.argv[1])
s = p.read_text()
p.write_text(re.sub(r"\| State ABI \| \d+ \|", "| State ABI | 99 |", s, count=1))
DRIFT
expect 1 "a value that disagrees with the tree is refused"
cp "$backup" "$changelog"

# No versioned entry at all.
python3 - "$changelog" <<'STRIP'
import sys, pathlib, re
p = pathlib.Path(sys.argv[1])
p.write_text(re.sub(r"^## \[\d+\.\d+\.\d+\]", "## [Unreleased]", p.read_text(), flags=re.M))
STRIP
expect 1 "a changelog with no versioned entry is refused"
cp "$backup" "$changelog"

expect 0 "the restored CHANGELOG agrees again"

if [ "$failed" -ne 0 ]; then
    echo "release-markers-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "release-markers-selftest: OK ($passed cases)"
