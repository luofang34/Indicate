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
work="$(mktemp -d)"
changelog="$work/CHANGELOG.md"
output_file="$work/output"
export INDICATE_CHANGELOG="$changelog"
passed=0
failed=0

# Every case works on a copy. The committed CHANGELOG is never written,
# so a case that aborts cannot leave the repository dirty and a missing
# restore cannot hide behind the next case's own restore.
pristine="$work/pristine"
cp CHANGELOG.md "$pristine"
cp "$pristine" "$changelog"
trap 'rm -rf "$work"' EXIT

# `accepts <label>` expects a clean run. `refuses <label> <phrase>`
# expects a refusal AND that the refusal names `phrase`: the check has
# several ways to refuse, so a case that only counted the exit status
# would pass on a refusal it did not ask for. That is the shape the
# duplicate-version case was written in first, and it passed with the
# guard it named deleted.
run_check() {
    "$check" >"$output_file" 2>&1
}

accepts() {
    if run_check; then
        echo "ok: $1"
        passed=$((passed + 1))
    else
        echo "REGRESSION: $1 — refused, saying: $(head -1 "$output_file")" >&2
        failed=$((failed + 1))
    fi
}

refuses() {
    if run_check; then
        echo "REGRESSION: $1 was accepted; the guard did not notice" >&2
        failed=$((failed + 1))
        return
    fi
    if ! grep -Fq "$2" "$output_file"; then
        echo "REGRESSION: $1 was refused, but not for '$2'" >&2
        echo "  it said: $(head -1 "$output_file")" >&2
        failed=$((failed + 1))
        return
    fi
    echo "ok: $1 refused"
    passed=$((passed + 1))
}

accepts "the committed CHANGELOG agrees with the tree"

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
refuses "a version declared twice" "more than once"
cp "$pristine" "$changelog"

# A value that disagrees with the tree.
python3 - "$changelog" <<'DRIFT'
import sys, pathlib, re
p = pathlib.Path(sys.argv[1])
s = p.read_text()
p.write_text(re.sub(r"\| State ABI \| \d+ \|", "| State ABI | 99 |", s, count=1))
DRIFT
refuses "a value that disagrees with the tree" "DRIFT:"
cp "$pristine" "$changelog"

# No versioned entry at all.
python3 - "$changelog" <<'STRIP'
import sys, pathlib, re
p = pathlib.Path(sys.argv[1])
p.write_text(re.sub(r"^## \[\d+\.\d+\.\d+\]", "## [Unreleased]", p.read_text(), flags=re.M))
STRIP
refuses "a changelog with no versioned entry" "no versioned release entry found"
cp "$pristine" "$changelog"

accepts "the restored CHANGELOG agrees again"

if [ "$failed" -ne 0 ]; then
    echo "release-markers-selftest: FAILED ($failed of $((passed + failed)) cases)" >&2
    exit 1
fi

echo "release-markers-selftest: OK ($passed cases)"
