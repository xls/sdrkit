<#
Builds the C library of sdrkit on Windows with the cargo profile `shipped` (release code without debug information)
and lays out an installed copy of it, as tools/package.sh does on other systems:

  <Out>\sdrkit-<version>-<Target>\
    include\                 sdrkit.h, sdrkit\<family>.h, sdrkit\init.h, sdrkit_presets.h
    lib\                     sdrkit.dll, its import library and the static library
    lib\pkgconfig\           sdrkit.pc (the dynamic library), and for MinGW sdrkit-static.pc
    lib\cmake\sdrkit\        sdrkitConfig.cmake, sdrkitConfigVersion.cmake
    share\sdrkit\data\       a copy of the folder data\ of the repository
    share\doc\sdrkit\        the licence files

Usage, from any folder:
  powershell -ExecutionPolicy Bypass -File tools\package.ps1
  powershell -ExecutionPolicy Bypass -File tools\package.ps1 -Target x86_64-pc-windows-gnu
  -Target  x86_64-pc-windows-msvc (the default on an MSVC tool chain) or x86_64-pc-windows-gnu (MinGW). A target that
           is not the host needs `rustup target add <Target>` and its linker.
  -Out     the folder for the result. The default is dist\ at the root of the workspace.

The script prints the folder of the package on its last line.
#>
param(
    [string]$Target = "",
    [string]$Out = ""
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ffi = Join-Path $root "crates\hosts\sdrkit-host-ffi"
$hostTriple = ((rustc -vV) | Where-Object { $_ -like "host:*" }) -replace "^host:\s*", ""
if (-not $Target) { $Target = $hostTriple }
if (-not $Out) { $Out = Join-Path $root "dist" }
$version = (Select-String -Path (Join-Path $ffi "Cargo.toml") -Pattern '^version = "(.*)"$' |
    Select-Object -First 1).Matches[0].Groups[1].Value
$dist = Join-Path $Out "sdrkit-$version-$Target"

# The file names that cargo gives the libraries on the target.
switch -Wildcard ($Target) {
    "*-windows-msvc" { $static = "sdrkit.lib"; $dynamic = "sdrkit.dll"; $implib = "sdrkit.dll.lib" }
    "*-windows-gnu*" { $static = "libsdrkit.a"; $dynamic = "sdrkit.dll"; $implib = "libsdrkit.dll.a" }
    default { throw "tools\package.ps1 builds the Windows targets. Use tools/package.sh for $Target." }
}

# One build makes both libraries. `--print native-static-libs` names the system libraries that the static library
# needs; cargo shows the note again on a build that has nothing to do.
$cargoArgs = @("rustc", "--profile", "shipped", "-p", "sdrkit-host-ffi", "--features", "presets", "--lib",
    "--crate-type", "staticlib", "--crate-type", "cdylib")
if ($Target -ne $hostTriple) {
    $cargoArgs += @("--target", $Target)
    $built = Join-Path $root "target\$Target\shipped"
} else {
    $built = Join-Path $root "target\shipped"
}
$cargoArgs += @("--", "--print", "native-static-libs")

Write-Host "building sdrkit $version for $Target"
Push-Location $root
$ErrorActionPreference = "Continue"
$log = & cargo @cargoArgs 2>&1 | ForEach-Object { "$_" }
$code = $LASTEXITCODE
$ErrorActionPreference = "Stop"
Pop-Location
if ($code -ne 0) {
    $log | Write-Host
    throw "cargo failed with code $code"
}
$found = $log | Select-String -Pattern "native-static-libs: (.*)$" | Select-Object -Last 1
if (-not $found) { throw "cargo printed no native-static-libs line" }
$native = $found.Matches[0].Groups[1].Value.Trim()

if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
foreach ($d in @("include\sdrkit", "lib\pkgconfig", "lib\cmake\sdrkit", "share\sdrkit", "share\doc\sdrkit")) {
    New-Item -ItemType Directory -Force -Path (Join-Path $dist $d) | Out-Null
}
Copy-Item (Join-Path $ffi "include\sdrkit.h"), (Join-Path $ffi "include\sdrkit_presets.h") (Join-Path $dist "include")
Copy-Item (Join-Path $ffi "include\sdrkit\*.h") (Join-Path $dist "include\sdrkit")
Copy-Item -Recurse (Join-Path $root "data") (Join-Path $dist "share\sdrkit\data")
Copy-Item (Join-Path $root "LICENSE"), (Join-Path $root "THIRD_PARTY.md") (Join-Path $dist "share\doc\sdrkit")
foreach ($f in @($static, $dynamic, $implib)) {
    Copy-Item (Join-Path $built $f) (Join-Path $dist "lib\$f")
}

# The system libraries as CMake list items: `-lws2_32` becomes `ws2_32`, a `.lib` name stays as it is, and
# `/defaultlib:` is left to the compiler.
$cmakeLibs = @()
foreach ($word in ($native -split "\s+")) {
    if ($word -like "-l*") { $cmakeLibs += $word.Substring(2) }
    elseif ($word -like "/defaultlib:*" -or $word -eq "") { }
    else { $cmakeLibs += $word }
}
$cmakeLibs = $cmakeLibs -join ";"

# Writes `text` to `path` as UTF-8 without a byte order mark, with Unix line ends. Each `@KEY@` of the text is
# replaced by the value of KEY in `values`.
function Write-Text([string]$path, [string]$text, [hashtable]$values) {
    foreach ($key in $values.Keys) { $text = $text.Replace("@$key@", [string]$values[$key]) }
    [System.IO.File]::WriteAllText($path, ($text -replace "`r`n", "`n"), (New-Object System.Text.UTF8Encoding $false))
}
$values = @{
    VERSION = $version; TARGET = $Target; STATIC = $static; DYNAMIC = $dynamic; IMPLIB = $implib
    NATIVE = $native; CMAKE_LIBS = $cmakeLibs; MAJOR = $version.Split(".")[0]; MINOR = $version.Split(".")[1]
}

# pkg-config is for MinGW: it links the import library, which works for a DLL of either tool chain.
$pc = @'
prefix=${pcfiledir}/../..
includedir=${prefix}/include
libdir=${prefix}/lib
datadir=${prefix}/share/sdrkit/data

Name: sdrkit
Description: A software-defined radio engine (dynamic library)
Version: @VERSION@
Cflags: -I${includedir}
Libs: ${libdir}/@IMPLIB@
'@
Write-Text (Join-Path $dist "lib\pkgconfig\sdrkit.pc") $pc $values

if ($Target -like "*-windows-gnu*") {
    $pcStatic = @'
prefix=${pcfiledir}/../..
includedir=${prefix}/include
libdir=${prefix}/lib
datadir=${prefix}/share/sdrkit/data

Name: sdrkit-static
Description: A software-defined radio engine (static library)
Version: @VERSION@
Cflags: -I${includedir}
Libs: ${libdir}/@STATIC@ @NATIVE@
'@
    Write-Text (Join-Path $dist "lib\pkgconfig\sdrkit-static.pc") $pcStatic $values
}

$config = @'
# The CMake package of sdrkit @VERSION@ for @TARGET@.
#
#   find_package(sdrkit CONFIG REQUIRED)
#   target_link_libraries(app PRIVATE sdrkit::sdrkit)   # the static library and the system libraries it needs
#   target_link_libraries(app PRIVATE sdrkit::shared)   # the DLL, through its import library
#
# SDRKIT_DATA_DIR is the data folder of the package, for the presets calls. A program that links sdrkit::shared finds
# sdrkit.dll in its own folder or on PATH.

get_filename_component(_sdrkit_prefix "${CMAKE_CURRENT_LIST_DIR}/../../.." ABSOLUTE)
set(SDRKIT_INCLUDE_DIR "${_sdrkit_prefix}/include")
set(SDRKIT_DATA_DIR "${_sdrkit_prefix}/share/sdrkit/data")

if(NOT TARGET sdrkit::sdrkit)
    add_library(sdrkit::sdrkit STATIC IMPORTED)
    set_target_properties(sdrkit::sdrkit PROPERTIES
        IMPORTED_LOCATION "${_sdrkit_prefix}/lib/@STATIC@"
        INTERFACE_INCLUDE_DIRECTORIES "${SDRKIT_INCLUDE_DIR}"
        INTERFACE_LINK_LIBRARIES "@CMAKE_LIBS@")
endif()

if(NOT TARGET sdrkit::shared)
    add_library(sdrkit::shared SHARED IMPORTED)
    set_target_properties(sdrkit::shared PROPERTIES
        IMPORTED_LOCATION "${_sdrkit_prefix}/lib/@DYNAMIC@"
        IMPORTED_IMPLIB "${_sdrkit_prefix}/lib/@IMPLIB@"
        INTERFACE_INCLUDE_DIRECTORIES "${SDRKIT_INCLUDE_DIR}")
endif()

set(sdrkit_FOUND TRUE)
'@
Write-Text (Join-Path $dist "lib\cmake\sdrkit\sdrkitConfig.cmake") $config $values

$configVersion = @'
# sdrkit @VERSION@. A request is met at this version or below it with the same major version; before 1.0, with the
# same minor version too. The package is for 64-bit programs.
set(PACKAGE_VERSION "@VERSION@")
set(PACKAGE_VERSION_COMPATIBLE FALSE)
set(PACKAGE_VERSION_EXACT FALSE)
if(NOT PACKAGE_FIND_VERSION VERSION_GREATER PACKAGE_VERSION
   AND "${PACKAGE_FIND_VERSION_MAJOR}" STREQUAL "@MAJOR@"
   AND ("@MAJOR@" STREQUAL "0" AND "${PACKAGE_FIND_VERSION_MINOR}" STREQUAL "@MINOR@" OR NOT "@MAJOR@" STREQUAL "0"))
    set(PACKAGE_VERSION_COMPATIBLE TRUE)
endif()
if(PACKAGE_FIND_VERSION VERSION_EQUAL PACKAGE_VERSION)
    set(PACKAGE_VERSION_EXACT TRUE)
endif()
if(CMAKE_SIZEOF_VOID_P AND NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    set(PACKAGE_VERSION_UNSUITABLE TRUE)
endif()
'@
Write-Text (Join-Path $dist "lib\cmake\sdrkit\sdrkitConfigVersion.cmake") $configVersion $values

Write-Output $dist
