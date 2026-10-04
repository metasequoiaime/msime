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

  static func decode(_ value: [String: Any]) throws -> CandidatePanelSnapshot {
    guard let generationValue = value["generation"] as? NSNumber,
          let generation = CandidateGlossModel.integerValue(generationValue, maximum: UInt64.max),
          let preedit = value["preedit"] as? String,
          CandidateGlossModel.isBounded(preedit, allowingEmpty: true),
          let candidates = value["candidates"] as? [[String: Any]],
          candidates.count <= 4096 else { throw Failure.invalidResponse }
    var seen = Set<UInt64>()
    let entries = try candidates.map { candidate -> Entry in
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
      return Entry(text: text, code: code, translation: translation, annotation: annotation,
                   source: (candidate["source"] as? NSNumber)?.intValue ?? 0,
                   fixedPosition: (candidate["fixed_position"] as? NSNumber)?.intValue ?? 0, index: index)
    }
    return CandidatePanelSnapshot(generation: generation, preedit: preedit, entries: entries)
  }
}
