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
/// 韩语音节、注音转换结果、越南语单词和藏文音节已经就是正在写的正文，所以每一种边界都上屏它们。回车走原文上屏，运行时对韩语以未处理返回，所以换行（或输入框的发送动作）照常跟在音节后面，与所有韩语键盘一样；其他边界结束组字。音节的汉字列表打开时回车改为选汉字，与空格一样：msime_client.h 的韩语契约为它发 MSIME_COMMIT_CANDIDATE，以已处理上屏汉字，不换行。其他边界仍结束组字，关闭列表并上屏韩文。注音、越南语和藏文在回车时走同样的原文上屏：运行时对越南语单词以未处理返回，所以换行跟在后面；对注音转换结果以已处理上屏，所以回车只是确认，与大千键盘一样；对藏文以已处理上屏转出的藏文（不加音节点），回车同样只是确认。它们的其他边界结束组字，注音不论列表是否打开都上屏转换结果，藏文上屏显示的藏文。
enum CompositionBoundaryPolicy {
  static func action(composing: Bool, scheme: ChineseInputScheme, boundary: CompositionBoundary,
                     koreanHanjaListOpen: Bool = false) -> CompositionBoundaryAction {
    guard composing else { return .none }
    if scheme.isKorean {
      guard boundary == .returnKey else { return .finishComposition }
      return koreanHanjaListOpen ? .commitCandidate : .commitRaw
    }
    if scheme.isZhuyin || scheme.isVietnamese || scheme.isTibetan {
      return boundary == .returnKey ? .commitRaw : .finishComposition
    }
    if boundary == .deactivate || scheme.isJapanese || scheme == .nineKey { return .finishComposition }
    return .commitRaw
  }
}
