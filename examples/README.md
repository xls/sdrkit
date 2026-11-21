# Examples

Small, complete programs that use sdrkit from C, C++ and Rust. Copy one and change it.

Each program runs on a test signal by default (a source `synth:<stack>`), so it needs no radio and no network. A
program takes the address of a radio as its first argument, such as `rtl_tcp:192.168.1.20:1234`, and a frequency in
MHz where it tunes.

## The programs

| Program | What it shows |
|---|---|
| `c/fm_audio.c` | The engine, a session of the stack `fm`, the frequency with the typed call `sdrkit_session_set_tuned_hz`, audio pulled in a loop and written to a WAV file, and the typed FM result set: the station name from RDS, stereo and the signal quality. |
| `c/fm_play_sdl.c` | The same with the sound through SDL 3. |
| `c/list_stacks.c` | Every stack with its settings, tables and commands, read through the generic interface (the declarations). |
| `c/adsb_table.c` | The typed aircraft rows of the stack `adsb`, printed as they change. |
| `c/meshtastic.c` | The stack `meshtastic`: text messages and nodes as typed rows, the frequency and the settings of a country from the presets part (`sdrkit_presets_dial`, `sdrkit_presets_settings`), and a private channel whose key comes from an argument or the environment variable `SDRKIT_MESHTASTIC_KEY`. |
| `c/dvbt_services.c` | DVB-T: the channel of a country and the settings of the stack for it from the presets part (`sdrkit_presets_channels`, `sdrkit_presets_settings`), lock, the list of services, the selection of one with a typed command, and its audio. |
| `c/dvbt_play_sdl.c` | DVB-T with the audio of the first service through SDL 3. Video is not shown: `sdrkit_media_read` gives its packets to a host that has a decoder. |
| `c/scan_band.c` | A channel scan by the scan service, with the channels of a country from the presets part. |
| `c/settings_save.c` | Settings exported as bytes, written to a file, and imported into a second engine. |
| `cpp/fm_audio.cpp` | A small C++17 wrapper (handles that close themselves, exceptions from `SDRKIT_E_*` codes) and the FM program on it. |
| `cpp/any_stack.cpp` | Any stack by its id, with its result fields and tables printed through the generic interface. |
| `rust/fm_audio.rs` | The FM program with the Rust crates: typed fields through `FmFields`. |
| `rust/meshtastic.rs` | The mesh stack with the settings of a country from the presets helper (`sdrkit_presets::start`). |
| `rust/any_stack.rs` | Any stack by its id, with its results printed from their declarations. |

The programs that use the presets part read the data folder from the environment variable `SDRKIT_DATA`. Without it
they use the folder that the macro `SDRKIT_DATA_DIR` names. The CMake build sets it to `share/sdrkit/data` of the
installed copy; with another build, set `SDRKIT_DATA` or pass `-DSDRKIT_DATA_DIR='"<folder>"'`.

## Make an installed copy of the library

1. Install the Rust tool chain (`rustup`), a C compiler, and CMake if you want to build with it.
2. From the root of the repository, run the script for your system:

       sh tools/package.sh                                        # Linux, macOS
       powershell -ExecutionPolicy Bypass -File tools\package.ps1 # Windows

3. The script prints the folder of the copy on its last line, such as
   `dist/sdrkit-0.1.0-x86_64-unknown-linux-gnu`. The commands below call it `$SDRKIT`.

[docs/building.md](../docs/building.md) has the steps for each system and tool chain.

## Build with CMake

1. Configure the folder `examples` with the installed copy on the prefix path:

       cmake -S examples -B build/examples -DCMAKE_PREFIX_PATH=$SDRKIT

2. Build:

       cmake --build build/examples

3. Run a program:

       build/examples/fm_audio

The programs link the dynamic library by default. Add `-DSDRKIT_EXAMPLES_STATIC=ON` to link the static library and
the system libraries that it needs. Add `-DSDRKIT_EXAMPLES_SDL=ON` to also build `fm_play_sdl` and `dvbt_play_sdl`;
CMake must then find SDL 3. SDL is a dependency of those two programs only, not of the library.

In your own `CMakeLists.txt`:

    find_package(sdrkit CONFIG REQUIRED)
    target_link_libraries(app PRIVATE sdrkit::shared)   # or sdrkit::sdrkit for the static library

## Build with pkg-config

1. Put the folder `lib/pkgconfig` of the copy on the search path of pkg-config:

       export PKG_CONFIG_PATH=$SDRKIT/lib/pkgconfig

2. Build with the dynamic library. The run path lets the program find the library where it is:

       cc -std=c99 examples/c/fm_audio.c $(pkg-config --cflags --libs sdrkit) \
          -Wl,-rpath,$(pkg-config --variable=libdir sdrkit) -o fm_audio

3. Or build with the static library and its system libraries:

       cc -std=c99 examples/c/fm_audio.c $(pkg-config --cflags --libs sdrkit-static) -o fm_audio

## Build with a plain compiler line

1. Give the folder of the headers with `-I`, the folder of the libraries with `-L`, and the library with `-lsdrkit`:

       cc -std=c99 examples/c/fm_audio.c -I$SDRKIT/include -L$SDRKIT/lib -lsdrkit -Wl,-rpath,$SDRKIT/lib -o fm_audio

2. A C++ program builds in the same way with `c++ -std=c++17`.
3. With MSVC, link the import library and put `sdrkit.dll` next to the program:

       cl /I %SDRKIT%\include examples\c\fm_audio.c /link /LIBPATH:%SDRKIT%\lib sdrkit.dll.lib
       copy %SDRKIT%\lib\sdrkit.dll .

## Run

1. Run a program without arguments to use its test signal:

       ./fm_audio

2. The FM program prints a line each second and its last line, such as:

       wrote 5.0 s of audio to fm_audio.wav

3. Give a radio and a frequency to receive a station:

       ./fm_audio rtl_tcp:192.168.1.20:1234 10 98.5

## The Rust examples

The folder `rust` is the cargo package `sdrkit-examples` of the workspace. It uses the Rust crates directly and needs
no installed copy.

1. Run a program:

       cargo run -p sdrkit-examples --example fm_audio
       cargo run -p sdrkit-examples --example meshtastic -- synth:meshtastic 20
       cargo run -p sdrkit-examples --example meshtastic -- rtl_tcp:192.168.1.20:1234 60 AU
       cargo run -p sdrkit-examples --example any_stack -- adsb

2. Run the tests of the package. They run each program on its test signal:

       cargo test -p sdrkit-examples

## Tests

`crates/hosts/sdrkit-host-ffi/tests/c_api.rs` compiles every C and C++ program here as strict C99 and C++17.
`crates/hosts/sdrkit-host-ffi/tests/package.rs` runs `tools/package.sh` into a temporary folder, builds this folder
with CMake (if `cmake` is on the path), builds one program with a plain compiler line and one through pkg-config, and
runs `fm_audio`, `list_stacks`, `adsb_table` and `settings_save` on their test signals.
