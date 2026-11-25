// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "SDRKit",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "SDRKit", targets: ["SDRKit"])
    ],
    targets: [
        // The static C library with its headers. `sh build_xcframework.sh` writes it.
        .binaryTarget(name: "SDRKitC", path: "build/SDRKitC.xcframework"),
        .target(
            name: "SDRKit",
            dependencies: ["SDRKitC"],
            // `Generated` links to the generated Swift files of the stack families. Its examples are not part of
            // the library.
            exclude: ["Generated/Examples"],
            linkerSettings: [
                // What the static library needs at link time.
                .linkedFramework("Accelerate"),
                .linkedFramework("AudioToolbox"),
                .linkedFramework("CoreFoundation"),
                // USB radios, on the Mac only: iOS gives an app no USB access.
                .linkedFramework("IOKit", .when(platforms: [.macOS])),
                .linkedFramework("Security"),
                .linkedLibrary("iconv"),
            ]
        ),
        .testTarget(name: "SDRKitTests", dependencies: ["SDRKit"]),
    ]
)
