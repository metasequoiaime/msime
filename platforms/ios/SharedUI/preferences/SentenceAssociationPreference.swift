import Foundation

/// 整句联想 in the shared document's `sentence_association`, with the same three levels as the Android 表达 page and the shared settings page: 关闭 turns the word lattice off, 标准 (the default) is the lattice alone, and 增强 adds the keyboard sentence model (`neural_keyboard`).
///
/// 增强 is opt-in because the model runs inside every keystroke. Before host-api honoured the switch it ran for everyone whose resources ship the model, and iOS had no way to say which level it wanted; this page is that way.
enum SentenceAssociationPreference {
  enum Level: Int, CaseIterable, Identifiable {
    case off, standard, enhanced

    var id: Int { rawValue }

    var title: String {
      switch self {
      case .off: return "关闭"
      case .standard: return "标准"
      case .enhanced: return "增强"
      }
    }
  }

  static let documentKey = "sentence_association"

  /// The level a document asks for. A missing object or field reads as its default (`word_lattice` on, `neural_keyboard` off), which is 标准, and a lattice that is off is 关闭 whatever the model switch says, as on Android.
  static func level(in document: [String: Any]?) -> Level {
    let association = document?[documentKey] as? [String: Any]
    let lattice = association?["word_lattice"] as? Bool ?? true
    let neural = association?["neural_keyboard"] as? Bool ?? false
    if !lattice { return .off }
    return neural ? .enhanced : .standard
  }

  /// Write `level` into `sentence_association`, keeping its other fields (`neural_desktop`, `show_next_on_duplicate`): the document rejects unknown fields there, so only the two this page owns are set.
  @discardableResult
  static func save(_ level: Level, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) {
      var association = $0[documentKey] as? [String: Any] ?? [:]
      association["word_lattice"] = level != .off
      association["neural_keyboard"] = level == .enhanced
      $0[documentKey] = association
    }
  }
}
