# A program that builds sdrkit from its sources with cargo, through add_subdirectory. FetchContent works the same:
#
#   include(FetchContent)
#   FetchContent_Declare(sdrkit GIT_REPOSITORY https://github.com/xls/sdrkit.git GIT_TAG v0.5.0)
#   FetchContent_MakeAvailable(sdrkit)
#
#   cmake -S . -B build -DSDRKIT_SOURCE_DIR=<sdrkit> [-DBUILD_SHARED_LIBS=ON]
cmake_minimum_required(VERSION 3.21)
project(sdrkit_add_subdirectory LANGUAGES C)

set(SDRKIT_SOURCE_DIR "${CMAKE_CURRENT_LIST_DIR}/../../.." CACHE PATH "The source folder of sdrkit")
add_subdirectory("${SDRKIT_SOURCE_DIR}" sdrkit)

include("${CMAKE_CURRENT_LIST_DIR}/../common/consumer.cmake")
