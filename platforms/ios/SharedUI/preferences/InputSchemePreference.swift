import Foundation

enum ChineseInputScheme: String, CaseIterable {
  // 「高情商回复」已改为工具栏入口，不再是方案。旧版存下的 `thoughtfulReply`（App Group）或 `thoughtful_reply`（共享文档）在这里认不出来，与其他未知值一样走 `InputSchemePreference` 的回退：已选的落到全拼 26 键或第一个可用方案，启用列表里直接忽略。
  case quanpin, nineKey, shuangpin, ziranma, microsoft, shoudao, wubi, japaneseNineKey, japanese, korean, handwriting, cantonese, zhuyin, vietnamese, tibetan, stroke

  /// 全新安装默认不打开、要用户自己启用的方案，与设置宿主一致（MobilePlatformPlugin 里的 `optInSchemes`）。
  static let optInSchemes: [ChineseInputScheme] = [.cantonese, .zhuyin, .vietnamese, .tibetan, .stroke]

  var isJapanese: Bool { self == .japanese || self == .japaneseNineKey }

  /// Korean Hangul on the Dubeolsik layout: the Engine composes one syllable at a time and offers no candidates.
  var isKorean: Bool { self == .korean }

  /// Cantonese Jyutping on the 26 letter keys: Traditional candidates from the Cantonese dictionary, with nothing learned.
  var isCantonese: Bool { self == .cantonese }

  /// Zhuyin on the Dachen layout: the keys send the ASCII under each bopomofo, and the Engine converts the syllables in place into Traditional Chinese.
  var isZhuyin: Bool { self == .zhuyin }

  /// Vietnamese Telex or VNI on the 26 letter keys: the Engine composes the word in place and offers no candidates.
  var isVietnamese: Bool { self == .vietnamese }

  /// 26 个字母键上的藏文 EWTS（扩展威利转写）：组字是当前音节的威利原文，Engine 就地显示转出的藏文，不给候选。威利转写区分大小写，大写字母是拼写的一部分。
  var isTibetan: Bool { self == .tibetan }

  /// 笔画输入：五个笔画键 h s p n z 加通配 x，从笔画词库按笔顺查单字，候选只读、不学习。键面和预编辑都画笔画字形 一丨丿丶乛＊，ASCII 字母只是发给 Engine 的键。
  var isStroke: Bool { self == .stroke }

  /// 预编辑是否是按键的字形而不是按键的拼写：笔画的 一丨丿丶乛＊ 代表字母 h s p n z x。`editing_text` 里的字母只是按键发出的内容，所以无论行内还是候选栏，都不显示这些字母（msime_client.h）。
  var drawsKeysAsGlyphs: Bool { isStroke }

  /// 方案是否使用普通话那一套功能：繁体输出转换、候选释义、本地输入模式和候选菜单。日语、韩语、越南语和藏文写的是各自的文字，粤拼和注音直接从各自的词库写出繁体中文，笔画从自己的词库按笔顺查单字，这些功能都不用。
  var writesChinese: Bool { !isJapanese && !isKorean && !isCantonese && !isZhuyin && !isVietnamese && !isTibetan && !isStroke }

  /// 方案是否写半角 ASCII 标点，韩语、越南语和藏文是这样；其他方案给出中文或日文标点。
  var writesAsciiPunctuation: Bool { isKorean || isVietnamese || isTibetan }

  /// Whether the composition is a letter spelling with a caret the user can move: the Mandarin schemes and Cantonese. A Japanese reading converts as a whole, the in-place schemes have no caret inside what they compose, and a stroke sequence is drawn as glyphs whose letters the user never sees.
  var hasSpellingCaret: Bool { writesChinese || isCantonese }

  /// Engine 组出来的是否已经就是正文：韩语音节、注音转换结果、越南语单词或藏文音节。无论预编辑设置如何都写在输入框里，离开方案或结束组字时上屏而不是丢掉。
  var composesInPlace: Bool { isKorean || isZhuyin || isVietnamese || isTibetan }

  /// 字母键是否按 Shift 和大写锁定给出的大小写直接交给 Engine，而不是把 Shift 当成切到英文：越南语的大写就是大写字母，藏文威利转写的大写字母（T D N Sh A I U M H 等）是不同的拼写。
  var typesCasedLetters: Bool { isVietnamese || isTibetan }

  /// Whether the scheme reads a dictionary that ships apart from the resource set, and so is offered only where it is installed.
  var needsLanguageDictionary: Bool { isCantonese || isZhuyin || isStroke }

  /// Whether a held backspace and a quick space-bar flick edit the spelling a syllable at a time. Only a lettered pinyin spelling has syllables to step over: a nine-key digit run is still ambiguous, and a wubi code is not made of syllables, so those keep a hold that clears the composition.
  var editsBySyllable: Bool { self == .quanpin || shuangpinProfile != nil }

