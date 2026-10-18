// swift-tools-version: 5.9
import PackageDescription

let package = Package(
  name: "MSIMEBackend",
  platforms: [.macOS(.v12), .iOS(.v15)],
  products: [.library(name: "MSIMEBackend", targets: ["MSIMEBackend"])],
  targets: [
    // C 头文件的类型（`rust/module.modulemap`），不链接 Rust：函数指针由宿主经 `msime_rust_client` 交来。
    .systemLibrary(name: "CMSIMEClient", path: "rust"),
    .target(
      name: "MSIMEBackend",
      dependencies: ["CMSIMEClient"],
      path: ".",
      exclude: ["Package.swift", "Tests", "rust"],
      sources: [
        "account/BackendAccountClient.swift",
        "account/BackendRustClient.swift",
        "account/BackendRustAccount.swift",
        "account/BackendRustSessionStore.swift",
        "account/BackendAccountSession.swift",
        "account/BackendAnonymousAccount.swift",
        "clients/BackendChatClient.swift",
        "clients/BackendCandidateClient.swift",
        "clients/BackendClipboardClient.swift",
        "content/BackendCommunityResourceClient.swift",
        "content/BackendDictionaryClient.swift",
        "clients/BackendPreferencesClient.swift",
        "clients/BackendSnapshotClient.swift",
        "storage/BackendLocalStore.swift",
        "storage/BackendBoundedFileReader.swift",
      ]
    ),
    .testTarget(
      name: "MSIMEBackendTests",
      dependencies: ["MSIMEBackend"],
      path: "Tests"
    )
  ]
)
