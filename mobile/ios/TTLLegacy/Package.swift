// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TTLLegacy",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "TTLLegacy", targets: ["TTLLegacy"])
    ],
    dependencies: [],
    targets: [
        .target(
            name: "TTLLegacy",
            path: "Sources"
        ),
        .testTarget(
            name: "TTLLegacyTests",
            dependencies: ["TTLLegacy"],
            path: "Tests"
        )
    ]
)
