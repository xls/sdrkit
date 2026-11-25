<#
Builds the C library of sdrkit on Windows with the cargo profile `shipped` (release code without debug information)
and lays out an installed copy of it, as tools/package.sh does on other systems:

  <Out>\sdrkit-<version>-<Target>\
    include\                 sdrkit.h, sdrkit\<family>.h, sdrkit\init.h, sdrkit_presets.h
    lib\                     sdrkit.dll, its import library, the static library and the import library of the
                             windows crates that the static library names
    lib\pkgconfig\           sdrkit.pc (the shared library), and for MinGW sdrkit-static.pc
    lib\cmake\sdrkit\        sdrkitConfig.cmake, sdrkitConfigVersion.cmake, sdrkitTargets.cmake
    share\sdrkit\data\       a copy of the folder data\ of the repository
    share\doc\sdrkit\        the licence files

With -Release the script also builds the command-line tool into bin\, puts LICENSE and THIRD_PARTY.md at the top of
the folder, names the folder sdrkit-<version>-windows-x86_64 (windows-x86_64-gnu for MinGW), and packs it into
<Out>\sdrkit-<version>-windows-x86_64.zip.

Usage, from any folder:
  powershell -ExecutionPolicy Bypass -File tools\package.ps1
  powershell -ExecutionPolicy Bypass -File tools\package.ps1 -Release
  powershell -ExecutionPolicy Bypass -File tools\package.ps1 -Target x86_64-pc-windows-gnu
  -Target  x86_64-pc-windows-msvc (the default on an MSVC tool chain) or x86_64-pc-windows-gnu (MinGW). A target that
           is not the host needs `rustup target add <Target>` and its linker.
  -Out     the folder for the result. The default is dist\ at the root of the workspace.

The script prints the folder of the copy on its last line. The build uses CARGO_TARGET_DIR if it is set, else target\
of the workspace. The cargo arguments are those of CMakeLists.txt with the profile `shipped`.
#>
param(
    [string]$Target = "",
    [string]$Out = "",
    [switch]$Release
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$ffi = Join-Path $root "crates\hosts\sdrkit-host-ffi"
$templates = Join-Path $root "cmake"
$hostTriple = ((rustc -vV) | Where-Object { $_ -like "host:*" }) -replace "^host:\s*", ""
if (-not $Target) { $Target = $hostTriple }
if (-not $Out) { $Out = Join-Path $root "dist" }
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root "target" }
$versionLine = Select-String -Path (Join-Path $ffi "Cargo.toml") -Pattern '^version *= *"(.*)"$' |
    Select-Object -First 1
if (-not $versionLine) {
    $versionLine = Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version *= *"(.*)"$' |
        Select-Object -First 1
}
$version = $versionLine.Matches[0].Groups[1].Value

# The file names that cargo gives the libraries on the target, and the name of the platform in a release archive.
switch -Wildcard ($Target) {
    "*-windows-msvc" { $static = "sdrkit.lib"; $dynamic = "sdrkit.dll"; $implib = "sdrkit.dll.lib"; $suffix = "" }
    "*-windows-gnu*" { $static = "libsdrkit.a"; $dynamic = "sdrkit.dll"; $implib = "libsdrkit.dll.a"; $suffix = "-gnu" }
    default { throw "tools\package.ps1 builds the Windows targets. Use tools/package.sh for $Target." }
}
switch -Wildcard ($Target) {
    "x86_64-*" { $platform = "windows-x86_64$suffix" }
    "aarch64-*" { $platform = "windows-arm64$suffix" }
    default { $platform = $Target }
}
$name = if ($Release) { "sdrkit-$version-$platform" } else { "sdrkit-$version-$Target" }
$dist = Join-Path $Out $name

# One build makes both libraries. `--print native-static-libs` names the system libraries that the static library
# needs; cargo shows the note again on a build that has nothing to do.
$cross = @()
if ($Target -ne $hostTriple) {
    $cross = @("--target", $Target)
    $built = Join-Path $targetDir "$Target\shipped"
} else {
    $built = Join-Path $targetDir "shipped"
}
$cargoArgs = @("rustc", "--profile", "shipped", "-p", "sdrkit-host-ffi", "--lib", "--features", "presets") + $cross +
    @("--", "--print", "native-static-libs")

# Runs cargo with `arguments` and returns its output lines; throws if cargo fails.
function Invoke-Cargo([string[]]$arguments) {
    Push-Location $root
    $ErrorActionPreference = "Continue"
    $lines = & cargo @arguments 2>&1 | ForEach-Object { "$_" }
    $code = $LASTEXITCODE
    $ErrorActionPreference = "Stop"
    Pop-Location
    if ($code -ne 0) {
        $lines | Write-Host
        throw "cargo failed with code $code"
    }
    return $lines
}