  /// 这个入口背后的输入方案，即版本表和共享偏好 `scheme` 里的方案名。手写的识别由平台识别器完成，不属于任何一个方案，这里的 `quanpin` 只用来归类；它写进偏好的方案见 `MetasequoiaInputSessionBridge.schemeMapping`。
  var engineScheme: String {
    switch self {
    case .quanpin, .nineKey, .handwriting: "quanpin"
    case .shuangpin, .ziranma, .microsoft, .shoudao: "shuangpin"
    case .wubi: "wubi"
    case .japanese, .japaneseNineKey: "japanese"
    case .korean: "korean"
    case .cantonese: "cantonese"
    case .zhuyin: "zhuyin"
    case .vietnamese: "vietnamese"
    case .tibetan: "tibetan"
    case .stroke: "stroke"
    }
  }

  /// 本版本是否提供这个入口：入口背后的方案在本版本里时提供。手写面板写出的是汉字，所以只在提供中文方案的版本里有（full、拼音版、五笔版），日文、越南文和藏文版没有。与 client-core 的 `Edition::offers_touch_scheme` 一致。
  var isOfferedByEdition: Bool {
    guard self == .handwriting else { return MSIMEAppEdition.offers(engineScheme) }
    return Self.chineseEngineSchemes.contains(where: MSIMEAppEdition.offers)
  }

  /// 写中文的方案（版本表里的方案名），与 client-core 的 `ChineseScheme::of` 相同；提供其中任何一个的版本才有手写。
  static let chineseEngineSchemes = ["quanpin", "shuangpin", "wubi", "cantonese", "zhuyin", "stroke"]

  /// 偏好里的方案本版本没有、或一个入口都没剩下时退回的入口：本版本默认方案的 26 键入口。full 是全拼 26 键，与引入版本之前相同。
  static var editionFallback: ChineseInputScheme { ChineseInputScheme(rawValue: MSIMEAppEdition.defaultScheme) ?? .quanpin }

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
    case .cantonese: "cantonese"
    case .zhuyin: "zhuyin"
    case .vietnamese: "vietnamese"
    case .tibetan: "tibetan"
    case .stroke: "stroke"
    }
  }
  /// 这个方案在云端设置文档里的 `input.schema`，云端带不了的方案为 nil。云端只认 quanpin、shuangpin、wubi、japanese 和 korean，任何设备收到其他值都会拒绝整份文档（`IOSPreferencePlan`），所以粤拼、注音、越南语、藏文和笔画不写这个字段，保留账号里的方案，与 Tauri 的 `local_account_preferences` 一致。
  var cloudSchema: String? {
    switch self {
    case .quanpin, .nineKey, .handwriting: "quanpin"
    case .shuangpin, .ziranma, .microsoft, .shoudao: "shuangpin"
    case .wubi: "wubi"
    case .japanese, .japaneseNineKey: "japanese"
    case .korean: "korean"
    case .cantonese, .zhuyin, .vietnamese, .tibetan, .stroke: nil
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
    case .wubi: WubiProfilePreference.title(WubiProfilePreference.profile)
    case .japanese: "日语 26 键"
    case .japaneseNineKey: "日语 9 键"
    case .korean: "韩语 26 键"
    case .handwriting: "手写"
    case .cantonese: "粤拼 26 键"
    case .zhuyin: "大千注音"
    case .vietnamese: "越南语 26 键"
    case .tibetan: "藏文 26 键"
    case .stroke: "笔画"
    }
  }
}

/// 五笔用 86 还是 98 码表。方案仍只有一个 `wubi`，版本是共享偏好文档里与它并列的 `wubi_profile`，就像双拼方案旁边的 `shuangpin_profile`；Engine 按它读写 `wubi86` 或 `wubi98` 词库。
///
/// 文档是唯一的权威来源。App Group 里的这份只是镜像，给方案名、键盘方案卡片这类不读文档的同步取值用，由读到文档的一方（设置页、键盘重载文档时）写入。
enum WubiProfilePreference {
  static let profiles = ["wubi86", "wubi98"]
  static let documentKey = "wubi_profile"
  static let profileKey = "wubi.profile"
  private static var defaults: UserDefaults { UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard }

  static var profile: String {
    get { known(defaults.string(forKey: profileKey)) }
    set { defaults.set(profiles.contains(newValue) ? newValue : "wubi86", forKey: profileKey) }
  }

  /// 文档里记的版本；没有记过或值不认识时按 86 处理，与 client-core 的缺省一致。
  static func profile(in document: [String: Any]?) -> String {
    known(document?[documentKey] as? String)
  }

  private static func known(_ value: String?) -> String {
    value.flatMap { profiles.contains($0) ? $0 : nil } ?? "wubi86"
  }

