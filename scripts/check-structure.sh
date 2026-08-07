#!/usr/bin/env bash
# Enforces the structural limits from ADR-0015 that are not expressible as
# rustc/clippy lints:
#   - no mod.rs files
#   - no utils.rs / helpers.rs / common.rs files
#   - no tracked .rs file over 500 lines (excluding target/ and any
#     /generated/ path)
#   - no lib.rs over 100 lines
#   - no function body over 80 lines
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
        -type d \( -name target -o -name generated \) -prune -o \
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
    done < <(find . -name Cargo.toml -not -path './target/*' -not -path './.git/*' -mindepth 2)
}

check_forbidden_filenames
check_file_length
check_function_length
check_safety_palette_aliases
check_safety_constant_count
check_crate_naming

if [ "$status" -ne 0 ]; then
    echo "check-structure: FAILED" >&2
    exit 1
fi

echo "check-structure: OK"
