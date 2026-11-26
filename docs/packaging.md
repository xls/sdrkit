# Packaging on Windows and Linux

Nothing on this page was run on Windows or Linux; it comes from the files of the repository, and the lines marked "not confirmed" name what those files do not show.

## What gets built

- `sdrkit`, the console tool (crate `sdrkit-host-cli`). See [cli.md](cli.md).
- `sdrkit-tui`, the terminal application (crate `sdrkit-host-tui`). It links the C library as a Rust library. See [tui.md](tui.md).
- The C library (crate `sdrkit-host-ffi`, library name `sdrkit`): a static and a dynamic library. The `Cargo.toml` of the crate lists `staticlib` and `rlib`; `tools/package.sh` and `tools/package.ps1` add the dynamic library (`cdylib`) on the command line.
- The header `sdrkit.h`, the family headers `sdrkit/<family>.h` and `sdrkit_presets.h`, all in `crates/hosts/sdrkit-host-ffi/include/`. They are in git. `cbindgen` wrote `sdrkit.h`; the command is in `cbindgen.toml` of the crate. The build does not write them again.
- The data folder `data/` of the repository: plans, presets and station lists that a program reads at run time. `data/README.md` describes it.

## Prerequisites

Install these on each platform:

- Rust 1.92 or later (`rust-version` in the workspace `Cargo.toml`). The repository has no `rust-toolchain` file. Install it with rustup.
- A C compiler. Some dependencies compile C code (`cc` and `ring` are in `Cargo.lock`).
- `git` to fetch the sources.

Windows: install the Build Tools for Visual Studio 2022 with the workload "Desktop development with C++", and the Rust toolchain `stable-x86_64-pc-windows-msvc`. For MinGW use the target `x86_64-pc-windows-gnu`; see [building.md](building.md). No other library is needed.

Linux: install `gcc` (or `clang`), `make`, `pkg-config` and the development files of ALSA. The crate `alsa-sys` needs them for sound output (`cpal`).

| System | Command |
|---|---|
| Debian, Ubuntu | `sudo apt install build-essential pkg-config libasound2-dev` |
| Fedora | `sudo dnf install gcc gcc-c++ make pkgconf-pkg-config alsa-lib-devel` |

A program that is built reads the ALSA library at run time: the package is `libasound2` (Debian, Ubuntu) or `alsa-lib` (Fedora).

### System libraries by crate

| Crate | Needs |
|---|---|
| `alsa-sys` | ALSA (`libasound`) and `pkg-config`, on Linux only. |
| `windows-sys` | Nothing: bindings to the Windows system libraries. |
| `rustls`, `ring`, `socket2` | Nothing installed. `ring` and `cc` need the C compiler. |
| `coreaudio-sys`, `clang-sys` | macOS only. `clang-sys` needs libclang there. Not used on Windows or Linux. |
| `oboe-sys`, `ndk-sys`, `jni-sys` | Android only. Not used on Windows or Linux. |

### Licences

`cargo metadata` shows no GPL or LGPL dependency that is required. One crate offers LGPL as a choice: `r-efi` is `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. It is used by `getrandom` only for the target `uefi`, and MIT or Apache-2.0 applies. Every other licence field is MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense, 0BSD, BlueOak or BSL-1.0, alone or in a choice. `THIRD_PARTY.md` holds the notices that the licences ask for.

## Build

Use the profile `shipped`. It is `release` without debug information (`debug = 0`, `strip = "debuginfo"`). `release` itself keeps `debug = 1` for profiling. The profile `dist` adds whole-program optimisation and builds slowly; the project uses it for the iOS library.

Build the tools, on Windows (PowerShell) and on Linux alike:

```sh
cargo build --profile shipped -p sdrkit-host-cli -p sdrkit-host-tui
```

Build the C library and lay out an installed copy, on Linux:

```sh
sh tools/package.sh
```

On Windows:

```powershell
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

