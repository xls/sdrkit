#!/bin/sh
# Builds the C library of sdrkit for the Apple platforms and packs the static libraries with the headers and a module
# map into apple/build/SDRKitC.xcframework.
#
# Usage: sh apple/build_xcframework.sh
#        SDRKIT_PRESETS=0 sh apple/build_xcframework.sh
#        SDRKIT_TARGETS="aarch64-apple-ios aarch64-apple-ios-sim" sh apple/build_xcframework.sh
#
# SDRKIT_TARGETS names the targets to build and pack, for a framework with fewer parts in less time: an app for the
# phone and the simulator needs no macOS library. The default is aarch64-apple-ios, aarch64-apple-ios-sim and
# aarch64-apple-darwin. The release workflow builds all of these:
#
#   aarch64-apple-ios                                  slice ios-arm64
#   aarch64-apple-ios-sim, x86_64-apple-ios            slice ios-arm64_x86_64-simulator
#   aarch64-apple-darwin, x86_64-apple-darwin          slice macos-arm64_x86_64
#   aarch64-apple-ios-macabi, x86_64-apple-ios-macabi  slice ios-arm64_x86_64-maccatalyst
#
# Two targets of one slice are joined with lipo. SDRKIT_OPTIONAL_TARGETS names more targets that the script tries: a
# target of it that fails to build is left out of the framework with a warning, and the script goes on.
# SDRKIT_DROP_BUILDS=1 removes target/<triple>/ after its library is copied, for a machine with little disk space;
# the next run then builds that target from the start.
#
# The library holds every stack family (the default features of sdrkit-host-ffi) and the presets part of the C API
# (the cargo feature `presets`, header `sdrkit_presets.h`). With SDRKIT_PRESETS=0 the library has no presets part,
# reads no data folder, and the module has no `sdrkit_presets.h`.
#
# The module SDRKitC has `sdrkit.h`, the family headers of `include/sdrkit/` and, with presets, the submodule
# `SDRKitC.Presets` with `sdrkit_presets.h`. The Swift class `Presets` exists only with it.
# The Swift package compiles the generated Swift files of `swift/` through the link `Sources/SDRKit/Generated`.
#
# The cargo builds use the target/ folder of the workspace. A second run builds only what changed and replaces the
# framework.
#
# The deployment targets below are those of the Swift package (iOS 17, Mac Catalyst 17, macOS 14). The C and
# assembly parts of the library are built for them; without them the linker warns that objects are built for a newer
# system.

set -eu

apple=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$apple/.." && pwd)
ffi="$root/crates/hosts/sdrkit-host-ffi"
build="$apple/build"
framework="$build/SDRKitC.xcframework"
headers="$build/include"
libs="$build/lib"
targets="${SDRKIT_TARGETS:-aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin}"
optional="${SDRKIT_OPTIONAL_TARGETS:-}"

features=""
if [ "${SDRKIT_PRESETS:-1}" = 1 ]; then
    features="--features presets"
fi

export IPHONEOS_DEPLOYMENT_TARGET=17.0
export MACOSX_DEPLOYMENT_TARGET=14.0

cd "$root"
for target in $targets $optional; do
    case "$target" in
        aarch64-apple-ios | aarch64-apple-ios-sim | x86_64-apple-ios | aarch64-apple-darwin | x86_64-apple-darwin | \
            aarch64-apple-ios-macabi | x86_64-apple-ios-macabi) ;;
        *) echo "$target is not an Apple target of the framework" >&2; exit 1 ;;
    esac
done
rm -rf "$libs" "$framework"

