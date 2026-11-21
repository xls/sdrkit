#!/bin/sh
# Builds the C library of sdrkit for iOS, the iOS simulator and macOS (all arm64), and packs the three static
# libraries with the headers and a module map into apple/build/SDRKitC.xcframework.
#
# Usage: sh apple/build_xcframework.sh
#        SDRKIT_PRESETS=0 sh apple/build_xcframework.sh
#        SDRKIT_TARGETS="aarch64-apple-ios aarch64-apple-ios-sim" sh apple/build_xcframework.sh
#
# SDRKIT_TARGETS names the targets to build and pack, for a framework with fewer parts in less time: an app for the
# phone and the simulator needs no macOS library. The default is all three.
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
# The deployment targets below are those of the Swift package (iOS 17, macOS 14). The C and assembly parts of the
# library are built for them; without them the linker warns that objects are built for a newer system.

set -eu

apple=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$apple/.." && pwd)
ffi="$root/crates/hosts/sdrkit-host-ffi"
build="$apple/build"
framework="$build/SDRKitC.xcframework"
headers="$build/include"
libs="$build/lib"
targets="${SDRKIT_TARGETS:-aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin}"

features=""
if [ "${SDRKIT_PRESETS:-1}" = 1 ]; then
    features="--features presets"
fi

export IPHONEOS_DEPLOYMENT_TARGET=17.0
export MACOSX_DEPLOYMENT_TARGET=14.0

cd "$root"

for target in $targets; do
    start=$(date +%s)
    # shellcheck disable=SC2086
    cargo build --profile shipped -p sdrkit-host-ffi $features --target "$target"
    echo "built $target in $(($(date +%s) - start)) s"
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

# The profile `shipped` writes no debug information, so the libraries go into the framework as cargo wrote them.
# Never run `strip` on a library: it cuts the `.llvm.<number>` end off the symbol names, and that end is all that
# tells two copies of one function in two objects apart. The copies can take their arguments in different registers,
# and the linker then joins a call to the wrong copy. The check below refuses a library that has such a pair.
rm -rf "$libs" "$framework"
set --
for target in $targets; do
    mkdir -p "$libs/$target"
    cp "$root/target/$target/shipped/libsdrkit.a" "$libs/$target/libsdrkit.a"
    twice=$(nm -g --defined-only "$libs/$target/libsdrkit.a" 2> /dev/null | awk 'NF == 3 { print $3 }' | sort | uniq -d)
    if [ -n "$twice" ]; then
        echo "$target: $(echo "$twice" | wc -l | tr -d ' ') symbols are defined in two objects, the first:" >&2
        echo "$twice" | head -n 1 >&2
        exit 1
    fi
    echo "$target: $(du -h "$libs/$target/libsdrkit.a" | cut -f1)"
    set -- "$@" -library "$libs/$target/libsdrkit.a" -headers "$headers"
done
xcodebuild -create-xcframework "$@" -output "$framework" > /dev/null

rm -rf "$headers" "$libs"
echo "wrote $framework"