  /// 方案名：86 五笔或 98 五笔。
  static func title(_ profile: String) -> String { profile == "wubi98" ? "98 五笔" : "86 五笔" }

  /// 键盘方案卡片的角标：86 或 98。
  static func badge(_ profile: String) -> String { profile == "wubi98" ? "98" : "86" }

  /// 把文档里的版本抄进 App Group 镜像。
  static func mirror(_ document: [String: Any]) {
    if profile(in: document) != profile { profile = profile(in: document) }
  }

  /// 先写共享文档，写成功了再更新镜像；文档没写进去时返回 false，镜像保持原值。
  @discardableResult
  static func save(_ value: String, stateRoot: URL? = nil) -> Bool {
    guard profiles.contains(value),
          MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { $0[documentKey] = value }) else { return false }
    profile = value
    return true
  }
}

enum InputSchemePreference {
  static let enabledSchemesKey = "enabledInputSchemes"
  private static var defaults: UserDefaults { UserDefaults(suiteName: appGroupIdentifier) ?? .standard }

  static var enabledSchemes: [ChineseInputScheme] {
    get {
      // 本版本不提供的入口（比如 full 那边存下的拼音落到五笔版）不算启用，一个都不剩时退回本版本的默认入口。没存过列表时，要用户自己打开的那几个不启用；只有一个方案的版本例外，越南文版、藏文版的入口就是这个版本本身，与 client-core 的 `TouchKeyboardSchemePreferences::for_edition` 一致。
      guard let stored = defaults.stringArray(forKey: enabledSchemesKey) else {
        let optInEnabled = (MSIMEAppEdition.inputSchemes?.count ?? .max) <= 1
        let initial = ChineseInputScheme.allCases.filter {
          (optInEnabled || !ChineseInputScheme.optInSchemes.contains($0)) && $0.isOfferedByEdition
        }
        return initial.isEmpty ? [.editionFallback] : initial
      }
      let enabled = ChineseInputScheme.allCases.filter { stored.contains($0.rawValue) && $0.isOfferedByEdition }
      return enabled.isEmpty ? [.editionFallback] : enabled
    }
    set {
      let ordered = ChineseInputScheme.allCases.filter { newValue.contains($0) && $0.isOfferedByEdition }
      let enabled = ordered.isEmpty ? [.editionFallback] : ordered
      defaults.set(enabled.map(\.rawValue), forKey: enabledSchemesKey)
      scheme = scheme
    }
  }

  /// The enabled schemes this process can run: an enabled Cantonese, Zhuyin or Stroke whose dictionary is not installed is left out, because the Engine would answer it with another scheme. The enabled list itself keeps it, so the choice holds once the dictionary arrives. A process that carries no Engine (the App) cannot tell and leaves every enabled scheme in, so its settings never rewrite a selection the keyboard can run.
  static var offeredSchemes: [ChineseInputScheme] {
    offeredSchemes(enabled: enabledSchemes, installed: installedLanguageSchemes)
  }

  /// `enabled` without the Cantonese, Zhuyin and Stroke schemes missing from `installed`; nil `installed` means unknown and filters nothing.
  static func offeredSchemes(enabled: [ChineseInputScheme], installed: Set<ChineseInputScheme>?) -> [ChineseInputScheme] {
    guard let installed else { return enabled }
    let offered = enabled.filter { !$0.needsLanguageDictionary || installed.contains($0) }
    return offered.isEmpty ? [.editionFallback] : offered
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
    let files: [(ChineseInputScheme, String)] = [(.cantonese, "msime-cantonese.db"), (.zhuyin, "msime-zhuyin.db"), (.stroke, "msime-stroke.db")]
    return Set(files.filter { FileManager.default.fileExists(atPath: directory.appendingPathComponent($0.1).path) }.map(\.0))
  }

  /// Read once: the bundle does not change while the process runs, and `scheme` is read on every keystroke.
  static let installedLanguageSchemes = installedLanguageSchemes(in: languageDictionaryDirectory(in: .main))

  private static let schemeKey = "chineseInputScheme"
  static var scheme: ChineseInputScheme {
    get {
      let defaults = UserDefaults(suiteName: appGroupIdentifier) ?? .standard
      let offered = offeredSchemes
      let stored = defaults.string(forKey: schemeKey).flatMap(ChineseInputScheme.init(rawValue:)) ?? .editionFallback
      return offered.contains(stored) ? stored : offered[0]
    }
    set {
      let defaults = UserDefaults(suiteName: appGroupIdentifier) ?? .standard
      let selected = enabledSchemes.contains(newValue) ? newValue : enabledSchemes[0]
      defaults.set(selected.rawValue, forKey: schemeKey)
    }
  }

