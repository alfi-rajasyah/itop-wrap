// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "ITopWrap",
    platforms: [.macOS(.v26)],
    products: [
        .executable(name: "ITopWrap", targets: ["ITopWrap"]),
        .library(name: "ITopKit", targets: ["ITopKit"]),
    ],
    targets: [
        .target(name: "ITopKit"),
        .executableTarget(name: "ITopWrap", dependencies: ["ITopKit"]),
        .testTarget(
            name: "ITopKitTests",
            dependencies: ["ITopKit"],
            resources: [.copy("Fixtures")]
        ),
    ]
)
