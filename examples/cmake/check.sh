#!/bin/sh
# Builds and runs the consumer projects of examples/cmake on this computer and prints PASS or FAIL for each case:
#
#   add_subdirectory  the sdrkit source tree in the project; CMake runs cargo
#   find_package      an install prefix from `cmake --install` of sdrkit, moved after the install
#   prebuilt          the source tree with SDRKIT_PREBUILT_DIR set to the copy of tools/package.sh, with no cargo or
#                     rustc on PATH
#
# Each case runs with the static library and with the shared library. Two more checks: the system libraries that
# CMakeLists.txt lists hold every library that cargo printed for the package, and a program that links the shared
# library records it by its name (@rpath/libsdrkit.dylib or libsdrkit.so), not by a path.
#
# Usage: sh examples/cmake/check.sh [static|shared|both] [work folder]
#   The default is both, in a new temporary folder. The work folder is kept; the script prints its path.
#
# All builds use the cargo profile `shipped` in the target folder of the workspace, so the library is built once.
# On Windows run it from Git Bash; it calls tools/package.ps1 through pwsh or powershell. The exit status is 1 if a case failed.
#
# The second form checks an unpacked release archive, for any target, with no cargo:
#
#   sh examples/cmake/check.sh archive <unpacked archive> [work folder]
#
#   layout            the archive has the headers, both libraries, the CMake and pkg-config files, bin/sdrkit,
#                     LICENSE and THIRD_PARTY.md
#   find_package      the project find_package against the archive, with the static and the shared library
#   pkg-config        stack_count.c linked through sdrkit.pc and sdrkit-static.pc (not on Windows; only if pkg-config
#                     is installed)
#   command line      `sdrkit --version` prints the version and `sdrkit stacks` lists the stacks
#
# The environment controls the archive form:
#   SDRKIT_CHECK_RUN=0          build and link only: for a target whose programs this computer cannot run
#   SDRKIT_CHECK_CMAKE_ARGS     more arguments for each CMake configure, separated by spaces, such as `-A Win32`,
#                               `-DCMAKE_C_FLAGS=-m32` or a cross compiler
#   SDRKIT_CHECK_EMULATOR       a program that runs the target's programs, such as qemu-arm; CMake gets it as
#                               CMAKE_CROSSCOMPILING_EMULATOR
#   CC, CFLAGS                  the compiler and its flags for the pkg-config case (default cc)

set -u

mode=host
if [ "${1:-}" = archive ]; then
    mode=archive
    if [ -z "${2:-}" ] || [ ! -d "$2" ]; then
        echo "usage: sh examples/cmake/check.sh archive <unpacked archive> [work folder]" >&2
        exit 2
    fi
    archive=$(cd "$2" && pwd)
    shift 2
    set -- both "$@"
fi
kinds=${1:-both}
case "$kinds" in
    both) kinds="static shared" ;;
    static | shared) ;;
    *) echo "usage: sh examples/cmake/check.sh [static|shared|both] [work folder]" >&2; exit 2 ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
work=${2:-$(mktemp -d "${TMPDIR:-/tmp}/sdrkit-cmake-check.XXXXXX")}
mkdir -p "$work"
work=$(cd "$work" && pwd)
cmake=$(command -v cmake)
ctest=$(command -v ctest)
system=$(uname -s)
case "$system" in
    MINGW* | MSYS* | CYGWIN*) windows=1 ;;
    *) windows=0 ;;
esac
run=${SDRKIT_CHECK_RUN:-1}
emulator=${SDRKIT_CHECK_EMULATOR:-}
extra=${SDRKIT_CHECK_CMAKE_ARGS:-}
if [ -n "$emulator" ]; then
    extra="$extra -DCMAKE_CROSSCOMPILING_EMULATOR=$emulator"
fi

failed=0
summary=""
# Records the result of the case $1: $2 is 0 for a pass.
result() {
    if [ "$2" = 0 ]; then
        summary="$summary
PASS  $1"
    else
        summary="$summary
FAIL  $1 (log: $work/$(echo "$1" | tr ' ' '-').log)"
        failed=1
    fi
}

# Runs the command after $1 with its output in the log of the case $1.
logged() {
    name=$1
    shift
    echo "+ $*" >> "$work/$(echo "$name" | tr ' ' '-').log"
    "$@" >> "$work/$(echo "$name" | tr ' ' '-').log" 2>&1
}

# A native path for the Windows tools.
native() {
    if [ $windows = 1 ] && command -v cygpath > /dev/null; then cygpath -m "$1"; else echo "$1"; fi
}