Write-Host "building sdrkit $version for $Target"
$log = Invoke-Cargo $cargoArgs
$found = $log | Select-String -Pattern "native-static-libs: (.*)$" | Select-Object -Last 1
if (-not $found) { throw "cargo printed no native-static-libs line" }
$native = $found.Matches[0].Groups[1].Value.Trim()
if ($Release) {
    Write-Host "building the command-line tool for $Target"
    Invoke-Cargo (@("build", "--profile", "shipped", "-p", "sdrkit-host-cli") + $cross) | Out-Null
}

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
if ($Release) {
    New-Item -ItemType Directory -Force -Path (Join-Path $dist "bin") | Out-Null
    Copy-Item (Join-Path $built "sdrkit.exe") (Join-Path $dist "bin\sdrkit.exe")
    Copy-Item (Join-Path $root "LICENSE"), (Join-Path $root "THIRD_PARTY.md") $dist
}

# The static library names the import library of the crate windows_<arch>_msvc or windows_<arch>_gnu (such as
# windows.0.52.0.lib). The linker must find it, so it goes into lib\ beside the library.
if ($native -match "windows\.[0-9]") {
    # The metadata is read with a pattern, not as JSON: it is several megabytes long.
    $metadata = (Invoke-Cargo @("metadata", "--format-version", "1", "--filter-platform", $Target, "--manifest-path",
        (Join-Path $root "Cargo.toml"))) -join "`n"
    $pattern = '"manifest_path":"([^"]*?windows_[a-z0-9_]+_(?:msvc|gnu|gnullvm)-[0-9][^"]*?)[\\/]+Cargo\.toml"'
    $filter = if ($Target -like "*-msvc") { "windows*.lib" } else { "libwindows*.a" }
    foreach ($match in [regex]::Matches($metadata, $pattern)) {
        $libDir = Join-Path ($match.Groups[1].Value -replace '\\\\', '\') "lib"
        Get-ChildItem -Path $libDir -Filter $filter -ErrorAction SilentlyContinue |
            ForEach-Object { Copy-Item $_.FullName (Join-Path $dist "lib") }
    }
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

# Fills the template `template` of cmake\ into the file `path`, as UTF-8 without a byte order mark and with Unix line
# ends: each @KEY@ becomes its value in `values`. CMakeLists.txt fills the same templates for `cmake --install`.
function Write-Template([string]$template, [string]$path, [hashtable]$values) {
    $text = [System.IO.File]::ReadAllText((Join-Path $templates $template))
    foreach ($key in $values.Keys) { $text = $text.Replace("@$key@", [string]$values[$key]) }
    [System.IO.File]::WriteAllText($path, ($text -replace "`r`n", "`n"), (New-Object System.Text.UTF8Encoding $false))
}
$values = @{
    SDRKIT_VERSION = $version; SDRKIT_VERSION_MAJOR = $version.Split(".")[0]
    SDRKIT_VERSION_MINOR = $version.Split(".")[1]; SDRKIT_RUST_TARGET = $Target
    SDRKIT_CONFIG_TO_PREFIX = "../../.."; SDRKIT_PC_TO_PREFIX = "../.."
    SDRKIT_INSTALL_INCLUDEDIR = "include"; SDRKIT_INSTALL_LIBDIR = "lib"; SDRKIT_INSTALL_SHAREDDIR = "lib"
    SDRKIT_INSTALL_DATADIR = "share/sdrkit/data"
    SDRKIT_STATIC_NAME = $static; SDRKIT_SHARED_NAME = $dynamic; SDRKIT_IMPLIB_NAME = $implib; SDRKIT_SONAME = ""
    SDRKIT_SIZEOF_VOID_P = "8"; SDRKIT_SYSTEM_LIBRARIES = $cmakeLibs
    SDRKIT_PC_SHARED_LIBS = "`${libdir}/$implib"; SDRKIT_PC_SYSTEM_LIBS = $native
}
$cmakeDir = Join-Path $dist "lib\cmake\sdrkit"
Write-Template "sdrkitConfig.cmake.in" (Join-Path $cmakeDir "sdrkitConfig.cmake") $values
Write-Template "sdrkitConfigVersion.cmake.in" (Join-Path $cmakeDir "sdrkitConfigVersion.cmake") $values
Copy-Item (Join-Path $templates "sdrkitTargets.cmake") $cmakeDir
# pkg-config is for MinGW: sdrkit.pc links the import library, which works for a DLL of either tool chain.
Write-Template "sdrkit.pc.in" (Join-Path $dist "lib\pkgconfig\sdrkit.pc") $values
if ($Target -like "*-windows-gnu*") {
    Write-Template "sdrkit-static.pc.in" (Join-Path $dist "lib\pkgconfig\sdrkit-static.pc") $values
}

if ($Release) {
    $archive = Join-Path $Out "$name.zip"
    if (Test-Path $archive) { Remove-Item -Force $archive }
    Compress-Archive -Path $dist -DestinationPath $archive
    Write-Host "wrote $archive"
}
Write-Output $dist
