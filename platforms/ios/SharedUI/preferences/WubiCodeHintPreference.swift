import Foundation

/// 控制五笔候选旁的剩余编码提示。
///
/// 选择放在共享偏好文档（`wubi_code_hint`）里，设置页写、键盘经会话读，所以 App Group 里不留任何东西。有界的纯函数保证回退或其他无关候选的编码不会提示出无效按键。
@MainActor
enum WubiCodeHintPreference {
  static let documentKey = "wubi_code_hint"
  /// 文档里没有这个键时的值：接入共享偏好之前它只有一种实际行为，就是开。
  nonisolated static let documentDefault = true
  nonisolated static let maxCodeLength = 64

  /// 文档里的值；缺这个键时取 `documentDefault`。
  static func isEnabled(in document: [String: Any]?) -> Bool {
    document?[documentKey] as? Bool ?? documentDefault
  }

  nonisolated static func hint(
    code: String, typed: String, answeredByPinyinFallback: Bool
  ) -> String {
    guard !answeredByPinyinFallback, !typed.isEmpty,
          code.count <= maxCodeLength, typed.count <= maxCodeLength,
          code.count > typed.count, code.hasPrefix(typed) else { return "" }
    return String(code.dropFirst(typed.count))
  }
}
