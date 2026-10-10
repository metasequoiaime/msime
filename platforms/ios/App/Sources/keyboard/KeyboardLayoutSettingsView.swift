import SwiftUI
import UIKit

/// 键盘页，对应 Android 的 `KeyboardOptionsPage`：带高度拖柄的实时预览，然后是布局（中文键盘 26 / 14 / 9 键、数字键盘顺序、双拼键位提示、高度和间距）、按键反馈、手势、iPad 选项、恢复默认和更多（候选栏、键盘工具栏、AI 润色与回复）。
///
/// 设计稿里的按键弹出预览没有做：键盘本身不支持。手势里的滑动输入符号和长按空格语音输入与 Android 一样是本机开关（`KeyboardLayoutPreference.swipeSymbols`、`spaceVoice`），默认开。工具栏语音按钮的开关挪到了语音输入页，由那一页单独写 `touch_voice_shortcut`；这里每次只保存用户刚动的那一项几何尺寸，见 `save(_:)`。
struct KeyboardLayoutSettingsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @State private var keySpacing = KeyboardLayoutPreference.keySpacing
  @State private var rowSpacing = KeyboardLayoutPreference.rowSpacing
  @State private var height = KeyboardLayoutPreference.heightAdjustment
  @State private var skin = KeyboardTheme.current
  @State private var scheme = InputSchemePreference.scheme
  @State private var tabletFullKeys = KeyboardLayoutPreference.tabletFullKeys
  @State private var tabletSplit = KeyboardLayoutPreference.tabletSplit
  @State private var glideTyping = KeyboardLayoutPreference.glideTyping
  @State private var swipeSymbols = KeyboardLayoutPreference.swipeSymbols
  @State private var spaceVoice = KeyboardLayoutPreference.spaceVoice
  @State private var tabOpensCandidates = true
  @State private var numberKeypadOrder = KeyboardLayoutPreference.numberKeypadOrder
  @State private var twentySixKeyNumberLayout = KeyboardLayoutPreference.twentySixKeyNumberLayout
  @State private var shuangpinKeyHints = KeyboardLayoutPreference.shuangpinKeyHints
  @State private var dragBase: (height: Double, keySpacing: Double, rowSpacing: Double)?
  @State private var dragAxis: Axis?
  @State private var saveFailed = false
  @AppStorage(KeyboardFeedbackPreference.soundKey, store: KeyboardFeedbackPreference.defaults)
  private var soundEnabled = true
  @AppStorage(KeyboardFeedbackPreference.hapticsKey, store: KeyboardFeedbackPreference.defaults)
  private var hapticsEnabled = false
  @AppStorage(KeyboardFeedbackPreference.strengthKey, store: KeyboardFeedbackPreference.defaults)
  private var hapticStrength = KeyboardHapticStrength.medium.rawValue
  @State private var previewFeedback: UIImpactFeedbackGenerator?

  private enum Axis { case vertical, horizontal }
  /// 一次保存只写用户刚动的那一项，见 `save(_:)`。
  private enum Field { case height, keySpacing, rowSpacing }
  private static let spacingDragScale: Double = 18
  private static let saveFailedNote = "设置没有保存，键盘可能正在写入同一份设置，请再试一次。"

  var body: some View {
    VStack(spacing: 0) {
      preview
      ScrollView {
        VStack(spacing: 28) {
          layoutGroup
          feedbackGroup
          gestureGroup
          if UIDevice.current.userInterfaceIdiom == .pad { tabletGroup }
          resetGroup
          moreGroup
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, 32)
      }
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .tint(MetasequoiaTheme.accent)
    .navigationTitle("键盘").navigationBarTitleDisplayMode(.inline)
    .onAppear { readPreferences() }
    // 这一页停在后台时，用户可能在键盘里改了高度或间距；回到前台重新读一遍文档，页面上才不会留着旧值。
    .onChange(of: scenePhase) { _, phase in
      if phase == .active { readPreferences() }
    }
  }

  // MARK: - 预览

  private var nineKey: Bool { scheme == .nineKey }
  /// 键盘上有没有 iPad 的数字行：九键和全拼 14 键都不画它。
  private var drawsNumberRow: Bool { !nineKey && scheme != .fourteenKey }

  private var preview: some View {
    VStack(spacing: 6) {
      grip
      KeyboardSkinPreview(
        skin: skin, nineKey: nineKey,
        layout: KeyboardGeometry(keySpacing: keySpacing, rowSpacing: rowSpacing),
        heightAdjustment: height
      )
      .gesture(spacingDrag)
      .accessibilityIdentifier("keyboardLayoutPreview")
      Text(dragHint).font(.caption2).foregroundStyle(MetasequoiaTheme.sub)
    }
    .padding(.horizontal, 16).padding(.top, 6).padding(.bottom, 8)
  }

  private var grip: some View {
    Capsule()
      .fill(MetasequoiaTheme.accent.opacity(0.35))
      .frame(width: 44, height: 5)
      .frame(maxWidth: .infinity)
      .frame(height: 26)
      .contentShape(Rectangle())
      .gesture(heightDrag)
      .accessibilityIdentifier("keyboardHeightGrip")
      .accessibilityLabel("键盘高度")
      .accessibilityValue(heightPercent(height))
      .accessibilityAdjustableAction { direction in
        height = KeyboardGeometry.clamped(height + (direction == .increment ? 2 : -2), -12, 48)
        save(.height)
      }
  }

  private var dragHint: String {
    switch dragAxis {
    case .vertical: return "行间距 \(String(format: "%.1f", rowSpacing))"
    case .horizontal: return "按键间距 \(String(format: "%.1f", keySpacing))"
    case nil: return "拖上面的把手改高度，在键盘上左右拖改键距、上下拖改行间距"
    }
  }

  private var heightDrag: some Gesture {
    DragGesture(minimumDistance: 1)
      .onChanged { value in
        let base = dragBase ?? snapshot()
        if dragBase == nil { dragBase = base }
        height = KeyboardGeometry.clamped(base.height - Double(value.translation.height), -12, 48)
      }
      .onEnded { _ in
        dragBase = nil
        save(.height)
      }
  }

  private var spacingDrag: some Gesture {
    DragGesture(minimumDistance: 4)
      .onChanged { value in
        let base = dragBase ?? snapshot()
        if dragBase == nil { dragBase = base }
        let axis = dragAxis
          ?? (abs(value.translation.height) >= abs(value.translation.width) ? .vertical : .horizontal)
        dragAxis = axis
        switch axis {
        case .vertical:
          rowSpacing = KeyboardGeometry.clamped(base.rowSpacing + Double(value.translation.height) / Self.spacingDragScale, 4, 10)
        case .horizontal:
          keySpacing = KeyboardGeometry.clamped(base.keySpacing + Double(value.translation.width) / Self.spacingDragScale, 3, 6)
        }
      }
      .onEnded { _ in
        let axis = dragAxis
        dragBase = nil
        dragAxis = nil
        switch axis {
        case .vertical: save(.rowSpacing)
        case .horizontal: save(.keySpacing)
        case nil: break
        }
      }
  }

  private func snapshot() -> (height: Double, keySpacing: Double, rowSpacing: Double) {
    (height, keySpacing, rowSpacing)
  }

  // MARK: - 分组

  private var layoutGroup: some View {
    DesignGroup(title: "布局", footer: saveFailed ? Self.saveFailedNote : nil) {
      chineseKeyboardRow
      DesignDivider()
      // 与 Android 键盘页一样排在中文键盘之后、键盘高度之前。
      DesignSelectRow(
        title: "26 键数字键盘", subtitle: "26 键按 123 时的数字键盘",
        options: KeyboardLayoutPreference.TwentySixKeyNumberLayout.allCases.map { DesignOption(title: $0.title, value: $0) },
        selection: Binding(get: { twentySixKeyNumberLayout }, set: { saveTwentySixKeyNumberLayout($0) }),
        sheetTitle: "26 键数字键盘",
        sheetMessage: "一行把 1 到 0 排在符号上面；九宫格换成和 9 键一样的 3×3 数字键，排列跟着下面的数字键盘顺序，符号仍可长按 123 打开。iPad 全尺寸键盘有自己的数字行，始终是一行。",
        identifier: "appTwentySixKeyNumberLayoutPicker")
      DesignDivider()
      DesignSelectRow(
        title: "数字键盘顺序", subtitle: "九宫格数字键盘的排列",
        options: KeyboardLayoutPreference.NumberKeypadOrder.allCases.map { DesignOption(title: $0.title, value: $0) },
        selection: Binding(get: { numberKeypadOrder }, set: { saveNumberKeypadOrder($0) }),
        sheetTitle: "数字键盘顺序",
        sheetMessage: "电话顺序 1 2 3 在最上面；计算器顺序 7 8 9 在最上面、1 2 3 在最下面，和计算器、小键盘一样。字母键的排列不变。",
        identifier: "appNumberKeypadOrderPicker")
      DesignDivider()
      // 只有双拼方案的 26 键画键位提示；本版本没有双拼时不列这一行。
      if ChineseInputScheme.shuangpin.isOfferedByEdition {
        DesignToggleRow(
          title: "双拼键位提示", subtitle: "双拼方案的 26 键键盘在字母键底部显示这个键代表的声母和韵母；关闭后键面只留字母",
          isOn: Binding(get: { shuangpinKeyHints }, set: { saveShuangpinKeyHints($0) }))
        .accessibilityIdentifier("appShuangpinKeyHintsToggle")
        DesignDivider()
      }
      sliderRow("键盘高度", value: $height, range: -12...48, identifier: "appKeyboardHeightSlider",
                field: .height, format: heightPercent)
      DesignDivider()
      sliderRow("按键间距", value: $keySpacing, range: 3...6, identifier: "appKeySpacingSlider", field: .keySpacing)
      DesignDivider()
      sliderRow("行间距", value: $rowSpacing, range: 4...10, identifier: "appRowSpacingSlider", field: .rowSpacing)
    }
  }

  /// 在当前方案所属的一组触屏方案之间切换：全拼是 26 键、14 键、9 键三选一，日语是 26 键和 9 键。只有一种排列的方案（双拼、五笔、手写等）没有这一组；在那里提供全拼会悄悄把小鹤用户切到全拼，所以这一行只显示方案名。
  @ViewBuilder
  private var chineseKeyboardRow: some View {
    if let layouts = Self.layoutChoices(scheme), layouts.count > 1 {
      DesignSelectRow(
        title: "中文键盘",
        options: layouts.map { DesignOption(title: $0.title, value: $0.scheme, identifier: $0.identifier) },
        selection: Binding(get: { scheme }, set: { _ in }),
        sheetTitle: "中文键盘", identifier: "chineseKeyboardPicker",
        onSelect: selectScheme)
    } else {
      HStack(spacing: 12) {
        VStack(alignment: .leading, spacing: 2) {
          Text("中文键盘").font(.system(size: 17)).foregroundStyle(.primary)
          Text(Self.layoutChoices(scheme) == nil ? "当前方案只有一种键盘，在「输入」里换方案" : "本版本只有一种键盘")
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
        }
        Spacer(minLength: 8)
        Text(scheme.title).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
      }
      .padding(.vertical, 8)
      .padding(.horizontal, 20)
      .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
      .accessibilityElement(children: .combine)
      .accessibilityIdentifier("chineseKeyboardPicker")
    }
  }

  private var feedbackGroup: some View {
    DesignGroup(title: "按键反馈", footer: KeyboardFeedbackPreference.hapticsAvailable
      ? "按键音受系统静音设置控制；振动效果取决于设备与系统支持。"
      : "按键音受系统静音设置控制。") {
      DesignToggleRow(title: "按键音", isOn: $soundEnabled)
        .accessibilityIdentifier("keyboardSoundToggle")
      if KeyboardFeedbackPreference.hapticsAvailable {
        DesignDivider()
        DesignToggleRow(title: "按键振动", isOn: $hapticsEnabled)
          .accessibilityIdentifier("keyboardHapticsToggle")
          .onChange(of: hapticsEnabled) { _, enabled in if enabled { previewHaptics() } }
        if hapticsEnabled {
          DesignDivider()
          DesignSelectRow(
            title: "振动强度",
            options: KeyboardHapticStrength.allCases.map { DesignOption(title: $0.title, value: $0.rawValue) },
            selection: $hapticStrength, sheetTitle: "振动强度", identifier: "keyboardHapticStrengthPicker",
            onSelect: { _ in previewHaptics() })
          DesignDivider()
          DesignInlineButtonRow(title: "振动预览", buttonTitle: "试一下振动", identifier: "previewKeyboardHaptics",
                                action: previewHaptics)
        }
      }
    }
  }

  private var gestureGroup: some View {
    DesignGroup(title: "手势", footer: "滑行输入只在全拼 26 键的字母键盘上生效，轻点照常输入单个字母。") {
      DesignToggleRow(title: "滑动输入符号", subtitle: "在字母键上下滑，输入角标符号；长按字母键始终可以输入",
                      isOn: Binding(get: { swipeSymbols }, set: { enabled in
                        swipeSymbols = enabled
                        KeyboardLayoutPreference.swipeSymbols = enabled
                      }))
      .accessibilityIdentifier("appSwipeSymbolsSwitch")
      DesignDivider()
      DesignToggleRow(title: "滑行输入", subtitle: "在字母键上连续滑动输入拼音，停留可确认经过的键",
                      isOn: Binding(get: { glideTyping }, set: { enabled in
                        glideTyping = enabled
                        KeyboardLayoutPreference.glideTyping = enabled
                      }))
      .accessibilityIdentifier("appGlideTypingSwitch")
      DesignDivider()
      DesignToggleRow(title: "长按空格语音输入",
                      isOn: Binding(get: { spaceVoice }, set: { enabled in
                        spaceVoice = enabled
                        KeyboardLayoutPreference.spaceVoice = enabled
                      }))
      .accessibilityIdentifier("appSpaceVoiceSwitch")
    }
  }

  private var tabletGroup: some View {
    DesignGroup(title: "iPad", footer: "全尺寸键盘在字母上方多一排数字、Q 左边多一个 Tab 键。组字时数字键选候选，Tab 打开全部候选（桌面端的 Tab 翻页）；没有组字时照常输入。浮动键盘和分屏的窄窗口用 iPhone 布局，不显示这两样。\n\n横屏分离式键盘只在横屏时生效：字母、数字行和 123 符号页从中间分成左右两半，中间留空，方便双手握持时用拇指打字；九键、14 键、笔画、手写和注音不分。竖屏、浮动键盘和窄窗口照常显示整块键盘。\n\n外接实体键盘（妙控键盘、蓝牙键盘）时，iOS 不会把实体按键交给任何第三方键盘，实体键盘打出的是系统输入法的结果。要用水杉的拼音、候选和皮肤，请在屏幕键盘上输入。") {
      DesignToggleRow(title: "数字行与 Tab 键", isOn: Binding(get: { tabletFullKeys }, set: { enabled in
        tabletFullKeys = enabled
        KeyboardLayoutPreference.tabletFullKeys = enabled
      }))
      .accessibilityIdentifier("appTabletFullKeysSwitch")
      if tabletFullKeys {
        DesignDivider()
        DesignToggleRow(title: "组字时 Tab 打开全部候选", isOn: Binding(get: { tabOpensCandidates }, set: { saveTab($0) }))
          .accessibilityIdentifier("appTabletTabCandidatesSwitch")
      }
      DesignDivider()
      DesignToggleRow(title: "横屏分离式键盘", isOn: Binding(get: { tabletSplit }, set: { enabled in
        tabletSplit = enabled
        KeyboardLayoutPreference.tabletSplit = enabled
      }))
      .accessibilityIdentifier("appTabletSplitSwitch")
    }
  }

  private var resetGroup: some View {
    DesignGroup(footer: UIDevice.current.userInterfaceIdiom == .pad
      ? "把这一页的间距、高度和横屏分离式键盘恢复成默认值。" : "把这一页的间距和高度恢复成默认值。") {
      Button {
        saveFailed = !KeyboardLayoutPreference.resetGeometry()
        readPreferences()
      } label: {
        Text("恢复默认").font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.danger)
          .padding(.vertical, 8)
          .padding(.horizontal, 20)
          .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
          .contentShape(Rectangle())
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("appResetKeyboardSettings")
    }
  }

  private var moreGroup: some View {
    DesignGroup(title: "更多") {
      NavigationLink(destination: CandidateOptionsSettingsView()) {
        DesignNavRowLabel(title: "候选栏", subtitle: "字号、字体、预编辑和候选内容")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("candidateOptionsSettingsLink")
      DesignDivider()
      NavigationLink(destination: KeyboardToolbarSettingsView()) {
        DesignNavRowLabel(title: "键盘工具栏", subtitle: "键盘顶部显示哪些按钮")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("keyboardToolbarSettingsLink")
      DesignDivider()
      NavigationLink(destination: ServiceSettingsView(kind: .ai)) {
        DesignNavRowLabel(title: "AI 润色与回复", subtitle: "端点、模型、凭据和提示词")
      }
      .buttonStyle(PressFillButtonStyle())
      .accessibilityIdentifier("aiSettingsLink")
    }
  }

  // MARK: - 行

  /// 设计稿的行内滑块行（110pt 轨道，regular 宽度下 160pt，外加 46pt 数值列）。松开滑块后才写入：滑块每帧都会回报，而每次写入都要拿共享文档的锁。
  private func sliderRow(
    _ title: String,
    value: Binding<Double>,
    range: ClosedRange<Double>,
    identifier: String,
    field: Field,
    format: @escaping (Double) -> String = { String(format: "%.1f", $0) }
  ) -> some View {
    InlineSliderRow(title: title, value: value, range: range, identifier: identifier, format: format) { save(field) }
  }

  // MARK: - 存储

  /// 当前方案所属的一组键盘，按 26 键、14 键、9 键排列，只留本版本提供的：全拼有 26 键、14 键和 9 键，日语有 26 键和 9 键。其余方案都只有一种排列，为 nil。
  static func layoutChoices(_ scheme: ChineseInputScheme) -> [(title: String, scheme: ChineseInputScheme, identifier: String)]? {
    let choices: [(title: String, scheme: ChineseInputScheme, identifier: String)]
    switch scheme {
    case .quanpin, .fourteenKey, .nineKey:
      choices = [("26 键", .quanpin, "chineseKeyboard26Key"), ("14 键", .fourteenKey, "chineseKeyboard14Key"),
                 ("9 键", .nineKey, "chineseKeyboard9Key")]
    case .japanese, .japaneseNineKey:
      choices = [("26 键", .japanese, "chineseKeyboard26Key"), ("9 键", .japaneseNineKey, "chineseKeyboard9Key")]
    default:
      return nil
    }
    return choices.filter { $0.scheme.isOfferedByEdition }
  }

  /// 把存储的点数调整量换算成键盘自己的键盘高度条在竖屏下显示的百分比：同一个 `KeyboardHeightPercent` 公式，作用于本设备在当前行距下的按键区；数字行与 Tab 键打开且布局带数字行时，按带数字行的全宽 iPad 键盘计算。
  private func heightPercent(_ adjustment: Double) -> String {
    let tablet = UIDevice.current.userInterfaceIdiom == .pad
    let keyBlock = KeyboardHeightPercent.portraitKeyBlockHeight(
      tablet: tablet, numberRow: tablet && tabletFullKeys && drawsNumberRow, rowSpacing: CGFloat(rowSpacing))
    return "\(KeyboardHeightPercent.percent(adjustment: CGFloat(adjustment), keyBlock: keyBlock))%"
  }

  /// 选中正在使用的排列不写入任何东西；写入被拒绝时这一行停留在已存储的方案上。
  private func selectScheme(_ next: ChineseInputScheme) {
    guard next != scheme else { return }
    saveFailed = !InputSchemePreference.select(next)
    readPreferences()
  }

  private func previewHaptics() {
    guard hapticsEnabled else { return }
    let strength = KeyboardHapticStrength(rawValue: hapticStrength) ?? .medium
    let generator = UIImpactFeedbackGenerator(style: strength.style)
    previewFeedback = generator
    strength.impact(generator)
    generator.prepare()
  }

  /// The drags and sliders only move the preview while they run; the settled value is saved here, into the shared document the keyboard reloads. A save the document refuses puts the page back to what is stored.
  ///
  /// 只写动过的那一项：页面上其余的值可能已经过时（键盘里刚调过高度，这一页还停在后台），一起写回去就会把键盘里的调整盖掉。
  private func save(_ field: Field) {
    let saved = switch field {
    case .height: KeyboardLayoutPreference.saveGeometry(heightAdjustment: height)
    case .keySpacing: KeyboardLayoutPreference.saveGeometry(keySpacing: keySpacing)
    case .rowSpacing: KeyboardLayoutPreference.saveGeometry(rowSpacing: rowSpacing)
    }
    saveFailed = !saved
    if saveFailed { readPreferences() }
  }

  private func saveNumberKeypadOrder(_ order: KeyboardLayoutPreference.NumberKeypadOrder) {
    saveFailed = !KeyboardLayoutPreference.saveNumberKeypadOrder(order)
    numberKeypadOrder = saveFailed
      ? KeyboardLayoutPreference.NumberKeypadOrder.shared(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
      : order
  }

  private func saveTwentySixKeyNumberLayout(_ layout: KeyboardLayoutPreference.TwentySixKeyNumberLayout) {
    saveFailed = !KeyboardLayoutPreference.saveTwentySixKeyNumberLayout(layout)
    twentySixKeyNumberLayout = saveFailed
      ? KeyboardLayoutPreference.TwentySixKeyNumberLayout.shared(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
      : layout
  }

  private func saveShuangpinKeyHints(_ enabled: Bool) {
    saveFailed = !KeyboardLayoutPreference.saveShuangpinKeyHints(enabled)
    shuangpinKeyHints = saveFailed
      ? KeyboardLayoutPreference.sharedShuangpinKeyHints(in: MetasequoiaInputSessionBridge.loadSharedPreferences())
      : enabled
  }

  private func saveTab(_ enabled: Bool) {
    saveFailed = !KeyboardLayoutPreference.saveTabShowsMoreCandidates(enabled)
    tabOpensCandidates = saveFailed
      ? KeyboardLayoutPreference.tabShowsMoreCandidates(MetasequoiaInputSessionBridge.loadSharedPreferences()) : enabled
  }

  private func readPreferences() {
    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    // 文档里的选择（取不到时退回 App Group 镜像）就是键盘下次打开时用的方案。
    scheme = InputSchemePreference.current(in: preferences).scheme
    keySpacing = KeyboardLayoutPreference.keySpacing
    rowSpacing = KeyboardLayoutPreference.rowSpacing
    height = KeyboardLayoutPreference.heightAdjustment
    skin = KeyboardTheme.reload(preferences)
    tabletFullKeys = KeyboardLayoutPreference.tabletFullKeys
    tabletSplit = KeyboardLayoutPreference.tabletSplit
    glideTyping = KeyboardLayoutPreference.glideTyping
    swipeSymbols = KeyboardLayoutPreference.swipeSymbols
    spaceVoice = KeyboardLayoutPreference.spaceVoice
    // The document is what the keyboard will use, including a value synced from another device that no keyboard has mirrored into the App Group yet.
    guard let preferences else { return }
    tabOpensCandidates = KeyboardLayoutPreference.tabShowsMoreCandidates(preferences)
    // 文档是权威：先抄进 App Group，再从 App Group 读回页面，两边才是同一个值。
    KeyboardLayoutPreference.mirrorGeometry(preferences)
    keySpacing = KeyboardLayoutPreference.keySpacing
    rowSpacing = KeyboardLayoutPreference.rowSpacing
    height = KeyboardLayoutPreference.heightAdjustment
    numberKeypadOrder = KeyboardLayoutPreference.NumberKeypadOrder.shared(in: preferences)
    KeyboardLayoutPreference.numberKeypadOrder = numberKeypadOrder
    twentySixKeyNumberLayout = KeyboardLayoutPreference.TwentySixKeyNumberLayout.shared(in: preferences)
    KeyboardLayoutPreference.twentySixKeyNumberLayout = twentySixKeyNumberLayout
    shuangpinKeyHints = KeyboardLayoutPreference.sharedShuangpinKeyHints(in: preferences)
    KeyboardLayoutPreference.shuangpinKeyHints = shuangpinKeyHints
  }
}

/// 键盘页的一行行内滑块：标题、短轨道和格式化后的数值，布局同 DesignSliderRow，但会回报拖动结束，让页面每次手势只保存一次。
private struct InlineSliderRow: View {
  let title: String
  @Binding var value: Double
  let range: ClosedRange<Double>
  let identifier: String
  let format: (Double) -> String
  let commit: () -> Void
  @Environment(\.horizontalSizeClass) private var widthClass

  init(title: String, value: Binding<Double>, range: ClosedRange<Double>, identifier: String,
       format: @escaping (Double) -> String, commit: @escaping () -> Void) {
    self.title = title
    _value = value
    self.range = range
    self.identifier = identifier
    self.format = format
    self.commit = commit
  }

  var body: some View {
    HStack(spacing: 12) {
      Text(title).font(.system(size: 17)).foregroundStyle(.primary)
      Spacer(minLength: 8)
      HStack(spacing: 8) {
        Slider(value: $value, in: range) { Text(title) } onEditingChanged: { editing in
          if !editing { commit() }
        }
        .tint(MetasequoiaTheme.accent)
        .frame(width: widthClass == .regular ? 160 : 110)
        .accessibilityValue(format(value))
        .accessibilityIdentifier(identifier)
        Text(format(value)).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
          .monospacedDigit().lineLimit(1).minimumScaleFactor(0.8)
          .frame(width: 46, alignment: .trailing)
          .accessibilityHidden(true)
      }
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
  }
}
