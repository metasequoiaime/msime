import Foundation

/// iOS 应用与键盘扩展共用的目录和文件写入策略。
enum IOSSharedPaths {
  /// 未签名的模拟器构建取不到 App Group 容器时，使用应用自己的 Application Support。
  static let applicationSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]

  /// 共享数据优先使用 App Group 容器，无法获取时使用应用自己的目录。
  static func sharedContainerOrApplicationSupport(for identifier: String) -> URL {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier) ?? applicationSupport
  }

  /// Shared protection policy for JSON state written to the shared container.
  static let protectedAtomicWriteOptions: Data.WritingOptions = [
    .atomic,
    .completeFileProtectionUntilFirstUserAuthentication,
  ]

  /// 将已存在的 iOS 数据目录排除在设备备份之外。
  static func excludeFromBackup(_ path: URL, fileManager: FileManager = .default) {
    guard fileManager.fileExists(atPath: path.path) else { return }
    var path = path
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try? path.setResourceValues(values)
  }
}
