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
