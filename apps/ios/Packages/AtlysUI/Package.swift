// swift-tools-version: 6.0
// SwiftUI screens and components.

import PackageDescription

let package = Package(
    name: "AtlysUI",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "AtlysUI", targets: ["AtlysUI"]),
    ],
    dependencies: [
        .package(path: "../AtlysCore"),
        .package(path: "../AtlysCollectors"),
    ],
    targets: [
        .target(name: "AtlysUI", dependencies: ["AtlysCore", "AtlysCollectors"]),
        .testTarget(name: "AtlysUIUnitTests", dependencies: ["AtlysUI", "AtlysCore"]),
    ],
    swiftLanguageModes: [.v6]
)
