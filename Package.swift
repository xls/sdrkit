// swift-tools-version:5.9
// The Swift package of sdrkit for an app that adds it by the repository URL and a version:
//
//     .package(url: "https://github.com/xls/sdrkit", from: "0.5.0")
//
// At a version tag the two constants below name the XCFramework of that GitHub release and its checksum; the release
// workflow writes them into the commit of the tag. Elsewhere they are empty, and the package uses
// apple/build/SDRKitC.xcframework, which `sh apple/build_xcframework.sh` writes. apple/Package.swift is the same
// package for a local path (`.package(path: "<sdrkit>/apple")`); keep the targets of the two files the same.
import PackageDescription

let releaseURL = ""
let releaseChecksum = ""

let core: Target = releaseChecksum.isEmpty
    ? .binaryTarget(name: "SDRKitC", path: "apple/build/SDRKitC.xcframework")
    : .binaryTarget(name: "SDRKitC", url: releaseURL, checksum: releaseChecksum)

let package = Package(
    name: "SDRKit",
    platforms: [.iOS(.v17), .macCatalyst(.v17), .macOS(.v14)],
    products: [
        .library(name: "SDRKit", targets: ["SDRKit"])
    ],
    targets: [
        core,
        .target(
            name: "SDRKit",
            dependencies: ["SDRKitC"],
            path: "apple/Sources/SDRKit",
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
        .testTarget(name: "SDRKitTests", dependencies: ["SDRKit"], path: "apple/Tests/SDRKitTests"),
    ]
)
