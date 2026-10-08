import SwiftUI

struct KeyboardLayoutSettingsView: View {
  @State private var keySpacing = KeyboardLayoutPreference.keySpacing
  @State private var rowSpacing = KeyboardLayoutPreference.rowSpacing
  @State private var height = KeyboardLayoutPreference.heightAdjustment
  @State private var skin = KeyboardTheme.current
  @State private var nineKey = InputSchemePreference.scheme == .nineKey
  @State private var voice = KeyboardLayoutPreference.voiceShortcutEnabled
  @State private var tabletFullKeys = KeyboardLayoutPreference.tabletFullKeys
  @State private var tabletSplit = KeyboardLayoutPreference.tabletSplit
  @State private var glideTyping = KeyboardLayoutPreference.glideTyping
  @State private var tabOpensCandidates = true
  @State private var numberKeypadOrder = KeyboardLayoutPreference.numberKeypadOrder
  @State private var dragBase: (height: Double, keySpacing: Double, rowSpacing: Double)?
  @State private var dragAxis: Axis?
  @State private var saveFailed = false
  @Environment(\.scenePhase) private var scenePhase

  private enum Axis { case vertical, horizontal }
  /// 一次保存只写用户刚动的那一项，见 `save(_:)`。
  private enum Field { case height, keySpacing, rowSpacing, voice }
  private static let spacingDragScale: Double = 18

  var body: some View {
    VStack(spacing: 0) {
      preview
      form
    }
    .tint(MetasequoiaTheme.accent)
    .navigationTitle("键盘").navigationBarTitleDisplayMode(.inline)
    .onAppear { readPreferences() }
    // 这一页停在后台时，用户可能在键盘里改了高度或间距；回到前台重新读一遍文档，页面上才不会留着旧值。
    .onChange(of: scenePhase) { phase in
      if phase == .active { readPreferences() }
    }
  }

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
      Text(dragHint).font(.caption2).foregroundStyle(.secondary)
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
      .accessibilityValue(KeyboardGeometry.formattedHeightAdjustment(height))
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

  private var form: some View {
    Form {
      Section {
        spacingRow("键盘高度", value: $height, range: -12...48, identifier: "appKeyboardHeightSlider",
                   field: .height, format: KeyboardGeometry.formattedHeightAdjustment)
      } header: {
        Text("键盘高度")
      } footer: {
        Text(saveFailed ? "设置没有保存，键盘可能正在写入同一份设置，请再试一次。" : "在系统键盘高度的基础上增减，按键会跟着变高。上面的预览实时跟着走；已经打开的键盘要重新唤出才生效。")
      }
      Section {
        spacingRow("按键间距", value: $keySpacing, range: 3...6, identifier: "appKeySpacingSlider", field: .keySpacing)
        spacingRow("行间距", value: $rowSpacing, range: 4...10, identifier: "appRowSpacingSlider", field: .rowSpacing)
      } header: {
        Text("按键间距")
      } footer: {
        Text("间距只改变键位外观，不影响输入方案。键盘布局在键盘的布局按钮里切换。")
      }
      Section {
        Toggle("顶部语音入口", isOn: $voice)
          .accessibilityIdentifier("appVoiceShortcutSwitch")
          .onChange(of: voice) { _ in save(.voice) }
      } header: {
        Text("快捷入口")
      } footer: {
        Text("语音入口用于打开已识别的语音结果。识别服务在「设置 → 语音输入」里配置，工具栏上的其他按钮在「设置 → 键盘工具栏」里。")
      }
      Section {
        Picker("数字键盘顺序", selection: Binding(
          get: { numberKeypadOrder },
          set: { saveNumberKeypadOrder($0) })) {
          ForEach(KeyboardLayoutPreference.NumberKeypadOrder.allCases, id: \.self) { order in
            Text(order.title).tag(order)
          }
        }
        .accessibilityIdentifier("appNumberKeypadOrderPicker")
      } header: {
        Text("数字键盘")
      } footer: {
        Text("九键切到数字时的排列。电话顺序 1 2 3 在最上面；计算器顺序 7 8 9 在最上面、1 2 3 在最下面，和计算器、小键盘一样。字母键的排列不变。")
      }
      Section {
        Toggle(isOn: $glideTyping) {
          VStack(alignment: .leading, spacing: 2) {
            Text("滑行输入")
            Text("在字母键上连续滑动输入拼音，停留可确认经过的键").font(.footnote).foregroundStyle(.secondary)
          }
        }
        .accessibilityIdentifier("appGlideTypingSwitch")
        .onChange(of: glideTyping) { KeyboardLayoutPreference.glideTyping = $0 }
      } header: {
        Text("手势")
      } footer: {
        Text("只在全拼 26 键的字母键盘上生效：手指从一个字母键滑到其他字母键即开始滑行，抬起后输入经过的拼音；轻点照常输入单个字母。双拼、九键、五笔、英文和本地输入模式下不滑行。")
      }
      if UIDevice.current.userInterfaceIdiom == .pad {
        Section {
          Toggle("数字行与 Tab 键", isOn: $tabletFullKeys)
            .accessibilityIdentifier("appTabletFullKeysSwitch")
            .onChange(of: tabletFullKeys) { KeyboardLayoutPreference.tabletFullKeys = $0 }
          if tabletFullKeys {
            Toggle("组字时 Tab 打开全部候选", isOn: Binding(
              get: { tabOpensCandidates },
              set: { saveTab($0) }))
            .accessibilityIdentifier("appTabletTabCandidatesSwitch")
          }
          Toggle("横屏分离式键盘", isOn: $tabletSplit)
            .accessibilityIdentifier("appTabletSplitSwitch")
            .onChange(of: tabletSplit) { KeyboardLayoutPreference.tabletSplit = $0 }
        } header: {
          Text("iPad")
        } footer: {
          Text("全尺寸键盘在字母上方多一排数字、Q 左边多一个 Tab 键。组字时数字键选候选，Tab 打开全部候选（桌面端的 Tab 翻页）；没有组字时照常输入。浮动键盘和分屏的窄窗口用 iPhone 布局，不显示这两样。\n\n横屏分离式键盘只在横屏时生效：字母、数字行和 123 符号页从中间分成左右两半，中间留空，方便双手握持时用拇指打字；九键、笔画、手写和注音不分。竖屏、浮动键盘和窄窗口照常显示整块键盘。\n\n外接实体键盘（妙控键盘、蓝牙键盘）时，iOS 不会把实体按键交给任何第三方键盘，实体键盘打出的是系统输入法的结果。要用水杉的拼音、候选和皮肤，请在屏幕键盘上输入。")
        }
      }
      Section {
        Button("恢复默认", role: .destructive) {
          saveFailed = !KeyboardLayoutPreference.resetGeometry()
          readPreferences()
        }
        .accessibilityIdentifier("appResetKeyboardSettings")
      } footer: {
        Text(UIDevice.current.userInterfaceIdiom == .pad
          ? "把这一页的间距、高度和横屏分离式键盘恢复成默认值。" : "把这一页的间距和高度恢复成默认值。")
      }
    }
  }

