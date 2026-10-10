import Foundation
import CoreFoundation

enum KeyboardLayoutPreference {
  static var defaults: UserDefaults { UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier) ?? .standard }
  static let keySpacingKey = "keyboard.spacing.keys"
  static let rowSpacingKey = "keyboard.spacing.rows"
  static let voiceShortcutKey = "keyboard.shortcut.voice"
  static let heightAdjustmentKey = "keyboard.height.adjustment"
  static let tabletFullKeysKey = "keyboard.tablet.fullKeys"
  static let tabletSplitKey = "keyboard.tablet.split"
  static let glideTypingKey = "keyboard.gesture.glide"
  static let spaceVoiceKey = "keyboard.gesture.spaceVoice"
  static let swipeSymbolsKey = "keyboard.gesture.swipeSymbols"
  static let oneHandedKey = "keyboard.oneHanded"
  static var keySpacing: Double {
    get { spacing(key: keySpacingKey, fallback: 6, range: 3...6) }
    set { defaults.set(SharedNumber.clamped(newValue, to: 3...6), forKey: keySpacingKey) }
  }
  static var rowSpacing: Double {
    get { spacing(key: rowSpacingKey, fallback: 7, range: 4...10) }
    set { defaults.set(SharedNumber.clamped(newValue, to: 4...10), forKey: rowSpacingKey) }
  }
  /// Keyboard height adjustment relative to the platform's current default, in points.
  ///
  /// The extension adds this value after accounting for orientation and candidate rows, so the
  /// same setting remains useful when the candidate surface changes height.
  static var heightAdjustment: Double {
    get { spacing(key: heightAdjustmentKey, fallback: 0, range: -12...48) }
    set { defaults.set(SharedNumber.clamped(newValue, to: -12...48), forKey: heightAdjustmentKey) }
  }
  /// Whether Tab opens the full candidate panel while composing on the full-size iPad keyboard: the shared `navigation.tab` (Windows `paging_tab`, on by default). Read by the keyboard and edited by the App's iPad section.
  static func tabShowsMoreCandidates(_ preferences: [String: Any]?) -> Bool {
    (preferences?["navigation"] as? [String: Any])?["tab"] as? Bool ?? true
  }

  /// Writes `navigation.tab` into the shared document, keeping the other paging keys in the same object.
  static func saveTabShowsMoreCandidates(_ enabled: Bool, stateRoot: URL? = nil) -> Bool {
    MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { document in
      var navigation = document["navigation"] as? [String: Any] ?? [:]
      navigation["tab"] = enabled
      document["navigation"] = navigation
    }
  }

  static func resetToDefaults() {
    for stored in [keySpacingKey, rowSpacingKey, heightAdjustmentKey, voiceShortcutKey] {
      defaults.removeObject(forKey: stored)
    }
  }
  /// Save the geometry where the keyboard reads it. The keyboard copies the shared document's `touch_*` fields over the App Group every time it appears, so a value written only to the App Group lasted until then. The App Group is written after the document, and only when the document took the change.
  ///
  /// 只写传了值的那几项，其余保持文档里的样子。设置页原先每次都把四项一起写：键盘里调过高度之后，设置页上停留的还是旧高度，这时动一下任何别的控件，旧高度就被写回文档，键盘下次出现又变回设置页的值。
  @discardableResult
  static func saveGeometry(keySpacing: Double? = nil, rowSpacing: Double? = nil, heightAdjustment: Double? = nil,
                           voiceShortcut: Bool? = nil, stateRoot: URL? = nil) -> Bool {
    guard let mapping = MetasequoiaInputSessionBridge.geometryMapping(
      keySpacing: keySpacing, rowSpacing: rowSpacing, heightAdjustment: heightAdjustment, voiceEnabled: voiceShortcut),
      MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else { return false }
    if let keySpacing { self.keySpacing = keySpacing }
    if let rowSpacing { self.rowSpacing = rowSpacing }
    if let heightAdjustment { self.heightAdjustment = heightAdjustment }
    if let voiceShortcut { voiceShortcutEnabled = voiceShortcut }
    return true
  }

  /// 把共享文档里的键距、行距、高度和语音入口抄进 App Group，文档里没有的项不动。键盘每次出现、设置页每次回到前台都按文档对齐，两边读到的才是同一个值。
  static func mirrorGeometry(_ preferences: [String: Any]) {
    if let spacing = sharedKeySpacing(preferences["touch_key_spacing_tenths"]) { keySpacing = spacing }
    if let spacing = sharedRowSpacing(preferences["touch_row_spacing_tenths"]) { rowSpacing = spacing }
    if let adjustment = sharedHeightAdjustment(preferences["touch_keyboard_height_adjustment"]) {
      heightAdjustment = adjustment
    }
    if let voice = preferences["touch_voice_shortcut"] as? Bool { voiceShortcutEnabled = voice }
  }

  /// 「数字键盘顺序」：九键数字层 1 2 3 在上（电话）还是 7 8 9 在上（计算器），26 键选了九宫格数字键盘时它的数字层同样按这个顺序。共享文档里是 `touch_number_keypad_order`，键盘从 App Group 镜像读。
  enum NumberKeypadOrder: String, CaseIterable {
    case phone, calculator

    static let documentKey = "touch_number_keypad_order"

    var title: String {
      switch self {
      case .phone: "电话（123 在上）"
      case .calculator: "计算器（789 在上）"
      }
    }

    /// 文档里缺这一项或是认不得的值时按电话顺序。
    static func shared(in preferences: [String: Any]?) -> NumberKeypadOrder {
      (preferences?[documentKey] as? String).flatMap(NumberKeypadOrder.init(rawValue:)) ?? .phone
    }

    /// 3×3 网格里第 `row` 行第 `column` 列（都从 0 起）的数字键显示并输入的数字。电话顺序第一行是 1 2 3；计算器顺序上下颠倒，第一行是 7 8 9，列不变。
    func digit(row: Int, column: Int) -> Int {
      switch self {
      case .phone: row * 3 + column + 1
      case .calculator: (2 - row) * 3 + column + 1
      }
    }
  }

  static let numberKeypadOrderKey = "keyboard.numberKeypad.order"
  static var numberKeypadOrder: NumberKeypadOrder {
    get { defaults.string(forKey: numberKeypadOrderKey).flatMap(NumberKeypadOrder.init(rawValue:)) ?? .phone }
    set { defaults.set(newValue.rawValue, forKey: numberKeypadOrderKey) }
  }

  /// 把数字键盘顺序写进共享文档，只改这一项；文档接受了才更新 App Group 镜像。
  @discardableResult
  static func saveNumberKeypadOrder(_ order: NumberKeypadOrder, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[NumberKeypadOrder.documentKey] = order.rawValue
    }) else { return false }
    numberKeypadOrder = order
    return true
  }

  /// 「26 键数字键盘」：手机 26 键按 123 时出一行 1 到 0 的数字符号页（`row`，默认），还是九键那样的 3×3 数字层（`nine_key`）。共享文档里是 `touch_twenty_six_key_number_layout`，键盘从 App Group 镜像读。iPad 全尺寸键盘不受它影响，见 `KeyboardViewController.opensNineKeyDigitPad`。
  enum TwentySixKeyNumberLayout: String, CaseIterable {
    case row
    case nineKey = "nine_key"

    static let documentKey = "touch_twenty_six_key_number_layout"

    var title: String {
      switch self {
      case .row: "一行"
      case .nineKey: "九宫格"
      }
    }

    /// 文档里缺这一项或是认不得的值时按一行。
    static func shared(in preferences: [String: Any]?) -> TwentySixKeyNumberLayout {
      (preferences?[documentKey] as? String).flatMap(TwentySixKeyNumberLayout.init(rawValue:)) ?? .row
    }
  }

  static let twentySixKeyNumberLayoutKey = "keyboard.twentySixKey.numberLayout"
  static var twentySixKeyNumberLayout: TwentySixKeyNumberLayout {
    get { defaults.string(forKey: twentySixKeyNumberLayoutKey).flatMap(TwentySixKeyNumberLayout.init(rawValue:)) ?? .row }
    set { defaults.set(newValue.rawValue, forKey: twentySixKeyNumberLayoutKey) }
  }

  /// 把 26 键数字键盘写进共享文档，只改这一项；文档接受了才更新 App Group 镜像。
  @discardableResult
  static func saveTwentySixKeyNumberLayout(_ layout: TwentySixKeyNumberLayout, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[TwentySixKeyNumberLayout.documentKey] = layout.rawValue
    }) else { return false }
    twentySixKeyNumberLayout = layout
    return true
  }

  /// 「双拼键位提示」：双拼方案的 26 键在字母键底部画这个键代表的声母和韵母。共享文档里是 `touch_shuangpin_key_hints`，缺省为开；键盘从 App Group 镜像读。
  static let shuangpinKeyHintsDocumentKey = "touch_shuangpin_key_hints"
  static let shuangpinKeyHintsKey = "keyboard.shuangpin.keyHints"

  /// 文档里缺这一项或不是布尔值时按开，与 client-core 的默认值一致。
  static func sharedShuangpinKeyHints(in preferences: [String: Any]?) -> Bool {
    preferences?[shuangpinKeyHintsDocumentKey] as? Bool ?? true
  }

  static var shuangpinKeyHints: Bool {
    get { defaults.object(forKey: shuangpinKeyHintsKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: shuangpinKeyHintsKey) }
  }

  /// 把双拼键位提示开关写进共享文档，只改这一项；文档接受了才更新 App Group 镜像。
  @discardableResult
  static func saveShuangpinKeyHints(_ enabled: Bool, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[shuangpinKeyHintsDocumentKey] = enabled
    }) else { return false }
    shuangpinKeyHints = enabled
    return true
  }

  /// `resetToDefaults`, for the shared document as well.
  @discardableResult
  static func resetGeometry(stateRoot: URL? = nil) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { preferences in
      for key in ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
                  "touch_voice_shortcut"] {
        preferences.removeValue(forKey: key)
      }
    }
    guard written else { return false }
    resetToDefaults()
    // 「横屏分离式键盘」和间距、高度在 App 的同一页，一起恢复成默认（关）。键盘里的间距面板也调 `resetToDefaults`，那里不显示这个开关，所以不放进上面的列表，免得在键盘里点「恢复默认」把分离式键盘悄悄关掉。
    defaults.removeObject(forKey: tabletSplitKey)
    return true
  }
  static var voiceShortcutEnabled: Bool {
    get { defaults.bool(forKey: voiceShortcutKey) }
    set { defaults.set(newValue, forKey: voiceShortcutKey) }
  }
  /// 「数字行与 Tab 键」: the full-size iPad keyboard carries a digit row above the letters and a Tab key before Q, as a desktop keyboard does. On by default; phones and the compact iPad keyboards never show them.
  static var tabletFullKeys: Bool {
    get { defaults.object(forKey: tabletFullKeysKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: tabletFullKeysKey) }
  }
  /// 「横屏分离式键盘」：iPad 全宽键盘横屏时把键区从中间分成左右两半，方便双手握持时用拇指打字。默认关；手机、iPad 的浮动键盘和窄窗口、以及竖屏都不分离，见 `KeyboardSplitLayout`。
  static var tabletSplit: Bool {
    get { defaults.object(forKey: tabletSplitKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: tabletSplitKey) }
  }
  /// 「滑行输入」：在全拼 26 键的字母键上连续滑动输入拼音（#5347），由 Engine 把一笔滑行解码成拼音字母。默认关；只存在本机 App Group，不进共享文档，也不随设置同步。
  static var glideTyping: Bool {
    get { defaults.object(forKey: glideTypingKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: glideTypingKey) }
  }
  /// 「长按空格语音输入」：在空格键上按住 450 ms 打开语音面板。默认开，与 Android 的 `platform.android.space_voice` 相同；只存在本机 App Group，不进共享文档，也不随设置同步。「语音输入」页的启动方式由它和 `touch_voice_shortcut` 一起派生。
  static var spaceVoice: Bool {
    get { defaults.object(forKey: spaceVoiceKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: spaceVoiceKey) }
  }
  /// 「滑动输入符号」：在字母键上下滑输入右上角的角标符号。默认开，与 Android 的 `platform.android.swipe_down_symbols` 相同；关掉后长按字母键仍能输入角标，与 Android 一致。只存在本机 App Group，不随设置同步。
  static var swipeSymbols: Bool {
    get { defaults.object(forKey: swipeSymbolsKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: swipeSymbolsKey) }
  }
  /// 「单手模式」：手机键盘的按键收窄到 85% 并靠向一侧，另一侧是「换到另一侧」和「退出单手」组成的一列。默认关闭；它只存在本设备的 App Group 里，不进共享文档，iOS 的设置同步也不携带它；Android 的 `platform.android.one_handed` 会同步。无法识别的存储值按关闭处理。
  static var oneHanded: KeyboardOneHandedMode {
    get { defaults.string(forKey: oneHandedKey).flatMap(KeyboardOneHandedMode.init(rawValue:)) ?? .off }
    set { defaults.set(newValue.rawValue, forKey: oneHandedKey) }
  }
  static var geometry: KeyboardGeometry { KeyboardGeometry(keySpacing: keySpacing, rowSpacing: rowSpacing) }

  /// Values from the canonical document are integer tenths/points. Reject booleans and fractions before clamping so malformed synced data cannot silently become a valid geometry setting.
  static func sharedKeySpacing(_ value: Any?) -> Double? {
    guard let integer = SharedNumber.strictInt(value) else { return nil }
    return SharedNumber.clamped(Double(integer) / 10, to: 3...6)
  }

  static func sharedRowSpacing(_ value: Any?) -> Double? {
    guard let integer = SharedNumber.strictInt(value) else { return nil }
    return SharedNumber.clamped(Double(integer) / 10, to: 4...10)
  }

  static func sharedHeightAdjustment(_ value: Any?) -> Double? {
    guard let integer = SharedNumber.strictInt(value) else { return nil }
    return Double(SharedNumber.clamped(integer, to: -12...48))
  }

  private static func spacing(key: String, fallback: Double, range: ClosedRange<Double>) -> Double {
    guard let value = defaults.object(forKey: key) as? NSNumber, value.doubleValue.isFinite else { return fallback }
    return SharedNumber.clamped(value.doubleValue, to: range)
  }
}

