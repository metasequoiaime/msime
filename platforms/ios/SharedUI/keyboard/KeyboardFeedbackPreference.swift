import UIKit

/// 振动强度的唯一一份映射：键盘、App 的「试一下振动」都从这里取样式和力度。Tauri 插件（`crates/tauri-mobile-platform/ios`）是另一个 Swift 包，引用不到这里，`previewKeyboardHaptics` 里抄了同样的数值，改这里要一起改。
///
/// 原先中、强两档的力度都是 1.0，只差在样式 `.medium` 和 `.heavy`，单次轻敲几乎分不出来；现在三档的样式和力度都拉开。
enum KeyboardHapticStrength: String, CaseIterable {
  /// `system` 是「跟随系统」。iOS 没有接口读取系统设置里的「键盘反馈 → 触感」开关和强度，所以这一档的意思是用系统默认的冲击反馈：`.light` 样式、`impactOccurred()` 不带力度参数，不再套我们自己的力度。
  case system, light, medium, strong
  var title: String {
    switch self { case .system: "跟随系统"; case .light: "轻"; case .medium: "中"; case .strong: "强" }
  }
  var style: UIImpactFeedbackGenerator.FeedbackStyle {
    switch self { case .system, .light: .light; case .medium: .medium; case .strong: .heavy }
  }
  /// nil 表示不覆盖系统默认力度（`system`）。
  var intensity: CGFloat? {
    switch self { case .system: nil; case .light: 0.5; case .medium: 0.8; case .strong: 1.0 }
  }

  /// 按这一档振一下。所有触发振动的地方都走这里，`system` 才不会在某一处被套上力度。
  func impact(_ generator: UIImpactFeedbackGenerator) {
    if let intensity { generator.impactOccurred(intensity: intensity) } else { generator.impactOccurred() }
  }
}

enum KeyboardFeedbackPreference {
  static let soundKey = "keyboardSoundEnabled"
  static let hapticsKey = "keyboardHapticsEnabled"
  static let strengthKey = "keyboardHapticStrength"
  static var defaults: UserDefaults {
    UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard
  }

  static var soundEnabled: Bool {
    defaults.object(forKey: soundKey) as? Bool ?? true
  }

  /// Only iPhones have the Taptic Engine keyboard feedback drives. Elsewhere the vibration controls are hidden, but the stored value is left alone: settings sync still carries it to the user's phone.
  static var hapticsAvailable: Bool { UIDevice.current.userInterfaceIdiom == .phone }

  static var hapticsEnabled: Bool {
    defaults.bool(forKey: hapticsKey)
  }
  static var hapticStrength: KeyboardHapticStrength {
    KeyboardHapticStrength(rawValue: defaults.string(forKey: strengthKey) ?? "") ?? .medium
  }
}
