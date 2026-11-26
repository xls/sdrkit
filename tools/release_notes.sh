#!/bin/sh
# Prints the interface part of the notes of a release in Markdown, from the registry (registry/*.reg) of the working
# tree: each stack family with its version and its number of stacks. When a version tag below the version exists,
# the notes say instead what changed since the highest such tag: the families whose version changed, the families
# that are new or gone, and the stacks and items that were added, changed or removed.
#
# Usage: sh tools/release_notes.sh [version]
#   version  the version of the release, such as 0.5.0. The default is the version of the crate sdrkit-host-ffi.
#
# The repository needs its tags (a clone with `fetch-depth: 0` in a workflow). The release workflow adds the list of
# the files to the notes.

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/registry.sh"

version=${1:-$(registry_crate_version "$root")}
before=$(registry_tag_before "$root" "$version")

# Prints `<family> <version> <stacks>` for each family of the registry folder $1, sorted by family. A stack is an
# item whose key has no dot.
families() {
    registry_flat "$1" | awk -F '\t' '
        $1 == "F" { v[$2] = $3 }
        $1 == "I" && index($3, ".") == 0 { s[$2]++ }
        END { for (f in v) print f "\t" v[f] "\t" (s[f] + 0) }' | sort
}

current=$(families "$root/registry")
count=$(printf '%s\n' "$current" | grep -c .)
stacks=$(printf '%s\n' "$current" | awk -F '\t' '{ n += $3 } END { print n + 0 }')

echo "## Interface"
echo
echo "sdrkit $version has $count stack families with $stacks stacks. A family has its own version: a higher minor"
echo "number adds to it, a higher major number breaks it. While sdrkit is 0.x, a new minor number of sdrkit may break"
echo "the interface; a new patch number only adds to it."
echo

if [ -z "$before" ]; then
    echo "| Family | Version | Stacks |"
    echo "|---|---|---|"
    printf '%s\n' "$current" | awk -F '\t' '{ print "| `" $1 "` | " $2 " | " $3 " |" }'
    exit 0
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/sdrkit-notes.XXXXXX")
trap 'rm -rf "$work"' EXIT
registry_at "$root" "$before" "$work/registry"
diff=$(registry_compare "$work/registry" "$root/registry")
class=$(printf '%s\n' "$diff" | registry_class)
case "$class" in
    none) echo "The interface is the same as in $before." ; exit 0 ;;
    grew) echo "Since $before the interface grew; nothing was removed or broken." ;;
    broke) echo "Since $before the interface changed in a way that can break programs built for it: see the families whose major number rose and the removed items." ;;
esac
echo
echo "| Family | $before | $version | Added | Changed | Removed |"
echo "|---|---|---|---|---|---|"
printf '%s\n' "$current" > "$work/families"
printf '%s\n' "$diff" | awk -F '\t' -v cur="$work/families" '
    BEGIN { while ((getline line < cur) > 0) { split(line, p, "\t"); now[p[1]] = p[2] } }
    $1 == "family-version" { old[$2] = $3; seen[$2] = 1 }
    $1 == "family-added" { old[$2] = "new"; seen[$2] = 1 }
    $1 == "family-removed" { old[$2] = $3; now[$2] = "removed"; seen[$2] = 1 }
    $1 == "added" { a[$2]++; seen[$2] = 1 }
    $1 == "changed" { c[$2]++; seen[$2] = 1 }
    $1 == "removed" { r[$2]++; seen[$2] = 1 }
    END {
        for (f in seen) {
            o = (f in old) ? old[f] : now[f]
            print "| `" f "` | " o " | " now[f] " | " (a[f] + 0) " | " (c[f] + 0) " | " (r[f] + 0) " |"
        }
    }' | sort

# The items of each family, at most 40 lines a family.
echo
echo "### Details"
printf '%s\n' "$diff" | awk -F '\t' '
    $1 == "added" || $1 == "changed" || $1 == "removed" {
        f = $2
        if (!(f in n)) { order[++families] = f }
        n[f]++
        if (n[f] <= 40) {
            what = (index($3, ".") == 0) ? "stack" : "item"
            line = "- " $1 " " what " `" $3 "`"
            if ($1 == "changed") line = line ": `" $4 "` to `" $5 "`"
            text[f] = text[f] line "\n"
        }
    }
    END {
        for (i = 1; i <= families; i++) {
            f = order[i]
            printf "\n#### %s\n\n%s", f, text[f]
            if (n[f] > 40) printf "- and %d more\n", n[f] - 40
        }
    }'
