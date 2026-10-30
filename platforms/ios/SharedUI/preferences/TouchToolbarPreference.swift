import Foundation

/// 工具栏按钮: the optional buttons on the keyboard's shortcut bar, from the shared `touch_toolbar`, the touch counterpart of the Windows floating toolbar's component switches.
///
/// A missing document or a missing switch reads as its default, which is the bar the keyboard always had: settings, emoji and skin on, the rest off. There is no voice button: iOS starts voice from a long press on space or the 语音输入 tile, and `touch_voice_shortcut` is Android's toolbar voice button, which iOS neither reads nor writes. The brand, scheme and dismiss buttons are not optional.
struct TouchToolbarPreference: Equatable {
  static let key = "touch_toolbar"

  /// 共享设置页的「键盘设置」按钮。iOS 键盘不画这个按钮，也不在 App 里给它开关，但写回文档时照样带上它：`touch_toolbar` 要求完整的对象，其他平台还在用它。
  var layout = true
  var emoji = true
  var skin = true
  var clipboard = false
  var ai = false
  var characterSet = false
  var fullwidth = false
  var punctuation = false
  /// Android 工具栏提供的撤销、重做开关；iOS 不显示按钮，但保存共享对象时必须保留这两个字段。
  var undo = false
  var redo = false

  init() {}

  init(in preferences: [String: Any]?) {
    let stored = preferences?[Self.key] as? [String: Any] ?? [:]
    func read(_ name: String, _ fallback: Bool) -> Bool { stored[name] as? Bool ?? fallback }
    layout = read("layout", layout)
    emoji = read("emoji", emoji)
    skin = read("skin", skin)
    clipboard = read("clipboard", clipboard)
    ai = read("ai", ai)
    characterSet = read("character_set", characterSet)
    fullwidth = read("fullwidth", fullwidth)
    punctuation = read("punctuation", punctuation)
    undo = read("undo", undo)
    redo = read("redo", redo)
  }

  /// The switches in the order and wording of the shared settings page, keyed by the document name.
  static let options: [(name: String, title: String, keyPath: WritableKeyPath<TouchToolbarPreference, Bool>)] = [
    ("layout", "键盘设置", \.layout),
    ("emoji", "表情", \.emoji),
    ("skin", "切换皮肤", \.skin),
    ("clipboard", "剪贴板历史", \.clipboard),
    ("ai", "AI 润色", \.ai),
    ("character_set", "简繁切换", \.characterSet),
    ("fullwidth", "全角 / 半角", \.fullwidth),
    ("punctuation", "中英文标点", \.punctuation),
    ("undo", "撤销", \.undo),
    ("redo", "重做", \.redo),
  ]

  /// The whole object, so a document holding only some switches is completed the same way the shared page completes it.
  var documentValue: [String: Bool] {
    Dictionary(uniqueKeysWithValues: Self.options.map { ($0.name, self[keyPath: $0.keyPath]) })
  }

  static func load(stateRoot: URL? = nil) -> TouchToolbarPreference {
    TouchToolbarPreference(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: stateRoot))
  }

  /// Written into the shared document, which the keyboard reads each time it appears.
  static func save(_ toolbar: TouchToolbarPreference, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { $0[key] = toolbar.documentValue }
  }
}

/// 共享的 `touch_toolbar` 装不下的工具栏设置，保存在本设备的 App Group 里，就像 Android 把它们存在本地设置里一样（`platform.android.toolbar_phrase`、`toolbar_scheme`、`toolbar_hidden`）：「常用语」和「输入方式」按钮，默认开启；以及「显示方式」。选「隐藏」时不画工具栏内容；iOS 保留顶栏高度以避免组字时重排键盘，Android 仍会收起整行。共享文档的校验器会丢弃它不认识的成员，所以这些设置不能放进 `touch_toolbar`；iOS 的设置同步不携带它们，所以不随账号同步；Android 的这三项会经 `android_local` 同步。
enum TouchToolbarLocalPreference {
  static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }
  static let phrasesKey = "keyboard.toolbar.phrases"
  static let schemeKey = "keyboard.toolbar.scheme"
  static let hiddenKey = "keyboard.toolbar.hidden"
  static var keys: [String] { [phrasesKey, schemeKey, hiddenKey] }

  static var phrases: Bool {
    get { defaults.object(forKey: phrasesKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: phrasesKey) }
  }

  static var scheme: Bool {
    get { defaults.object(forKey: schemeKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: schemeKey) }
  }

  /// 显示方式：false 为「输入时显示」，true 为「隐藏」。
  static var hidden: Bool {
    get { defaults.object(forKey: hiddenKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: hiddenKey) }
  }
}

/// 键盘预览里的一个工具栏按钮。图标与键盘自己的工具栏一致；字符类项目画单个字符而不是符号。放在 SharedUI 里，`KeyboardToolbarPreviewRow` 的规则才能在键盘测试里直接测。
enum KeyboardPreviewToolbarItem: String, CaseIterable {
  /// 26 键「长空格」从底行挪到工具栏的 符（键盘的 `symbolsShortcut`），排在品牌键后面。
  case symbols
  case emoji, phrases, clipboard, skin, scheme, ai, characterSet, fullwidth, punctuation

  /// 该项目的 SF Symbol；画成字符的项目为 nil。
  var symbol: String? {
    switch self {
    case .emoji: "face.smiling"
    case .phrases: "text.bubble"
    case .clipboard: "doc.on.clipboard"
    case .skin: "paintpalette"
    case .scheme: "keyboard"
    case .ai: "sparkles"
    case .symbols, .characterSet, .fullwidth, .punctuation: nil
    }
  }

  /// 没有符号的项目所画的字符。
  var character: String {
    switch self {
    case .symbols: "符"
    case .characterSet: "简"
    case .fullwidth: "全"
    case .punctuation: "，"
    default: ""
    }
  }
}

/// 「键盘工具栏」设置页预览里，品牌键和收起键之间画哪些按钮；nil 表示空闲时不画工具栏内容，只保留顶栏高度。规则与键盘的 `toolbarRowHidden`、`longSpaceToolbarRowKept` 相同。预览画的是中文 26 键，所以 26 键选了「长空格」时 符 跟键盘一样排在品牌键后面，这一行也总要画：显示方式为「隐藏」或可选按钮全关时只剩品牌、符 和收起。见 .agents/notes/implemented/bug-fix/2026-10-29-keyboard-previews-follow-layout.md。
enum KeyboardToolbarPreviewRow {
  static func items(_ toolbar: TouchToolbarPreference, phrases: Bool, scheme: Bool, hidden: Bool,
                    keyLayout: TouchKeyLayoutStyle) -> [KeyboardPreviewToolbarItem]? {
    let symbols = keyLayout.twentySixKey == .longSpace
    // 按键盘绘制顺序：表情、常用语、剪贴板、皮肤、输入方式，然后是 iOS 独有项。
    let ordered: [(enabled: Bool, item: KeyboardPreviewToolbarItem)] = [
      (toolbar.emoji, .emoji), (phrases, .phrases), (toolbar.clipboard, .clipboard), (toolbar.skin, .skin),
      (scheme, .scheme), (toolbar.ai, .ai), (toolbar.characterSet, .characterSet),
      (toolbar.fullwidth, .fullwidth), (toolbar.punctuation, .punctuation),
    ]
    let tools = hidden ? [] : ordered.filter(\.enabled).map(\.item)
    if !symbols && (hidden || tools.isEmpty) { return nil }
    return (symbols ? [.symbols] : []) + tools
  }
}
