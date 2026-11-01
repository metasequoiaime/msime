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

/// 候选读音的取词、请求与回答校验，规则与 Android 的 `CandidatePronunciationPolicy`、`CandidatePronunciationModel` 相同：英文候选读它本身（英文），中文候选读画出来的释义的第一行，按释义的语言读；首个词组和去冠词交给共享层。英文给 `/…/` 音标，法、西、俄、德给维基词典的 IPA（`/…/` 或 `[…]`），日文给赫本式罗马字，韩文给文化观光部式罗马字。
///
/// 读音只取决于查的那段文字、它的语言和本机的表，与候选代次无关，所以键盘按「语言 + 换行 + 文字」这个键（`key`）缓存回答，请求的代次固定为 0：同样的字母在别的语言里是另一个词（法文的 chat）。
enum CandidatePronunciationModel {
  /// 一次查询最多带这么多段文字；缓存超过它的八倍时整份清空。
  static let maxItems = 256
  /// 共享层有读音表的语言，和 `msime_client_pronunciation_request` 认的一样。
  static let languages: Set<String> = ["en", "fr", "ja", "es", "ru", "de", "ko"]

  /// 一段文字和它的语言合成的键；释义只取第一行，文字里不会有换行。
  static func key(language: String, text: String) -> String { "\(language)\n\(text)" }

  /// 交给共享表的那一段的键，没有可读的文字时为 nil。`gloss` 是这一行画出来的释义，`language` 是它的目标语言（小写代码）；没有释义就传 nil。
  static func lookupText(candidate: String, gloss: String?, language: String) -> String? {
    if plainEnglish(candidate) { return CandidateGlossModel.isBounded(candidate) ? key(language: "en", text: candidate) : nil }
    guard let gloss, languages.contains(language) else { return nil }
    let first = gloss.split(whereSeparator: \.isNewline).first.map(String.init)?
      .trimmingCharacters(in: .whitespaces) ?? ""
    return !first.isEmpty && CandidateGlossModel.isBounded(first) && !hasControl(first) ? key(language: language, text: first) : nil
  }

  /// 去重后的有界请求，最多 `maxItems` 段；`keys` 是 `lookupText` 给的键。
  static func request(keys: [String], supplementaryResources: String?) throws -> Data {
    var seen = Set<String>()
    var items: [[String: Any]] = []
    for key in keys where seen.insert(key).inserted {
      guard let cut = key.firstIndex(of: "\n"), cut != key.startIndex else { continue }
      let language = String(key[..<cut])
      let text = String(key[key.index(after: cut)...])
      guard !text.isEmpty, languages.contains(language), CandidateGlossModel.isBounded(text), !hasControl(text) else { continue }
      items.append(["text": text, "language": language])
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

  /// 键到读音。没有读音的文字不在回答里；形状不对的条目丢掉，整份不对时抛错，键盘不缓存。
  static func decode(_ value: [String: Any]) throws -> [String: String] {
    guard let entries = value["pronunciations"] as? [[String: Any]], entries.count <= 4096 else {
      throw CandidateGlossModel.Failure.invalidResponse
    }
    var readings: [String: String] = [:]
    for entry in entries {
      guard let language = entry["language"] as? String, languages.contains(language),
            let text = entry["text"] as? String, let reading = entry["pronunciation"] as? String,
            CandidateGlossModel.isBounded(text), CandidateGlossModel.isBounded(reading),
            wellFormed(language: language, reading: reading) else { continue }
      readings[key(language: language, text: text)] = reading
    }
    return readings
  }

  /// 一条读音的形状对不对：英文是 `/…/`，日文和韩文是罗马字，其余是 `/…/` 或 `[…]`。
  static func wellFormed(language: String, reading: String) -> Bool {
    if language == "ja" || language == "ko" { return !reading.trimmingCharacters(in: .whitespaces).isEmpty }
    guard reading.count >= 3 else { return false }
    if reading.hasPrefix("/"), reading.hasSuffix("/") { return true }
    return language != "en" && reading.hasPrefix("[") && reading.hasSuffix("]")
  }

  /// 读屏念的读音名称：IPA 叫音标，日文和韩文的罗马字叫读音。
  static func accessibilityLabel(_ reading: String) -> String {
    reading.hasPrefix("/") || reading.hasPrefix("[") ? "音标 \(reading)" : "读音 \(reading)"
  }

  /// 共享层的请求只要有一段带控制字符（Rust 的 `char::is_control`，即 Cc 类）就整批拒绝；这样的释义（比如在线或 AI 译文里的制表符）不查，免得一条释义让整页都没有音标。
  private static func hasControl(_ text: String) -> Bool {
    text.unicodeScalars.contains { $0.properties.generalCategory == .control }
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
