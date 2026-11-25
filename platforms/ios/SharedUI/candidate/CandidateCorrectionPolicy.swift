import Foundation

/// 引擎改过拼写的候选（View JSON 的 `corrected`）在候选条和展开网格里的画法与读法，与 Android 的 `CandidateCorrectionMarkPolicy`、Windows/macOS/Linux 候选窗的 `*` 同一条规则（#11844）。
enum CandidateCorrectionPolicy {
  /// 纠错候选在候选字后面画的标记；只是画出来的，不上屏，也不参与选择。
  static let mark = "*"

  /// 候选字后面跟 `mark`。读屏念的是候选字本身，不经过这里。
  static func markedWord(_ text: String, corrected: Bool) -> String {
    corrected ? text + mark : text
  }

  /// 候选字下面的注释。纠错行没有辅助码时，引擎把用户打的原串（`corrected_from`，如 `zhognguo`）放进注释，它和读音行、输入框里的拼写是同一串，画在每个纠错候选下面只是重复，这里去掉；辅助码以「(」开头，照旧画；五笔方案里混输拼音的纠错行是「原串 五笔码」，只去掉打头的原串。
  static func visibleAnnotation(_ annotation: String, corrected: Bool) -> String {
    guard corrected, let raw = correctedFrom(annotation) else { return annotation }
    return annotation.dropFirst(raw.count).trimmingCharacters(in: .whitespaces)
  }

  /// 注释里打头的原串：纠错行没有辅助码时，它就是引擎放进去的 `corrected_from`；带辅助码（「(」开头）或注释为空时没有。
  static func correctedFrom(_ annotation: String) -> String? {
    guard !annotation.isEmpty, !annotation.hasPrefix("(") else { return nil }
    let raw = annotation.split(separator: " ", maxSplits: 1, omittingEmptySubsequences: false)[0]
    return raw.isEmpty ? nil : String(raw)
  }

  /// 读屏念注释时的说法。原串是纠错前的拼写，不是还要输入的键，所以不说成「还需输入」；带辅助码或五笔码时照旧念剩下的那段。
  static func spokenAnnotation(_ annotation: String, corrected: Bool) -> String? {
    guard corrected, let raw = correctedFrom(annotation) else { return nil }
    return "纠错前的拼写 \(raw)"
  }
}
