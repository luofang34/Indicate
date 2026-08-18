#!/usr/bin/env bash
# Advisory: says whether this branch's recorded baselines would survive a
# squash merge (#69).
#
# A recorded run names the commit it was produced against, and the gate
# refuses a baseline it cannot reach from HEAD — a fresh clone cannot
# fetch one. A squash merge replaces exactly that commit. So a branch
# whose baseline is not already on the base branch must merge with a
# merge commit, or main goes red the moment it lands.
#
# This cannot be a hard gate, and the reason is worth stating: on a pull
# request build the checkout is the synthetic merge ref, where the lane
# commit IS an ancestor, so the hard gate passes before the merge and
# fails only after it. That asymmetry is why every episode of this was
# found late. The point of an advisory step is to put the requirement on
# the pull request page at the moment someone picks a merge button.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

graph="${INDICATE_EVIDENCE_GRAPH:-docs/instruments/evidence-graph.evg}"
base="${INDICATE_MERGE_BASE:-origin/main}"

[ -f "$graph" ] || {
    echo "check-baseline-survives-merge: no graph at $graph" >&2
    exit 0
}

if ! git rev-parse --verify --quiet "$base" >/dev/null; then
    echo "check-baseline-survives-merge: $base is not fetched; skipping"
    exit 0
fi

branch_local=0
for digest in $(sed -n 's/^attr config-digest \([0-9a-f]\{40\}\)$/\1/p' "$graph" | sort -u); do
    if ! git cat-file -e "$digest^{commit}" 2>/dev/null; then
        echo "UNKNOWN: baseline $digest is not an object in this clone" >&2
        continue
    fi
    if git merge-base --is-ancestor "$digest" "$base" 2>/dev/null; then
        echo "ok: baseline $digest is already on $base"
    else
        echo "BRANCH-LOCAL: baseline $digest exists only on this branch" >&2
        branch_local=$((branch_local + 1))
    fi
done

if [ "$branch_local" -ne 0 ]; then
    echo "check-baseline-survives-merge: $branch_local baseline(s) would be orphaned by a squash." >&2
    echo "Merge this pull request with a merge commit. See CONTRIBUTING.md." >&2
    exit 0
fi

echo "check-baseline-survives-merge: every baseline is already on $base"
