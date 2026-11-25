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

set -u

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

# Configures the project $2 in the folder $3 with the arguments after $3, builds it and runs its test, as the case $1.
consumer() {
    name=$1 source=$2 build=$3
    shift 3
    rm -rf "$build"
    logged "$name" "$cmake" -S "$(native "$source")" -B "$(native "$build")" -DCMAKE_BUILD_TYPE=Release "$@" &&
        logged "$name" "$cmake" --build "$(native "$build")" --config Release &&
        logged "$name" "$ctest" --test-dir "$(native "$build")" -C Release --output-on-failure
}

# Checks that the program stack_count in the folder $1 records the shared library by its name.
records_name() {
    exe=$(find "$1" -name stack_count -type f 2> /dev/null | head -n 1)
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
listed=$(tr ';' '\n' < "$work/sdrkit-build/sdrkit-system-libraries.txt" 2> /dev/null | sed 's|^.*/||; s|^-framework ||')
printed=$(sed -n 's/^set(_sdrkit_system_libraries "\(.*\)")$/\1/p' "$pkg/lib/cmake/sdrkit/sdrkitConfig.cmake" |
    tr ';' '\n' | sed 's|^-framework ||')
missing=""
for item in $printed; do
    if ! echo "$listed" | grep -qx "$item"; then missing="$missing $item"; fi
done
if [ -n "$missing" ]; then
    echo "system libraries that cargo printed and CMakeLists.txt does not list:$missing" > "$work/system-libraries.log"
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
