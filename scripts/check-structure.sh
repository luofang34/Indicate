#!/usr/bin/env bash
# Enforces the structural limits from ADR-0015 that are not expressible as
# rustc/clippy lints:
#   - no mod.rs files
#   - no utils.rs / helpers.rs / common.rs files
#   - no tracked .rs file over 500 lines (excluding target/ and any
#     /generated/ path)
#   - no lib.rs over 100 lines
#   - no function body over 80 lines
#   - no retired Apple-backend term (InstrumentSceneKit,
#     IndicateAppleDisplay, "Swift SceneKit backend") in an active
#     contract document under docs/instruments/
#
# The function-length check is an AWK brace-depth heuristic: it counts lines
# between a `fn` header and the point where brace depth returns to the level
# it had when the function opened. It does not parse Rust; it can be
# confused by braces inside string literals, char literals, or comments.
# Treat violations it reports as a strong signal, not ground truth.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root_dir"

status=0
function_baseline="scripts/structure-function-baseline.tsv"

is_excluded_path() {
    case "$1" in
        */target/*|target/*) return 0 ;;
        */generated/*) return 0 ;;
        *) return 1 ;;
    esac
}

collect_rs_files() {
    find . \
        -type d \( -name target -o -name generated -o -name .worktrees \
        -o -name .claude \) -prune -o \
        -type f -name '*.rs' -print
}

check_forbidden_filenames() {
    local file base
    while IFS= read -r file; do
        is_excluded_path "$file" && continue
        base="$(basename "$file")"
        case "$base" in
            mod.rs)
                echo "FORBIDDEN: $file (no mod.rs; use foo.rs + foo/)" >&2
                status=1
                ;;
            utils.rs|helpers.rs|common.rs)
                echo "FORBIDDEN: $file (no generic utils/helpers/common modules)" >&2
                status=1
                ;;
        esac
    done < <(collect_rs_files)
}

check_file_length() {
    local file base lines limit
    while IFS= read -r file; do
        is_excluded_path "$file" && continue
        base="$(basename "$file")"
        lines="$(wc -l < "$file" | tr -d ' ')"
        limit=500
        if [ "$base" = "lib.rs" ]; then
            limit=100
        fi
        if [ "$lines" -gt "$limit" ]; then
            echo "FORBIDDEN: $file has $lines lines (limit $limit)" >&2
            status=1
        fi
    done < <(collect_rs_files)
}

check_function_length() {
    local file
    while IFS= read -r file; do
        is_excluded_path "$file" && continue
        awk -v fname="$file" -v baseline="$function_baseline" '
            function report(name, len, start, key, limit) {
                key = fname SUBSEP name
                seen[key] = 1
                if (key in allowed) {
                    limit = allowed[key]
                    if (len != limit) {
                        printf "FORBIDDEN: %s:%d function %s has %d lines; baseline requires exactly %d\n", fname, start, name, len, limit > "/dev/stderr"
                        bad = 1
                    }
                } else if (len > 80) {
                    printf "FORBIDDEN: %s:%d function body has %d lines (limit 80)\n", fname, start, len > "/dev/stderr"
                    bad = 1
                }
            }
            BEGIN {
                while ((getline entry < baseline) > 0) {
                    if (entry ~ /^[ \t]*#/ || entry ~ /^[ \t]*$/) {
                        continue
                    }
                    split(entry, fields, "\t")
                    key = fields[1] SUBSEP fields[2]
                    allowed[key] = fields[3] + 0
                    allowed_file[key] = fields[1]
                }
                close(baseline)
                depth = 0
                in_fn = 0
                fn_depth = 0
                fn_start = 0
                body_lines = 0
                bad = 0
            }
            {
                line = $0
                if (!in_fn && line ~ /(^|[^[:alnum:]_])fn[ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t]*(<[^>]*>)?[ \t]*\(/) {
                    match(line, /fn[ \t]+[A-Za-z_][A-Za-z0-9_]*/)
                    fn_name = substr(line, RSTART, RLENGTH)
                    sub(/^fn[ \t]+/, "", fn_name)
                    in_fn = 1
                    fn_depth = depth
                    fn_start = NR
                    body_lines = 0
                    has_opened = 0
                }
                if (in_fn) {
                    body_lines++
                }
                n_open = gsub(/\{/, "{", line)
                n_close = gsub(/\}/, "}", line)
                depth += n_open
                if (in_fn && n_open > 0) {
                    has_opened = 1
                }
                depth -= n_close
                if (in_fn && has_opened && depth <= fn_depth) {
                    report(fn_name, body_lines, fn_start)
                    in_fn = 0
                }
            }
            END {
                for (key in allowed) {
                    if (allowed_file[key] == fname && !(key in seen)) {
                        split(key, parts, SUBSEP)
                        printf "FORBIDDEN: baseline function %s in %s was not found\n", parts[2], fname > "/dev/stderr"
                        bad = 1
                    }
                }
                exit bad
            }
        ' "$file" || status=1
    done < <(collect_rs_files)
}

