# The C library of sdrkit for CMake projects. It builds the crate sdrkit-host-ffi with cargo and defines the targets
# sdrkit::sdrkit, sdrkit::static and sdrkit::shared. It works as a top-level project, through add_subdirectory and
# through FetchContent. A top-level build also installs the library as a CMake package (find_package(sdrkit CONFIG)),
# with pkg-config files.
#
# Options and cache variables:
#   BUILD_SHARED_LIBS        sdrkit::sdrkit is the shared library if true, the static library if not (the default).
#                            SDRKIT_SHARED, if it is set, takes the place of BUILD_SHARED_LIBS for sdrkit alone.
#   SDRKIT_PREBUILT_DIR      an unpacked release archive or an installed copy. The build uses its libraries and runs
#                            no cargo, so no Rust tool chain is needed.
#   SDRKIT_RUST_TARGET       the Rust target triple. The default comes from the CMake target: the host, one macOS
#                            architecture (CMAKE_OSX_ARCHITECTURES), iOS and its simulator, or the Android ABI.
#   SDRKIT_CARGO_PROFILE     the cargo profile: dev, release, shipped or dist. The default is dev for the build type
#                            Debug and release for the others. A multi-configuration generator uses one profile for
#                            every configuration.
#   SDRKIT_CARGO_TARGET_DIR  the target folder of cargo. The default is CARGO_TARGET_DIR of the environment, else
#                            target/ in this folder.
#   SDRKIT_FEATURES          the cargo features, a CMake list. The default is "presets" (the calls of sdrkit_presets.h).
#   SDRKIT_ALL_FAMILIES      build every stack family (the default features of the crate). OFF builds only the
#                            families named in SDRKIT_FEATURES.
#   SDRKIT_CARGO_ARGS        more arguments for cargo, a CMake list.
#   SDRKIT_INSTALL           make the install rules. The default is ON for a top-level project.
#
# Variables for the parent project: SDRKIT_INCLUDE_DIR, SDRKIT_DATA_DIR (the data folder for the presets calls),
# SDRKIT_RUST_TARGET.
cmake_minimum_required(VERSION 3.21)

# The version of the library is the version of the crate sdrkit-host-ffi.
file(STRINGS "${CMAKE_CURRENT_LIST_DIR}/crates/hosts/sdrkit-host-ffi/Cargo.toml" _sdrkit_version
    REGEX "^version *= *\"[^\"]+\"" LIMIT_COUNT 1)
if(NOT _sdrkit_version)
    file(STRINGS "${CMAKE_CURRENT_LIST_DIR}/Cargo.toml" _sdrkit_version REGEX "^version *= *\"[^\"]+\"" LIMIT_COUNT 1)
endif()
string(REGEX REPLACE "^version *= *\"([0-9]+\\.[0-9]+\\.[0-9]+).*$" "\\1" _sdrkit_version "${_sdrkit_version}")

project(sdrkit VERSION ${_sdrkit_version} DESCRIPTION "A software-defined radio engine" LANGUAGES C)

include(GNUInstallDirs)
include("${CMAKE_CURRENT_LIST_DIR}/cmake/sdrkitPlatform.cmake")
include("${CMAKE_CURRENT_LIST_DIR}/cmake/sdrkitTargets.cmake")

set(SDRKIT_PREBUILT_DIR "" CACHE PATH "An unpacked release archive of sdrkit; the build then runs no cargo")
option(SDRKIT_INSTALL "Make the install rules of sdrkit" ${PROJECT_IS_TOP_LEVEL})

set(_sdrkit_include "${CMAKE_CURRENT_LIST_DIR}/crates/hosts/sdrkit-host-ffi/include")

