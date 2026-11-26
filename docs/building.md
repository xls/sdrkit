# Building the C library and the examples

This page tells how to make an installed copy of the C library of sdrkit on each system, how to build the programs of
`examples/` against it, and what a first run prints. One command makes the copy on each system:

- Linux and macOS: `sh tools/package.sh`
- Windows: `powershell -ExecutionPolicy Bypass -File tools\package.ps1`

The copy is a folder `dist/sdrkit-<version>-<target>/`. The steps below call it `SDRKIT`. Its parts:

| Folder | Contents |
|---|---|
| `include/` | `sdrkit.h`, the family headers `sdrkit/<family>.h` with `sdrkit/init.h`, and `sdrkit_presets.h` |
| `lib/` | the static library, the dynamic library and, on Windows, the import library |
| `lib/pkgconfig/` | `sdrkit.pc` (dynamic library) and `sdrkit-static.pc` (static library with its system libraries) |
| `lib/cmake/sdrkit/` | `sdrkitConfig.cmake` and `sdrkitConfigVersion.cmake`: the targets `sdrkit::sdrkit` (static) and `sdrkit::shared` (dynamic) |
| `share/sdrkit/data/` | the data folder for the presets calls |
| `share/doc/sdrkit/` | the licence files |

## What was tested

| Platform | Target | State |
|---|---|---|
| macOS, Apple silicon | `aarch64-apple-darwin` | Tested: the script, CMake, pkg-config, the plain line and the runs below. |
| macOS, Intel | `x86_64-apple-darwin` | Written from the rules of the tool chain. Not run. |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | Written from the rules of the tool chain. Not run. |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | Written from the rules of the tool chain. Not run. |
| Windows, MSVC | `x86_64-pc-windows-msvc` | Written from the rules of the tool chain. Not run. `tools/package.ps1` was not run. |
| Windows, MinGW | `x86_64-pc-windows-gnu` | Written from the rules of the tool chain. Not run. |
| Linux x86 (32-bit) | `i686-unknown-linux-gnu` | Built and checked by the release workflow only. |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | Built and linked by the release workflow only. |
| Windows x86 (32-bit) | `i686-pc-windows-msvc` | Built and checked by the release workflow only. |
| Windows ARM64 | `aarch64-pc-windows-msvc` | Built and checked by the release workflow only. |

The release workflow (`.github/workflows/build.yml`) builds and checks every target of this table except MinGW; the
commands it runs are the steps for each system. [packaging.md](packaging.md) lists how each target is checked.

The things most likely to differ on a platform that was not run:

- The file names of the libraries. The scripts use the names that cargo gives: `libsdrkit.a` and `libsdrkit.so` on
  Linux, `libsdrkit.a` and `libsdrkit.dylib` on macOS, `sdrkit.lib`, `sdrkit.dll` and `sdrkit.dll.lib` with MSVC,
  `libsdrkit.a`, `sdrkit.dll` and `libsdrkit.dll.a` with MinGW.
- The system libraries of the static library. The scripts take them from `rustc --print native-static-libs` at build
  time and write them into `sdrkit-static.pc` and `sdrkitConfig.cmake`. The lists in this page are what the tool chain
  usually prints; use the list in the files of your copy.
- Path separators. The CMake files use forward slashes. A data folder path with backslashes passed as
  `SDRKIT_DATA_DIR` becomes a C string with escapes in it: set the environment variable `SDRKIT_DATA` instead.
- `long` is 32 bits on Windows. The examples print 64-bit numbers with `%lld` and a cast to `long long`, and use `long`
  only for the nanoseconds of a sleep outside Windows.
- The run path. Linux and macOS programs find the dynamic library through `-Wl,-rpath`. A Windows program finds
  `sdrkit.dll` in its own folder or on `PATH`.

## The expected output

Every system runs the FM program on the test signal in the same way. Without arguments it writes 5 s of audio to
`fm_audio.wav`, prints a line about once a second, and ends with this line:

    wrote 5.0 s of audio to fm_audio.wav

A line before it looks like this:

    100.000 MHz  station "SDRKIT  "  stereo  SNR 60.0 dB  RDS 95 %

## macOS

1. Install the tools:
   - Xcode or the Command Line Tools (`xcode-select --install`): the C compiler.
   - Rust: `curl https://sh.rustup.rs -sSf | sh`. For an Intel Mac on Apple silicon: `rustup target add x86_64-apple-darwin`.
   - CMake (optional): `brew install cmake`. pkg-config (optional): `brew install pkgconf`.
   - SDL 3 (optional, for the programs that play sound): `brew install sdl3`.
2. Make the copy:

       sh tools/package.sh
       SDRKIT=$PWD/dist/sdrkit-0.5.0-aarch64-apple-darwin

3. Build the examples with CMake:

       cmake -S examples -B build/examples -DCMAKE_PREFIX_PATH=$SDRKIT
       cmake --build build/examples

4. Or build one program with a plain compiler line:

       cc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include -L$SDRKIT/lib -lsdrkit -Wl,-rpath,$SDRKIT/lib -o fm_audio

5. For the static library, add its system libraries:

       cc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include $SDRKIT/lib/libsdrkit.a \
          -framework AudioToolbox -framework Security -framework CoreFoundation -framework Accelerate \
          -framework IOKit -liconv -lc -lm -o fm_audio

6. Run the program on the test signal:

       ./fm_audio

## Linux x86_64

1. Install the tools:
   - A C compiler: `sudo apt install build-essential` (Debian, Ubuntu) or `sudo dnf install gcc gcc-c++ make`.
   - Rust: `curl https://sh.rustup.rs -sSf | sh`.
   - CMake and pkg-config (optional): `sudo apt install cmake pkg-config`.
   - SDL 3 (optional): the package `libsdl3-dev` where the distribution has it, else SDL built from its sources.