# The palette names RED, AMBER, YELLOW, and BAND_YELLOW alias the
# never-skinnable safety set (ADR-0029). Outside the symbology crate,
# safety-semantic paints must reference `safety::` directly so a future
# palette-to-theme sweep cannot silently make failure, caution, or
# reference colors skinnable.
# Text-level ratchet, not a proof: it catches direct `palette::RED`
# uses and `use ...::palette::RED` imports, but a module alias
# (`use ... as p; p::RED`) slips through, like the AWK heuristics above.
check_safety_palette_aliases() {
    local file
    while IFS= read -r file; do
        is_excluded_path "$file" && continue
        case "$file" in
            ./crates/indicate-instrument-symbology/*) continue ;;
        esac
        if grep -Eq 'palette::(RED|AMBER|YELLOW|BAND_YELLOW)\b' "$file"; then
            echo "FORBIDDEN: $file references a safety palette alias; use the safety:: constants outside indicate-instrument-symbology" >&2
            status=1
        fi
    done < <(collect_rs_files)
}

# The theme imitation screen (SAFETY_HUES) and its exemption list are
# hand-maintained; a new safety constant must visit them deliberately.
check_safety_constant_count() {
    local expected=5 actual
    actual=$(grep -c '^pub const' ./crates/indicate-instrument-symbology/src/safety.rs)
    if [ "$actual" -ne "$expected" ]; then
        echo "FORBIDDEN: safety.rs public constant count moved ($actual, pinned $expected); add the new constant to theme.rs SAFETY_HUES or its documented exemption, then update this pin" >&2
        status=1
    fi
}

# The family is named after this repository, not after a consumer of it.
# This checks NAMES ONLY — crate directories and package names. Values
# that are hashed or pinned downstream keep whatever string they were
# minted with, because rewriting one moves a digest for no change in
# what is painted; `crates/README.md` lists which those are.
#
# Every Cargo.toml in the tree is read, not a fixed set of roots, so a
# new tier of crates is covered the day it appears rather than the day
# someone remembers to add it here.
package_name() {
    # The name from the [package] table only: a [[bin]] or [lib] name
    # above it would otherwise answer for the package. Accepts either
    # quote style and any spacing around the `=`.
    awk '
        /^[[:space:]]*\[/ { in_package = ($0 ~ /^[[:space:]]*\[package\][[:space:]]*$/) }
        in_package && /^[[:space:]]*name[[:space:]]*=/ {
            line = $0
            sub(/^[[:space:]]*name[[:space:]]*=[[:space:]]*/, "", line)
            sub(/^["'"'"']/, "", line)
            sub(/["'"'"'].*$/, "", line)
            print line
            exit
        }
    ' "$1"
}

check_crate_naming() {
    local manifest dir name
    while IFS= read -r manifest; do
        dir="$(dirname "$manifest")"
        case "$(basename "$dir")" in
            pilotage-*)
                echo "FORBIDDEN: $dir is named after a downstream consumer; crate directories are indicate-*" >&2
                status=1
                ;;
        esac
        # An unnamed package is a malformed manifest, not a pass.
        name="$(package_name "$manifest" || true)"
        if [ -z "$name" ]; then
            echo "FORBIDDEN: $manifest declares no [package] name" >&2
            status=1
            continue
        fi
        case "$name" in
            pilotage-*)
                echo "FORBIDDEN: $manifest declares package $name; crates are indicate-*" >&2
                status=1
                ;;
        esac
    done < <(find . -name Cargo.toml -not -path './target/*' -not -path './.git/*' \
        -not -path './.worktrees/*' -not -path './.claude/*' -mindepth 2)
}

# The tier law from #13. The tiers are only real if the tree enforces
# them, so the dependency direction gets the same treatment as the
# consumer boundary: stated once here, failed in CI.
#
#   kernel        the no_std closure a panel may draw against; depends
#                 on the kernel only.
#   verification  raster, conformance, registry, evidence. Consumes
#                 sets; is never a normal dependency of one.
#   sets          panel providers under sets/, one crate per set. Normal
#                 dependencies are kernel-only. The registry is allowed
#                 as a DEV dependency so a set can pin its own scene
#                 digest without a shell — a test-graph edge is not a
#                 shipping one.
#   tools         unconstrained; they are shells, not library tiers.
KERNEL_CRATES="indicate-frames indicate-alerts indicate-sha256 \
indicate-instrument-state indicate-instrument-scene indicate-instrument-glyphs \
indicate-instrument-symbology indicate-instrument-descriptor \
indicate-instrument-feeder"
VERIFICATION_CRATES="indicate-instrument-raster indicate-instrument-conformance \
indicate-instrument-registry indicate-evidence"

