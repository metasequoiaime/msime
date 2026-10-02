import Foundation

enum ChineseInputScheme: String, CaseIterable {
  case quanpin, nineKey, shuangpin, ziranma, microsoft, shoudao, wubi, japaneseNineKey, japanese, korean, handwriting, thoughtfulReply, cantonese, zhuyin, vietnamese

  /// The schemes a fresh install leaves off until the user enables them, as the settings host does (`optInSchemes` in MobilePlatformPlugin).
  static let optInSchemes: [ChineseInputScheme] = [.cantonese, .zhuyin, .vietnamese]

  var isJapanese: Bool { self == .japanese || self == .japaneseNineKey }

  /// Korean Hangul on the Dubeolsik layout: the Engine composes one syllable at a time and offers no candidates.
  var isKorean: Bool { self == .korean }

  /// Cantonese Jyutping on the 26 letter keys: Traditional candidates from the Cantonese dictionary, with nothing learned.
  var isCantonese: Bool { self == .cantonese }

  /// Zhuyin on the Dachen layout: the keys send the ASCII under each bopomofo, and the Engine converts the syllables in place into Traditional Chinese.
  var isZhuyin: Bool { self == .zhuyin }

  /// Vietnamese Telex or VNI on the 26 letter keys: the Engine composes the word in place and offers no candidates.
  var isVietnamese: Bool { self == .vietnamese }

  /// Whether the scheme takes the Mandarin feature set: traditional output conversion, candidate glosses, the local input modes and the candidate menu. Japanese, Korean and Vietnamese write their own scripts, and Cantonese and Zhuyin write Traditional Chinese straight from their own dictionaries without any of these.
  var writesChinese: Bool { !isJapanese && !isKorean && !isCantonese && !isZhuyin && !isVietnamese }

  /// Whether the scheme writes half-width ASCII punctuation, as Korean and Vietnamese do; every other scheme offers Chinese or Japanese marks.
  var writesAsciiPunctuation: Bool { isKorean || isVietnamese }

  /// Whether the composition is a letter spelling with a caret the user can move: the Mandarin schemes and Cantonese. A Japanese reading converts as a whole, and the in-place schemes have no caret inside what they compose.
  var hasSpellingCaret: Bool { writesChinese || isCantonese }

  /// Whether what the Engine composes is already the text: a Korean syllable, a Zhuyin conversion or a Vietnamese word. It is marked inline whatever the preedit setting says, and leaving the scheme or the composition commits it rather than throwing it away.
  var composesInPlace: Bool { isKorean || isZhuyin || isVietnamese }

  /// Whether the scheme reads a dictionary that ships apart from the resource set, and so is offered only where it is installed.
  var needsLanguageDictionary: Bool { isCantonese || isZhuyin }

  /// Whether a held backspace and a quick space-bar flick edit the spelling a syllable at a time. Only a lettered pinyin spelling has syllables to step over: a nine-key digit run is still ambiguous, and a wubi code is not made of syllables, so those keep a hold that clears the composition.
  var editsBySyllable: Bool { self == .quanpin || shuangpinProfile != nil }

  var shuangpinProfile: String? {
    switch self {
    case .shuangpin: "xiaohe"
    case .ziranma, .microsoft, .shoudao: rawValue
    default: nil
    }
  }
  /// Identifier this scheme carries in the shared PreferencesStore. The raw values stay camel case
  /// for Swift and for the App Group mirror, but the shared schema is snake cased and rejects a
  /// document that uses the wrong spelling, which fails the whole preferences update.
  var sharedIdentifier: String {
    switch self {
    case .quanpin: "quanpin"
    case .nineKey: "nine_key"
    case .shuangpin: "xiaohe"
    case .ziranma: "ziranma"
    case .microsoft: "microsoft"
    case .shoudao: "shoudao"
    case .wubi: "wubi"
    case .japaneseNineKey: "japanese_nine_key"
    case .japanese: "japanese"
    case .korean: "korean"
    case .handwriting: "handwriting"
    case .thoughtfulReply: "thoughtful_reply"
    case .cantonese: "cantonese"
    case .zhuyin: "zhuyin"
    case .vietnamese: "vietnamese"
    }
  }
  static func scheme(sharedIdentifier value: String) -> ChineseInputScheme? {
    allCases.first { $0.sharedIdentifier == value }
  }
  var title: String {
    switch self {
    case .quanpin: "全拼 26 键"
    case .nineKey: "全拼 9 键"
    case .shuangpin: "小鹤双拼"
    case .ziranma: "自然码双拼"
    case .microsoft: "微软双拼"
    case .shoudao: "首道双拼"
    case .wubi: "86 五笔"
    case .japanese: "日语 26 键"
    case .japaneseNineKey: "日语 9 键"
    case .korean: "韩语 26 键"
    case .handwriting: "手写"
    case .thoughtfulReply: "高情商回复"
    case .cantonese: "粤拼 26 键"
    case .zhuyin: "大千注音"
    case .vietnamese: "越南语 26 键"
    }
  }
}

