import Foundation

/// A point where the keyboard has to end an open composition.
enum CompositionBoundary {
  /// The 中/英 key, Shift in a scheme without helper codes, or the globe key.
  case modeSwitch
  /// Return while composing.
  case returnKey
  /// Anything else that takes the text away from the composition: a cursor move, a panel, the keyboard going away.
  case deactivate
}

enum CompositionBoundaryAction: Equatable {
  case none
  case commitRaw
  case finishComposition
  /// MSIME_COMMIT_CANDIDATE: choose the leading Hanja of an open Korean Hanja list.
  case commitCandidate
}

/// What a boundary does to an open composition, the rule the Windows host and the HarmonyOS keyboard share: switching modes or pressing Return keeps what was typed as typed (any half-chosen phrase, then the raw letters), so `iphone` + Return is `iphone` and a wubi code + 英 is the code, while leaving the composition any other way commits the conversion. Japanese always converts, since its raw romaji is not what anyone meant to write, and so does nine-key, whose raw keys are digits rather than letters.
///
/// A Korean syllable, a Zhuyin conversion and a Vietnamese word are already the text being written, so every boundary commits them. Return takes the raw commit, which the runtime answers unhandled, so the newline (or the field's send action) still follows the syllable as it does on every Korean keyboard; the other boundaries finish it. With the syllable's Hanja list open Return chooses a Hanja instead, as Space does: the Korean contract in msime_client.h sends MSIME_COMMIT_CANDIDATE for it, which commits the Hanja handled, so no newline follows. The other boundaries still finish, which closes the list and commits the Hangul. Zhuyin and Vietnamese take the same raw commit on Return: the runtime answers a Vietnamese word unhandled, so the newline follows it, and commits the Zhuyin conversion handled, so Return only confirms it, as on the Dachen keyboards; their other boundaries finish, which in Zhuyin commits the conversion whether or not its list is open.
enum CompositionBoundaryPolicy {
  static func action(composing: Bool, scheme: ChineseInputScheme, boundary: CompositionBoundary,
                     koreanHanjaListOpen: Bool = false) -> CompositionBoundaryAction {
    guard composing else { return .none }
    if scheme.isKorean {
      guard boundary == .returnKey else { return .finishComposition }
      return koreanHanjaListOpen ? .commitCandidate : .commitRaw
    }
    if scheme.isZhuyin || scheme.isVietnamese {
      return boundary == .returnKey ? .commitRaw : .finishComposition
    }
    if boundary == .deactivate || scheme.isJapanese || scheme == .nineKey { return .finishComposition }
    return .commitRaw
  }
}
