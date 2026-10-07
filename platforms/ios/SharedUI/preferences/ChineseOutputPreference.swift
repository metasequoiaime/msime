import Foundation

// Output script shared by the host app and the keyboard extension, kept in the App Group.
enum ChineseOutputPreference {
  private static let key = "chineseOutputUsesTraditional"

  static var usesTraditional: Bool {
    get { defaults.bool(forKey: key) }
    set { defaults.set(newValue, forKey: key) }
  }

  /// Save the output form where the keyboard reads it: the keyboard copies the shared document's `traditional_chinese_output` over the App Group every time it appears.
  @discardableResult
  static func save(_ traditional: Bool, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { $0[documentKey] = traditional })
    else { return false }
    usesTraditional = traditional
    return true
  }

  static let documentKey = "traditional_chinese_output"

  /// 文档里记的输出字形；没有记过时为 nil。
  static func traditional(in document: [String: Any]?) -> Bool? {
    document?[documentKey] as? Bool
  }

  /// 按这份文档会用的输出字形：文档记了以文档为准，没记时用 App Group 镜像，因为键盘这时也按镜像行事。
  ///
  /// 权威来源是共享文档：设置页、云端同步和键盘都写它，App Group 里的 `usesTraditional` 只是镜像，可能落后于文档（键盘先写镜像、文档那边没写进去，或者文档被别的写入方改过）。要把字形带出本机的地方（上传设置）从这里取值，而不是直接读镜像，否则会把镜像里的旧字形传到云端，再经「下载并应用」写回来。
  static func current(in document: [String: Any]?) -> Bool {
    traditional(in: document) ?? usesTraditional
  }

  /// 把文档里记的输出字形抄进 App Group 镜像；文档没记时镜像保持原值。
  static func mirror(_ document: [String: Any]?) {
    if let traditional = traditional(in: document), traditional != usesTraditional { usesTraditional = traditional }
  }

  private static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }
}