if(SDRKIT_PREBUILT_DIR)
    # The libraries of a package: its sdrkitConfig.cmake defines the targets, global so that the parent sees them.
    set(_sdrkit_config "${SDRKIT_PREBUILT_DIR}/lib/cmake/sdrkit/sdrkitConfig.cmake")
    if(NOT EXISTS "${_sdrkit_config}")
        message(FATAL_ERROR "sdrkit: ${_sdrkit_config} does not exist. SDRKIT_PREBUILT_DIR must be an unpacked "
            "release archive or an installed copy of sdrkit.")
    endif()
    set(SDRKIT_IMPORTED_GLOBAL TRUE)
    include("${_sdrkit_config}")
    if(NOT sdrkit_FOUND)
        message(FATAL_ERROR "sdrkit: ${sdrkit_NOT_FOUND_MESSAGE}")
    endif()
    message(STATUS "sdrkit ${PROJECT_VERSION}: the prebuilt libraries of ${SDRKIT_PREBUILT_DIR} "
        "(${SDRKIT_RUST_TARGET})")
    if(NOT PROJECT_IS_TOP_LEVEL)
        set(SDRKIT_INCLUDE_DIR "${SDRKIT_INCLUDE_DIR}" PARENT_SCOPE)
        set(SDRKIT_DATA_DIR "${SDRKIT_DATA_DIR}" PARENT_SCOPE)
        set(SDRKIT_RUST_TARGET "${SDRKIT_RUST_TARGET}" PARENT_SCOPE)
    endif()
    return()
endif()

# --- The build with cargo ---

find_program(SDRKIT_CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin"
    DOC "The cargo program")
if(NOT SDRKIT_CARGO)
    message(FATAL_ERROR "sdrkit: cargo was not found. Install Rust (https://rustup.rs), set SDRKIT_CARGO, or set "
        "SDRKIT_PREBUILT_DIR to an unpacked release archive.")
endif()

sdrkit_detect_rust_target(_sdrkit_detected)
set(SDRKIT_RUST_TARGET "${_sdrkit_detected}" CACHE STRING "The Rust target triple; empty for the host")
set(SDRKIT_CARGO_PROFILE "" CACHE STRING "The cargo profile; empty for dev (Debug) or release")
set(SDRKIT_CARGO_TARGET_DIR "" CACHE PATH "The target folder of cargo; empty for CARGO_TARGET_DIR or target/")
set(SDRKIT_FEATURES "presets" CACHE STRING "The cargo features of sdrkit-host-ffi, a CMake list")
option(SDRKIT_ALL_FAMILIES "Build every stack family of sdrkit" ON)
set(SDRKIT_CARGO_ARGS "" CACHE STRING "More arguments for cargo, a CMake list")

# The host triple names the files when no target is given, and tells whether the build cross-compiles.
get_filename_component(_sdrkit_cargo_bin "${SDRKIT_CARGO}" DIRECTORY)
find_program(SDRKIT_RUSTC rustc HINTS "${_sdrkit_cargo_bin}" DOC "The rustc program")
execute_process(COMMAND "${SDRKIT_RUSTC}" -vV OUTPUT_VARIABLE _sdrkit_rustc RESULT_VARIABLE _sdrkit_result)
if(NOT _sdrkit_result EQUAL 0)
    message(FATAL_ERROR "sdrkit: '${SDRKIT_RUSTC} -vV' failed")
endif()
string(REGEX MATCH "host: ([^\n]+)" _sdrkit_host "${_sdrkit_rustc}")
set(_sdrkit_host "${CMAKE_MATCH_1}")
if(SDRKIT_RUST_TARGET AND NOT SDRKIT_RUST_TARGET STREQUAL _sdrkit_host)
    set(_sdrkit_triple "${SDRKIT_RUST_TARGET}")
    set(_sdrkit_cross TRUE)
else()
    set(_sdrkit_triple "${_sdrkit_host}")
    set(_sdrkit_cross FALSE)
endif()