# Configures the project $2 in the folder $3 with the arguments after $3 and those of SDRKIT_CHECK_CMAKE_ARGS, builds
# it and runs its test (unless SDRKIT_CHECK_RUN=0), as the case $1.
consumer() {
    name=$1 source=$2 build=$3
    shift 3
    rm -rf "$build"
    # shellcheck disable=SC2086
    logged "$name" "$cmake" -S "$(native "$source")" -B "$(native "$build")" -DCMAKE_BUILD_TYPE=Release $extra "$@" &&
        logged "$name" "$cmake" --build "$(native "$build")" --config Release || return 1
    if [ "$run" = 0 ]; then
        echo "not run: SDRKIT_CHECK_RUN=0" >> "$work/$(echo "$name" | tr ' ' '-').log"
        return 0
    fi
    logged "$name" "$ctest" --test-dir "$(native "$build")" -C Release --output-on-failure
}

# Checks that the program stack_count in the folder $1 records the shared library by its name.
records_name() {
    exe=$(find "$1" \( -name stack_count -o -name stack_count.exe \) -type f 2> /dev/null | head -n 1)
    if [ -z "$exe" ]; then
        return 1
    fi
    case "$system" in
        Darwin) otool -L "$exe" | grep -q "@rpath/libsdrkit.dylib" ;;
        Linux) readelf -d "$exe" | grep NEEDED | grep -q "\[libsdrkit.so\]" ;;
        *) true ;;
    esac
}

echo "work folder: $work"

# Runs the program $1 with the arguments after it, through SDRKIT_CHECK_EMULATOR if it is set.
target_run() {
    if [ -n "$emulator" ]; then "$emulator" "$@"; else "$@"; fi
}

if [ $mode = archive ]; then
    echo "archive: $archive"
    config="$archive/lib/cmake/sdrkit/sdrkitConfig.cmake"
    names=$(sed -n -E 's/^ *(STATIC|SHARED|IMPLIB) "(.*)"$/\2/p' "$config" 2> /dev/null)
    exe=sdrkit
    if [ $windows = 1 ]; then exe=sdrkit.exe; fi
    missing=""
    for f in include/sdrkit.h include/sdrkit_presets.h lib/cmake/sdrkit/sdrkitConfig.cmake \
        lib/cmake/sdrkit/sdrkitConfigVersion.cmake lib/cmake/sdrkit/sdrkitTargets.cmake lib/pkgconfig/sdrkit.pc \
        "bin/$exe" LICENSE THIRD_PARTY.md share/sdrkit/data; do
        if [ ! -e "$archive/$f" ]; then missing="$missing $f"; fi
    done
    for f in $names; do
        if [ ! -f "$archive/lib/$f" ]; then missing="$missing lib/$f"; fi
    done
    if [ -z "$names" ]; then missing="$missing (no library names in sdrkitConfig.cmake)"; fi
    if [ -n "$missing" ]; then echo "missing:$missing" > "$work/layout.log"; cat "$work/layout.log"; fi
    result "layout" "$([ -z "$missing" ] && echo 0 || echo 1)"

    for kind in $kinds; do
        if [ "$kind" = shared ]; then shared=ON; else shared=OFF; fi
        echo "case find_package, $kind"
        consumer "find_package $kind" "$here/find_package" "$work/find_package-$kind" \
            -DCMAKE_PREFIX_PATH="$(native "$archive")" -DBUILD_SHARED_LIBS=$shared
        result "find_package $kind" $?
    done
    if echo "$kinds" | grep -q shared; then
        records_name "$work/find_package-shared"
        result "library name" $?
    fi

    if [ $windows = 0 ] && command -v pkg-config > /dev/null; then
        cc=${CC:-cc}
        for pc in sdrkit sdrkit-static; do
            echo "case pkg-config, $pc"
            name="pkg-config $pc"
            out="$work/stack_count-$pc"
            # shellcheck disable=SC2046,SC2086
            logged "$name" $cc ${CFLAGS:-} "$here/common/stack_count.c" \
                $(PKG_CONFIG_PATH="$archive/lib/pkgconfig" PKG_CONFIG_LIBDIR="$archive/lib/pkgconfig" \
                    pkg-config --cflags --libs "$pc") -Wl,-rpath,"$archive/lib" -o "$out"
            status=$?
            if [ $status = 0 ] && [ "$run" != 0 ]; then
                logged "$name" target_run "$out"
                status=$?
            fi
            result "$name" $status
        done
    fi

    if [ "$run" != 0 ]; then
        echo "case command line"
        version=$(sed -n 's/^set(PACKAGE_VERSION "\([0-9.]*\)")$/\1/p' "$archive/lib/cmake/sdrkit/sdrkitConfigVersion.cmake")
        printed=$(target_run "$archive/bin/$exe" --version 2>> "$work/command-line.log" | tr -d '\r')
        echo "sdrkit --version: $printed" >> "$work/command-line.log"
        stacks=$(target_run "$archive/bin/$exe" stacks 2>> "$work/command-line.log" | wc -l | tr -d ' ')
        echo "sdrkit stacks: $stacks lines" >> "$work/command-line.log"
        echo "sdrkit --version: $printed; sdrkit stacks: $stacks lines"
        ok=1
        if [ -n "$version" ] && echo "$printed" | grep -q "$version" && [ "$stacks" -gt 1 ]; then ok=0; fi
        result "command line" $ok
    fi

    echo "$summary"
    exit $failed
