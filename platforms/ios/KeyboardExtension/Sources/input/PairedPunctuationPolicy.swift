import Foundation

/// A closing mark the keyboard writes after the Engine commits the opening one, for 成对标点自动补全.
struct PairedPunctuationCompletion: Equatable {
  /// The ASCII key that opened the pair.
  let opening: String
  let closing: String
}

/// Host-side completion for punctuation pairs that the Engine opens one mark at a time. The Harmony keyboard's `PairedPunctuationPolicy` does the same.
enum PairedPunctuationPolicy {
  private static let completions: [(opening: String, openingMark: String, closing: String)] = [
    ("\"", "“", "”"), ("'", "‘", "’"), ("(", "（", "）"),
    ("<", "《", "》"), ("<", "〈", "〉"), ("[", "【", "】"),
  ]

  /// With pairing on, every quote press opens a fresh pair, as the reference's `KeyHandler.cpp` does. The Engine alternates the quote keys (“ then ”) because a host without pairing needs that, but a host that supplies the closing half itself never sends the press that would have produced it, so the next quote would otherwise arrive as a lone ”. A closing quote at the end of the commit is therefore rewritten to the opening one before the pair is completed; with pairing off the commit is left to the Engine's alternation.
  static func reopenQuote(_ commit: String?, ascii: String, enabled: Bool) -> String? {
    guard enabled, let commit else { return commit }
    if ascii == "\"", commit.hasSuffix("”") { return String(commit.dropLast()) + "“" }
    if ascii == "'", commit.hasSuffix("’") { return String(commit.dropLast()) + "‘" }
    return commit
  }

  /// A closing mark only when the Engine's commit ends in a known opening mark.
  static func completion(_ commit: String?, enabled: Bool) -> PairedPunctuationCompletion? {
    guard enabled, let commit, !commit.isEmpty else { return nil }
    return completions.first { commit.hasSuffix($0.openingMark) }
      .map { PairedPunctuationCompletion(opening: $0.opening, closing: $0.closing) }
  }
}

/// The pairs the keyboard has closed whose closing half still waits to the right of the caret, innermost last, as the reference's `_pairedPunctuationStack` keeps them.
///
/// Typing the closing key of the innermost pair steps the caret over the closing half already there instead of writing a second one: without this （内容 followed by ） reads （内容））, and because every quote press opens a fresh pair (`reopenQuote`), the closing quote could not be typed at all. Anything that deletes text or moves the caret clears the stack, and a pair only counts in the editor it was opened in.
struct PairedPunctuationStack {
  private struct Entry: Equatable {
    let closing: String
    let editor: UInt64
  }

  /// Closing marks a key can step over. `<` opens either 《 or 〈 depending on nesting, so `>` matches both.
  private static let closings: [String: Set<String>] = [
    "\"": ["”"], "'": ["’"], ")": ["）"], "]": ["】"], ">": ["》", "〉"],
  ]
  /// The reference's cap on remembered pairs; deeper nesting forgets the outermost.
  static let limit = 16

  private var entries: [Entry] = []

  var isEmpty: Bool { entries.isEmpty }

  mutating func push(closing: String, editor: UInt64) {
    guard editor != 0 else { return }
    entries.append(Entry(closing: closing, editor: editor))
    if entries.count > Self.limit { entries.removeFirst(entries.count - Self.limit) }
  }

  mutating func clear() { entries.removeAll() }

  /// Whether pressing `ascii` steps over the innermost closing half, popping it if so.
  ///
  /// `following` is the text right after the caret. A host that reports nothing there (`nil`) leaves the stack as the only evidence, which the reference trusts too; a host that reports anything else proves the closing half has gone (a tap elsewhere, an edit by the app), so the stack is cleared. A key that closes nothing leaves the stack alone, since typing inside a pair keeps the caret in front of its closing half.
  mutating func stepOver(ascii: String, editor: UInt64, following: String?) -> Bool {
    guard let candidates = Self.closings[ascii], let top = entries.last else { return false }
    guard candidates.contains(top.closing), top.editor == editor, editor != 0 else {
      clear()
      return false
    }
    if let following, !following.hasPrefix(top.closing) {
      clear()
      return false
    }
    entries.removeLast()
    return true
  }
}

extension MetasequoiaInputSnapshot {
  /// The same snapshot committing `text` instead.
  func replacingCommit(_ text: String?) -> MetasequoiaInputSnapshot {
    MetasequoiaInputSnapshot(
      isHandled: isHandled, commitText: text, preedit: preedit, reading: reading, phrasePrefix: phrasePrefix,
      candidates: candidates, candidateCodes: candidateCodes, candidateGlosses: candidateGlosses,
      candidateAnnotations: candidateAnnotations, candidateSources: candidateSources,
      candidateFixedPositions: candidateFixedPositions, candidatePageCount: candidatePageCount,
      answeredByPinyinFallback: answeredByPinyinFallback, diagnosticText: diagnosticText, localMode: localMode,
      nineKeySpellings: nineKeySpellings, nineKeyReading: nineKeyReading,
      nineKeySingleCharacter: nineKeySingleCharacter, nineKeyStrokes: nineKeyStrokes,
      editingText: editingText, caretPosition: caretPosition)
  }
}
