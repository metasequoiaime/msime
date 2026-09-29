import Foundation

/// Bounded request/response validation for the session-free offline gloss worker.
enum CandidateGlossModel {
  static let maxEntryBytes = 4096
  static let maxRequestBytes = 262_144
  static let maxResponseBytes = 1_048_576

  enum Failure: Error {
    case invalidRequest
    case invalidResponse
    case oversized
  }

  static func isBounded(_ value: String, allowingEmpty: Bool = false) -> Bool {
    (allowingEmpty || !value.isEmpty) && value.utf8.count <= maxEntryBytes
  }

  /// `targetLanguage` is one of `CandidateTranslationPreference.offlineGlossCodes`, to read that language's offline dictionary instead of English.
  static func request(generation: UInt64, candidates: [[String: Any]], targetLanguage: String? = nil) throws -> Data {
    guard !candidates.isEmpty, candidates.count <= 4096 else { throw Failure.invalidRequest }
    let copied = try candidates.map { candidate -> [String: Any] in
      guard let text = candidate["text"] as? String,
            let source = candidate["source"] as? NSNumber,
            source.intValue >= 0, source.intValue <= 255,
            isBounded(text) else { throw Failure.invalidRequest }
      return ["text": text, "source": source.intValue]
    }
    var object: [String: Any] = ["generation": generation, "candidates": copied]
    if let targetLanguage {
      guard CandidateTranslationPreference.offlineGlossCodes.contains(targetLanguage) else { throw Failure.invalidRequest }
      object["target_language"] = targetLanguage.lowercased()
    }
    let data = try JSONSerialization.data(withJSONObject: object)
    guard data.count <= maxRequestBytes else { throw Failure.oversized }
    return data
  }

  static func decode(_ value: [String: Any]) throws -> (generation: UInt64, translations: Data) {
    guard let generationValue = value["generation"] as? NSNumber,
          generationValue.int64Value >= 0,
          let entries = value["translations"] as? [[String: Any]], entries.count <= 4096 else {
      throw Failure.invalidResponse
    }
    let generation = generationValue.uint64Value
    for entry in entries {
      guard let text = entry["text"] as? String,
            let translation = entry["translation"] as? String,
            isBounded(text), isBounded(translation) else { throw Failure.invalidResponse }
    }
    let translations = try JSONSerialization.data(withJSONObject: entries)
    guard translations.count <= maxResponseBytes else { throw Failure.oversized }
    return (generation, translations)
  }

}