/// 「单手模式」的取值，与 Android 的 `platform.android.one_handed` 存法相同：`off`，或按键靠向的一侧 `left` / `right`。
enum KeyboardOneHandedMode: String, CaseIterable {
  case off, left, right

  /// 「单手模式」图块和侧边列下一次要写入的值，与 Android 的 `toggleOneHanded` 一致：单击在关闭和右侧之间切换；换边（长按图块，或点侧边列的「换到另一侧」）在左右之间切换，从关闭状态换边则在左侧打开。
  func toggled(swapSide: Bool) -> KeyboardOneHandedMode {
    swapSide ? (self == .left ? .right : .left) : (self == .off ? .right : .off)
  }
}

/// 「全角输入」: the shared `character_width`, the width a keyboard session starts in.
///
/// The document spells it in lower case, `fullwidth` and `halfwidth` (client-core's serde rename); the runtime view spells it `Fullwidth`, so the keyboard never reads its width back from the view. As on the other hosts, the keyboard's own 全角 card switches the running keyboard only, and a reloaded document replaces that switch only when `character_width` itself changed, so saving any other setting never clears a switch the user just pressed.
enum CharacterWidthPreference {
  static let key = "character_width"
  static let fullwidth = "fullwidth"
  static let halfwidth = "halfwidth"

