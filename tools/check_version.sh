#!/bin/sh
# Checks the version of sdrkit against the interface registry (registry/*.reg). While the version is 0.x, a breaking
# change of the interface raises the minor number (0.5.0 to 0.6.0) and an addition raises the patch number (0.5.0 to
# 0.5.1). From 1.0 on, a breaking change raises the major number and an addition the minor or patch number.
#
# Usage: sh tools/check_version.sh
#        sh tools/check_version.sh --compare <old registry folder> <old version> <new registry folder> <new version>
#
# The first form compares registry/ of the working tree, at the version of the crate sdrkit-host-ffi, with registry/
# at a version tag v<major>.<minor>.<patch>:
#   - if the tag of the current version exists, the registry must be the same as at that tag;
#   - else the tag is the highest version below the current one, and the rule above applies.
# It passes when there is no such tag. The repository needs its tags (a clone with `fetch-depth: 0` in a workflow).
# The second form applies the rule to two folders and two versions, with no git.
#
# The script prints the class of the difference (none, grew or broke), the families that changed and the verdict. The
# exit status is 0 for a pass and 1 for a fail.

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/registry.sh"

# Prints the differences of the folders $1 and $2, checks the versions $3 (old) and $4 (new), and exits.
verdict() {
    diff=$(registry_compare "$1" "$2")
    class=$(printf '%s\n' "$diff" | registry_class)
    echo "registry: $class from $3 to $4"
    printf '%s\n' "$diff" | awk -F '\t' '
        $1 == "family-version" { print "  " $2 " " $3 " -> " $4 }
        $1 == "family-added" { print "  " $2 " new at " $3 }
        $1 == "family-removed" { print "  " $2 " removed (was " $3 ")" }'
    old=$3 new=$4
    omaj=${old%%.*} rest=${old#*.} omin=${rest%%.*} opat=${rest#*.}
    nmaj=${new%%.*} rest=${new#*.} nmin=${rest%%.*} npat=${rest#*.}
    case "$class" in
        none)
            echo "pass: the interface did not change"
            exit 0 ;;
        broke)
            if [ "$omaj" = 0 ]; then
                if [ "$nmaj" -gt 0 ] || { [ "$nmaj" = 0 ] && [ "$nmin" -gt "$omin" ]; }; then
                    echo "pass: the interface broke and the major or minor number rose"
                    exit 0
                fi
                echo "fail: the interface broke since $old; raise the minor number (0.$((omin + 1)).0)"
            else
                if [ "$nmaj" -gt "$omaj" ]; then
                    echo "pass: the interface broke and the major number rose"
                    exit 0
                fi
                echo "fail: the interface broke since $old; raise the major number ($((omaj + 1)).0.0)"
            fi
            exit 1 ;;
        grew)
            if [ "$(registry_version_lower "$old" "$new")" = 1 ]; then
                echo "pass: the interface grew and the version rose"
                exit 0
            fi
            echo "fail: the interface grew since $old; raise the patch number ($omaj.$omin.$((opat + 1)))"
            exit 1 ;;
    esac
}

if [ "${1:-}" = --compare ]; then
    if [ $# != 5 ]; then
        echo "usage: sh tools/check_version.sh --compare <old folder> <old version> <new folder> <new version>" >&2
        exit 2
    fi
    verdict "$2" "$4" "$3" "$5"
fi

version=$(registry_crate_version "$root")
if [ -z "$version" ]; then
    echo "no version in crates/hosts/sdrkit-host-ffi/Cargo.toml" >&2
    exit 2
fi
if git -C "$root" rev-parse -q --verify "refs/tags/v$version" > /dev/null; then
    tag=v$version
else
    tag=$(registry_tag_before "$root" "$version")
fi
if [ -z "$tag" ]; then
    echo "pass: no version tag below or at $version"
    exit 0
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/sdrkit-version.XXXXXX")
trap 'rm -rf "$work"' EXIT
registry_at "$root" "$tag" "$work/registry"
echo "comparing registry/ with $tag"
if [ "$tag" = "v$version" ]; then
    diff=$(registry_compare "$work/registry" "$root/registry")
    if [ -z "$diff" ]; then
        echo "pass: the interface is the same as at $tag"
        exit 0
    fi
    echo "registry: $(printf '%s\n' "$diff" | registry_class) since $tag"
    echo "fail: the interface changed since $tag and the version is still $version; raise it"
    exit 1
fi
verdict "$work/registry" "$root/registry" "${tag#v}" "$version"
