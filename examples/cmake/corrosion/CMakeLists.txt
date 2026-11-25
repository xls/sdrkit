# A program that imports the crate sdrkit-host-ffi with Corrosion (https://github.com/corrosion-rs/corrosion, MIT)
# in place of the CMakeLists.txt of sdrkit. Corrosion runs cargo itself and adds the system libraries of the static
# library. sdrkit does not need Corrosion; this project shows how to use it if a project already does.
#
#   cmake -S . -B build -DSDRKIT_SOURCE_DIR=<sdrkit>
#   cmake --build build
#
# Corrosion builds in its own cargo target folder inside the build folder, so the first build compiles every crate
# again. It needs CMake 3.22 and a Rust tool chain.
cmake_minimum_required(VERSION 3.22)
project(sdrkit_corrosion LANGUAGES C)

include(FetchContent)
FetchContent_Declare(Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.1)
FetchContent_MakeAvailable(Corrosion)

set(SDRKIT_SOURCE_DIR "${CMAKE_CURRENT_LIST_DIR}/../../.." CACHE PATH "The source folder of sdrkit")

# The target `sdrkit` (the [lib] name of the crate) links the static library. CRATE_TYPES cdylib gives the shared
# library in its place.
corrosion_import_crate(
    MANIFEST_PATH "${SDRKIT_SOURCE_DIR}/Cargo.toml"
    CRATES sdrkit-host-ffi
    CRATE_TYPES staticlib
    PROFILE release
    FEATURES presets)
target_include_directories(sdrkit INTERFACE "${SDRKIT_SOURCE_DIR}/crates/hosts/sdrkit-host-ffi/include")

add_executable(stack_count "${CMAKE_CURRENT_LIST_DIR}/../common/stack_count.c")
target_link_libraries(stack_count PRIVATE sdrkit)
set_target_properties(stack_count PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON)
