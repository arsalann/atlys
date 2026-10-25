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
    ],
    targets: [
        .target(name: "AtlysUI", dependencies: ["AtlysCore"]),
    ],
    swiftLanguageModes: [.v6]
)
