// swift-tools-version: 6.0
// The Rust core (XCFramework) plus its UniFFI-generated Swift bindings.
// Run scripts/build-ios-core.sh before building: both the XCFramework and
// Sources/AtlysCore/Generated/ are build outputs and are not committed.

import PackageDescription

let package = Package(
    name: "AtlysCore",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "AtlysCore", targets: ["AtlysCore"]),
    ],
    targets: [
        .binaryTarget(name: "AtlysCoreFFI", path: "AtlysCoreFFI.xcframework"),
        .target(
            name: "AtlysCore",
            dependencies: ["AtlysCoreFFI"],
            linkerSettings: [
                // SQLCipher's CommonCrypto provider.
                .linkedFramework("Security"),
            ]
        ),
        .testTarget(name: "AtlysCoreTests", dependencies: ["AtlysCore"]),
    ],
    swiftLanguageModes: [.v6]
)
