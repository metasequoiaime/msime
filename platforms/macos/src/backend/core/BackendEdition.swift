import Foundation

/// 本输入法属于哪个版本，与 src/core/EditionIdentity.h 读的是同一份 Info.plist：没有 `MSIMEEdition` 就是 full，每个值都取 full 今天的那个。
enum BackendEdition {
  static let fullInputMethodBundleIdentifier = "app.msime.inputmethod.MetasequoiaIME"

  private static var isFull: Bool {
    guard let edition = Bundle.main.object(forInfoDictionaryKey: "MSIMEEdition") as? String, !edition.isEmpty else { return true }
    return edition == "full"
  }

  /// 输入法 bundle 的标识，也是统一日志的子系统。
  static var inputMethodBundleIdentifier: String {
    isFull ? fullInputMethodBundleIdentifier : (Bundle.main.bundleIdentifier ?? fullInputMethodBundleIdentifier)
  }

  /// 本版本提供的方案；nil 表示 full，即全部方案。
  static var inputSchemes: [String]? {
    isFull ? nil : Bundle.main.object(forInfoDictionaryKey: "MSIMEInputSchemes") as? [String]
  }
}
