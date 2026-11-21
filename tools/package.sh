#!/bin/sh
# Builds the C library of sdrkit with the cargo profile `shipped` (release code without debug information) and lays
# out an installed copy of it:
#
#   <out>/sdrkit-<version>-<target>/
#     include/                 sdrkit.h, sdrkit/<family>.h, sdrkit/init.h, sdrkit_presets.h
#     lib/                     the static library and the dynamic library (and the import library on Windows)
#     lib/pkgconfig/           sdrkit.pc (dynamic), sdrkit-static.pc (static, with the system libraries)
#     lib/cmake/sdrkit/        sdrkitConfig.cmake, sdrkitConfigVersion.cmake
#     share/sdrkit/data/       a copy of the folder data/ of the repository
#     share/doc/sdrkit/        the licence files
#
# Usage: sh tools/package.sh [target] [out]
#   target  a target triple. The default is the host. Another triple builds with `cargo --target`; its rustup target
#           and a linker for it must be installed.
#   out     the folder for the result. The default is dist/ at the root of the workspace.
#
# The library holds every stack family and the presets part of the C interface (the cargo feature `presets`). The
# build uses the target/ folder of the workspace. On Windows, use tools/package.ps1.

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
ffi="$root/crates/hosts/sdrkit-host-ffi"
host=$(rustc -vV | sed -n 's/^host: //p')
target=${1:-$host}
out=${2:-$root/dist}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ffi/Cargo.toml" | head -n 1)
name="sdrkit-$version-$target"
dist="$out/$name"

# The file names that cargo gives the libraries on the target.
case "$target" in
    *-windows-msvc) static=sdrkit.lib; dynamic=sdrkit.dll; implib=sdrkit.dll.lib ;;
    *-windows-gnu*) static=libsdrkit.a; dynamic=sdrkit.dll; implib=libsdrkit.dll.a ;;
    *-apple-*) static=libsdrkit.a; dynamic=libsdrkit.dylib; implib= ;;
    *) static=libsdrkit.a; dynamic=libsdrkit.so; implib= ;;
esac

cd "$root"
if [ "$target" = "$host" ]; then
    set --
    built="$root/target/shipped"
else
    set -- --target "$target"
    built="$root/target/$target/shipped"
fi

# One build makes both libraries. `--print native-static-libs` names the system libraries that the static library
# needs; cargo shows the note again on a build that has nothing to do.
log="$out/.build-$target.log"
mkdir -p "$out"
echo "building sdrkit $version for $target"
cargo rustc --profile shipped -p sdrkit-host-ffi --features presets --lib --crate-type staticlib --crate-type cdylib "$@" \
    -- --print native-static-libs 2> "$log" || { cat "$log" >&2; rm -f "$log"; exit 1; }
native=$(sed -n 's/^.*native-static-libs: //p' "$log" | tail -n 1)
rm -f "$log"
if [ -z "$native" ]; then
    echo "cargo printed no native-static-libs line" >&2
    exit 1
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

# A program that links the dynamic library finds it through its run path.
case "$target" in
    *-apple-*) install_name_tool -id "@rpath/$dynamic" "$dist/lib/$dynamic" ;;
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

cat > "$dist/lib/pkgconfig/sdrkit.pc" <<EOF
prefix=\${pcfiledir}/../..
includedir=\${prefix}/include
libdir=\${prefix}/lib
datadir=\${prefix}/share/sdrkit/data

Name: sdrkit
Description: A software-defined radio engine (dynamic library)
Version: $version
Cflags: -I\${includedir}
Libs: -L\${libdir} -lsdrkit
EOF

cat > "$dist/lib/pkgconfig/sdrkit-static.pc" <<EOF
prefix=\${pcfiledir}/../..
includedir=\${prefix}/include
libdir=\${prefix}/lib
datadir=\${prefix}/share/sdrkit/data

Name: sdrkit-static
Description: A software-defined radio engine (static library)
Version: $version
Cflags: -I\${includedir}
Libs: \${libdir}/$static $native
EOF

if [ -n "$implib" ]; then
    shared_location="\${_sdrkit_prefix}/lib/$dynamic\"
        IMPORTED_IMPLIB \"\${_sdrkit_prefix}/lib/$implib"
else
    shared_location="\${_sdrkit_prefix}/lib/$dynamic"
fi

cat > "$dist/lib/cmake/sdrkit/sdrkitConfig.cmake" <<EOF
# The CMake package of sdrkit $version for $target.
#
#   find_package(sdrkit CONFIG REQUIRED)
#   target_link_libraries(app PRIVATE sdrkit::sdrkit)   # the static library and the system libraries it needs
#   target_link_libraries(app PRIVATE sdrkit::shared)   # the dynamic library
#
# SDRKIT_DATA_DIR is the data folder of the package, for the presets calls.

get_filename_component(_sdrkit_prefix "\${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
set(SDRKIT_INCLUDE_DIR "\${_sdrkit_prefix}/include")
set(SDRKIT_DATA_DIR "\${_sdrkit_prefix}/share/sdrkit/data")

if(NOT TARGET sdrkit::sdrkit)
    add_library(sdrkit::sdrkit STATIC IMPORTED)
    set_target_properties(sdrkit::sdrkit PROPERTIES
        IMPORTED_LOCATION "\${_sdrkit_prefix}/lib/$static"
        INTERFACE_INCLUDE_DIRECTORIES "\${SDRKIT_INCLUDE_DIR}"
        INTERFACE_LINK_LIBRARIES "$cmake_libs")
endif()

if(NOT TARGET sdrkit::shared)
    add_library(sdrkit::shared SHARED IMPORTED)
    set_target_properties(sdrkit::shared PROPERTIES
        IMPORTED_LOCATION "$shared_location"
        INTERFACE_INCLUDE_DIRECTORIES "\${SDRKIT_INCLUDE_DIR}")
endif()

set(sdrkit_FOUND TRUE)
EOF

major=${version%%.*}
minor=${version#*.}
minor=${minor%%.*}
cat > "$dist/lib/cmake/sdrkit/sdrkitConfigVersion.cmake" <<EOF
# sdrkit $version. A request is met at this version or below it with the same major version; before 1.0, with the
# same minor version too. The package is for 64-bit programs.
set(PACKAGE_VERSION "$version")
set(PACKAGE_VERSION_COMPATIBLE FALSE)
set(PACKAGE_VERSION_EXACT FALSE)
if(NOT PACKAGE_FIND_VERSION VERSION_GREATER PACKAGE_VERSION
   AND "\${PACKAGE_FIND_VERSION_MAJOR}" STREQUAL "$major"
   AND ("$major" STREQUAL "0" AND "\${PACKAGE_FIND_VERSION_MINOR}" STREQUAL "$minor" OR NOT "$major" STREQUAL "0"))
    set(PACKAGE_VERSION_COMPATIBLE TRUE)
endif()
if(PACKAGE_FIND_VERSION VERSION_EQUAL PACKAGE_VERSION)
    set(PACKAGE_VERSION_EXACT TRUE)
endif()
if(CMAKE_SIZEOF_VOID_P AND NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    set(PACKAGE_VERSION_UNSUITABLE TRUE)
endif()
EOF

echo "$dist"
