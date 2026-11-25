#!/bin/sh
# Builds the C library of sdrkit with the cargo profile `shipped` (release code without debug information) and lays
# out an installed copy of it:
#
#   <out>/sdrkit-<version>-<target>/
#     include/                 sdrkit.h, sdrkit/<family>.h, sdrkit/init.h, sdrkit_presets.h
#     lib/                     the static library and the shared library (and the import library on Windows)
#     lib/pkgconfig/           sdrkit.pc (shared), sdrkit-static.pc (static, with the system libraries)
#     lib/cmake/sdrkit/        sdrkitConfig.cmake, sdrkitConfigVersion.cmake, sdrkitTargets.cmake
#     share/sdrkit/data/       a copy of the folder data/ of the repository
#     share/doc/sdrkit/        the licence files
#
# With --release the script also builds the command-line tool into bin/, puts LICENSE and THIRD_PARTY.md at the top
# of the folder, names the folder sdrkit-<version>-<os>-<arch> (such as sdrkit-0.5.0-macos-arm64), and packs it into
# <out>/sdrkit-<version>-<os>-<arch>.tar.gz.
#
# Usage: sh tools/package.sh [--release] [target] [out]
#   target  a target triple. The default (also an empty argument) is the host. Another triple builds with
#           `cargo --target`; its rustup target and a linker for it must be installed.
#   out     the folder for the result. The default is dist/ at the root of the workspace.
#
# The script prints the folder of the copy on its last line. The library holds every stack family and the presets
# part of the C interface (the cargo feature `presets`). The build uses the target folder of cargo: CARGO_TARGET_DIR
# if it is set, else target/ of the workspace. The cargo arguments are those of CMakeLists.txt with the profile
# `shipped`, so the two builds share their results. On Windows, use tools/package.ps1.

set -eu

release=0
if [ "${1:-}" = "--release" ]; then
    release=1
    shift
fi

root=$(cd "$(dirname "$0")/.." && pwd)
ffi="$root/crates/hosts/sdrkit-host-ffi"
templates="$root/cmake"
host=$(rustc -vV | sed -n 's/^host: //p')
target=${1:-$host}
out=${2:-$root/dist}
tdir=${CARGO_TARGET_DIR:-$root/target}
version=$(sed -n 's/^version *= *"\(.*\)"$/\1/p' "$ffi/Cargo.toml" | head -n 1)
if [ -z "$version" ]; then
    version=$(sed -n 's/^version *= *"\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)
fi

# The name of the platform in a release archive: <os>-<arch>.
case "$target" in
    aarch64-apple-darwin) platform=macos-arm64 ;;
    x86_64-apple-darwin) platform=macos-x86_64 ;;
    x86_64-unknown-linux-gnu) platform=linux-x86_64 ;;
    aarch64-unknown-linux-gnu) platform=linux-aarch64 ;;
    x86_64-pc-windows-msvc) platform=windows-x86_64 ;;
    x86_64-pc-windows-gnu) platform=windows-x86_64-gnu ;;
    *) platform=$target ;;
esac
if [ $release = 1 ]; then
    name="sdrkit-$version-$platform"
else
    name="sdrkit-$version-$target"
fi
dist="$out/$name"

# The file names that cargo gives the libraries on the target, and the name that a program records for the shared
# library.
soname=
case "$target" in
    *-windows-msvc) static=sdrkit.lib; dynamic=sdrkit.dll; implib=sdrkit.dll.lib; exe=sdrkit.exe ;;
    *-windows-gnu*) static=libsdrkit.a; dynamic=sdrkit.dll; implib=libsdrkit.dll.a; exe=sdrkit.exe ;;
    *-apple-*) static=libsdrkit.a; dynamic=libsdrkit.dylib; implib=; exe=sdrkit; soname=@rpath/libsdrkit.dylib ;;
    *) static=libsdrkit.a; dynamic=libsdrkit.so; implib=; exe=sdrkit; soname=libsdrkit.so ;;
esac
case "$target" in
    *-apple-*) set -- -C "link-arg=-Wl,-install_name,$soname" ;;
    *-linux-*) set -- -C "link-arg=-Wl,-soname,$soname" ;;
    *) set -- ;;
esac

cd "$root"
if [ "$target" = "$host" ]; then
    built="$tdir/shipped"
    cross=
else
    built="$tdir/$target/shipped"
    cross="--target $target"
fi

# One build makes both libraries. `--print native-static-libs` names the system libraries that the static library
# needs; cargo shows the note again on a build that has nothing to do.
log="$out/.build-$target.log"
mkdir -p "$out"
echo "building sdrkit $version for $target"
# shellcheck disable=SC2086
cargo rustc --profile shipped -p sdrkit-host-ffi --lib --features presets $cross \
    -- --print native-static-libs "$@" 2> "$log" || { cat "$log" >&2; rm -f "$log"; exit 1; }
native=$(sed -n 's/^.*native-static-libs: //p' "$log" | tail -n 1)
rm -f "$log"
if [ -z "$native" ]; then
    echo "cargo printed no native-static-libs line" >&2
    exit 1
fi
if [ $release = 1 ]; then
    echo "building the command-line tool for $target"
    # shellcheck disable=SC2086
    cargo build --profile shipped -p sdrkit-host-cli $cross
