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

The script does not copy the two tools. To ship them, follow these steps after the script ran:

1. Make the folder `bin/` in the package folder.
2. Copy `sdrkit` and `sdrkit-tui` (with `.exe` on Windows) from `target/shipped/` into `bin/`.
3. Make the archive: `.zip` on Windows, `.tar.gz` on Linux.

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

The repository supports one cross target on Linux: `aarch64-unknown-linux-gnu`, built on an x86_64 machine with `gcc-aarch64-linux-gnu`. [building.md](building.md) gives the command. `tools/package.ps1 -Target x86_64-pc-windows-gnu` builds the MinGW target from Windows. No other cross build is described.

## Known gaps

- Not confirmed: that any build or command on this page ran on Windows or Linux. `building.md` marks both as written and not run.
- Not confirmed: the Debian, Ubuntu and Fedora package names. They are the usual names for `libasound` and `pkg-config`, not read from the repository.
- Not confirmed: that `sdrkit-tui` runs in a Windows terminal. The terminal library is `ratatui` with `crossterm`.
- Not confirmed: the home folder on Windows. The data lookup reads the variable `HOME`, and uses `.` when it is not set. `~/.sdrkit` may therefore not resolve as it does on Linux.
- Not confirmed: a script or command that packs `bin/` for you. `tools/package.sh` and `tools/package.ps1` pack the library only.
- Not confirmed: a Linux library that needs no `libasound` at run time; whether the C library links `alsa-sys` was not traced.
- Not confirmed: the runtime libraries a Linux static program needs. Use the list in `sdrkit-static.pc` of your package.