The script builds `sdrkit-host-ffi` with the feature `presets` and both library types. This is the command it runs, if you want the libraries alone:

```sh
cargo rustc --profile shipped -p sdrkit-host-ffi --features presets --lib --crate-type staticlib --crate-type cdylib
```

The library holds every stack family. To build a smaller one, use `--no-default-features` and name the families as features (`fm`, `general`, `adsb`, and so on).

## Outputs

Cargo writes to `target/shipped/` for the host target, and to `target/<triple>/shipped/` when you pass `--target`.

| File | Windows (MSVC) | Linux |
|---|---|---|
| Console tool | `sdrkit.exe` | `sdrkit` |
| Terminal application | `sdrkit-tui.exe` | `sdrkit-tui` |
| Static library | `sdrkit.lib` | `libsdrkit.a` |
| Dynamic library | `sdrkit.dll` | `libsdrkit.so` |
| Import library | `sdrkit.dll.lib` | none |

With MinGW the names are `libsdrkit.a`, `sdrkit.dll` and `libsdrkit.dll.a`. The script puts the package in `dist/sdrkit-<version>-<triple>/`.

## Archive layout

The script writes this layout for the C library:

```
include/                 sdrkit.h, sdrkit_presets.h, sdrkit/<family>.h
lib/                     static library, dynamic library, import library
lib/pkgconfig/           sdrkit.pc, sdrkit-static.pc
lib/cmake/sdrkit/        sdrkitConfig.cmake, sdrkitConfigVersion.cmake
share/sdrkit/data/       a copy of data/
share/doc/sdrkit/        LICENSE, THIRD_PARTY.md
```

With `--release` (`-Release` on Windows) the script also builds `sdrkit` into `bin/`, puts `LICENSE` and
`THIRD_PARTY.md` at the top, and packs the folder into `sdrkit-<version>-<os>-<arch>.tar.gz` (`.zip` on Windows). It
does not copy `sdrkit-tui`. To ship it too, copy `sdrkit-tui` (with `.exe` on Windows) from `target/shipped/` into
`bin/` and pack the folder again.

Keep `data/` as `share/sdrkit/data/`. A tool that has no `--data` option given looks in these folders, in this order, and takes the first that holds a `plans` folder: `data` in the storage folder, `~/.sdrkit/data`, then for each folder above the program file `data` and `share/sdrkit/data`, then `data` in the current folder. A program in `bin/` therefore finds `share/sdrkit/data` of the same archive. A program of the C library gets its data folder from the path that its code passes to `sdrkit_presets_open`; the package sets `SDRKIT_DATA_DIR` for CMake. See [building.md](building.md).

The tools keep their settings in the storage folder `~/.sdrkit`. `--storage <folder>` sets another one.

## Check the result

Run this with the test radio. It needs no hardware:

```sh
sdrkit verify fm general
```

On Windows run `.\bin\sdrkit.exe verify fm general`. The command prints a table with one row for each check. It is a success when the last line reads `<m> checks, 0 failed` and the exit code is 0. A failure ends with the line `<n> of <m> checks failed` and exit code 1. A stack that has no test signal shows as not checked. Run `sdrkit stacks` to list the stack ids. `cw` is a variant of the stack `general`, not a stack of its own.

To check the library, build the C examples of `examples/` against the package; the steps and the expected output are in [building.md](building.md). The script test `cargo test -p sdrkit-host-ffi --test package` does this on Linux and macOS.

## Cross-compiling

`.github/workflows/build.yml` holds the commands for every target of a release: the packages it installs and the
variables it sets are the steps for a machine of your own. In short:

- `i686-unknown-linux-gnu` on x86_64 Linux: `gcc-multilib` and `libasound2-dev:i386`, with `PKG_CONFIG_ALLOW_CROSS=1`
  and `PKG_CONFIG_LIBDIR_i686_unknown_linux_gnu=/usr/lib/i386-linux-gnu/pkgconfig:/usr/share/pkgconfig`.
