# Defines the imported targets of sdrkit for one folder of libraries. The source tree (CMakeLists.txt) and the
# installed package (sdrkitConfig.cmake) both use it.
#
#   sdrkit_import_targets(
#       LIBRARY_DIR <dir> [SHARED_DIR <dir>] INCLUDE_DIR <dir>
#       STATIC <file name> SHARED <file name> [IMPLIB <file name>] [SONAME <name>]
#       [SYSTEM_LIBRARIES <item>...] [GLOBAL] [EXISTING])
#
# Makes these targets:
#   sdrkit::static  the static library, with the system libraries that it needs
#   sdrkit::shared  the shared library; on Windows the DLL and its import library
#   sdrkit::sdrkit  an alias: sdrkit::shared if SDRKIT_SHARED is true, or else if BUILD_SHARED_LIBS is true;
#                   sdrkit::static if not
#
# SHARED_DIR is the folder of the shared library (on Windows the DLL) if it is not LIBRARY_DIR. GLOBAL makes the
# targets visible in every folder of the build. EXISTING makes a target only for a file that exists now: a package can
# hold only one of the two libraries. A target that exists already is kept.

if(COMMAND sdrkit_import_targets)
    return()
endif()

function(sdrkit_import_targets)
    cmake_parse_arguments(PARSE_ARGV 0 arg "GLOBAL;EXISTING"
        "LIBRARY_DIR;SHARED_DIR;INCLUDE_DIR;STATIC;SHARED;IMPLIB;SONAME" "SYSTEM_LIBRARIES")
    set(scope)
    if(NOT arg_SHARED_DIR)
        set(arg_SHARED_DIR "${arg_LIBRARY_DIR}")
    endif()
    if(arg_GLOBAL)
        set(scope GLOBAL)
    endif()

    if(NOT TARGET sdrkit::static AND (NOT arg_EXISTING OR EXISTS "${arg_LIBRARY_DIR}/${arg_STATIC}"))
        add_library(sdrkit::static STATIC IMPORTED ${scope})
        set_target_properties(sdrkit::static PROPERTIES
            IMPORTED_LOCATION "${arg_LIBRARY_DIR}/${arg_STATIC}"
            IMPORTED_LINK_INTERFACE_LANGUAGES C
            INTERFACE_INCLUDE_DIRECTORIES "${arg_INCLUDE_DIR}"
            INTERFACE_LINK_DIRECTORIES "${arg_LIBRARY_DIR}"
            INTERFACE_LINK_LIBRARIES "${arg_SYSTEM_LIBRARIES}")
    endif()

    if(NOT TARGET sdrkit::shared AND (NOT arg_EXISTING OR EXISTS "${arg_SHARED_DIR}/${arg_SHARED}"))
        add_library(sdrkit::shared SHARED IMPORTED ${scope})
        set_target_properties(sdrkit::shared PROPERTIES
            IMPORTED_LOCATION "${arg_SHARED_DIR}/${arg_SHARED}"
            INTERFACE_INCLUDE_DIRECTORIES "${arg_INCLUDE_DIR}")
        if(arg_IMPLIB)
            set_target_properties(sdrkit::shared PROPERTIES IMPORTED_IMPLIB "${arg_LIBRARY_DIR}/${arg_IMPLIB}")
        endif()
        if(arg_SONAME)
            set_target_properties(sdrkit::shared PROPERTIES IMPORTED_SONAME "${arg_SONAME}")
        endif()
    endif()

    if(NOT TARGET sdrkit::sdrkit)
        if(DEFINED SDRKIT_SHARED)
            set(shared "${SDRKIT_SHARED}")
        else()
            set(shared "${BUILD_SHARED_LIBS}")
        endif()
        if(shared AND TARGET sdrkit::shared)
            add_library(sdrkit::sdrkit ALIAS sdrkit::shared)
        elseif(TARGET sdrkit::static)
            add_library(sdrkit::sdrkit ALIAS sdrkit::static)
        elseif(TARGET sdrkit::shared)
            add_library(sdrkit::sdrkit ALIAS sdrkit::shared)
        endif()
    endif()
endfunction()
