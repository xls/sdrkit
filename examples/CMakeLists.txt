# The C and C++ examples of sdrkit, built against an installed copy of the library (tools/package.sh or
# tools/package.ps1 makes one). Give its folder with -DCMAKE_PREFIX_PATH=<folder>.
#
# Options:
#   SDRKIT_EXAMPLES_STATIC  link the static library and its system libraries (OFF: the dynamic library)
#   SDRKIT_EXAMPLES_SDL     also build the programs that play sound through SDL 3
cmake_minimum_required(VERSION 3.16)
project(sdrkit_examples LANGUAGES C CXX)

set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

option(SDRKIT_EXAMPLES_STATIC "Link the static library of sdrkit" OFF)
option(SDRKIT_EXAMPLES_SDL "Build the examples that play sound through SDL 3" OFF)

find_package(sdrkit CONFIG REQUIRED)
if(SDRKIT_EXAMPLES_STATIC)
    set(sdrkit_library sdrkit::sdrkit)
else()
    set(sdrkit_library sdrkit::shared)
endif()
if(SDRKIT_EXAMPLES_SDL)
    find_package(SDL3 CONFIG REQUIRED)
endif()

# Builds the program `name` from `source`, linked with the library and `extra`.
function(sdrkit_example name source)
    add_executable(${name} ${source})
    target_link_libraries(${name} PRIVATE ${sdrkit_library} ${ARGN})
    target_compile_definitions(${name} PRIVATE "SDRKIT_DATA_DIR=\"${SDRKIT_DATA_DIR}\"")
    if(MSVC)
        target_compile_options(${name} PRIVATE /W4 /D_CRT_SECURE_NO_WARNINGS)
    else()
        target_compile_options(${name} PRIVATE -Wall -Wextra
            $<$<COMPILE_LANGUAGE:CXX>:-Wno-missing-field-initializers>)
    endif()
    # A program finds the dynamic library next to it on Windows, and through its run path elsewhere.
    if(WIN32 AND NOT SDRKIT_EXAMPLES_STATIC)
        add_custom_command(TARGET ${name} POST_BUILD
            COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_FILE:sdrkit::shared> $<TARGET_FILE_DIR:${name}>)
    endif()
endfunction()

sdrkit_example(fm_audio c/fm_audio.c)
sdrkit_example(list_stacks c/list_stacks.c)
sdrkit_example(adsb_table c/adsb_table.c)
sdrkit_example(meshtastic c/meshtastic.c)
sdrkit_example(dvbt_services c/dvbt_services.c)
sdrkit_example(scan_band c/scan_band.c)
sdrkit_example(settings_save c/settings_save.c)
sdrkit_example(fm_audio_cpp cpp/fm_audio.cpp)
sdrkit_example(any_stack cpp/any_stack.cpp)

if(SDRKIT_EXAMPLES_SDL)
    sdrkit_example(fm_play_sdl c/fm_play_sdl.c SDL3::SDL3)
    sdrkit_example(dvbt_play_sdl c/dvbt_play_sdl.c SDL3::SDL3)
endif()
