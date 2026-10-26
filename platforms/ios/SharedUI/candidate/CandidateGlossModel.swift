import Foundation
import CoreFoundation

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
            let sourceValue = integerValue(source, maximum: 255),
            isBounded(text) else { throw Failure.invalidRequest }
      return ["text": text, "source": sourceValue]
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

  /// 读离线释义 worker 的回答，整理成交给 `applyTranslations` 的数组。每条都标 `"gloss": true`：这些是离线释义，不是候选词翻译，候选词翻译的范围是「仅当前高亮」时共享运行时照样把它们画在每一行；不标的回答会被当作候选词翻译，只画在高亮行。
  static func decode(_ value: [String: Any]) throws -> (generation: UInt64, translations: Data) {
    guard let generationValue = value["generation"] as? NSNumber,
          let generation = integerValue(generationValue, maximum: UInt64.max),
          let entries = value["translations"] as? [[String: Any]], entries.count <= 4096 else {
      throw Failure.invalidResponse
    }
    let answers = try entries.map { entry -> [String: Any] in
      guard let text = entry["text"] as? String,
            let translation = entry["translation"] as? String,
            isBounded(text), isBounded(translation) else { throw Failure.invalidResponse }
      return ["text": text, "translation": translation, "gloss": true]
    }
    let translations = try JSONSerialization.data(withJSONObject: answers)
    guard translations.count <= maxResponseBytes else { throw Failure.oversized }
    return (generation, translations)
  }

  static func integerValue(_ value: NSNumber, maximum: UInt64) -> UInt64? {
    guard let integer = SharedNumber.strictUInt64(value), integer <= maximum else { return nil }
    return integer
  }

}
