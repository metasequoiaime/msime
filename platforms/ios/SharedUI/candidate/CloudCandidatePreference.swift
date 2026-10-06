import Foundation

/// Whether the keyboard may ask the cloud candidate service while a composition is paused.
///
/// 共享偏好文档里的 `cloud_candidates` 新装是关闭的，但旧文档或从桌面同步来的文档可能是开启。iOS 键盘默认离线并在 App 里这样写明，所以会话的 `cloud_candidates` 只看这个开关：它存在 App Group 里、起始关闭，由 bridge 盖在共享文档的值之上。于是一份云候选开着的桌面文档同步过来，也不会让 iPhone 自己把输入发给远程服务。请求另外还需要键盘的「完全访问」。
enum CloudCandidatePreference {
  static let key = "candidate.cloudCandidates"

  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }

  static var enabled: Bool {
    get { defaults.object(forKey: key) as? Bool ?? false }
    set { defaults.set(newValue, forKey: key) }
  }
}