fi

# The package of tools/package.sh: the input of the case prebuilt and the list of system libraries that cargo printed.
echo "making the package"
if [ $windows = 1 ]; then
    # PowerShell 7 (pwsh) if it is installed, else Windows PowerShell.
    ps=$(command -v pwsh || command -v powershell)
    pkg=$("$ps" -NoProfile -ExecutionPolicy Bypass -File "$(native "$root/tools/package.ps1")" \
        -Out "$(native "$work/dist")" | tr -d '\r' | tail -n 1)
else
    pkg=$(sh "$root/tools/package.sh" "" "$work/dist" | tail -n 1)
fi
if [ -z "$pkg" ] || [ ! -f "$pkg/lib/cmake/sdrkit/sdrkitConfig.cmake" ]; then
    echo "FAIL  the package (tools/package.sh)"
    exit 1
fi

# The install prefix of the case find_package. It moves after the install, to show that the package is relocatable.
echo "building and installing sdrkit with CMake"
rm -rf "$work/sdrkit-build" "$work/installed" "$work/prefix"
logged "install" "$cmake" -S "$(native "$root")" -B "$(native "$work/sdrkit-build")" -DCMAKE_BUILD_TYPE=Release \
    -DSDRKIT_CARGO_PROFILE=shipped -DCMAKE_INSTALL_PREFIX="$(native "$work/installed")" &&
    logged "install" "$cmake" --build "$(native "$work/sdrkit-build")" --config Release &&
    logged "install" "$cmake" --install "$(native "$work/sdrkit-build")" --config Release &&
    mv "$work/installed" "$work/prefix"
result "install" $?

# Every system library that cargo printed must be in the list of CMakeLists.txt. The names are compared without their
# folders and without the `-framework ` word.
# On Windows a name may carry `.lib` and differ in case; both are removed before the comparison.
plain() { sed 's|^.*[/\\]||; s|^-framework ||; s|\.[Ll][Ii][Bb]$||' | tr 'A-Z' 'a-z'; }
listed=$(tr ';' '\n' < "$work/sdrkit-build/sdrkit-system-libraries.txt" 2> /dev/null | plain)
printed=$(sed -n 's/^set(_sdrkit_system_libraries "\(.*\)")$/\1/p' "$pkg/lib/cmake/sdrkit/sdrkitConfig.cmake" |
    tr ';' '\n' | plain)
missing=""
for item in $printed; do
    if ! echo "$listed" | grep -qx "$item"; then missing="$missing $item"; fi
done
if [ -n "$missing" ]; then
    echo "system libraries that cargo printed and CMakeLists.txt does not list:$missing" > "$work/system-libraries.log"
    cat "$work/system-libraries.log"
    echo "CMakeLists.txt lists: $(echo $listed)"
fi
result "system libraries" "$([ -z "$missing" ] && echo 0 || echo 1)"

# The PATH without cargo and rustc, for the case prebuilt.
norust=""
old_ifs=$IFS
IFS=:
for dir in $PATH; do
    if [ ! -e "$dir/cargo" ] && [ ! -e "$dir/rustc" ] && [ ! -e "$dir/cargo.exe" ]; then norust="$norust:$dir"; fi
done
IFS=$old_ifs
norust=${norust#:}

for kind in $kinds; do
    if [ "$kind" = shared ]; then shared=ON; else shared=OFF; fi
    echo "case add_subdirectory, $kind"
    consumer "add_subdirectory $kind" "$here/add_subdirectory" "$work/add_subdirectory-$kind" \
        -DSDRKIT_SOURCE_DIR="$(native "$root")" -DSDRKIT_CARGO_PROFILE=shipped -DBUILD_SHARED_LIBS=$shared
    result "add_subdirectory $kind" $?

    echo "case find_package, $kind"
    consumer "find_package $kind" "$here/find_package" "$work/find_package-$kind" \
        -DCMAKE_PREFIX_PATH="$(native "$work/prefix")" -DBUILD_SHARED_LIBS=$shared
    result "find_package $kind" $?

    echo "case prebuilt, $kind"
    (
        PATH=$norust
        consumer "prebuilt $kind" "$here/prebuilt" "$work/prebuilt-$kind" -DSDRKIT_SOURCE_DIR="$(native "$root")" \
            -DSDRKIT_PREBUILT_DIR="$(native "$pkg")" -DBUILD_SHARED_LIBS=$shared
    )
    result "prebuilt $kind" $?

    if [ "$kind" = shared ]; then
        ok=0
        for case in add_subdirectory find_package prebuilt; do
            if ! records_name "$work/$case-shared"; then
                ok=1
                echo "$case: the program does not record the library by its name" >> "$work/library-name.log"
            fi
        done
        result "library name" $ok
    fi
done

echo "$summary"
exit $failed
