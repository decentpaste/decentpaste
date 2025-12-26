// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "tauri-plugin-share-intent",
    platforms: [
        .iOS(.v14)
    ],
    products: [
        .library(
            name: "tauri-plugin-share-intent",
            targets: ["ShareIntentPlugin"]
        )
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api")
    ],
    targets: [
        .target(
            name: "ShareIntentPlugin",
            dependencies: [
                .product(name: "Tauri", package: "Tauri")
            ],
            path: "Sources/ShareIntentPlugin"
        )
    ]
)