set(_sdrkit_profile "${SDRKIT_CARGO_PROFILE}")
get_property(_sdrkit_multi GLOBAL PROPERTY GENERATOR_IS_MULTI_CONFIG)
if(NOT _sdrkit_profile)
    if(NOT _sdrkit_multi AND CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(_sdrkit_profile dev)
    else()
        set(_sdrkit_profile release)
    endif()
endif()
# cargo writes the profile dev into the folder debug/.
if(_sdrkit_profile STREQUAL "dev")
    set(_sdrkit_profile_dir debug)
else()
    set(_sdrkit_profile_dir "${_sdrkit_profile}")
endif()

if(SDRKIT_CARGO_TARGET_DIR)
    set(_sdrkit_target_dir "${SDRKIT_CARGO_TARGET_DIR}")
elseif(DEFINED ENV{CARGO_TARGET_DIR})
    set(_sdrkit_target_dir "$ENV{CARGO_TARGET_DIR}")
else()
    set(_sdrkit_target_dir "${CMAKE_CURRENT_LIST_DIR}/target")
endif()
get_filename_component(_sdrkit_target_dir "${_sdrkit_target_dir}" ABSOLUTE BASE_DIR "${CMAKE_CURRENT_LIST_DIR}")
if(_sdrkit_cross)
    set(_sdrkit_built "${_sdrkit_target_dir}/${_sdrkit_triple}/${_sdrkit_profile_dir}")
else()
    set(_sdrkit_built "${_sdrkit_target_dir}/${_sdrkit_profile_dir}")
endif()

sdrkit_library_names("${_sdrkit_triple}" _sdrkit)
sdrkit_system_libraries("${_sdrkit_triple}" _sdrkit_system)
sdrkit_windows_import_libraries("${SDRKIT_CARGO}" "${CMAKE_CURRENT_LIST_DIR}/Cargo.toml" "${_sdrkit_triple}"
    _sdrkit_system)

# The cargo line. tools/package.sh and tools/package.ps1 use the same arguments, so that the builds share their
# results in one target folder. The link argument gives the shared library the name that a program records for it.
set(_sdrkit_cargo_args rustc --manifest-path "${CMAKE_CURRENT_LIST_DIR}/Cargo.toml"
    --target-dir "${_sdrkit_target_dir}" --profile "${_sdrkit_profile}" -p sdrkit-host-ffi --lib)
if(SDRKIT_FEATURES)
    list(JOIN SDRKIT_FEATURES "," _sdrkit_features)
    list(APPEND _sdrkit_cargo_args --features "${_sdrkit_features}")
endif()
if(NOT SDRKIT_ALL_FAMILIES)
    list(APPEND _sdrkit_cargo_args --no-default-features)
endif()
if(_sdrkit_cross)
    list(APPEND _sdrkit_cargo_args --target "${_sdrkit_triple}")
endif()
list(APPEND _sdrkit_cargo_args ${SDRKIT_CARGO_ARGS} -- --print native-static-libs)
if(_sdrkit_triple MATCHES "-apple-")
    list(APPEND _sdrkit_cargo_args -C "link-arg=-Wl,-install_name,@rpath/${_sdrkit_SHARED}")
elseif(_sdrkit_triple MATCHES "-linux-")
    list(APPEND _sdrkit_cargo_args -C "link-arg=-Wl,-soname,${_sdrkit_SHARED}")
endif()

# The environment of cargo: the deployment target of Apple systems, and the NDK compiler as the linker on Android.
set(_sdrkit_env)
if(CMAKE_OSX_DEPLOYMENT_TARGET)
    if(_sdrkit_triple MATCHES "-apple-ios")
        list(APPEND _sdrkit_env "IPHONEOS_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET}")
    elseif(_sdrkit_triple MATCHES "-apple-darwin")
        list(APPEND _sdrkit_env "MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET}")
    endif()
endif()
if(CMAKE_SYSTEM_NAME STREQUAL "Android" AND CMAKE_C_COMPILER)
    # The NDK has one compiler script for each target and API level, such as aarch64-linux-android24-clang.
    string(REPLACE "armv7-" "armv7a-" _sdrkit_ndk_triple "${_sdrkit_triple}")
    get_filename_component(_sdrkit_ndk_bin "${CMAKE_C_COMPILER}" DIRECTORY)
    set(_sdrkit_ndk_cc "${_sdrkit_ndk_bin}/${_sdrkit_ndk_triple}${CMAKE_SYSTEM_VERSION}-clang")
    if(CMAKE_HOST_WIN32)
        string(APPEND _sdrkit_ndk_cc ".cmd")
    endif()
    if(EXISTS "${_sdrkit_ndk_cc}")
        string(TOUPPER "${_sdrkit_triple}" _sdrkit_upper)
        string(REPLACE "-" "_" _sdrkit_upper "${_sdrkit_upper}")
        string(REPLACE "-" "_" _sdrkit_lower "${_sdrkit_triple}")
        list(APPEND _sdrkit_env "CARGO_TARGET_${_sdrkit_upper}_LINKER=${_sdrkit_ndk_cc}"
            "CC_${_sdrkit_lower}=${_sdrkit_ndk_cc}" "AR_${_sdrkit_lower}=${CMAKE_AR}")
    else()
        message(WARNING "sdrkit: ${_sdrkit_ndk_cc} does not exist. Set the linker of cargo for ${_sdrkit_triple} "
            "in the environment (CARGO_TARGET_<TRIPLE>_LINKER).")
    endif()
endif()

set(_sdrkit_outputs "${_sdrkit_built}/${_sdrkit_STATIC}" "${_sdrkit_built}/${_sdrkit_SHARED}")
if(_sdrkit_IMPLIB)
    list(APPEND _sdrkit_outputs "${_sdrkit_built}/${_sdrkit_IMPLIB}")
endif()

# cargo knows what changed, so the step runs on every build; it does nothing when the library is up to date.
add_custom_target(sdrkit_cargo ALL
    COMMAND "${CMAKE_COMMAND}" -E env ${_sdrkit_env} "${SDRKIT_CARGO}" ${_sdrkit_cargo_args}
    WORKING_DIRECTORY "${CMAKE_CURRENT_LIST_DIR}"
    BYPRODUCTS ${_sdrkit_outputs}
    USES_TERMINAL
    VERBATIM
    COMMENT "cargo: sdrkit-host-ffi (${_sdrkit_profile}, ${_sdrkit_triple})")

sdrkit_import_targets(
    LIBRARY_DIR "${_sdrkit_built}"
    INCLUDE_DIR "${_sdrkit_include}"
    STATIC "${_sdrkit_STATIC}"
    SHARED "${_sdrkit_SHARED}"
    IMPLIB "${_sdrkit_IMPLIB}"
    SONAME "${_sdrkit_SONAME}"
    SYSTEM_LIBRARIES ${_sdrkit_system}
    GLOBAL)
add_dependencies(sdrkit::static sdrkit_cargo)
add_dependencies(sdrkit::shared sdrkit_cargo)

set(SDRKIT_INCLUDE_DIR "${_sdrkit_include}")
set(SDRKIT_DATA_DIR "${CMAKE_CURRENT_LIST_DIR}/data")
if(NOT PROJECT_IS_TOP_LEVEL)
    set(SDRKIT_INCLUDE_DIR "${SDRKIT_INCLUDE_DIR}" PARENT_SCOPE)
    set(SDRKIT_DATA_DIR "${SDRKIT_DATA_DIR}" PARENT_SCOPE)
    set(SDRKIT_RUST_TARGET "${_sdrkit_triple}" PARENT_SCOPE)
endif()

# The system libraries of the static library, one per line, for examples/cmake/check.sh.
list(JOIN _sdrkit_system "\n" _sdrkit_system_lines)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/sdrkit-system-libraries.txt" "${_sdrkit_system_lines}\n")

message(STATUS "sdrkit ${PROJECT_VERSION}: cargo profile ${_sdrkit_profile}, target ${_sdrkit_triple}, "
    "libraries in ${_sdrkit_built}")

# --- Install: headers, libraries, data folder, CMake package and pkg-config files ---

if(NOT SDRKIT_INSTALL)
    return()
endif()

set(SDRKIT_INSTALL_CMAKEDIR "${CMAKE_INSTALL_LIBDIR}/cmake/sdrkit" CACHE STRING
    "The folder of sdrkitConfig.cmake, relative to the install prefix")
set(SDRKIT_INSTALL_DATADIR "${CMAKE_INSTALL_DATADIR}/sdrkit/data")

install(DIRECTORY "${_sdrkit_include}/" DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}" FILES_MATCHING PATTERN "*.h")
install(FILES "${_sdrkit_built}/${_sdrkit_STATIC}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
# A Windows DLL goes into bin/, beside the programs; its import library into lib/.
if(WIN32)
    install(PROGRAMS "${_sdrkit_built}/${_sdrkit_SHARED}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
    install(FILES "${_sdrkit_built}/${_sdrkit_IMPLIB}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
else()
    install(PROGRAMS "${_sdrkit_built}/${_sdrkit_SHARED}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
endif()
install(DIRECTORY "${CMAKE_CURRENT_LIST_DIR}/data/" DESTINATION "${SDRKIT_INSTALL_DATADIR}")
install(FILES "${CMAKE_CURRENT_LIST_DIR}/LICENSE" "${CMAKE_CURRENT_LIST_DIR}/THIRD_PARTY.md"
    DESTINATION "${CMAKE_INSTALL_DOCDIR}")

# The fields of the templates in cmake/. Paths in the package are relative to its prefix, so it can move.
set(SDRKIT_VERSION "${PROJECT_VERSION}")
set(SDRKIT_VERSION_MAJOR "${PROJECT_VERSION_MAJOR}")
set(SDRKIT_VERSION_MINOR "${PROJECT_VERSION_MINOR}")
set(SDRKIT_INSTALL_INCLUDEDIR "${CMAKE_INSTALL_INCLUDEDIR}")
set(SDRKIT_INSTALL_LIBDIR "${CMAKE_INSTALL_LIBDIR}")
if(WIN32)
    set(SDRKIT_INSTALL_SHAREDDIR "${CMAKE_INSTALL_BINDIR}")
else()
    set(SDRKIT_INSTALL_SHAREDDIR "${CMAKE_INSTALL_LIBDIR}")
endif()
set(SDRKIT_STATIC_NAME "${_sdrkit_STATIC}")
set(SDRKIT_SHARED_NAME "${_sdrkit_SHARED}")
set(SDRKIT_IMPLIB_NAME "${_sdrkit_IMPLIB}")
set(SDRKIT_SONAME "${_sdrkit_SONAME}")
set(SDRKIT_SIZEOF_VOID_P "${CMAKE_SIZEOF_VOID_P}")
# An installed package names the import libraries of the windows crates by file name; they are installed beside it.
set(_sdrkit_installed_system)
foreach(_sdrkit_item IN LISTS _sdrkit_system)
    if(IS_ABSOLUTE "${_sdrkit_item}")
        install(FILES "${_sdrkit_item}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
        get_filename_component(_sdrkit_item "${_sdrkit_item}" NAME)
    endif()
    list(APPEND _sdrkit_installed_system "${_sdrkit_item}")
endforeach()
set(SDRKIT_SYSTEM_LIBRARIES "${_sdrkit_installed_system}")
file(RELATIVE_PATH SDRKIT_CONFIG_TO_PREFIX "/prefix/${SDRKIT_INSTALL_CMAKEDIR}" "/prefix")
file(RELATIVE_PATH SDRKIT_PC_TO_PREFIX "/prefix/${CMAKE_INSTALL_LIBDIR}/pkgconfig" "/prefix")
if(_sdrkit_triple MATCHES "-windows-msvc")
    set(SDRKIT_PC_SHARED_LIBS "\${libdir}/${_sdrkit_IMPLIB}")
elseif(_sdrkit_triple MATCHES "-windows-gnu")
    set(SDRKIT_PC_SHARED_LIBS "\${libdir}/${_sdrkit_IMPLIB}")
else()
    set(SDRKIT_PC_SHARED_LIBS "-L\${libdir} -lsdrkit")
endif()
sdrkit_pkg_config_flags("${_sdrkit_installed_system}" SDRKIT_PC_SYSTEM_LIBS)

set(_sdrkit_gen "${CMAKE_CURRENT_BINARY_DIR}/sdrkit-package")
# A normal variable hides the cache variable SDRKIT_RUST_TARGET for the templates.
set(SDRKIT_RUST_TARGET "${_sdrkit_triple}")
configure_file(cmake/sdrkitConfig.cmake.in "${_sdrkit_gen}/sdrkitConfig.cmake" @ONLY)
configure_file(cmake/sdrkitConfigVersion.cmake.in "${_sdrkit_gen}/sdrkitConfigVersion.cmake" @ONLY)
configure_file(cmake/sdrkit.pc.in "${_sdrkit_gen}/sdrkit.pc" @ONLY)
configure_file(cmake/sdrkit-static.pc.in "${_sdrkit_gen}/sdrkit-static.pc" @ONLY)
unset(SDRKIT_RUST_TARGET)
install(FILES "${_sdrkit_gen}/sdrkitConfig.cmake" "${_sdrkit_gen}/sdrkitConfigVersion.cmake"
    cmake/sdrkitTargets.cmake DESTINATION "${SDRKIT_INSTALL_CMAKEDIR}")
install(FILES "${_sdrkit_gen}/sdrkit.pc" "${_sdrkit_gen}/sdrkit-static.pc"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig")