# Builds the target $1 and copies its library to $libs/$1/libsdrkit.a.
#
# The profile `shipped` writes no debug information, so the libraries go into the framework as cargo wrote them.
# Never run `strip` on a library: it cuts the `.llvm.<number>` end off the symbol names, and that end is all that
# tells two copies of one function in two objects apart. The copies can take their arguments in different registers,
# and the linker then joins a call to the wrong copy. The check below refuses a library that has such a pair.
build_target() {
    start=$(date +%s)
    # shellcheck disable=SC2086
    cargo build --profile shipped -p sdrkit-host-ffi $features --target "$1" || return 1
    echo "built $1 in $(($(date +%s) - start)) s"
    mkdir -p "$libs/$1"
    cp "$root/target/$1/shipped/libsdrkit.a" "$libs/$1/libsdrkit.a" || return 1
    if [ "${SDRKIT_DROP_BUILDS:-0}" = 1 ]; then
        rm -rf "$root/target/$1"
    fi
    twice=$(nm -g --defined-only "$libs/$1/libsdrkit.a" 2> /dev/null | awk 'NF == 3 { print $3 }' | sort | uniq -d)
    if [ -n "$twice" ]; then
        echo "$1: $(echo "$twice" | wc -l | tr -d ' ') symbols are defined in two objects, the first:" >&2
        echo "$twice" | head -n 1 >&2
        exit 1
    fi
    echo "$1: $(du -h "$libs/$1/libsdrkit.a" | cut -f1)"
}

for target in $targets; do
    build_target "$target" || exit 1
done
for target in $optional; do
    if ! build_target "$target"; then
        rm -rf "$libs/$target"
        echo "warning: $target did not build; the framework has no part for it" >&2
        if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::warning::$target did not build; the framework has no part for it"; fi
    fi
done

# The headers of the framework, in a folder with the name of the module so that the module map does not collide
# with that of another framework in the same project. The family headers include "../sdrkit.h", so they keep their
# folder `sdrkit/`.
rm -rf "$headers"
mkdir -p "$headers/SDRKitC"
cp "$ffi/include/sdrkit.h" "$headers/SDRKitC/sdrkit.h"
cp -R "$ffi/include/sdrkit" "$headers/SDRKitC/sdrkit"
{
    echo "module SDRKitC {"
    echo "    header \"sdrkit.h\""
    for h in "$headers"/SDRKitC/sdrkit/*.h; do
        echo "    header \"sdrkit/$(basename "$h")\""
    done
    echo "    export *"
    # The presets part is a submodule, so that Swift code can ask `canImport(SDRKitC.Presets)`. `import SDRKitC`
    # imports it too.
    if [ -n "$features" ]; then
        cp "$ffi/include/sdrkit_presets.h" "$headers/SDRKitC/sdrkit_presets.h"
        echo "    module Presets {"
        echo "        header \"sdrkit_presets.h\""
        echo "        export *"
        echo "    }"
    fi
    echo "}"
} > "$headers/SDRKitC/module.modulemap"

# One library for each slice: the targets of a slice are joined with lipo.
set --
for slice in ios ios-simulator macos maccatalyst; do
    case $slice in
        ios) members="aarch64-apple-ios" ;;
        ios-simulator) members="aarch64-apple-ios-sim x86_64-apple-ios" ;;
        macos) members="aarch64-apple-darwin x86_64-apple-darwin" ;;
        maccatalyst) members="aarch64-apple-ios-macabi x86_64-apple-ios-macabi" ;;
    esac
    found=""
    for target in $members; do
        if [ -f "$libs/$target/libsdrkit.a" ]; then found="$found $libs/$target/libsdrkit.a"; fi
    done
    if [ -z "$found" ]; then
        continue
    fi
    mkdir -p "$libs/$slice"
    # shellcheck disable=SC2086
    lipo -create $found -output "$libs/$slice/libsdrkit.a"
    echo "$slice: $(lipo -archs "$libs/$slice/libsdrkit.a")"
    set -- "$@" -library "$libs/$slice/libsdrkit.a" -headers "$headers"
done
xcodebuild -create-xcframework "$@" -output "$framework" > /dev/null

rm -rf "$headers" "$libs"
echo "wrote $framework"
