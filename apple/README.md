# SDRKit for Swift

A Swift package for the C library of sdrkit, for iOS 17 and macOS 14 on arm64.

The package has two targets:

- `SDRKitC`: the static C library as an XCFramework. Its module has the header `sdrkit.h`, the headers of the stack
  families (`sdrkit/fm.h`, `sdrkit/session.h` ...) and the header of the presets part, `sdrkit_presets.h`.
- `SDRKit`: Swift types for the C API (`Engine`, `Object`, `Radio`, `Session`, `Feed`, `Value`, `Snapshot`,
  `SDRKitError`) and the generated typed Swift files of the stack families (`FmFields`, `SessionSettings` ...).
  `Sources/SDRKit/Generated` is a link to `crates/hosts/sdrkit-host-ffi/swift`; the package compiles those files
  as they are.

## Build the framework

You need Xcode, and rustup with the targets `aarch64-apple-ios`, `aarch64-apple-ios-sim` and `aarch64-apple-darwin`.

1. Run the script from the root of the workspace:

       sh apple/build_xcframework.sh

2. The script builds the crate `sdrkit-host-ffi` with the profile `shipped` for the three targets, with every stack family and the
   cargo feature `presets`. The builds use the `target/` folder of the workspace.
3. The script copies the headers, writes the module map, checks that no symbol of a static library is defined in
   two of its objects, prints the size of each library, and writes `apple/build/SDRKitC.xcframework`. The profile
   writes no debug information. Do not run `strip` on a library: it makes symbol names equal that are not.

The slices are `ios-arm64` (iOS 17.0), `ios-arm64-simulator` (iOS 17.0) and `macos-arm64` (macOS 14.0). Each static
library is about 240 MB without debug information. Run the script again after a change of the Rust code: it builds
what changed and replaces the framework. `apple/build/` is not in git. Build the framework before you open or build
the package.

The presets part reads the regional data (plans, channels, scan channels, frequency entries of a country) from a
data folder that the application ships. The Swift class `Presets` gives the channels, the start frequency and the
settings of a stack in a country:

    let presets = try Presets(folder: dataFolder)
    let dial = try presets.dial(stack: "dvbt", country: "AU")            // nil if the data has nothing
    let settings = try presets.settings(stack: "dvbt", country: "AU", engine: engine)
    let session = try engine.start(stack: "dvbt", on: radio, frequency: dial?.frequencyHz, settings: settings)

The class also lists the countries (`countries()`) and the plans of a country (`planIDs(country:)`), gives the dials
of a stack as rows (`dials(stack:country:submode:)`), loads the scan channels and the frequency entries into an engine
(`setScanChannels(on:plans:)`, `setFrequencyEntries(on:)`), and reads the user's plan file (`personalPlans(file:)`),
whose plans for a stack come with the channels of the stack. The other calls of `sdrkit_presets.h` (search, labels,
import and export of plans ...) are called from the module `SDRKitC`, with `engine.object.cHandle` as the handle of the
engine. The presets part is the submodule
`SDRKitC.Presets`, which `import SDRKitC` imports too. A host that does not want the presets part builds without it:

    SDRKIT_PRESETS=0 sh apple/build_xcframework.sh

The library then reads no data folder, the module has no `sdrkit_presets.h`, and the package has no class `Presets`.

## Test

1. Build the framework.
2. Run the tests on the Mac:

       cd apple
       swift build
       swift test

The tests use the test signals of the library (`synth:` addresses). They use no radio, no network and no audio
device.

## Add the package to an Xcode project

1. Build the framework.
2. In Xcode, select File > Add Package Dependencies > Add Local, and select the folder `apple`.
3. Add the library `SDRKit` to the target of the app.
4. Write `import SDRKit` in the Swift files that use it. Write `import SDRKitC` too in a file that calls the C API
   directly, such as the `sdrkit_presets_*` calls.

