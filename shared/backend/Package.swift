// swift-tools-version: 5.9
import PackageDescription

let package = Package(
  name: "MSIMEBackend",
  platforms: [.macOS(.v12), .iOS(.v15)],
  products: [.library(name: "MSIMEBackend", targets: ["MSIMEBackend"])],
  targets: [
    .target(
      name: "MSIMEBackend",
      path: ".",
      exclude: ["Package.swift", "Tests"],
      sources: [
        "account/BackendAccountClient.swift",
        "clients/BackendAiClient.swift",
        "account/BackendAccountSession.swift",
        "account/BackendAnonymousAccount.swift",
        "clients/BackendChatClient.swift",
        "clients/BackendCandidateClient.swift",
        "clients/BackendClipboardClient.swift",
        "content/BackendCommunityResourceClient.swift",
        "content/BackendDictionaryClient.swift",
        "clients/BackendPreferencesClient.swift",
        "clients/BackendSnapshotClient.swift",
        "clients/BackendSkinArtworkClient.swift",
        "storage/IOSPreferencePlan.swift",
        "storage/BackendLocalStore.swift",
      ]
    ),
    .testTarget(
      name: "MSIMEBackendTests",
      dependencies: ["MSIMEBackend"],
      path: "Tests"
    )
  ]
)
