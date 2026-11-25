# A program that links an installed copy of sdrkit: `cmake --install` of the sdrkit project, or an unpacked release
# archive. It needs no Rust tool chain.
#
#   cmake -S . -B build -DCMAKE_PREFIX_PATH=<prefix> [-DBUILD_SHARED_LIBS=ON]
cmake_minimum_required(VERSION 3.21)
project(sdrkit_find_package LANGUAGES C)

find_package(sdrkit CONFIG REQUIRED)
message(STATUS "sdrkit ${sdrkit_VERSION} for ${SDRKIT_RUST_TARGET} in ${SDRKIT_INCLUDE_DIR}")

include("${CMAKE_CURRENT_LIST_DIR}/../common/consumer.cmake")