enum InputSchemePreference {
  static let enabledSchemesKey = "enabledInputSchemes"
  private static var defaults: UserDefaults { UserDefaults(suiteName: appGroupIdentifier) ?? .standard }

  static var enabledSchemes: [ChineseInputScheme] {
    get {
      guard let stored = defaults.stringArray(forKey: enabledSchemesKey) else {
        return ChineseInputScheme.allCases.filter { !ChineseInputScheme.optInSchemes.contains($0) }
      }
      let enabled = ChineseInputScheme.allCases.filter { stored.contains($0.rawValue) }
      return enabled.isEmpty ? [.quanpin] : enabled
    }
    set {
      let ordered = ChineseInputScheme.allCases.filter { newValue.contains($0) }
      let enabled = ordered.isEmpty ? [.quanpin] : ordered
      defaults.set(enabled.map(\.rawValue), forKey: enabledSchemesKey)
      scheme = scheme
    }
  }

  /// The enabled schemes this process can run: an enabled Cantonese or Zhuyin whose dictionary is not installed is left out, because the Engine would answer it with another scheme. The enabled list itself keeps it, so the choice holds once the dictionary arrives. A process that carries no Engine (the App) cannot tell and leaves every enabled scheme in, so its settings never rewrite a selection the keyboard can run.
  static var offeredSchemes: [ChineseInputScheme] {
    offeredSchemes(enabled: enabledSchemes, installed: installedLanguageSchemes)
  }

  /// `enabled` without the Cantonese and Zhuyin schemes missing from `installed`; nil `installed` means unknown and filters nothing.
  static func offeredSchemes(enabled: [ChineseInputScheme], installed: Set<ChineseInputScheme>?) -> [ChineseInputScheme] {
    guard let installed else { return enabled }
    let offered = enabled.filter { !$0.needsLanguageDictionary || installed.contains($0) }
    return offered.isEmpty ? [.quanpin] : offered
  }

  /// The language dictionary directory host-api reads: `language-dictionaries/` beside the bundle's EngineResources, or nil for a bundle without EngineResources. Only the keyboard extension and its test host carry them; the App bundle runs no Engine.
  static func languageDictionaryDirectory(in bundle: Bundle) -> URL? {
    guard let resources = bundle.resourceURL,
          FileManager.default.fileExists(atPath: resources.appendingPathComponent("EngineResources", isDirectory: true).path)
    else { return nil }
    return resources.appendingPathComponent("language-dictionaries", isDirectory: true)
  }

  /// The schemes whose dictionary `directory` holds, by the file names host-api looks for; nil when there is no directory to look in, which `offeredSchemes` reads as unknown.
  static func installedLanguageSchemes(in directory: URL?) -> Set<ChineseInputScheme>? {
    guard let directory else { return nil }
    let files: [(ChineseInputScheme, String)] = [(.cantonese, "cantonese.db"), (.zhuyin, "zhuyin.db")]
    return Set(files.filter { FileManager.default.fileExists(atPath: directory.appendingPathComponent($0.1).path) }.map(\.0))
  }

  /// Read once: the bundle does not change while the process runs, and `scheme` is read on every keystroke.
  static let installedLanguageSchemes = installedLanguageSchemes(in: languageDictionaryDirectory(in: .main))

  private static let schemeKey = "chineseInputScheme"
  static var scheme: ChineseInputScheme {
    get {
      let defaults = UserDefaults(suiteName: appGroupIdentifier) ?? .standard
      let offered = offeredSchemes
      let stored = defaults.string(forKey: schemeKey).flatMap(ChineseInputScheme.init(rawValue:)) ?? .quanpin
      return offered.contains(stored) ? stored : offered[0]
    }
    set {
      let defaults = UserDefaults(suiteName: appGroupIdentifier) ?? .standard
      let selected = enabledSchemes.contains(newValue) ? newValue : enabledSchemes[0]
      defaults.set(selected.rawValue, forKey: schemeKey)
    }
  }

  /// Save a selection and the enabled list where the keyboard reads them.
  ///
  /// Once the keyboard has recorded a scheme in the shared document it copies that selection over the App Group every time it appears, so a choice written only to the App Group was undone the next time the keyboard opened. The App Group is written after the document, and only when the document took the change.
  @discardableResult
  static func save(scheme: ChineseInputScheme, enabled: [ChineseInputScheme], stateRoot: URL? = nil) -> Bool {
    guard let mapping = MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: enabled),
          MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else { return false }
    enabledSchemes = enabled
    self.scheme = scheme
    return true
  }

  static let appGroupIdentifier = "group.app.msime.ios"
}
