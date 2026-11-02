import Foundation
import CoreFoundation

@_silgen_name("msime_client_touch_quick_symbols")
private func msimeClientTouchQuickSymbols(_ value: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

enum KeyboardLayoutPreference {
  static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }
  static let keySpacingKey = "keyboard.spacing.keys"
  static let rowSpacingKey = "keyboard.spacing.rows"
  static let heightAdjustmentKey = "keyboard.height.adjustment"
  static let tabletFullKeysKey = "keyboard.tablet.fullKeys"
  static let tabletSplitKey = "keyboard.tablet.split"
  static let phoneLandscapeSplitKey = "keyboard.phone.landscapeSplit"
  static let glideTypingKey = "keyboard.gesture.glide"
  static let cursorSwipeKey = "keyboard.gesture.cursorSwipe"
  static let spaceVoiceKey = "keyboard.gesture.spaceVoice"
  static let swipeSymbolsKey = "keyboard.gesture.swipeSymbols"
  static let swipeSymbolsDirectionKey = "keyboard.gesture.swipeSymbolsDirection"
  static let letterCornerHintsKey = "keyboard.gesture.letterCornerHints"
  enum SwipeDirection: String { case up, down }
  static let quickPunctuationSymbolsKey = "keyboard.quickPunctuationSymbols"
  static var defaultQuickPunctuationSymbols: [String] { normalizeQuickPunctuationSymbols([]) }
  static let hideLanguageKeyKey = "keyboard.bottom.hideLanguageKey"
  static var hideLanguageKey: Bool {
    get { defaults.object(forKey: hideLanguageKeyKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: hideLanguageKeyKey) }
  }
  static let oneHandedKey = "keyboard.oneHanded"
  /// 单手模式以前只在手机形态的键盘上画（iPhone、iPad 浮动键盘和窄窗口），全宽的 iPad 键盘一律不画；全宽键盘补上单手模式（#10203）之后，升级前存下的值不能直接带过去，否则在浮动键盘上开过单手的人一升级，停靠的全宽键盘就缩成单手块。用户之后每次切换单手模式都会打上这个标记；没有标记时，全宽键盘第一次画单手之前把存下的值改成关闭（`adoptOneHandedOnDockedTablet`）。
  static let oneHandedDockedTabletKey = "keyboard.oneHandedDockedTablet"
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
    for stored in [keySpacingKey, rowSpacingKey, heightAdjustmentKey] {
      defaults.removeObject(forKey: stored)
    }
  }
  /// Save the geometry where the keyboard reads it. The keyboard copies the shared document's `touch_*` fields over the App Group every time it appears, so a value written only to the App Group lasted until then. The App Group is written after the document, and only when the document took the change.
  ///
  /// 只写传了值的那几项，其余保持文档里的样子。设置页原先每次都把四项一起写：键盘里调过高度之后，设置页上停留的还是旧高度，这时动一下任何别的控件，旧高度就被写回文档，键盘下次出现又变回设置页的值。
  @discardableResult
  static func saveGeometry(keySpacing: Double? = nil, rowSpacing: Double? = nil, heightAdjustment: Double? = nil,
                           stateRoot: URL? = nil) -> Bool {
    guard let mapping = MetasequoiaInputSessionBridge.geometryMapping(
      keySpacing: keySpacing, rowSpacing: rowSpacing, heightAdjustment: heightAdjustment),
      MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, mapping) else { return false }
    if let keySpacing { self.keySpacing = keySpacing }
    if let rowSpacing { self.rowSpacing = rowSpacing }
    if let heightAdjustment { self.heightAdjustment = heightAdjustment }
    return true
  }

  /// 把共享文档里的几何与快捷符号镜像到 App Group。缺少几何项时保持原值，快捷符号缺省时采用共享默认；键盘和设置页每次出现都按文档对齐。文档里的 `touch_voice_shortcut` 是 Android 的工具栏语音按钮，iOS 工具栏不放语音按钮，不镜像它。
  static func mirrorGeometry(_ preferences: [String: Any]) {
    defaults.set(normalizeQuickPunctuationSymbols(preferences["touch_quick_punctuation_symbols"] as? [String] ?? []),
                 forKey: quickPunctuationSymbolsKey)
    if let spacing = sharedKeySpacing(preferences["touch_key_spacing_tenths"]) { keySpacing = spacing }
    if let spacing = sharedRowSpacing(preferences["touch_row_spacing_tenths"]) { rowSpacing = spacing }
    if let adjustment = sharedHeightAdjustment(preferences["touch_keyboard_height_adjustment"]) {
      heightAdjustment = adjustment
    }
  }

  /// 「双拼键位提示」：双拼方案的 26 键在字母键底部画这个键代表的声母和韵母。共享文档里是 `touch_shuangpin_key_hints`，缺省为关；键盘从 App Group 镜像读。
  static let shuangpinKeyHintsDocumentKey = "touch_shuangpin_key_hints"
  static let shuangpinKeyHintsKey = "keyboard.shuangpin.keyHints"

  /// 文档里缺这一项或不是布尔值时按关，与 client-core 的默认值一致。
  static func sharedShuangpinKeyHints(in preferences: [String: Any]?) -> Bool {
    preferences?[shuangpinKeyHintsDocumentKey] as? Bool ?? false
  }

  static var shuangpinKeyHints: Bool {
    get { defaults.object(forKey: shuangpinKeyHintsKey) as? Bool ?? false }
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

  /// 「组字时保留 Shift」：全拼 26 键组字时第三排左端默认保留 ⇧，点一下臂辅助码（Shift+字母）；关闭后换成「分词」（送 `'` 断音节）。共享文档里是 `touch_quanpin_shift`，缺省为开；键盘从 App Group 镜像读。九键、14 键和 18 键的分词键不受它影响。
  static let quanpinShiftDocumentKey = "touch_quanpin_shift"
  static let quanpinShiftKey = "keyboard.quanpin.shift"

  /// 文档里缺这一项或不是布尔值时按开，与 client-core 的默认值一致。
  static func sharedQuanpinShift(in preferences: [String: Any]?) -> Bool {
    preferences?[quanpinShiftDocumentKey] as? Bool ?? true
  }

  static var quanpinShift: Bool {
    get { defaults.object(forKey: quanpinShiftKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: quanpinShiftKey) }
  }

  /// 把「组字时保留 Shift」写进共享文档，只改这一项；文档接受了才更新 App Group 镜像。
  @discardableResult
  static func saveQuanpinShift(_ enabled: Bool, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[quanpinShiftDocumentKey] = enabled
    }) else { return false }
    quanpinShift = enabled
    return true
  }

  /// `resetToDefaults`, for the shared document as well.
  @discardableResult
  static func resetGeometry(stateRoot: URL? = nil) -> Bool {
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot) { preferences in
      for key in ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
                  "touch_quick_punctuation_symbols"] {
        preferences.removeValue(forKey: key)
      }
    }
    guard written else { return false }
    resetToDefaults()
    // 「横屏分离式键盘」和间距、高度在 App 的同一页，一起恢复成默认（关）。键盘里的间距面板也调 `resetToDefaults`，那里不显示这个开关，所以不放进上面的列表，免得在键盘里点「恢复默认」把分离式键盘悄悄关掉。
    defaults.removeObject(forKey: quickPunctuationSymbolsKey)
    defaults.removeObject(forKey: tabletSplitKey)
    defaults.removeObject(forKey: phoneLandscapeSplitKey)
    defaults.removeObject(forKey: hideLanguageKeyKey)
    return true
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
  /// 「分离式键盘」的 iPhone 版（#10224）：iPhone 横屏时把 26 键字母布局从中间分成左右两半，几何与 iPad 的横屏分离式键盘相同。默认关，现有用户的横屏键盘不变；键盘视图窄得分开后字母键不到 `KeyboardSplitLayout.phoneMinimumLetterKeyWidth` 时不分，真机上的 iPhone 横屏键盘都够宽。只存在本机 App Group，不随设置同步，与 iPad 的开关一样。和 iPad 的开关分开存，免得同一个值在两种设备上意思不同。
  static var phoneLandscapeSplit: Bool {
    get { defaults.object(forKey: phoneLandscapeSplitKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: phoneLandscapeSplitKey) }
  }
  /// 「滑行输入」：在全拼 26 键的字母键上连续滑动输入拼音（#5347），由 Engine 把一笔滑行解码成拼音字母。默认关；只存在本机 App Group，不进共享文档，也不随设置同步。
  static var glideTyping: Bool {
    get { defaults.object(forKey: glideTypingKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: glideTypingKey) }
  }
  /// 「横滑键盘移动光标」：无组字时从普通字符键横滑移动文档光标。默认关，与滑行输入一样只保存在本机 App Group。
  static var cursorSwipe: Bool {
    get { defaults.object(forKey: cursorSwipeKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: cursorSwipeKey) }
  }
  /// 「长按空格语音输入」：在空格键上按住 450 ms 打开语音面板。默认开，与 Android 的 `platform.android.space_voice` 相同；只存在本机 App Group，不进共享文档，也不随设置同步。「语音输入」页的「长按空格说话」开关直接读写它；iOS 没有工具栏语音按钮。
  static var spaceVoice: Bool {
    get { defaults.object(forKey: spaceVoiceKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: spaceVoiceKey) }
  }
  /// 「滑动输入符号」：沿设定方向滑动字母键，输入右上角的角标符号。默认开，与 Android 的 `platform.android.swipe_down_symbols` 相同；关掉后长按字母键仍能输入角标，与 Android 一致。只存在本机 App Group，不随设置同步。
  static var swipeSymbols: Bool {
    get { defaults.object(forKey: swipeSymbolsKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: swipeSymbolsKey) }
  }
  /// 「显示字母键角标」：26 键字母键右上角画角标符号。默认开，与 Android 的 `platform.android.letter_corner_hints`、HarmonyOS 的 `letterCornerHints` 相同；关掉只是不画，长按和「滑动输入符号」照样输入角标（#10587）。只存在本机 App Group，不随设置同步，与「滑动输入符号」一样。
  static var letterCornerHints: Bool {
    get { defaults.object(forKey: letterCornerHintsKey) as? Bool ?? true }
    set { defaults.set(newValue, forKey: letterCornerHintsKey) }
  }
  /// 三端 26 键默认上滑输入角标，保留用户明确选择的下滑方向；只保存在本机 App Group。
  static var swipeSymbolsDirection: SwipeDirection {
    get { SwipeDirection(rawValue: defaults.string(forKey: swipeSymbolsDirectionKey) ?? "") ?? .up }
    set { defaults.set(newValue.rawValue, forKey: swipeSymbolsDirectionKey) }
  }
  /// App Group 仅缓存共享文档的快捷符号，默认值和校验由 Rust 提供。
  static var quickPunctuationSymbols: [String] {
    normalizeQuickPunctuationSymbols(defaults.array(forKey: quickPunctuationSymbolsKey) as? [String] ?? [])
  }

  @discardableResult
  static func saveQuickPunctuationSymbols(_ symbols: [String], stateRoot: URL? = nil) -> Bool {
    let normalized = normalizeQuickPunctuationSymbols(symbols)
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document["touch_quick_punctuation_symbols"] = normalized
    }) else { return false }
    defaults.set(normalized, forKey: quickPunctuationSymbolsKey)
    return true
  }

  static func normalizeQuickPunctuationSymbols(_ symbols: [String]) -> [String] {
    // 桥不可用时仅保留一个可点击的标点，正常默认列表由共享层返回。
    ClientRequestEnvelope.call(msimeClientTouchQuickSymbols, ["symbols": symbols]) as? [String] ?? ["，"]
  }

  /// 文本框按字素收集选项，规范化规则留在共享层。
  static func quickPunctuationSymbols(from text: String) -> [String] {
    normalizeQuickPunctuationSymbols(text.map(String.init))
  }
  /// 「单手模式」：手机键盘的按键收窄到 85% 并靠向一侧，另一侧是「换到另一侧」和「退出单手」组成的一列。默认关闭；它只存在本设备的 App Group 里，不进共享文档，iOS 的设置同步也不携带它；Android 的 `platform.android.one_handed` 会同步。无法识别的存储值按关闭处理。
  static var oneHanded: KeyboardOneHandedMode {
    get { defaults.string(forKey: oneHandedKey).flatMap(KeyboardOneHandedMode.init(rawValue:)) ?? .off }
    set {
      defaults.set(newValue.rawValue, forKey: oneHandedKey)
      defaults.set(true, forKey: oneHandedDockedTabletKey)
    }
  }
  /// 全宽 iPad 键盘画单手模式之前调用：存下的值是升级前留下的（没有 `oneHandedDockedTabletKey` 标记）就改成关闭并打上标记，只做一次。返回此后该画的存储值。
  static func adoptOneHandedOnDockedTablet() -> KeyboardOneHandedMode {
    if !defaults.bool(forKey: oneHandedDockedTabletKey) { oneHanded = .off }
    return oneHanded
  }
  static let bottomLiftKey = "keyboard.bottomLift"
  static let widthPercentKey = "keyboard.widthPercent"
  static let positionXKey = "keyboard.positionX"
  /// `placement` 读写的三个键。App「开发者 → 重置所有设置」由 Rust 整份恢复共享文档，只存本机的这三项要另外清掉（`DeveloperDiagnostics.resetLocalPreferences`），与 Android 的 `AndroidLocalSettings.restoreDefaults` 一样回到全宽、不抬高、居中。
  static let placementKeys = [bottomLiftKey, widthPercentKey, positionXKey]
  /// 「键盘调整」里的底部抬高、宽度和水平位置（`KeyboardPlacement`）。与单手模式一样只存在本设备的 App Group 里，不进共享文档，也不随设置同步；高度仍是共享的 `touch_keyboard_height_adjustment`（`heightAdjustment`）。存储值不是数字时按缺省，超出范围的限制到范围内。
  static var placement: KeyboardPlacement {
    get {
      KeyboardPlacement(
        bottomLift: storedInt(bottomLiftKey) ?? KeyboardPlacement.standard.bottomLift,
        widthPercent: storedInt(widthPercentKey) ?? KeyboardPlacement.standard.widthPercent,
        positionX: storedInt(positionXKey) ?? KeyboardPlacement.standard.positionX)
    }
    set {
      defaults.set(newValue.bottomLift, forKey: bottomLiftKey)
      defaults.set(newValue.widthPercent, forKey: widthPercentKey)
      defaults.set(newValue.positionX, forKey: positionXKey)
    }
  }

  private static func storedInt(_ key: String) -> Int? {
    guard let value = defaults.object(forKey: key) as? NSNumber, value.doubleValue.isFinite,
          CFGetTypeID(value) != CFBooleanGetTypeID() else { return nil }
    // 先收到一个远大于各项范围的区间再转整数：`Int(Double)` 遇到超出 `Int` 的值（如 1e300）会直接崩掉键盘，而 `KeyboardPlacement` 随后照样把它限制到范围内。
    return Int(SharedNumber.clamped(value.doubleValue, to: -1_000_000...1_000_000).rounded())
  }
  static var geometry: KeyboardGeometry { KeyboardGeometry(keySpacing: keySpacing, rowSpacing: rowSpacing, showsBottomLanguage: !hideLanguageKey) }

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

