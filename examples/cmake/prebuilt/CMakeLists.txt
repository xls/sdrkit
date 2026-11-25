# A program that uses the sdrkit source tree with the libraries of an unpacked release archive
# (SDRKIT_PREBUILT_DIR). The build runs no cargo and needs no Rust tool chain.
#
#   cmake -S . -B build -DSDRKIT_SOURCE_DIR=<sdrkit> -DSDRKIT_PREBUILT_DIR=<archive folder> [-DBUILD_SHARED_LIBS=ON]
cmake_minimum_required(VERSION 3.21)
project(sdrkit_prebuilt LANGUAGES C)

set(SDRKIT_SOURCE_DIR "${CMAKE_CURRENT_LIST_DIR}/../../.." CACHE PATH "The source folder of sdrkit")
if(NOT SDRKIT_PREBUILT_DIR)
    message(FATAL_ERROR "Set SDRKIT_PREBUILT_DIR to an unpacked release archive of sdrkit.")
endif()
add_subdirectory("${SDRKIT_SOURCE_DIR}" sdrkit)

include("${CMAKE_CURRENT_LIST_DIR}/../common/consumer.cmake")