fi

rm -rf "$dist"
mkdir -p "$dist/include/sdrkit" "$dist/lib/pkgconfig" "$dist/lib/cmake/sdrkit" "$dist/share/sdrkit" \
    "$dist/share/doc/sdrkit"

cp "$ffi/include/sdrkit.h" "$ffi/include/sdrkit_presets.h" "$dist/include/"
cp "$ffi/include/sdrkit/"*.h "$dist/include/sdrkit/"
cp -R "$root/data" "$dist/share/sdrkit/data"
cp "$root/LICENSE" "$root/THIRD_PARTY.md" "$dist/share/doc/sdrkit/"
for f in $static $dynamic $implib; do
    cp "$built/$f" "$dist/lib/$f"
done
if [ $release = 1 ]; then
    mkdir -p "$dist/bin"
    cp "$built/$exe" "$dist/bin/$exe"
    cp "$root/LICENSE" "$root/THIRD_PARTY.md" "$dist/"
fi

# A static library for a Windows target names the import library of the crate windows_<arch>_gnu (such as
# -lwindows.0.52.0); the linker must find it, so it goes into lib/ beside the library.
case "$native" in
    *-lwindows.*)
        cargo metadata --format-version 1 --filter-platform "$target" --manifest-path "$root/Cargo.toml" |
            tr ',' '\n' | sed -n 's/^"manifest_path":"\(.*windows_[a-z0-9_]*_gnu[a-z]*-[^"]*\)\/Cargo.toml"$/\1/p' |
            while read -r dir; do
                for f in "$dir"/lib/libwindows*.a; do
                    if [ -f "$f" ]; then cp "$f" "$dist/lib/"; fi
                done
            done
        ;;
esac

# The system libraries as CMake list items: `-lm` becomes `m`, `-framework X` stays one item, a `.lib` name stays as
# it is, and `/defaultlib:` is left to the compiler.
cmake_libs=""
framework=0
for word in $native; do
    if [ $framework = 1 ]; then
        cmake_libs="$cmake_libs;-framework $word"
        framework=0
        continue
    fi
    case "$word" in
        -framework) framework=1 ;;
        -l*) cmake_libs="$cmake_libs;${word#-l}" ;;
        /defaultlib:*) ;;
        *) cmake_libs="$cmake_libs;$word" ;;
    esac
done
cmake_libs=${cmake_libs#;}

case "$target" in
    *-windows-*) pc_shared="\${libdir}/$implib" ;;
    *) pc_shared="-L\${libdir} -lsdrkit" ;;
esac
major=${version%%.*}
minor=${version#*.}
minor=${minor%%.*}

# Fills the template $1 of cmake/ into the file $2: each @SDRKIT_<KEY>@ becomes its value. CMakeLists.txt fills the
# same templates for `cmake --install`.
fill() {
    sed -e "s|@SDRKIT_VERSION@|$version|g" \
        -e "s|@SDRKIT_VERSION_MAJOR@|$major|g" \
        -e "s|@SDRKIT_VERSION_MINOR@|$minor|g" \
        -e "s|@SDRKIT_RUST_TARGET@|$target|g" \
        -e "s|@SDRKIT_CONFIG_TO_PREFIX@|../../..|g" \
        -e "s|@SDRKIT_PC_TO_PREFIX@|../..|g" \
        -e "s|@SDRKIT_INSTALL_INCLUDEDIR@|include|g" \
        -e "s|@SDRKIT_INSTALL_LIBDIR@|lib|g" \
        -e "s|@SDRKIT_INSTALL_SHAREDDIR@|lib|g" \
        -e "s|@SDRKIT_INSTALL_DATADIR@|share/sdrkit/data|g" \
        -e "s|@SDRKIT_STATIC_NAME@|$static|g" \
        -e "s|@SDRKIT_SHARED_NAME@|$dynamic|g" \
        -e "s|@SDRKIT_IMPLIB_NAME@|$implib|g" \
        -e "s|@SDRKIT_SONAME@|$soname|g" \
        -e "s|@SDRKIT_SIZEOF_VOID_P@|8|g" \
        -e "s|@SDRKIT_SYSTEM_LIBRARIES@|$cmake_libs|g" \
        -e "s|@SDRKIT_PC_SHARED_LIBS@|$pc_shared|g" \
        -e "s|@SDRKIT_PC_SYSTEM_LIBS@|$native|g" \
        "$templates/$1" > "$2"
}
fill sdrkitConfig.cmake.in "$dist/lib/cmake/sdrkit/sdrkitConfig.cmake"
fill sdrkitConfigVersion.cmake.in "$dist/lib/cmake/sdrkit/sdrkitConfigVersion.cmake"
cp "$templates/sdrkitTargets.cmake" "$dist/lib/cmake/sdrkit/"
fill sdrkit.pc.in "$dist/lib/pkgconfig/sdrkit.pc"
fill sdrkit-static.pc.in "$dist/lib/pkgconfig/sdrkit-static.pc"

if [ $release = 1 ]; then
    archive="$out/$name.tar.gz"
    (cd "$out" && COPYFILE_DISABLE=1 tar -czf "$name.tar.gz" "$name")
    echo "wrote $archive"
fi
echo "$dist"