/// 「九键滑动输入数字」（#10229）：拼音九键字母层 1–9 键沿所选方向滑动输入键上的数字，反方向滑动弹出数字和字母（与长按相同）。共享文档里是 `touch_nine_key_swipe_digits`（client-core `TouchNineKeySwipeDigits`），写法 `off` / `up` / `down` 与 Android、鸿蒙相同，缺省为关；键盘从 App Group 镜像读。iOS 以前没有这项设置，没有旧值要迁移，也和其他触屏按键布局偏好一样暂不进 iOS 的设置同步。
enum NineKeySwipeDigits: String, CaseIterable, Hashable {
  case off, up, down

  static let documentKey = "touch_nine_key_swipe_digits"
  static let mirrorKey = "keyboard.nineKeySwipeDigits"

  /// 设置页这一行的标题、说明和选项单标题，三端逐字相同。
  static let title = "九键滑动输入数字"
  static let subtitle = "沿所选方向滑动输入数字，反方向弹出字母。"

  /// 选项单里的文字，按 `allCases` 的次序：关闭、上滑、下滑。
  var optionTitle: String {
    switch self {
    case .off: "关闭"
    case .up: "上滑"
    case .down: "下滑"
    }
  }

  /// 文档里的值：缺键、不是字符串或不认识的写法（较新版本加的方向）一律按关，与 client-core 的宽松读取一致。
  init(in document: [String: Any]?) {
    self = (document?[Self.documentKey] as? String).flatMap(Self.init(rawValue:)) ?? .off
  }