- `armv7-unknown-linux-gnueabihf` on x86_64 Linux: `gcc-arm-linux-gnueabihf` and `libasound2-dev:armhf` (the armhf
  packages of Ubuntu come from `ports.ubuntu.com`), with the linker `arm-linux-gnueabihf-gcc` and the pkg-config
  folder `/usr/lib/arm-linux-gnueabihf/pkgconfig`.
- `aarch64-unknown-linux-gnu` on x86_64 Linux: `gcc-aarch64-linux-gnu`; [building.md](building.md) gives the command.
- `i686-pc-windows-msvc` and `aarch64-pc-windows-msvc` on x64 Windows: `rustup target add <target>` and the x86 or
  ARM64 build tools of Visual Studio, then `tools\package.ps1 -Target <target>`. The release builds ARM64 natively on
  an ARM64 runner.
- `tools/package.ps1 -Target x86_64-pc-windows-gnu` builds the MinGW target from Windows.

Musl (static Linux programs) is not built: the command-line tool links ALSA, which a musl tool chain does not
provide.

## Releases

`.github/workflows/build.yml` builds and checks one archive for each target, each in a job of its own, so that one
failing target does not hide the others. `release.yml` calls it for a pushed tag `v<version>` and by hand;
`nightly.yml` calls it once a day. The table lists the targets, the runner, how the archive is built and how it is
checked.

