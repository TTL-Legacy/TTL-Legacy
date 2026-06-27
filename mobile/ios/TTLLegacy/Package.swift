// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TTLLegacy",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "TTLLegacy", targets: ["TTLLegacy"]),
        .library(name: "TTLWidget", targets: ["TTLWidget"])
    ],
    dependencies: [],
    targets: [
        .target(
            name: "TTLLegacy",
            path: "Sources",
            exclude: ["Widget"]
        ),
        // WidgetKit extension target — deploy as a separate app extension in Xcode.
        // Requires Associated Domains (applinks:ttl-legacy.app) entitlement and
        // BGTaskSchedulerPermittedIdentifiers in Info.plist for the host app.
        .target(
            name: "TTLWidget",
            dependencies: ["TTLLegacy"],
            path: "Sources/Widget"
        ),
        .testTarget(
            name: "TTLLegacyTests",
            dependencies: ["TTLLegacy", "TTLWidget"],
            path: "Tests"
        )
    ]
)
