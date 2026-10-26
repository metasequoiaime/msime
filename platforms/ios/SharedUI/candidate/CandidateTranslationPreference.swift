import Foundation

struct CandidateTranslationLanguage: Equatable, Sendable {
  let title: String
  let code: String
}

/// 候选词翻译的范围，取值与共享 Rust 的 `CandidateTranslationScope`（共享文档键 `candidate_translation_scope`）相同。只管候选词翻译（在线、账号、AI 翻译服务）；「显示英文释义」的离线释义和离线释义开关打开的非英文离线词典不受它影响，照旧每个候选都有。
enum CandidateTranslationScope: String, CaseIterable, Sendable {
  /// 仅当前高亮：只翻译高亮的候选，高亮移动时译文跟着走。新装和缺键的老用户都是它。
  case highlighted
  /// 全部：翻译这一页的每个候选（旧行为）。
  case all

  /// 共享文档里的值：缺键或不认识的值按「仅当前高亮」读，与共享 Rust 的反序列化一致。
  init(sharedValue: Any?) {
    self = (sharedValue as? String).flatMap(Self.init(rawValue:)) ?? .highlighted
  }
}

enum CandidateTranslationPreference {
  static let primaryKey = "candidate.translationLanguage"
  static let secondaryKey = "candidate.translationSecondaryLanguage"
  static let onlineKey = "candidate.translationOnline"
  static let scopeKey = "candidate.translationScope"
  static var defaults: UserDefaults {
    InputSchemePreference.appGroupDefaults
  }
  static let languages = [
    CandidateTranslationLanguage(title: "英语", code: "EN"),
    CandidateTranslationLanguage(title: "日语", code: "JA"),
    CandidateTranslationLanguage(title: "韩语", code: "KO"),
    CandidateTranslationLanguage(title: "西班牙语", code: "ES"),
    CandidateTranslationLanguage(title: "法语", code: "FR"),
    CandidateTranslationLanguage(title: "德语", code: "DE"),
    // Appended rather than sorted in: the App Group stores positions in this list.
    CandidateTranslationLanguage(title: "俄语", code: "RU"),
  ]
  static func language(at index: Int) -> CandidateTranslationLanguage {
    languages.indices.contains(index) ? languages[index] : languages[0]
  }
  static var primaryIndex: Int {
    get { languageIndex(defaults.object(forKey: primaryKey) as? Int ?? 0, fallback: 0) }
    set { defaults.set(languageIndex(newValue, fallback: 0), forKey: primaryKey) }
  }
  static var secondaryIndex: Int {
    get {
      guard let value = defaults.object(forKey: secondaryKey) as? Int else { return -1 }
      return languages.indices.contains(value) ? value : -1
    }
    set { defaults.set(languages.indices.contains(newValue) ? newValue : -1, forKey: secondaryKey) }
  }
  static var onlineEnabled: Bool {
    get { defaults.object(forKey: onlineKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: onlineKey) }
  }
  /// App Group 里的范围镜像；没写过时按「仅当前高亮」读。
  static var translationScope: CandidateTranslationScope {
    get { CandidateTranslationScope(sharedValue: defaults.string(forKey: scopeKey)) }
    set { defaults.set(newValue.rawValue, forKey: scopeKey) }
  }
  /// 范围生效：候选词翻译开着且范围是「仅当前高亮」，与共享 Rust 的 `translation_scope_highlighted` 是同一条判断。这时候选词翻译只为视图里高亮的那一个候选请求、只画在那一行。
  static var scopedToHighlight: Bool { onlineEnabled && translationScope == .highlighted }
  /// 候选条第 `index` 个候选画不画宿主缓存里的候选词翻译：范围生效时只有视图里高亮的那一个画，其他候选只画离线释义；范围不生效时每个都画。
  static func drawsTranslation(at index: Int, highlighted: [Bool], scoped: Bool) -> Bool {
    !scoped || (highlighted.indices.contains(index) && highlighted[index])
  }
  static var primary: CandidateTranslationLanguage { language(at: primaryIndex) }
  /// 手机键盘只显示一种候选翻译：第二种语言会让每个候选下再多一行，候选栏太高，所以触屏键盘不取它，行高、翻译请求和离线释义都只按首选语言。共享文档里的 `translation_secondary_language`（`secondaryIndex`）照样同步、原样保留，桌面端仍用它。
  static var secondary: CandidateTranslationLanguage? { nil }
  /// `offline` is the codes whose offline gloss dictionary is installed (see `offlineGlossLanguages`); English always comes from the bundled msime-english.db.
  static func needsNetwork(_ language: CandidateTranslationLanguage, offline: Set<String> = []) -> Bool {
    language.code != "EN" && !offline.contains(language.code)
  }
  /// The languages an offline gloss dictionary can exist for, as host-api's OFFLINE_GLOSS_LANGUAGES names them.
  static let offlineGlossCodes = ["FR", "JA", "ES", "RU", "DE", "KO"]
  /// Which of them are installed beside the resource directory: `offline-glosses/zh-<code>.db`, the sibling host-api reads the non-English glosses from. The bundle does not change while the keyboard runs, so a caller may keep the answer.
  static func offlineGlossLanguages(resources: String?) -> Set<String> {
    guard let resources, !resources.isEmpty else { return [] }
    let directory = URL(fileURLWithPath: resources, isDirectory: true).deletingLastPathComponent()
      .appendingPathComponent("offline-glosses", isDirectory: true)
    return Set(offlineGlossCodes.filter {
      FileManager.default.fileExists(atPath: directory.appendingPathComponent("zh-\($0.lowercased()).db").path)
    })
  }
  private static func languageIndex(_ value: Int, fallback: Int) -> Int {
    languages.indices.contains(value) ? value : fallback
  }
}