The framework is not signed. Xcode signs it with the app.

In another Swift package, add the dependency by path:

    .package(path: "../sdrkit/apple")

and add `.product(name: "SDRKit", package: "apple")` to the dependencies of the target.

## System libraries

The static library needs these at link time. The package declares them in its linker settings, so an app that uses
the package needs no setting of its own. A project that links `libsdrkit.a` without the package must add them.

- `Accelerate.framework` (vDSP)
- `AudioToolbox.framework` (the audio decoders of the system)
- `CoreFoundation.framework`
- `Security.framework`
- `libiconv`

`rustc` prints this list:

    MACOSX_DEPLOYMENT_TARGET=14.0 cargo rustc --release -p sdrkit-host-ffi \
        --target aarch64-apple-darwin --lib --crate-type staticlib -- --print native-static-libs

## Example

```swift
import Foundation
import SDRKit

let engine = try Engine(folder: NSTemporaryDirectory() + "sdrkit")
try engine.object.set("callsign", text: "VK2ABC")
let radio = try engine.openRadio("synth:fm")  // A test signal. A real radio: "iiod:<host>" or "rtl_tcp:<host>:<port>".
let session = try engine.start(stack: "fm", on: radio, frequency: 104.9e6)

var audio = [Float](repeating: 0, count: 9600)
for pass in 0..<200 {
    let status = try session.snapshot().status
    let fm = try session.typed(FmFields.read)  // The typed FM result set of that snapshot.
    print(status.state, status.snrDb ?? .nan, fm.station)
    let got = try session.readAudio(into: &audio)
    if got.frames > 0 {
        // audio[0 ..< got.frames * got.channels] holds interleaved frames at got.rateHz frames per second.
    }
    if pass == 100 {
        try session.typed { SessionSettings.setSessionTunedHz($0, 104.5e6) }  // A change of frequency.
    }
    Thread.sleep(forTimeInterval: 0.05)
}
let saved = try engine.exportSettings()  // Bytes to store; engine.importSettings(saved) applies them in a later run.
try session.stop()
try session.close()
try radio.close()
```

## Rules of the API

- Calls are synchronous. No call waits for a signal: the read calls return what is ready.
- Threads: the library allows one thread at a time on a handle. Each `Object`, `Radio`, `Session` and `Feed` has one
  handle and no lock, and no class is `Sendable`. Use each from one thread or one serial queue. A second thread opens
  its own handle of the same object with `Engine.find(_:)` and the key of the object (`session.key`). One call is an
  exception: `Session.writeAudio(_:channels:rateHz:)` never waits and allocates nothing, so the audio thread of the system can
  call it while another thread uses the same `Session`. Open the microphone only while the result field
  `session.audio_in_wanted` is true.
- Lifetime: releasing an object frees its handle only. A session runs until `stop()`, and a radio stays open until
  `close()`. The engine is freed when the `Engine` and all its objects are released; that stops every session and
  closes every radio.
- Strings and buffers: the library returns views that are valid until the next call on the same handle. The package
  copies every view at once, so every Swift value is a copy that the caller owns.
- Errors: every failing call throws `SDRKitError` with the code, the name of the code (`SDRKIT_E_...`), one English
  sentence and the detail of the library. The generated calls return `Result<_, SDRKitCode>`; `object.typed(_:)`
  runs one on the handle of the object and throws `SDRKitError`, and `engine.typed(_:)` does the same on the engine
  (`EngineSettings`, `EngineFields` ...). A value of the generated files that is a place, such as the engine setting
  `position`, is an `SDRKitPosition(lat:lon:altM:)`.
- Saved settings are bytes that only the library reads: `Engine.exportSettings()` and `Engine.importSettings(_:)`.
- Settings, commands, result fields, tables and events are declared by the library. Read the declarations
  (`settings`, `commands`, `resultsDeclaration()`) to learn the keys, types, ranges and options.