  /// The drags and sliders only move the preview while they run; the settled value is saved here, into the shared document the keyboard reloads. A save the document refuses puts the page back to what is stored.
  ///
  /// 只写动过的那一项：页面上其余的值可能已经过时（键盘里刚调过高度，这一页还停在后台），一起写回去就会把键盘里的调整盖掉。
  private func save(_ field: Field) {
    let saved = switch field {
    case .height: KeyboardLayoutPreference.saveGeometry(heightAdjustment: height)
    case .keySpacing: KeyboardLayoutPreference.saveGeometry(keySpacing: keySpacing)
    case .rowSpacing: KeyboardLayoutPreference.saveGeometry(rowSpacing: rowSpacing)
    case .voice: KeyboardLayoutPreference.saveGeometry(voiceShortcut: voice)
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

  private func saveTab(_ enabled: Bool) {
    saveFailed = !KeyboardLayoutPreference.saveTabShowsMoreCandidates(enabled)
    tabOpensCandidates = saveFailed
      ? KeyboardLayoutPreference.tabShowsMoreCandidates(MetasequoiaInputSessionBridge.loadSharedPreferences()) : enabled
  }

  private func readPreferences() {
    keySpacing = KeyboardLayoutPreference.keySpacing
    rowSpacing = KeyboardLayoutPreference.rowSpacing
    height = KeyboardLayoutPreference.heightAdjustment
    skin = KeyboardTheme.reload(MetasequoiaInputSessionBridge.loadSharedPreferences())
    nineKey = InputSchemePreference.scheme == .nineKey
    voice = KeyboardLayoutPreference.voiceShortcutEnabled
    tabletFullKeys = KeyboardLayoutPreference.tabletFullKeys
    tabletSplit = KeyboardLayoutPreference.tabletSplit
    glideTyping = KeyboardLayoutPreference.glideTyping
    // The document is what the keyboard will use, including a value synced from another device that no keyboard has mirrored into the App Group yet.
    guard let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    tabOpensCandidates = KeyboardLayoutPreference.tabShowsMoreCandidates(preferences)
    // 文档是权威：先抄进 App Group，再从 App Group 读回页面，两边才是同一个值。
    KeyboardLayoutPreference.mirrorGeometry(preferences)
    keySpacing = KeyboardLayoutPreference.keySpacing
    rowSpacing = KeyboardLayoutPreference.rowSpacing
    height = KeyboardLayoutPreference.heightAdjustment
    voice = KeyboardLayoutPreference.voiceShortcutEnabled
    numberKeypadOrder = KeyboardLayoutPreference.NumberKeypadOrder.shared(in: preferences)
    KeyboardLayoutPreference.numberKeypadOrder = numberKeypadOrder
  }

  private func spacingRow(
    _ title: String,
    value: Binding<Double>,
    range: ClosedRange<Double>,
    identifier: String,
    field: Field,
    format: @escaping (Double) -> String = { String(format: "%.1f", $0) }
  ) -> some View {
    VStack(alignment: .leading, spacing: 4) {
      HStack {
        Text(title)
        Spacer()
        Text(KeyboardGeometry.formattedHeightAdjustment(value.wrappedValue)).font(.callout).monospacedDigit()
          .foregroundStyle(.secondary)
      }
      // Written once the thumb is let go: the slider reports every frame, and each write takes the shared document's lock.
      Slider(value: value, in: range) { editing in
        if !editing { save(field) }
      }.accessibilityIdentifier(identifier).accessibilityLabel(title)
    }
  }
}
