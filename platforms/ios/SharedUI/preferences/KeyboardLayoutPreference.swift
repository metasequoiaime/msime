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
  static let bilingualGlideKey = "keyboard.gesture.bilingualGlide"
  static let cursorSwipeKey = "keyboard.gesture.cursorSwipe"
  static let backspaceSwipeSpeedKey = "keyboard.gesture.backspaceSwipeSpeed"
  static let spaceVoiceKey = "keyboard.gesture.spaceVoice"
  static let swipeSymbolsKey = "keyboard.gesture.swipeSymbols"
  static let swipeSymbolsDirectionKey = "keyboard.gesture.swipeSymbolsDirection"
  static let letterCornerHintsKey = "keyboard.gesture.letterCornerHints"
  enum SwipeDirection: String {
    case up, down, both

    /// 这个偏好下，给定方向是否回落成角标。`both` 时上下滑都算，单选时只算自己。
    func covers(_ direction: SwipeDirection) -> Bool {
      self == .both || self == direction
    }
  }
  static let nineKeyFlickThresholdDocumentKey = "touch_nine_key_flick_threshold"
  static let nineKeyFlickThresholdKey = "keyboard.gesture.nineKeyFlickThreshold"
  static let nineKeyFlickThresholdDefault: Double = 24
  static let nineKeyFlickThresholdRange: ClosedRange<Double> = 14...48
  static let nineKeyFlickThresholdStep: Double = 2
  static var nineKeyFlickThreshold: Double {
    get { spacing(key: nineKeyFlickThresholdKey, fallback: nineKeyFlickThresholdDefault, range: nineKeyFlickThresholdRange) }
    set { defaults.set(normalizedNineKeyFlickThreshold(newValue), forKey: nineKeyFlickThresholdKey) }
  }
  static func normalizedNineKeyFlickThreshold(_ value: Double) -> Double {
    let clamped = SharedNumber.clamped(value, to: nineKeyFlickThresholdRange)
    let steps = ((clamped - nineKeyFlickThresholdRange.lowerBound) / nineKeyFlickThresholdStep).rounded()
    return nineKeyFlickThresholdRange.lowerBound + steps * nineKeyFlickThresholdStep
  }
  static func sharedNineKeyFlickThreshold(_ value: Any?) -> Double? {
    guard let integer = SharedNumber.strictInt(value) else { return nil }
    let value = Double(integer)
    guard nineKeyFlickThresholdRange.contains(value),
          (integer - Int(nineKeyFlickThresholdRange.lowerBound)) % Int(nineKeyFlickThresholdStep) == 0 else { return nil }
    return value
  }
  static let quickPunctuationSymbolsKey = "keyboard.quickPunctuationSymbols"
  static var defaultQuickPunctuationSymbols: [String] { normalizeQuickPunctuationSymbols([]) }
  static let hideLanguageKeyKey = "keyboard.bottom.hideLanguageKey"
  static var hideLanguageKey: Bool {
    get { defaults.object(forKey: hideLanguageKeyKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: hideLanguageKeyKey) }
  }
  /// 隐藏底行的逗号句号键，份额并给空格；只作用于手机中文 26 键字母层的「标准」「长空格」「表情键」样式（逗号句号样式本身在空格两侧、不适用）。与「隐藏中/英键」一样是本机偏好，逗号句号仍可通过上下滑、快捷符号等输入。
  static let hidePunctuationKeyKey = "keyboard.bottom.hidePunctuationKey"
  static var hidePunctuationKey: Bool {
    get { defaults.object(forKey: hidePunctuationKeyKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: hidePunctuationKeyKey) }
  }
  /// 隐藏底行的「符」键（符号键盘入口）并把份额并给空格；只作用于手机中文 26 键字母层的「标准」「逗号句号」样式（长空格、表情键没有符键）。符号和数字仍可从 123 进入，也是本机偏好。
  static let hideSymbolKeyKey = "keyboard.bottom.hideSymbolKey"
  static var hideSymbolKey: Bool {
    get { defaults.object(forKey: hideSymbolKeyKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: hideSymbolKeyKey) }
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
  ///
  /// 本机 App Group 里存的是完整的调整量，范围是 `KeyboardHeightPercent.adjustmentRange`（-12...80）；共享文档的 `touch_keyboard_height_adjustment` 只收 -12...48，写文档时截到 48，镜像回来时由 `reconciledHeightAdjustment` 保留本机超出的部分。
  static var heightAdjustment: Double {
    get { spacing(key: heightAdjustmentKey, fallback: 0, range: heightAdjustmentRange) }
    set { defaults.set(SharedNumber.clamped(newValue, to: heightAdjustmentRange), forKey: heightAdjustmentKey) }
  }
  static var heightAdjustmentRange: ClosedRange<Double> {
    Double(KeyboardHeightPercent.adjustmentRange.lowerBound)...Double(KeyboardHeightPercent.adjustmentRange.upperBound)
  }
  static var sharedHeightAdjustmentRange: ClosedRange<Double> {
    Double(KeyboardHeightPercent.sharedAdjustmentRange.lowerBound)...Double(KeyboardHeightPercent.sharedAdjustmentRange.upperBound)
  }

  /// 共享文档里的高度与本机存的高度合在一起：本机的值截到文档范围后与文档相同，说明文档就是本机写下去的（超过 48 的部分被截掉了），保留本机的完整值；不同则说明文档在别处改过，以文档为准。
  static func reconciledHeightAdjustment(document: Double, local: Double) -> Double {
    SharedNumber.clamped(local, to: sharedHeightAdjustmentRange) == document ? local : document
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
    for stored in [keySpacingKey, rowSpacingKey, heightAdjustmentKey, nineKeyFlickThresholdKey] {
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

  /// 把共享文档里的几何与快捷符号镜像到 App Group。缺少几何项时保持原值，快捷符号缺省时采用共享默认；九键 flick 阈值缺键或不合规时是默认 24（client-core 保存时不写默认值，缺键就是 24）；键盘和设置页每次出现都按文档对齐。文档里的 `touch_voice_shortcut` 是 Android 的工具栏语音按钮，iOS 工具栏不放语音按钮，不镜像它。
  static func mirrorGeometry(_ preferences: [String: Any]?) {
    guard let preferences else { return }
    defaults.set(normalizeQuickPunctuationSymbols(preferences["touch_quick_punctuation_symbols"] as? [String] ?? []),
                 forKey: quickPunctuationSymbolsKey)
    if let spacing = sharedKeySpacing(preferences["touch_key_spacing_tenths"]) { keySpacing = spacing }
    if let spacing = sharedRowSpacing(preferences["touch_row_spacing_tenths"]) { rowSpacing = spacing }
    if let adjustment = sharedHeightAdjustment(preferences["touch_keyboard_height_adjustment"]) {
      heightAdjustment = reconciledHeightAdjustment(document: adjustment, local: heightAdjustment)
    }
    nineKeyFlickThreshold = sharedNineKeyFlickThreshold(preferences[nineKeyFlickThresholdDocumentKey])
      ?? nineKeyFlickThresholdDefault
  }

  /// 首次加载键盘时只镜像九键 flick 阈值，不覆盖本机已经应用的键距、行距和高度。文档读不到（`nil`）时保持原值；文档里缺键或不合规时是默认 24：默认值不写进文件，别处调回 24 后这里必须跟着回到 24。
  static func mirrorNineKeyFlickThreshold(_ preferences: [String: Any]?) {
    guard let preferences else { return }
    nineKeyFlickThreshold = sharedNineKeyFlickThreshold(preferences[nineKeyFlickThresholdDocumentKey])
      ?? nineKeyFlickThresholdDefault
  }

  /// 保存九键 flick 阈值，只修改共享文档这一项；写入成功后同步 App Group 镜像。
  @discardableResult
  static func saveNineKeyFlickThreshold(_ value: Double, stateRoot: URL? = nil) -> Bool {
    let normalized = normalizedNineKeyFlickThreshold(value)
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[nineKeyFlickThresholdDocumentKey] = Int(normalized)
    }) else { return false }
    nineKeyFlickThreshold = normalized
    return true
  }

  /// 「双拼键位提示」：双拼方案和粤语双拼的 26 键在字母键底部画这个键代表的声母和韵母。共享文档里是 `touch_shuangpin_key_hints`，缺省为关；键盘从 App Group 镜像读。
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
  /// 中英混合滑行只在本机启用，缺省关闭，不改变现有中文滑行。
  static var bilingualGlide: Bool {
    get { defaults.object(forKey: bilingualGlideKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: bilingualGlideKey) }
  }
  /// 「横滑键盘移动光标」：无组字时从普通字符键横滑移动文档光标。默认关，与滑行输入一样只保存在本机 App Group。
  static var cursorSwipe: Bool {
    get { defaults.object(forKey: cursorSwipeKey) as? Bool ?? false }
    set { defaults.set(newValue, forKey: cursorSwipeKey) }
  }
  /// 「滑动删除速度」：按住 ⌫ 往左滑删除时的快慢，1 到 10 档，档位越高越快。只存本机 App Group，不进共享文档、不随设置同步（与横滑光标一样）。
  static let backspaceSwipeSpeedRange: ClosedRange<Int> = 1...10
  static let backspaceSwipeSpeedDefault = 7
  static var backspaceSwipeSpeed: Int {
    get {
      let stored = defaults.object(forKey: backspaceSwipeSpeedKey) as? Int ?? backspaceSwipeSpeedDefault
      return min(max(stored, backspaceSwipeSpeedRange.lowerBound), backspaceSwipeSpeedRange.upperBound)
    }
    set {
      defaults.set(min(max(newValue, backspaceSwipeSpeedRange.lowerBound), backspaceSwipeSpeedRange.upperBound),
                   forKey: backspaceSwipeSpeedKey)
    }
  }
  /// 某一档对应的每删一个要往左滑的距离（pt）：第 1 档 22pt，每高一档少 2pt，第 10 档 4pt；默认第 7 档 10pt。
  static func backspaceSwipeStep(speed: Int) -> Double {
    let level = min(max(speed, backspaceSwipeSpeedRange.lowerBound), backspaceSwipeSpeedRange.upperBound)
    return Double(24 - 2 * level)
  }
  /// 键盘横滑删除实际用的每格距离。
  static var backspaceSwipeStep: Double { backspaceSwipeStep(speed: backspaceSwipeSpeed) }
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
  static var geometry: KeyboardGeometry {
    KeyboardGeometry(keySpacing: keySpacing, rowSpacing: rowSpacing, showsBottomLanguage: !hideLanguageKey,
                     hidesPunctuation: hidePunctuationKey, hidesSymbol: hideSymbolKey)
  }

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
    return Double(SharedNumber.clamped(integer, to: Int(KeyboardHeightPercent.sharedAdjustmentRange.lowerBound)...Int(KeyboardHeightPercent.sharedAdjustmentRange.upperBound)))
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
  /// `touch_keyboard_height_adjustment` 经过校验的取值范围，单位为点。它是跨平台、随设置同步的字段（`crates/client-core/src/preferences.rs` 只接受 `-12..=48`），iOS 不改它。
  static let sharedAdjustmentRange: ClosedRange<CGFloat> = -12...48

  /// iOS 上可调的范围，单位为点。手机键帽从 52pt 降到 44pt 后默认键块矮了 32pt（229pt → 197pt），上限随之放宽 32pt，到 80：手机竖屏最高仍是 197 + 80 = 277pt，与以前 229 + 48 相同，调高过的用户升级后能调回原来的高度。下限不变。超过 48 的部分只存在本机 App Group（`KeyboardLayoutPreference.heightAdjustment`），共享文档里写 48。
  static let adjustmentRange: ClosedRange<CGFloat> = -12...80

  /// 竖屏的按键高度：手机按键 44pt，iPad 按键 54pt（`dc.html` 的 `keyH`）。键盘高度由它累加而来（`KeyboardFormFactor.keyboardHeight`），所以 100% 画出的正好是这样的按键。
  ///
  /// 手机曾是 52pt（为了九键的宽键帽不显得扁），26 键另收到 44pt 再在键区上下垫回 52pt 的外框。结果整块键盘比系统键盘和其他第三方键盘高出一截，26 键和数字九宫格之间切换时按键区一高一低。现在所有键面统一 44pt，与 26 键原来的键帽一样；觉得矮的用户仍可在「键盘高度」里加高。
  static func portraitKeyHeight(tablet: Bool) -> CGFloat { tablet ? 54 : 44 }

  /// 默认高度下的按键区：`rows` 行高为 `keyHeight` 的按键，加上行与行之间的间距。手机竖屏、默认行距 7pt 时为 197pt。
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

/// 26 键字母键右上角的角标符号，取自设计稿：q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。
/// 键盘画角标、按角标上屏、设置页显示「角标符号 ?」都用这一张表；App 与键盘扩展都编入 SharedUI，所以放在这里。
/// 与 Android 的 `LetterHintTable` 是同一张表。
enum LetterHintSymbols {
  private static let table: [Character: String] = [
    "q": "1", "w": "2", "e": "3", "r": "4", "t": "5", "y": "6", "u": "7", "i": "8", "o": "9", "p": "0",
    "a": "@", "s": "#", "d": "¥", "f": "%", "g": "&", "h": "*", "j": "(", "k": ")", "l": "\"",
    "z": "~", "x": "…", "c": "、", "v": "?", "b": "!", "n": "-", "m": "/",
  ]

  /// 字母键的角标符号，大小写字母都可传入；不是 26 个 ASCII 字母之一时为 nil。
  static func hint(for letter: String) -> String? {
    guard letter.count == 1, let character = letter.first, character.isASCII else { return nil }
    return table[Character(character.lowercased())]
  }
}

/// 字母键一个方向上的滑动动作：26 键每个字母的上滑、下滑各自可配，互不影响。
/// 未单独配置的方向按「滑动方向」（`KeyboardLayoutPreference.swipeSymbolsDirection`）回落成角标，保持旧行为。
enum LetterSwipeAction: String, CaseIterable, Codable, Hashable {
  case none
  case symbol
  case paste
  case copy
  case cut
  case undo
  case redo
  case selectAll
  case text
  case clipboard
  case language
  case cursorLeft
  case cursorRight
  /// 以下对应水杉图标菜单（更多菜单）里的快捷操作，滑动一下即切换或调用。
  case fullWidth
  case chinesePunctuation
  case traditional
  case handwriting
  case oneHand
  case incognito
  case emoji
  case aiAssist
  case reply
  case voiceInput

  /// 设置页和预览气泡上的名字；`symbol` 带上这个键的角标，`nil` 时只写「角标符号」。
  func title(symbol: String? = nil) -> String {
    switch self {
    case .none: "无"
    case .symbol: symbol.map { "角标符号 \($0)" } ?? "角标符号"
    case .paste: "粘贴"
    case .copy: "复制选中文字"
    case .cut: "剪切选中文字"
    case .undo: "撤销"
    case .redo: "重做"
    case .selectAll: "全选"
    case .text: "自定义文本"
    case .clipboard: "剪贴板历史"
    case .language: "切换中英文"
    case .cursorLeft: "光标左移"
    case .cursorRight: "光标右移"
    case .fullWidth: "切换全角/半角"
    case .chinesePunctuation: "切换中文标点"
    case .traditional: "切换繁简"
    case .handwriting: "切换手写"
    case .oneHand: "切换单手模式"
    case .incognito: "切换隐私模式"
    case .emoji: "表情"
    case .aiAssist: "AI 润色"
    case .reply: "高情商回复"
    case .voiceInput: "语音输入"
    }
  }

  /// 预览图键面徽标上的短名：只写动作本身，不写方向；`symbol` 直接写角标符号。
  func shortTitle(symbol: String? = nil) -> String {
    switch self {
    case .none: "无"
    case .symbol: symbol ?? "角标"
    case .paste: "粘贴"
    case .copy: "复制"
    case .cut: "剪切"
    case .undo: "撤销"
    case .redo: "重做"
    case .selectAll: "全选"
    case .text: "文本"
    case .clipboard: "剪贴板"
    case .language: "中英"
    case .cursorLeft: "左移"
    case .cursorRight: "右移"
    case .fullWidth: "全半角"
    case .chinesePunctuation: "标点"
    case .traditional: "繁简"
    case .handwriting: "手写"
    case .oneHand: "单手"
    case .incognito: "隐私"
    case .emoji: "表情"
    case .aiAssist: "润色"
    case .reply: "回复"
    case .voiceInput: "语音"
    }
  }

  /// 写系统剪贴板的动作要键盘有完全访问；其余动作不需要。
  var writesPasteboard: Bool { self == .copy || self == .cut }
  /// 读系统剪贴板的动作要键盘有完全访问。
  var readsPasteboard: Bool { self == .paste }
}

/// 一个方向上的一条配置。`text` 只在动作为 `.text` 时使用；为空时这条配置等于「无」。
struct LetterSwipeBinding: Codable, Equatable {
  var action: LetterSwipeAction = .none
  var text: String?

  static let none = LetterSwipeBinding(action: .none, text: nil)
  static let symbol = LetterSwipeBinding(action: .symbol, text: nil)

  /// 选中的自定义文本，去掉首尾空白和换行后为空就是没有。
  var trimmedText: String {
    (text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
  }

  /// 这次滑动是否真会做事。选了自定义文本却没填时不算。
  var isActive: Bool {
    action != .none && (action != .text || !trimmedText.isEmpty)
  }

  /// 设置页一行里的短名字。`symbol` 传这个键的角标符号，让「角标符号」直接显示成真实的符号（如 Q 键显示「角标符号 1」）。
  func summary(symbol: String? = nil) -> String {
    action == .text && isActive ? "文本：\(trimmedText)" : action.title(symbol: symbol)
  }
}

/// 每个字母键上滑、下滑的配置，存本机 App Group（不随设置同步，与「滑动输入符号」一样）。
/// JSON 形如 `{"v":{"up":{"action":"symbol"},"down":{"action":"paste"}}}`，缺的键、坏的值都回落到默认。
enum LetterSwipePreference {
  static let key = "keyboard.gesture.letterSwipeBindings"
  /// 26 键从上到下的字母顺序，设置页按它排列。
  static let letters: [String] = "qwertyuiopasdfghjklzxcvbnm".map(String.init)
  static let upKey = "up"
  static let downKey = "down"

  static func directionKey(_ direction: KeyboardLayoutPreference.SwipeDirection) -> String {
    direction == .up ? upKey : downKey
  }

  /// 全部已存配置；没有配置或解析失败时为空。
  static var all: [String: [String: LetterSwipeBinding]] {
    guard let data = KeyboardLayoutPreference.defaults.data(forKey: key),
          let decoded = try? JSONDecoder().decode([String: [String: LetterSwipeBinding]].self, from: data) else {
      return [:]
    }
    return decoded
  }

  /// 用户单独存过的那条配置；没存过时为 nil。
  static func stored(letter: String, direction: KeyboardLayoutPreference.SwipeDirection) -> LetterSwipeBinding? {
    all[letter.lowercased()]?[directionKey(direction)]
  }

  /// 实际生效的配置：存过的用它，没存过的按「滑动方向」回落成角标。
  static func binding(letter: String, direction: KeyboardLayoutPreference.SwipeDirection) -> LetterSwipeBinding {
    if let stored = stored(letter: letter, direction: direction) { return stored }
    return KeyboardLayoutPreference.swipeSymbolsDirection.covers(direction) ? .symbol : .none
  }

  /// 写入一条配置；`binding` 为 nil 时删掉这一条，回到默认。「无」要照原样存下去才压得住默认的角标，
  /// 不能当成没配过：用户把上滑改成「无」时，默认回落会让它又变回角标。
  static func set(_ binding: LetterSwipeBinding?, letter: String, direction: KeyboardLayoutPreference.SwipeDirection) {
    var table = all
    let letter = letter.lowercased()
    var directions = table[letter] ?? [:]
    if let binding {
      directions[directionKey(direction)] = binding
    } else {
      directions.removeValue(forKey: directionKey(direction))
    }
    if directions.isEmpty { table.removeValue(forKey: letter) } else { table[letter] = directions }
    write(table)
  }

  /// 恢复一个键的两个方向。
  static func clear(letter: String) {
    var table = all
    table.removeValue(forKey: letter.lowercased())
    write(table)
  }

  /// 恢复全部 26 键。
  static func reset() {
    KeyboardLayoutPreference.defaults.removeObject(forKey: key)
  }

  static var hasCustomBindings: Bool { !all.isEmpty }

  /// 键盘读一次配置时用，省得每次手势都解一遍 JSON。
  static func load() -> [String: [String: LetterSwipeBinding]] { all }

  /// 从已读出的表里取一条，语义与 `binding(letter:direction:)` 相同。
  static func binding(in table: [String: [String: LetterSwipeBinding]], letter: String,
                      direction: KeyboardLayoutPreference.SwipeDirection) -> LetterSwipeBinding {
    if let stored = table[letter.lowercased()]?[directionKey(direction)] { return stored }
    return KeyboardLayoutPreference.swipeSymbolsDirection.covers(direction) ? .symbol : .none
  }

  private static func write(_ table: [String: [String: LetterSwipeBinding]]) {
    let defaults = KeyboardLayoutPreference.defaults
    guard !table.isEmpty else {
      defaults.removeObject(forKey: key)
      return
    }
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys]
    guard let data = try? encoder.encode(table) else { return }
    defaults.set(data, forKey: key)
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
  var hidesPunctuation = false
  var hidesSymbol = false
  var showsFullKeyboardSymbols: Bool { false }

  static func clamped(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
    SharedNumber.clamped(value, to: lower...upper)
  }

  static func formattedHeightAdjustment(_ value: Double) -> String {
    value > 0 ? "+\(Int(value))" : "\(Int(value))"
  }
}
