import Foundation

/// 混合滑行英文候选不显示解码拼写或冗余英语释义，首选和其他备选采用同一规则。
enum BilingualCandidateCaptionPolicy {
  static func englishText(_ text: String) -> Bool {
    let bytes = text.utf8
    return bytes.allSatisfy { $0 < 128 }
      && bytes.contains { (65...90).contains($0) || (97...122).contains($0) }
  }

  static func gloss(_ value: String?, candidate: String, target: String, enabled: Bool) -> String? {
    guard let value, enabled else { return value }
    let text = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return nil }
    if target.uppercased() == "EN", englishText(candidate), englishText(text) { return nil }
    return value
  }

  static func annotation(_ value: String, candidate: String, enabled: Bool) -> String {
    guard enabled, englishText(candidate), englishText(value) else { return value }
    return ""
  }
}
