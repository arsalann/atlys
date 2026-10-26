// swift-tools-version: 6.0
// On-device collectors (Health, Location, Motion, Device). Collectors only
// ever hold the ingest public key: they can add sealed data, never read it.

import PackageDescription

let package = Package(
    name: "AtlysCollectors",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "AtlysCollectors", targets: ["AtlysCollectors"]),
    ],
    dependencies: [
        .package(path: "../AtlysCore"),
    ],
    targets: [
        .target(name: "AtlysCollectors", dependencies: ["AtlysCore"]),
        .testTarget(name: "AtlysCollectorsTests", dependencies: ["AtlysCollectors", "AtlysCore"]),
    ],
    swiftLanguageModes: [.v6]
)