  /// 一次方案选择：选中的入口和启用的入口。
  struct Selection: Equatable {
    var scheme: ChineseInputScheme
    var enabled: [ChineseInputScheme]
  }

  /// 文档里记的选中入口（`touch_keyboard_schemes.selected`）；没有记过或值不认识时为 nil。
  static func selectedScheme(in document: [String: Any]?) -> ChineseInputScheme? {
    ((document?["touch_keyboard_schemes"] as? [String: Any])?["selected"] as? String)
      .flatMap(ChineseInputScheme.scheme(sharedIdentifier:))
  }

  /// 文档里记的启用列表（`touch_keyboard_schemes.enabled`）；没有记过或一个都认不出时为 nil。
  static func enabledSchemes(in document: [String: Any]?) -> [ChineseInputScheme]? {
    let enabled = ((document?["touch_keyboard_schemes"] as? [String: Any])?["enabled"] as? [String] ?? [])
      .compactMap(ChineseInputScheme.scheme(sharedIdentifier:))
    return enabled.isEmpty ? nil : enabled
  }

  /// 键盘按这份文档会用的方案选择：文档记了的以文档为准，没记的那一项用 App Group 镜像补上，因为键盘这时也按镜像行事。
  ///
  /// 方案的权威来源是共享文档：设置页、云端同步和键盘都写它，App Group 里的 `scheme` 与 `enabledSchemes` 只是镜像。镜像可能落后于文档（比如键盘的写入在文档那边没成功，或者文档被别的写入方改过），所以任何要把方案写回文档的地方都从这里取起点，而不是直接拿镜像——否则一次与方案无关的改动（开关手写、上传设置）会把镜像里的旧方案写回文档，用户选的方案就此丢失（#4288）。
  static func current(in document: [String: Any]?) -> Selection {
    Selection(scheme: selectedScheme(in: document) ?? scheme, enabled: enabledSchemes(in: document) ?? enabledSchemes)
  }

  /// 把文档里记的方案选择抄进 App Group 镜像；文档没记的那一项保持镜像原值。
  static func mirror(_ document: [String: Any]?) {
    if let enabled = enabledSchemes(in: document) { enabledSchemes = enabled }
    if let selected = selectedScheme(in: document) { scheme = selected }
  }

  static func mirror(_ selection: Selection) {
    enabledSchemes = selection.enabled
    scheme = selection.scheme
  }

  /// 在文档上改一次方案选择：`change` 拿到的是文档当前的选择（见 `current(in:)`），改完按 `schemeMapping` 写回文档。返回实际写进去的选择（本版本不提供的入口已去掉，选中的入口不在启用列表里时换成第一个）；一个入口都没启用时什么也不写，返回 nil。
  ///
  /// 要在 `updateSharedPreferences` 的闭包里调用：那里拿到的是这次比较并交换读出的文档，起点不会是过时的。
  static func write(_ change: (inout Selection) -> Void, into document: inout [String: Any]) -> Selection? {
    var selection = current(in: document)
    change(&selection)
    let enabled = ChineseInputScheme.allCases.filter { selection.enabled.contains($0) && $0.isOfferedByEdition }
    guard let mapping = MetasequoiaInputSessionBridge.schemeMapping(selection.scheme, enabledSchemes: enabled) else { return nil }
    mapping(&document)
    return Selection(scheme: enabled.contains(selection.scheme) ? selection.scheme : enabled[0], enabled: enabled)
  }

  /// 按文档当前的选择改一次方案并写回文档，写成功后再更新镜像。文档没写进去或一个入口都没启用时返回 nil，镜像保持原值。
  @discardableResult
  static func update(stateRoot: URL? = nil, _ change: (inout Selection) -> Void) -> Selection? {
    var written: Selection?
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { written = write(change, into: &$0) }),
          let written else { return nil }
    mirror(written)
    return written
  }

  /// 选中一个入口，它还没启用时一并启用；文档里的其他启用项保持原样。
  @discardableResult
  static func select(_ scheme: ChineseInputScheme, stateRoot: URL? = nil) -> Bool {
    let written = update(stateRoot: stateRoot) { selection in
      if !selection.enabled.contains(scheme) { selection.enabled.append(scheme) }
      selection.scheme = scheme
    }
    return written != nil
  }

  /// 启用或停用一个入口，选中的入口保持文档里的那个（它被停用时换成第一个启用的入口）。
  @discardableResult
  static func setEnabled(_ scheme: ChineseInputScheme, _ isEnabled: Bool, stateRoot: URL? = nil) -> Bool {
    let written = update(stateRoot: stateRoot) { selection in
      selection.enabled.removeAll { $0 == scheme }
      if isEnabled { selection.enabled.append(scheme) }
    }
    return written != nil
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

  static let appGroupIdentifier = MSIMEAppEdition.appGroupIdentifier
}