2. Make the copy:

       sh tools/package.sh
       SDRKIT=$PWD/dist/sdrkit-0.5.0-x86_64-unknown-linux-gnu

3. Build the examples with CMake:

       cmake -S examples -B build/examples -DCMAKE_PREFIX_PATH=$SDRKIT
       cmake --build build/examples

4. Or build one program with a plain compiler line:

       cc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include -L$SDRKIT/lib -lsdrkit -Wl,-rpath,$SDRKIT/lib -o fm_audio

5. For the static library, add its system libraries (usually these):

       cc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include $SDRKIT/lib/libsdrkit.a \
          -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc -o fm_audio

6. Run the program on the test signal:

       ./fm_audio

## Linux aarch64

On an aarch64 machine (a Raspberry Pi 4 or 5 with a 64-bit system, a Graviton server), the steps are those of
Linux x86_64. The copy is `dist/sdrkit-0.5.0-aarch64-unknown-linux-gnu`.

To make the copy on an x86_64 machine:

1. Install the target and a cross compiler:

       rustup target add aarch64-unknown-linux-gnu
       sudo apt install gcc-aarch64-linux-gnu

2. Make the copy with that compiler as the linker and as the compiler of the C parts:

       CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
       CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
       sh tools/package.sh aarch64-unknown-linux-gnu

3. Build the examples with `aarch64-linux-gnu-gcc` in place of `cc`, and run them on the aarch64 machine.

## Windows with MSVC

1. Install the tools:
   - Visual Studio 2022 or the Build Tools for Visual Studio, with the workload "Desktop development with C++". It
     has the compiler `cl` and CMake.
   - Rust: `rustup-init.exe` from rustup.rs, with the default tool chain `stable-x86_64-pc-windows-msvc`.
   - SDL 3 (optional): the archive `SDL3-devel-<version>-VC.zip` of the SDL releases. Give its folder to CMake in
     `CMAKE_PREFIX_PATH` too.
2. Open "Developer PowerShell for VS 2022" and go to the root of the repository.
3. Make the copy:

       powershell -ExecutionPolicy Bypass -File tools\package.ps1
       $SDRKIT = "$PWD\dist\sdrkit-0.5.0-x86_64-pc-windows-msvc"

4. Build the examples with CMake. The CMake files copy `sdrkit.dll` next to each program:

       cmake -S examples -B build\examples -DCMAKE_PREFIX_PATH="$SDRKIT"
       cmake --build build\examples --config Release

5. Or build one program with a plain compiler line, and put the DLL next to it:

       cl /nologo /I "$SDRKIT\include" examples\c\fm_audio.c /link /LIBPATH:"$SDRKIT\lib" sdrkit.dll.lib
       copy "$SDRKIT\lib\sdrkit.dll" .

6. For the static library, link `sdrkit.lib` and its system libraries (usually these; the list of your copy is in
   `lib\cmake\sdrkit\sdrkitConfig.cmake`):

       cl /nologo /I "$SDRKIT\include" examples\c\fm_audio.c /link /LIBPATH:"$SDRKIT\lib" sdrkit.lib `
          kernel32.lib advapi32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib bcrypt.lib

7. Run the program on the test signal:

       .\fm_audio.exe

   A program of the CMake build is `build\examples\Release\fm_audio.exe`.

## Windows with MinGW

1. Install the tools:
   - MSYS2 from msys2.org, then in its UCRT64 shell: `pacman -S mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-cmake
     mingw-w64-ucrt-x86_64-pkgconf`. SDL 3 (optional): `mingw-w64-ucrt-x86_64-sdl3`.
   - Rust with the GNU target: `rustup target add x86_64-pc-windows-gnu`, or the default tool chain
     `stable-x86_64-pc-windows-gnu`.
2. In PowerShell, at the root of the repository, with the `bin` folder of MinGW on `PATH`, make the copy:

       powershell -ExecutionPolicy Bypass -File tools\package.ps1 -Target x86_64-pc-windows-gnu

3. In the MinGW shell, build the examples with CMake:

       SDRKIT=$PWD/dist/sdrkit-0.5.0-x86_64-pc-windows-gnu
       cmake -S examples -B build/examples -G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=$SDRKIT
       cmake --build build/examples

4. Or build one program with a plain compiler line. `-lsdrkit` takes the import library `libsdrkit.dll.a`:

       gcc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include -L$SDRKIT/lib -lsdrkit -o fm_audio.exe
       cp $SDRKIT/lib/sdrkit.dll .

5. Or build it through pkg-config:

       export PKG_CONFIG_PATH=$SDRKIT/lib/pkgconfig
       gcc -std=c99 examples/c/fm_audio.c $(pkg-config --cflags --libs sdrkit) -o fm_audio.exe

6. For the static library, use `sdrkit-static.pc`, which holds the system libraries (usually `-lkernel32 -ladvapi32
   -lntdll -luserenv -lws2_32 -ldbghelp -lbcrypt` and the runtime of MinGW):

       gcc -std=c99 examples/c/fm_audio.c $(pkg-config --cflags --libs sdrkit-static) -o fm_audio.exe

7. Run the program on the test signal:

       ./fm_audio.exe

## The test of the package

On Linux and macOS, `cargo test -p sdrkit-host-ffi --test package` runs `tools/package.sh` into a temporary folder,
builds `examples/` with CMake if `cmake` is on the path, builds one program with the plain line and one through
pkg-config, and runs `fm_audio`, `list_stacks`, `adsb_table` and `settings_save` on their test signals. The first run
builds the library with the profile `shipped` and takes some minutes.
