# Shell functions that compare two copies of the interface registry (registry/*.reg). tools/check_version.sh and
# tools/release_notes.sh read this file with `.`; it does nothing on its own.
#
# A registry file holds one family: the line `family <name> version=<major>.<minor>`, then one line per item,
# `<key> <attributes>`. The registry test of sdrkit-host-catalog raises the major version of a family for a breaking
# change and the minor version for an addition (docs/stack-interface.md), so the family versions tell the class of a
# difference. A removed item counts as breaking too, in case a family version was not raised.

# Prints the registry of the folder $1 as tab-separated lines: `F <family> <version>` and `I <family> <key> <rest>`.
registry_flat() {
    for file in "$1"/*.reg; do
        if [ -f "$file" ]; then cat "$file"; fi
    done | awk '
        /^#/ || NF == 0 { next }
        $1 == "family" { family = $2; v = $3; sub(/^version=/, "", v); print "F\t" family "\t" v; next }
        { key = $1; $1 = ""; sub(/^ /, ""); print "I\t" family "\t" key "\t" $0 }'
}

# Compares the registry folders $1 (old) and $2 (new) and prints one line per difference, tab-separated:
#   family-added <family> <version>        family-removed <family> <version>
#   family-version <family> <old> <new>
#   added <family> <key> <attributes>      removed <family> <key> <attributes>
#   changed <family> <key> <old attributes> <new attributes>
registry_compare() {
    old_flat=$(mktemp "${TMPDIR:-/tmp}/sdrkit-registry.XXXXXX")
    registry_flat "$1" > "$old_flat"
    registry_flat "$2" | awk -F '\t' -v old="$old_flat" '
        BEGIN {
            while ((getline line < old) > 0) {
                split(line, f, "\t")
                if (f[1] == "F") { ofam[f[2]] = f[3] } else { oitem[f[2] "\t" f[3]] = f[4] }
            }
        }
        $1 == "F" { nfam[$2] = $3; next }
        { nitem[$2 "\t" $3] = $4 }
        END {
            for (k in nfam) {
                if (!(k in ofam)) print "family-added\t" k "\t" nfam[k]
                else if (ofam[k] != nfam[k]) print "family-version\t" k "\t" ofam[k] "\t" nfam[k]
            }
            for (k in ofam) if (!(k in nfam)) print "family-removed\t" k "\t" ofam[k]
            for (k in nitem) {
                if (!(k in oitem)) print "added\t" k "\t" nitem[k]
                else if (oitem[k] != nitem[k]) print "changed\t" k "\t" oitem[k] "\t" nitem[k]
            }
            for (k in oitem) if (!(k in nitem)) print "removed\t" k "\t" oitem[k]
        }' | sort
    rm -f "$old_flat"
}

# Reads the output of registry_compare on standard input and prints its class: none, grew or broke.
registry_class() {
    awk -F '\t' '
        $1 == "family-removed" || $1 == "removed" { broke = 1 }
        $1 == "family-version" {
            split($3, o, "."); split($4, n, ".")
            if (o[1] != n[1]) broke = 1; else grew = 1
        }
        $1 == "family-added" || $1 == "added" || $1 == "changed" { grew = 1 }
        END { print (broke ? "broke" : (grew ? "grew" : "none")) }'
}

# Prints the version of the crate sdrkit-host-ffi in the source tree $1.
registry_crate_version() {
    sed -n 's/^version *= *"\(.*\)"$/\1/p' "$1/crates/hosts/sdrkit-host-ffi/Cargo.toml" | head -n 1
}

# Prints the version tags v<major>.<minor>.<patch> of the git repository of the folder $1, highest version last.
registry_version_tags() {
    git -C "$1" tag -l 'v[0-9]*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sed 's/^v//' |
        sort -t . -k 1,1n -k 2,2n -k 3,3n | sed 's/^/v/'
}

# Prints 1 if the version $1 is lower than the version $2, else 0. Both are <major>.<minor>.<patch>.
registry_version_lower() {
    echo "$1 $2" | awk '{
        split($1, a, "."); split($2, b, ".")
        for (i = 1; i <= 3; i++) { if (a[i] + 0 < b[i] + 0) { print 1; exit } if (a[i] + 0 > b[i] + 0) { print 0; exit } }
        print 0 }'
}

# Prints the highest version tag of the repository of the folder $1 whose version is lower than $2, or nothing.
registry_tag_before() {
    before=""
    for tag in $(registry_version_tags "$1"); do
        if [ "$(registry_version_lower "${tag#v}" "$2")" = 1 ]; then before=$tag; fi
    done
    echo "$before"
}

# Writes the registry/ folder of the git revision $2 of the repository of the folder $1 into the folder $3.
registry_at() {
    mkdir -p "$3"
    git -C "$1" archive "$2" registry | tar -x -C "$3" --strip-components 1
}