  private static var defaults: UserDefaults { InputSchemePreference.appGroupDefaults }

  /// App Group 里的镜像，键盘读它，不读文档。
  static var mirrored: NineKeySwipeDigits {
    get { defaults.string(forKey: mirrorKey).flatMap(Self.init(rawValue:)) ?? .off }
    set { defaults.set(newValue.rawValue, forKey: mirrorKey) }
  }

  /// 把文档里的值抄进镜像；读不到文档时保留镜像。
  static func mirror(_ document: [String: Any]?) {
    guard document != nil else { return }
    let value = NineKeySwipeDigits(in: document)
    if value != mirrored { mirrored = value }
  }

  /// 写用户点的那一项（包括 `off`，client-core 会把默认值从文档里省掉）；文档接受了才更新镜像。
  @discardableResult
  static func save(_ value: NineKeySwipeDigits, stateRoot: URL? = nil) -> Bool {
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { $0[documentKey] = value.rawValue })
    else { return false }
    mirrored = value
    return true
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

/// 「键盘调整」存下的整块键盘的摆放：底部抬高（pt）、宽度（占键盘视图宽度的百分比）和水平位置（左右两侧空出的宽度里左侧占的千分比，500 居中）。三端同一套取值与缺省，各端只存本机。构造时把每一项限制到它的范围内。
struct KeyboardPlacement: Equatable {
  static let bottomLiftRange = 0...160
  static let widthPercentRange = 60...100
  static let positionXRange = 0...1000
  /// 缺省：不抬高、全宽、居中。「重置」回到它。
  static let standard = KeyboardPlacement(bottomLift: 0, widthPercent: 100, positionX: 500)

