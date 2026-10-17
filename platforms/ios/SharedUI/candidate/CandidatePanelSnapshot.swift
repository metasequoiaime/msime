import Foundation

/// A bounded, generation-stable copy of every candidate used by expanded keyboard panels.
struct CandidatePanelSnapshot: Equatable, Sendable {
  struct Entry: Equatable, Sendable {
    let text: String
    let code: String
    let translation: String
    /// The Engine's display suffix: a helpcode, or the spelling a correction replaced.
    var annotation = ""
    var source = 0
    var fixedPosition = 0
    let index: UInt64
  }

  let generation: UInt64
  let preedit: String
  let entries: [Entry]

  enum Failure: Error {
    case invalidResponse
  }

  /// 换了一代之后，是不是最多只有 AI 行变了：组字相同，AI 以外的各行逐行相同（文字和来源，先后不变）。AI 回答到达时第一条插在第一页最后一格，原来那一格和后面的行各往后挪一格，同一批多出来的接在末尾；回答的词全都已在列表里时引擎什么都不插，却照样接受并换一代，列表一行都没变。两种情况展开的候选网格都留着、原地换上新的一代，而不是在用户看着它时收起（#6888）。AI 行是来源 3，以及共享内核已经不再产出的占位行来源 12。
  func onlyTheAISeatChanged(since previous: CandidatePanelSnapshot) -> Bool {
    guard preedit == previous.preedit else { return false }
    return entries.lazy.filter { !Self.isAIRow($0) }
      .elementsEqual(previous.entries.lazy.filter { !Self.isAIRow($0) }) { $0.text == $1.text && $0.source == $1.source }
  }

  private static func isAIRow(_ entry: Entry) -> Bool {
    entry.source == CandidateMarker.aiSource || entry.source == CandidateMarker.aiPendingSource || entry.source == CandidateMarker.cloudPendingSource
  }

  static func decode(_ value: [String: Any]) throws -> CandidatePanelSnapshot {
    guard let generationValue = value["generation"] as? NSNumber,
          let generation = CandidateGlossModel.integerValue(generationValue, maximum: UInt64.max),
          let preedit = value["preedit"] as? String,
          CandidateGlossModel.isBounded(preedit, allowingEmpty: true),
          let candidates = value["candidates"] as? [[String: Any]],
          candidates.count <= 4096 else { throw Failure.invalidResponse }
    var seen = Set<UInt64>()
    let entries = try candidates.compactMap { candidate -> Entry? in
      guard let text = candidate["text"] as? String,
            CandidateGlossModel.isBounded(text),
            let identity = candidate["id"] as? [String: Any],
            let identityGeneration = identity["generation"] as? NSNumber,
            CandidateGlossModel.integerValue(identityGeneration, maximum: UInt64.max) == generation,
            let indexValue = identity["index"] as? NSNumber,
            let index = CandidateGlossModel.integerValue(indexValue, maximum: UInt64.max) else {
        throw Failure.invalidResponse
      }
      guard index < UInt64(candidates.count), seen.insert(index).inserted else {
        throw Failure.invalidResponse
      }
      let code = candidate["code"] as? String ?? ""
      let translation = candidate["translation"] as? String ?? ""
      let annotation = candidate["annotation"] as? String ?? ""
      guard code.utf8.count <= WubiCodeHintPreference.maxCodeLength,
            CandidateGlossModel.isBounded(translation, allowingEmpty: true),
            CandidateGlossModel.isBounded(annotation, allowingEmpty: true) else {
        throw Failure.invalidResponse
      }
      let source = try optionalInteger(candidate["source"])
      let fixedPosition = try optionalInteger(candidate["fixed_position"])
      if !CandidateMarker.isVisible(source: source) { return nil }
      return Entry(text: text, code: code, translation: translation, annotation: annotation,
                   source: source, fixedPosition: fixedPosition, index: index)
    }
    return CandidatePanelSnapshot(generation: generation, preedit: preedit, entries: entries)
  }

  private static func optionalInteger(_ value: Any?) throws -> Int {
    guard let value else { return 0 }
    guard let number = value as? NSNumber,
          let integer = CandidateGlossModel.integerValue(number, maximum: UInt64(Int.max)) else {
      throw Failure.invalidResponse
    }
    return Int(integer)
  }
}