  static func value(in preferences: [String: Any]?) -> String? { preferences?[key] as? String }

  /// The width a keyboard starts in: the document's.
  static func startsFullwidth(in preferences: [String: Any]?) -> Bool {
    value(in: preferences) == fullwidth
  }

  /// Whether a reloaded document's width replaces the keyboard's current switch.
  static func overridesToggle(previous: String?, next: String?) -> Bool { next != nil && previous != next }
}

/// Converts the text the keyboard commits itself, past the runtime: symbols, spaces, quick punctuation and English letters. Whatever the runtime commits it has already converted once told the width (`setCharacterWidth`); handwriting, local tools and service results keep their original identity.
enum FullWidthInputPolicy {
  static func output(_ text: String, enabled: Bool) -> String {
    guard enabled, !text.isEmpty else { return text }
    var result = ""
    result.unicodeScalars.reserveCapacity(text.unicodeScalars.count)
    for scalar in text.unicodeScalars {
      let value = scalar.value
      if value == 0x20 {
        result.unicodeScalars.append("\u{3000}")
      } else if (0x21...0x7E).contains(value), let converted = UnicodeScalar(value + 0xFEE0) {
        result.unicodeScalars.append(converted)
      } else {
        result.unicodeScalars.append(scalar)
      }
    }
    return result
  }
}