  let bottomLift: Int
  let widthPercent: Int
  let positionX: Int

  init(bottomLift: Int, widthPercent: Int, positionX: Int) {
    self.bottomLift = min(max(bottomLift, Self.bottomLiftRange.lowerBound), Self.bottomLiftRange.upperBound)
    self.widthPercent = min(max(widthPercent, Self.widthPercentRange.lowerBound), Self.widthPercentRange.upperBound)
    self.positionX = min(max(positionX, Self.positionXRange.lowerBound), Self.positionXRange.upperBound)
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

  /// 竖屏的按键高度：手机按键 52pt，九键的宽键帽保留足够的纵向空间；iPad 按键 54pt（`dc.html` 的 `keyH`）。键盘高度由它累加而来（`KeyboardFormFactor.keyboardHeight`），所以 100% 画出的正好是这样的按键。
  ///
  /// 手机原是设计稿的 42pt：九键每个键约 105pt 宽，42pt 高看上去是扁的，上面的顶栏又按读音行加候选行留着 56pt，整块键盘头重脚轻。
  static func portraitKeyHeight(tablet: Bool) -> CGFloat { tablet ? 54 : 52 }

  /// 默认高度下的按键区：`rows` 行高为 `keyHeight` 的按键，加上行与行之间的间距。手机竖屏、默认行距 7pt 时为 229pt。
  static func keyBlockHeight(keyHeight: CGFloat, rows: Int, rowSpacing: CGFloat) -> CGFloat {
    max(1, CGFloat(rows) * keyHeight + CGFloat(max(rows - 1, 0)) * rowSpacing)
  }

  /// 手机键盘或全宽 iPad 键盘的竖屏按键区，后者的数字行多出第五行。手机画「按键布局」的顶部数字行时（`phoneNumberRow`）同样多一排。
  static func portraitKeyBlockHeight(tablet: Bool, numberRow: Bool, rowSpacing: CGFloat, phoneNumberRow: Bool = false) -> CGFloat {
    keyBlockHeight(keyHeight: portraitKeyHeight(tablet: tablet), rows: (tablet ? numberRow : phoneNumberRow) ? 5 : 4,
                   rowSpacing: rowSpacing)
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
  var showsBottomLanguage = true
  var showsFullKeyboardSymbols: Bool { false }

  static func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
    SharedNumber.clamped(value, to: lower...upper)
  }

  static func formattedHeightAdjustment(_ value: Double) -> String {
    value > 0 ? "+\(Int(value))" : "\(Int(value))"
  }
}
