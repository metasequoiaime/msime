import Foundation

/// Filesystem locations shared by iOS app and keyboard storage fallbacks.
enum IOSSharedPaths {
  /// Application Support is the unsigned simulator fallback when an App Group container is unavailable.
  static let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]

  /// Shared protection policy for JSON state written to the shared container.
  static let protectedAtomicWriteOptions: Data.WritingOptions = [
    .atomic,
    .completeFileProtectionUntilFirstUserAuthentication,
  ]

  /// Excludes an existing shared-storage path from device backups.
  static func excludeFromBackup(_ path: URL) {
    guard FileManager.default.fileExists(atPath: path.path) else { return }
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try? path.setResourceValues(values)
  }
}