| Archive | Target | Runner | Built | Checked |
|---|---|---|---|---|
| `macos-arm64` | `aarch64-apple-darwin` | `macos-15` | native | in full |
| `macos-x86_64` | `x86_64-apple-darwin` | `macos-15-intel` | native | in full |
| `macos-universal` | both | `macos-15` | `lipo` of the two archives | archive, run on arm64 |
| `linux-x86_64` | `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | native | in full |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | `ubuntu-22.04-arm` | native | in full |
| `linux-x86` | `i686-unknown-linux-gnu` | `ubuntu-22.04` | multilib | archive, run |
| `linux-armv7` | `armv7-unknown-linux-gnueabihf` | `ubuntu-22.04` | cross gcc | archive, linked; run under qemu for information |
| `windows-x86_64` | `x86_64-pc-windows-msvc` | `windows-2025` | native | in full |
| `windows-x86` | `i686-pc-windows-msvc` | `windows-2025` | MSVC x86 tools | archive, run (`-A Win32`) |
| `windows-arm64` | `aarch64-pc-windows-msvc` | `windows-11-arm` | native | in full |
| `SDRKitC-<version>.xcframework.zip` | iOS, simulator, macOS, Mac Catalyst | `macos-15` | `apple/build_xcframework.sh` | `swift build --build-tests` on macOS |

"In full" is `sh examples/cmake/check.sh` (the CMake consumers of the sources) and the check of the unpacked
archive. The check of an archive is `sh examples/cmake/check.sh archive <unpacked folder>`: the layout,
`find_package` with the static and the shared library, a link through `sdrkit.pc` and `sdrkit-static.pc`, and
`sdrkit --version` and `sdrkit stacks`. `SDRKIT_CHECK_RUN=0` builds without running, `SDRKIT_CHECK_EMULATOR` runs
through an emulator, and `SDRKIT_CHECK_CMAKE_ARGS` passes a cross compiler or `-A Win32`.

The Linux archives are built on Ubuntu 22.04, so they need glibc 2.35 or newer. The job reads the highest `GLIBC_`
symbol version that `libsdrkit.so` and `bin/sdrkit` ask for, prints it, and fails above 2.35. The static library
`libsdrkit.a` has no such record: a program that links it needs the glibc of the machine that links it.

The XCFramework has the slices `ios-arm64`, `ios-arm64_x86_64-simulator`, `macos-arm64_x86_64` and
`ios-arm64_x86_64-maccatalyst`. Mac Catalyst is optional in the workflow: if it does not build, the framework is made
without it and the run shows a warning. Each static library is a few hundred megabytes, so the zip is large; the job
fails if it reaches the 2 GiB limit of a release file.

### A release

1. Raise the version of the crate `sdrkit-host-ffi` if `sh tools/check_version.sh` asks for it (see Versions).
2. Run the release workflow by hand on the commit and check the run: `gh workflow run release.yml --ref <branch>`.
3. Push the tag `v<version>`. The workflow builds again, writes the notes with `tools/release_notes.sh`, and makes a
   draft release with the archives, `SHA256SUMS.txt` and the XCFramework with its checksum.
4. For Swift Package Manager the job writes the URL and the checksum of the XCFramework into `Package.swift` at the
   root, commits that one change on top of the tagged commit, and moves the tag to that commit. SwiftPM reads
   `Package.swift` at the root of the repository at the tag; a checksum can only exist after the build, so it cannot
   be in the commit that was tagged. The new commit is on no branch; `main` keeps the empty constants, with which
   `Package.swift` uses `apple/build/SDRKitC.xcframework` of a local build.
5. Publish the draft. Until then nobody can download its files, and SwiftPM cannot resolve the version.

`apple/Package.swift` is the same package for a local path (`.package(path: "<sdrkit>/apple")`); it always uses the
local build. Keep the targets of the two files the same.

### Nightly

`nightly.yml` runs once a day and by hand. It takes the newest commit of `main` whose push passed `ci.yml`, and stops
if the tag `nightly` is already at it (a run by hand with `force` builds anyway). Otherwise it builds the archives of
that commit, moves the tag `nightly` to it, deletes every file of the pre-release `nightly`, and uploads the new ones
with notes that name the commit and any missing file. A failed target leaves its file out; the others are still
published. It uses only the token of the run. GitHub runs a schedule only from the default branch, so the workflow
works once it is on `main`.

## Versions

sdrkit uses three numbers, `<major>.<minor>.<patch>`, in the crate `sdrkit-host-ffi`. The interface registry
(`registry/<family>.reg`) gives each stack family its own version: the registry test raises the major number of a
family for a breaking change and the minor number for an addition (see [stack-interface.md](stack-interface.md)).
The rule for the version of sdrkit follows from it:

| Since the last version tag | While 0.x | From 1.0 |
|---|---|---|
| The interface broke: a family's major number rose, a family or an item went away | raise the minor number (0.5.0 to 0.6.0) | raise the major number |
| The interface grew: a family is new, a family's minor number rose, an item is new or changed | raise the patch number (0.5.0 to 0.5.1) or more | raise the minor or patch number |
| No change | any version | any version |

`sh tools/check_version.sh` applies the rule. It compares `registry/` of the working tree with `registry/` at a
version tag: the tag of the current version if it exists (the registry must then be the same), else the highest
version tag below the current version. It passes when no version tag exists. `ci.yml` runs it on every push.
`sh tools/check_version.sh --compare <old folder> <old version> <new folder> <new version>` applies the rule to two
folders. `tools/registry.sh` holds the comparison that this script and `tools/release_notes.sh` share.

## Known gaps

- Not confirmed: that any build or command on this page ran on Windows or Linux. `building.md` marks both as written and not run.
- Not confirmed: the Debian, Ubuntu and Fedora package names. They are the usual names for `libasound` and `pkg-config`, not read from the repository.
- Not confirmed: that `sdrkit-tui` runs in a Windows terminal. The terminal library is `ratatui` with `crossterm`.
- Not confirmed: the home folder on Windows. The data lookup reads the variable `HOME`, and uses `.` when it is not set. `~/.sdrkit` may therefore not resolve as it does on Linux.
- Not confirmed: a Linux library that needs no `libasound` at run time; whether the C library links `alsa-sys` was not traced.
- Not confirmed: the runtime libraries a Linux static program needs. Use the list in `sdrkit-static.pc` of your package.
