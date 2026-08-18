#!/usr/bin/env bash
# Advisory: says whether this branch's recorded baselines would survive a
# merge that rewrites its commits.
#
# A recorded run names the commit it was produced against, and the gate
# refuses a baseline it cannot reach from HEAD — a fresh clone cannot
# fetch one. A squash merge replaces exactly that commit, and so does a
# rebase merge. So a branch whose baseline is not already on the base
# branch must merge with a merge commit, or main goes red the moment it
# lands.
#
# This cannot be a hard gate, and the reason is worth stating: on a pull
# request build the checkout is the synthetic merge ref, where the lane
# commit IS an ancestor, so a hard gate passes before the merge and
# fails only after it. That asymmetry is why every episode of this was
# found late. The point of an advisory step is to put the requirement on
# the pull request page at the moment someone picks a merge button —
# which requires this script to exit non-zero when it has something to
# say. The workflow step carries `continue-on-error`, so a non-zero exit
# marks the step without failing the build.
#
# Every path other than "checked, and every baseline is on the base"
# exits non-zero. A check that cannot run and says nothing is
# indistinguishable from a check that ran and found nothing, and the
# second is the one a reviewer will assume.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

graph="${INDICATE_EVIDENCE_GRAPH:-docs/instruments/evidence-graph.evg}"
base="${INDICATE_MERGE_BASE:-origin/main}"

if [ ! -r "$graph" ]; then
    echo "UNCHECKED: no readable evidence graph at $graph" >&2
    exit 1
fi

if ! git rev-parse --verify --quiet "$base" >/dev/null; then
    echo "UNCHECKED: $base is not fetched, so no baseline can be placed" >&2
    exit 1
fi

# Every form the gate accepts must be parsed here, or the guardrail is
# narrower than the thing it guards and skips the difference in silence.
# The gate hands the attribute to git after a trim, and git resolves
# abbreviated and upper-case object ids, so this does too.
digests="$(sed -n 's/^attr config-digest *\([0-9a-fA-F]\{7,40\}\) *$/\1/p' "$graph" | sort -u)" || {
    echo "UNCHECKED: could not read baselines from $graph" >&2
    exit 1
}

# Distinct declared values, so this compares like with like: a gap
# between the two counts means the graph declares a baseline in a form
# this script did not parse, which is the shape that fails open.
declared="$(sed -n 's/^attr config-digest *//p' "$graph" | sed 's/ *$//' | sort -u | grep -c .)"
if [ -z "$digests" ]; then
    echo "UNCHECKED: $graph declares no baseline this script can parse" >&2
    exit 1
fi

branch_local=0
unknown=0
checked=0
for digest in $digests; do
    checked=$((checked + 1))
    if ! git cat-file -e "$digest^{commit}" 2>/dev/null; then
        echo "UNKNOWN: baseline $digest is not an object in this clone" >&2
        unknown=$((unknown + 1))
        continue
    fi
    if git merge-base --is-ancestor "$digest" "$base" 2>/dev/null; then
        echo "ok: baseline $digest is already on $base"
    else
        echo "BRANCH-LOCAL: baseline $digest exists only on this branch" >&2
        branch_local=$((branch_local + 1))
    fi
done

if [ "$unknown" -ne 0 ]; then
    echo "check-baseline-survives-merge: $unknown baseline(s) could not be placed." >&2
    exit 1
fi

if [ "$branch_local" -ne 0 ]; then
    # A workflow annotation rather than only a log line: the stated
    # purpose is to put the requirement on the pull request page at the
    # moment someone picks a merge button, and a message inside a
    # collapsed job log does not reach that page.
    echo "::warning file=$graph::$branch_local baseline(s) would be orphaned by a squash or" \
        "rebase merge; merge this pull request with a merge commit (see CONTRIBUTING.md)"
    if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
        echo "**$branch_local baseline(s) would be orphaned by a squash or rebase merge.**" \
            "Merge this pull request with a merge commit. See CONTRIBUTING.md." \
            >> "$GITHUB_STEP_SUMMARY"
    fi
    echo "check-baseline-survives-merge: $branch_local baseline(s) would be orphaned by a" >&2
    echo "squash or rebase merge. Merge this pull request with a merge commit." >&2
    echo "See CONTRIBUTING.md." >&2
    exit 1
fi

# Says how many it placed, not that a clean result was reached: a count
# a reader can compare against the graph is what distinguishes a check
# that ran from a check that matched nothing.
if [ "$checked" -ne "$declared" ]; then
    echo "UNCHECKED: $graph declares $declared distinct baseline(s) but only $checked parsed" >&2
    exit 1
fi

echo "check-baseline-survives-merge: all $checked declared baseline(s) are on $base"
