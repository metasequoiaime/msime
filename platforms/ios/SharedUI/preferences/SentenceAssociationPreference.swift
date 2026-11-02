import Foundation

/// 整句联想 in the shared document's `sentence_association`, with the same three levels as the Android 表达 page and the shared settings page: 关闭 turns the word lattice off, 标准 is the lattice alone, and 增强 (the default) adds the keyboard sentence model (`neural_keyboard`).
///
/// 增强 is the default because host-api attaches the model only where this device runs it inside the keystroke budget (the large or the small ONNX model by its own measurement); where it does not, 增强 types like 标准. The model runs inside every keystroke, so 标准 stays as the way to turn it off.
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

  /// The level a document asks for. A missing object or field reads as its default (`word_lattice` on, `neural_keyboard` on), which is 增强, and a lattice that is off is 关闭 whatever the model switch says, as on Android.
  static func level(in document: [String: Any]?) -> Level {
    let association = document?[documentKey] as? [String: Any]
    let lattice = association?["word_lattice"] as? Bool ?? true
    let neural = association?["neural_keyboard"] as? Bool ?? true
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