# Cargo answers, rather than a regex over TOML. A manifest may write a
# dependency six ways this file would otherwise have to anticipate —
# `dep.workspace = true`, a quoted key, a `[dependencies.dep]` section,
# a `[target.'cfg(…)'.dependencies]` table, a `package = ` rename, or a
# table header trailed by a comment — and a scanner that misses one of
# them prints OK over the edge it was written to hold. `cargo metadata`
# reports the RESOLVED package name, the dependency kind, and the target
# it applies to, so a rename cannot hide a crate behind another name.
check_tier_law() {
    local metadata
    metadata="$(mktemp)"
    if ! cargo metadata --format-version 1 --no-deps >"$metadata" 2>/dev/null; then
        echo "FORBIDDEN: cargo metadata failed; the tier law cannot be checked (fail-closed)" >&2
        status=1
        rm -f "$metadata"
        return
    fi
    python3 - "$metadata" "$KERNEL_CRATES" "$VERIFICATION_CRATES" <<'PY' || status=1
import json, os, sys

metadata, kernel, verification = sys.argv[1], sys.argv[2].split(), sys.argv[3].split()
packages = json.load(open(metadata))["packages"]
root = os.getcwd()
known = {p["name"] for p in packages}
bad = False

def report(message):
    global bad
    print("FORBIDDEN: " + message, file=sys.stderr)
    bad = True

# A tier list naming a crate that no longer exists is drift of the same
# kind the crate map guards against, one file over.
for name in kernel + verification:
    if name not in known:
        report(f"the tier lists name {name}, which is not a crate in this workspace")

for package in sorted(packages, key=lambda p: p["name"]):
    name = package["name"]
    where = os.path.relpath(package["manifest_path"], root)
    if where.startswith("tools/"):
        continue
    if where.startswith("sets/"):
        tier, allowed = "set", kernel
    elif name in kernel:
        tier, allowed = "kernel", kernel
    elif name in verification:
        tier, allowed = "verification", kernel + verification
    else:
        report(f"{name} is in no tier; add it to the kernel or verification list")
        continue
    for dependency in package["dependencies"]:
        # Only what the crate ships: dev- and build-dependencies do not
        # constrain a tier, which is what lets a set pin its own digest.
        if dependency.get("kind") is not None:
            continue
        depended = dependency["name"]
        if depended not in known or depended in allowed:
            continue
        target = dependency.get("target")
        qualifier = f" (under target {target})" if target else ""
        report(f"{tier} crate {name} depends on {depended}{qualifier}, which its tier may not reach")

sys.exit(1 if bad else 0)
PY
    rm -f "$metadata"
}

# The crate map is prose about the tree, so it drifts unless checked:
# every workspace library crate gets a row, and every row names a crate.
check_crate_map() {
    local map="crates/README.md" name manifest
    [ -f "$map" ] || { echo "FORBIDDEN: $map is missing" >&2; status=1; return; }
    while IFS= read -r manifest; do
        case "$(dirname "$manifest")" in ./tools/*) continue ;; esac
        name="$(package_name "$manifest" || true)"
        if ! grep -q "\`$name\`" "$map"; then
            echo "FORBIDDEN: $map has no row for $name" >&2
            status=1
        fi
    done < <(find . -name Cargo.toml -not -path './target/*' -not -path './.git/*' \
        -not -path './.worktrees/*' -not -path './.claude/*' -mindepth 2)
    while IFS= read -r name; do
        if [ ! -d "crates/$name" ] && [ ! -d "sets/$name" ]; then
            echo "FORBIDDEN: $map names $name, which is not a crate in this workspace" >&2
            status=1
        fi
    done < <(grep -oE '`indicate-[a-z0-9-]+`' "$map" | tr -d '`' | sort -u)
}

# The Apple backend is a Core Graphics consumer of the scene IR, not a
# SceneKit scene graph, and its pieces are named after this repository.
# These strings name the superseded boundary; an active contract
# document under docs/instruments/ must not re-introduce one. Fixed-string
# matching, so a term inside a longer word still counts — that is the
# intent for retired names.
check_backend_boundary_terms() {
    local file term
    while IFS= read -r file; do
        for term in "InstrumentSceneKit" "IndicateAppleDisplay" "Swift SceneKit backend"; do
            if grep -qF "$term" "$file"; then
                echo "FORBIDDEN: $file mentions '$term'; the Apple backend is the Apple Core Graphics backend" >&2
                status=1
            fi
        done
    done < <(find docs/instruments -type f -name '*.md')
}

check_forbidden_filenames
check_file_length
check_function_length
check_safety_palette_aliases
check_safety_constant_count
check_crate_naming
check_tier_law
check_crate_map
check_backend_boundary_terms

# The tier law is the one check here that a manifest can spell its way
# around, so it carries a selftest proving it refuses each spelling. The
# child guard stops the recursion, since the selftest runs this script.
if [ "${INDICATE_STRUCTURE_SELFTEST_CHILD:-0}" != "1" ]; then
    if ! INDICATE_STRUCTURE_SELFTEST_CHILD=1 bash "$root_dir/scripts/test-structure.sh"; then
        status=1
    fi
fi

if [ "$status" -ne 0 ]; then
    echo "check-structure: FAILED" >&2
    exit 1
fi

echo "check-structure: OK"