/// 以百分比表示的「键盘高度」：存储的点数调整量除以默认高度下按键区的高度。键盘的内联调节条和应用的「键盘」页都在这里换算，所以同一个存储值在两处显示为同一个百分比；设置本身仍以点为单位（`touch_keyboard_height_adjustment`）。
enum KeyboardHeightPercent {
  /// `touch_keyboard_height_adjustment` 经过校验的取值范围，单位为点。
  static let adjustmentRange: ClosedRange<CGFloat> = -12...48

  /// 竖屏的按键高度：手机按键 46pt，与 Android 新设计的 `KeyboardGeometry.DESIGN_KEY_HEIGHT_DP` 相同；iPad 按键 54pt（`dc.html` 的 `keyH`）。键盘高度由它累加而来（`KeyboardFormFactor.keyboardHeight`），所以 100% 画出的正好是这样的按键。
  ///
  /// 手机原是设计稿的 42pt：九键每个键约 105pt 宽，42pt 高看上去是扁的，上面的顶栏又按读音行加候选行留着 56pt，整块键盘头重脚轻。
  static func portraitKeyHeight(tablet: Bool) -> CGFloat { tablet ? 54 : 46 }

  /// 默认高度下的按键区：`rows` 行高为 `keyHeight` 的按键，加上行与行之间的间距。手机竖屏、默认行距 7pt 时为 205pt。
  static func keyBlockHeight(keyHeight: CGFloat, rows: Int, rowSpacing: CGFloat) -> CGFloat {
    max(1, CGFloat(rows) * keyHeight + CGFloat(max(rows - 1, 0)) * rowSpacing)
  }

