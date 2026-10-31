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

/// 候选英文音标的取词、请求与回答校验，规则与 Android 的 `CandidatePronunciationPolicy`、`CandidatePronunciationModel` 相同：英文候选读它本身，中文候选读画出来的英语释义的第一行，首个词组和去冠词交给共享层。
///
/// 音标只取决于查的那段文字和本机的表，与候选代次无关，所以键盘按文字缓存回答，请求的代次固定为 0。
enum CandidatePronunciationModel {
  /// 一次查询最多带这么多段文字；缓存超过它的八倍时整份清空。
  static let maxItems = 256

  /// 交给共享表的文字，没有可读的英文时为 nil。`englishGloss` 是这一行画出来的英语释义（没有就传 nil）。
  static func lookupText(candidate: String, englishGloss: String?) -> String? {
    if plainEnglish(candidate) { return CandidateGlossModel.isBounded(candidate) ? candidate : nil }
    guard let englishGloss else { return nil }
    let first = englishGloss.split(whereSeparator: \.isNewline).first.map(String.init)?
      .trimmingCharacters(in: .whitespaces) ?? ""
    return !first.isEmpty && CandidateGlossModel.isBounded(first) ? first : nil
  }

  /// 去重后的有界请求，最多 `maxItems` 段。
  static func request(texts: [String], supplementaryResources: String?) throws -> Data {
    var seen = Set<String>()
    var items: [[String: Any]] = []
    for text in texts where !text.isEmpty && CandidateGlossModel.isBounded(text) && seen.insert(text).inserted {
      items.append(["text": text, "language": "en"])
      if items.count >= maxItems { break }
    }
    guard !items.isEmpty else { throw CandidateGlossModel.Failure.invalidRequest }
    var object: [String: Any] = ["generation": 0, "items": items]
    if let supplementaryResources, !supplementaryResources.isEmpty {
      guard NSString(string: supplementaryResources).isAbsolutePath,
            supplementaryResources.utf8.count <= CandidateGlossModel.maxEntryBytes else {
        throw CandidateGlossModel.Failure.invalidRequest
      }
      object["supplementary_resources"] = supplementaryResources
    }
    let data = try JSONSerialization.data(withJSONObject: object)
    guard data.count <= CandidateGlossModel.maxRequestBytes else { throw CandidateGlossModel.Failure.oversized }
    return data
  }

  /// 文字到 `/…/` 音标。没有音标的文字不在回答里；形状不对的条目丢掉，整份不对时抛错，键盘不缓存。
  static func decode(_ value: [String: Any]) throws -> [String: String] {
    guard let entries = value["pronunciations"] as? [[String: Any]], entries.count <= 4096 else {
      throw CandidateGlossModel.Failure.invalidResponse
    }
    var readings: [String: String] = [:]
    for entry in entries {
      guard entry["language"] as? String == "en", let text = entry["text"] as? String,
            let reading = entry["pronunciation"] as? String,
            CandidateGlossModel.isBounded(text), CandidateGlossModel.isBounded(reading),
            reading.count >= 3, reading.hasPrefix("/"), reading.hasSuffix("/") else { continue }
      readings[text] = reading
    }
    return readings
  }

  private static func plainEnglish(_ text: String) -> Bool {
    var letter = false
    for scalar in text.unicodeScalars {
      switch scalar {
      case "A"..."Z", "a"..."z": letter = true
      case " ", "-", "'": continue
      default: return false
      }
    }
    return letter
  }
}