  /// 手机键盘或全宽 iPad 键盘的竖屏按键区，后者的数字行多出第五行。
  static func portraitKeyBlockHeight(tablet: Bool, numberRow: Bool, rowSpacing: CGFloat) -> CGFloat {
    keyBlockHeight(keyHeight: portraitKeyHeight(tablet: tablet), rows: tablet && numberRow ? 5 : 4, rowSpacing: rowSpacing)
  }

  /// 把以点为单位的存储调整量换算成它在 `keyBlock` 上画出的百分比。
  static func percent(adjustment: CGFloat, keyBlock: CGFloat) -> Int {
    Int((100 * (keyBlock + adjustment) / keyBlock).rounded())
  }

  /// 把百分比换回以整点为单位的存储调整量，限制在 `adjustmentRange` 内。
  static func adjustment(percent: Int, keyBlock: CGFloat) -> CGFloat {
    min(adjustmentRange.upperBound, max(adjustmentRange.lowerBound, (CGFloat(percent) / 100 * keyBlock - keyBlock).rounded()))
  }
}

struct KeyboardGeometry: Equatable {
  let keySpacing: Double
  let rowSpacing: Double
  var sidebarRatio: Double { 0.14 }
  /// 手机的中间字母行在两侧各缩进按键区的这一比例，约半个键宽，让九个键像设计稿那样居中排在上一行十个键的下方。
  var letterInsetRatio: Double { 0.05 }
  var centeredLetters: Bool { true }
  var showsBottomLanguage: Bool { true }
  var showsFullKeyboardSymbols: Bool { false }

  static func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
    SharedNumber.clamped(value, to: lower...upper)
  }

  static func formattedHeightAdjustment(_ value: Double) -> String {
    value > 0 ? "+\(Int(value))" : "\(Int(value))"
  }
}
