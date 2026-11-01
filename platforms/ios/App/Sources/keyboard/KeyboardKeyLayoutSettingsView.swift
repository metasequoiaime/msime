import SwiftUI

/// 「键盘 › 按键布局」：26 键、14 键与 18 键、9 键三类键面各选一种底行样式，26 键另有「26 键「符」与「123」互换」开关，每一类都有「中/英键放在空格左边」开关（三类是同一个偏好），9 键另有「九键左侧符号」一行，接着是「数字键盘」一组（空格在右列、0 右边或 0 左边，没选过时选中 iOS 原来的「空格在 0 右边」，再加「数字键盘左侧符号」一行），外加顶部数字行开关，对应共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_twenty_six_symbol_first`、`touch_language_key_side`、`touch_nine_key_sidebar_symbols`、`touch_number_pad_space`、`touch_nine_key_digit_sidebar_symbols` 与 `touch_number_row`（`TouchKeyLayoutStyle`）。三端同一套选项和画法，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md、2026-10-29-touch-symbol-first-and-number-pad-space.md、2026-10-29-key-layout-page-redesign.md、2026-10-29-nine-key-sidebar-shared.md 与 2026-10-29-language-key-left-of-space.md。
///
/// 自上而下：键盘类型分段控件（`TouchKeyLayoutKind`，只有一类时不画）、这一类键盘的整块预览、样式胶囊按钮、当前样式的说明（没有副作用的样式不画这一行），说明之后是开关：26 键分段先是「符」与「123」互换，每个分段都有「中/英键放在空格左边」，9 键分段再接「九键左侧符号」导航行（右侧是当前表的摘要）；样式卡片下面是「数字键盘」一组，选项是同一套等宽胶囊、说明在胶囊下方、再下面是「数字键盘左侧符号」导航行，不画预览，本版本提供任一类键面时都列出；最下面单独一组是顶部数字行开关。两张符号表的编辑页是 `NineKeySidebarEditorView`。分段默认停在用户当前在用的中文键盘那一类；切分段只换预览和样式按钮，不写偏好。本版本不提供的键面类型不列进分段，既没有中文 26 键也没有 14/18 键时不列顶部数字行开关（`TouchKeyLayoutOffering`）。改动只写刚选的那一项，键盘下次出现或重载文档时当场换上，不打断正在组的字。
struct KeyboardKeyLayoutSettingsView: View {
  @State private var style = TouchKeyLayoutStyle.mirrored
  @State private var saveFailed = false
  /// 分段控件停在哪一类；本版本一类都不列时为 nil。
  @State private var kind: TouchKeyLayoutKind?
  /// 用户点过分段后，回到前台重读文档时不再把分段拨回当前键盘那一类。
  @State private var kindPicked = false
  /// 14/18 键的预览画哪种字母格：用户当前在用 18 键时画 18 键，否则画 14 键。
  @State private var letterGrid: LetterKeyGrid
  /// 字母格第三排左端是「分词」还是「符」：五笔 14 键没有音节可分，键盘上这个键是「符」。
  @State private var gridSplitsSyllables: Bool
  /// 用户隐藏了中/英键时预览也不画它，份额并进空格，与键盘一致。
  @State private var hidesLanguageKey = KeyboardLayoutPreference.hideLanguageKey
  /// 本版本提供哪几类键面，决定分段列哪几类和要不要数字行开关。
  private let offering = TouchKeyLayoutOffering.edition

  init() {
    // 先按 App Group 镜像定下分段，页面第一帧就停在对的那一类；出现时再按文档校正（`readPreferences`）。
    let current = Self.currentKeyboard(in: nil)
    _kind = State(initialValue: TouchKeyLayoutOffering.edition.initialKind(current: current.kind))
    _letterGrid = State(initialValue: current.grid)
    _gridSplitsSyllables = State(initialValue: current.splitsSyllables)
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        if let kind {
          VStack(spacing: 16) {
            if offering.kinds.count > 1 { kindPicker(kind) }
            styleCard(kind)
          }
        }
        if offering.offersAnyGroup { numberPadGroup }
        if offering.numberRow {
          DesignGroup(footer: saveFailed ? SettingsSaveFailure.message : nil) {
            DesignToggleRow(title: "顶部数字行", info: TouchKeyLayoutOffering.numberRowSubtitle,
                            isOn: Binding(get: { style.numberRow }, set: { save(TouchKeyLayoutStyle.save(numberRow: $0)) }))
              .accessibilityIdentifier("keyLayoutNumberRowToggle")
          }
        } else if saveFailed {
          // 没有数字行开关时，保存失败的提示单独放在样式卡片下面。
          Text(SettingsSaveFailure.message).designSubtext()
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
      }
      .padding(.horizontal, 16)
      .padding(.top, 8)
      .padding(.bottom, 32)
    }
    .designCanvasBackground()
    .tint(MetasequoiaTheme.accent)
    .navigationTitle("按键布局").navigationBarTitleDisplayMode(.inline)
    .reloadOnAppearAndActive(readPreferences)
  }

  private func kindPicker(_ selected: TouchKeyLayoutKind) -> some View {
    DesignSegmentedControl(
      items: offering.kinds.map { ($0.title, $0) },
      selection: Binding(get: { selected }, set: { next in
        kind = next
        kindPicked = true
      }),
      style: .rounded, identifierPrefix: "keyLayoutKind", itemIdentifier: { $0.identifier })
  }

  /// 预览、样式胶囊和当前样式的说明放在同一张卡片里；说明之后各隔一道分隔线是开关：26 键先是「符」与「123」互换，每个分段都有「中/英键放在空格左边」。用户隐藏了中/英键时这个开关照样显示、照样写偏好（同步到别的设备照样生效），只是这台设备的键盘和预览里没有中/英，看不出变化。
  private func styleCard(_ kind: TouchKeyLayoutKind) -> some View {
    DesignCard {
      VStack(alignment: .leading, spacing: 14) {
        KeyLayoutKeyboardPreview(kind: kind, style: style, letterGrid: letterGrid,
                                 gridSplitsSyllables: gridSplitsSyllables, language: !hidesLanguageKey)
          .padding(.horizontal, 16)
        // 当前样式的说明收进胶囊行末的「?」，没有说明的样式不画。
        HStack(spacing: 0) {
          stylePills(kind)
          if let note = kind.note(style) {
            DesignInfoTip(text: note)
              .padding(.trailing, 12)
              .accessibilityIdentifier("keyLayoutStyleNote")
          }
        }
      }
      .padding(.vertical, 16)
      .frame(maxWidth: .infinity, alignment: .leading)
      if kind == .twentySixKey {
        DesignDivider()
        DesignToggleRow(title: TouchKeyLayoutOffering.symbolFirstTitle, info: TouchKeyLayoutOffering.symbolFirstSubtitle,
                        isOn: Binding(get: { style.symbolFirst }, set: { save(TouchKeyLayoutStyle.save(symbolFirst: $0)) }))
          .accessibilityIdentifier("keyLayoutSymbolFirstToggle")
      }
      DesignDivider()
      DesignToggleRow(title: TouchKeyLayoutOffering.languageLeftTitle, info: TouchKeyLayoutOffering.languageLeftSubtitle,
                      isOn: Binding(get: { style.languageKeySide == .left },
                                    set: { save(TouchKeyLayoutStyle.save(languageKeySide: $0 ? .left : .right)) }))
        .accessibilityIdentifier("keyLayoutLanguageLeftSwitch")
      if kind == .nineKey {
        DesignDivider()
        sidebarLink(.letter)
      }
    }
  }

  /// 打开一张符号栏表的编辑页的导航行：标题取自三端共用的常量，右侧是这张表的摘要，空表时是 iOS 出厂表的摘要（`NineKeySidebar.summary`）。
  private func sidebarLink(_ table: NineKeySidebarTable) -> some View {
    NavigationLink(destination: NineKeySidebarEditorView(table: table)) {
      DesignNavRowLabel(title: table.title, value: NineKeySidebar.summary(style.sidebarSymbols(table)))
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityIdentifier("keyLayoutSidebar_\(table.rawValue)")
  }

  /// 「数字键盘」一组：九宫格数字键盘从所有键面都进得去（九键自己的数字层，或其他中文键面、日文、韩文、越南文、藏文、英文和本地模式借来的数字九宫格），所以不跟着分段走。选项用和样式一样的等宽胶囊，说明在胶囊下方，隔一道分隔线是「数字键盘左侧符号」导航行，不画预览。
  private var numberPadGroup: some View {
    DesignGroup(title: TouchKeyLayoutOffering.numberPadTitle, info: TouchKeyLayoutOffering.numberPadFooter) {
      VStack(alignment: .leading, spacing: 14) {
        pills(TouchNumberPadSpace.allCases, selected: style.numberPadSpace, title: \.title,
              identifier: { "keyLayoutNumberPad_\($0.rawValue)" }) { save(TouchKeyLayoutStyle.save(numberPadSpace: $0)) }
      }
      .padding(.vertical, 16)
      .frame(maxWidth: .infinity, alignment: .leading)
      DesignDivider()
      sidebarLink(.digit)
    }
  }

  @ViewBuilder
  private func stylePills(_ kind: TouchKeyLayoutKind) -> some View {
    switch kind {
    case .twentySixKey:
      pills(TouchTwentySixKeyStyle.allCases, selected: style.twentySixKey, title: \.title,
            identifier: { "keyLayout26_\($0.rawValue)" }) { save(TouchKeyLayoutStyle.save(twentySixKey: $0)) }
    case .letterGrid:
      pills(TouchFourteenKeyStyle.allCases, selected: style.fourteenKey, title: \.title,
            identifier: { "keyLayout14_\($0.rawValue)" }) { save(TouchKeyLayoutStyle.save(fourteenKey: $0)) }
    case .nineKey:
      pills(TouchNineKeyStyle.allCases, selected: style.nineKey, title: \.title,
            identifier: { "keyLayout9_\($0.rawValue)" }) { save(TouchKeyLayoutStyle.save(nineKey: $0)) }
    }
  }

  /// 一排等宽的样式胶囊铺满整行，不横向滚动：每类最多四种、名字最长五个字，等分后放得下（数字键盘的三个选项名字长一些，三等分也放得下）；系统字号很大时字跟着缩小。选中的是强调色实底，其余是浅底；点一下立刻写偏好，点已选中的不做事。
  private func pills<Value: Hashable>(
    _ options: [Value], selected: Value, title: @escaping (Value) -> String,
    identifier: @escaping (Value) -> String, onSelect: @escaping (Value) -> Void
  ) -> some View {
    HStack(spacing: 6) {
      ForEach(options, id: \.self) { option in
        let isSelected = option == selected
        Button {
          guard !isSelected else { return }
          onSelect(option)
        } label: {
          Text(title(option))
            .font(.system(size: 14, weight: isSelected ? .semibold : .regular))
            .foregroundStyle(isSelected ? MetasequoiaTheme.onAccent : Color.primary)
            .lineLimit(1)
            .minimumScaleFactor(0.7)
            .padding(.horizontal, 6)
            .frame(maxWidth: .infinity, minHeight: 36)
            .background(isSelected ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg, in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(identifier(option))
        .accessibilityValue(isSelected ? "已选择" : "未选择")
        .accessibilityAddTraits(isSelected ? .isSelected : [])
      }
    }
    .padding(.horizontal, 16)
  }

  /// 写进文档成功时页面换成新值；写不进去时提示并回到文档里存着的值。
  private func save(_ saved: Bool) {
    saveFailed = !saved
    readPreferences()
  }

  private func readPreferences() {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    // 文档是权威：先抄进 App Group，再从镜像读回页面，两边才是同一个值。
    TouchKeyLayoutStyle.mirror(document)
    style = TouchKeyLayoutStyle.mirrored
    hidesLanguageKey = KeyboardLayoutPreference.hideLanguageKey
    let current = Self.currentKeyboard(in: document)
    letterGrid = current.grid
    gridSplitsSyllables = current.splitsSyllables
    if !kindPicked { kind = offering.initialKind(current: current.kind) }
  }

  /// 用户当前的中文键盘：属于哪一类、14/18 键画哪种字母格、字母格第三排左端是不是分词键。取文档里的方案和双拼、五笔布局，读不到文档时用 App Group 镜像，与键盘页「中文键盘」一行同一个来源。
  private static func currentKeyboard(in document: [String: Any]?) -> (kind: TouchKeyLayoutKind?, grid: LetterKeyGrid, splitsSyllables: Bool) {
    let scheme = InputSchemePreference.current(in: document).scheme
    let shuangpinLayout = document.map { TouchShuangpinLayoutPreference.layout(in: $0) } ?? TouchShuangpinLayoutPreference.layout
    let wubiLayout = document.map { TouchWubiLayoutPreference.layout(in: $0) } ?? TouchWubiLayoutPreference.layout
    return (TouchKeyLayoutKind.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout),
            LetterKeyGrid.of(scheme, shuangpinLayout: shuangpinLayout, wubiLayout: wubiLayout) ?? .fourteenKey,
            scheme != .wubi)
  }
}

/// 一类键盘在所选样式下的整块示意图：26 键是（数字行）+ 三排字母 + 底行（打开「符」与「123」互换时 符 在 123 左边），14/18 键是（数字行）+ 字母格 + 底行，9 键是左侧符号栏 + 九宫格 + 右列 + 底行，「右侧大回车」的回车竖跨最后两行、符 在符号栏下方。字母行取自热力图和 14/18 键键面的同一张表（`TypingKeyHeatmap`、`FourteenKeyLayout`、`EighteenKeyLayout`），底行取自 `TouchBottomRowLayout`（中/英键放在空格左边时照键盘重排），9 键的符号栏按用户的字母层符号表画一屏（`TouchKeyLayoutKind.previewSidebar`）。这个样式与「标准」不同的键用强调色描边，26 键互换后的 符 和 123、挪到空格左边的中/英也描边（`TouchKeyLayoutKind.highlightedSlots`），用自定义符号表时符号栏每一格也描边（`highlightsSidebar`）。
///
/// 只是示意，不可点，读屏跳过整个预览（样式按钮本身有名字和说明）。按 390pt 宽的手机键盘画，整体等比缩放到页面宽度，358pt 宽时约 198pt 高。
private struct KeyLayoutKeyboardPreview: View {
  let kind: TouchKeyLayoutKind
  let style: TouchKeyLayoutStyle
  let letterGrid: LetterKeyGrid
  let gridSplitsSyllables: Bool
  let language: Bool

  private static let canvasWidth: CGFloat = 390
  private static let canvasHeight: CGFloat = 216
  private static let padding: CGFloat = 6
  private static let keyGap: CGFloat = 5
  private static let rowGap: CGFloat = 6
  /// 九键的符号栏、九宫格和右列之间的间距，与键盘的 `nineKeyContainer.spacing` 相同。
  private static let nineKeyColumnGap: CGFloat = 6
  /// 14 键第三排两端的分词键和 ⌫ 在手机上固定 44pt（键盘的 `phoneLetterEdgeWidth`）；画布与 390pt 宽的手机同宽，照抄这个宽度。
  private static let letterGridEdgeWidth: CGFloat = 44
  private static let keyRadius: CGFloat = 6
  /// 键排得下的宽度和高度：画布减去四周的留边。
  private static let width = canvasWidth - 2 * padding
  private static let height = canvasHeight - 2 * padding

  /// 预览里的一个键。
  private struct Cap {
    enum Face {
      case text(String)
      case symbol(String)
    }

    enum Role { case letter, function, action }

    let face: Face
    var role: Role = .letter
    /// 在本行里的宽度份额；`fixed` 不为 nil 时改用这个绝对宽度（画布点）。
    var weight: CGFloat = 1
    var fixed: CGFloat?
    var highlighted = false
  }

  var body: some View {
    KeyboardPreviewCanvas(referenceHeight: Self.canvasHeight) {
      keyboard
        .padding(Self.padding)
        .frame(width: Self.canvasWidth, height: Self.canvasHeight)
        .background(Self.keyboardFill)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
    .allowsHitTesting(false)
    // 整块预览只是示意，读屏跳过：先收成一个元素再隐藏，键面上的字不会一个个露出来。不给它标识，加了标识的子元素会重新出现在读屏和 UI 测试的树里。
    .accessibilityElement(children: .ignore)
    .accessibilityHidden(true)
  }

  @ViewBuilder
  private var keyboard: some View {
    switch kind {
    case .twentySixKey: letterKeyboard(twentySixKeyRows)
    case .letterGrid: letterKeyboard(letterGrid == .eighteenKey ? eighteenKeyRows : fourteenKeyRows)
    case .nineKey: nineKeyKeyboard
    }
  }

  // MARK: - 26 键与 14/18 键

  /// 字母行加上（数字行和）底行，各行等高。`rows` 每一行带着它两侧的缩进。
  private func letterKeyboard(_ rows: [(caps: [Cap], inset: CGFloat)]) -> some View {
    VStack(spacing: Self.rowGap) {
      if kind.drawsNumberRow(style) { row(TypingKeyHeatmap.numberRow.map { Self.cap($0) }) }
      ForEach(Array(rows.enumerated()), id: \.offset) { _, line in row(line.caps, inset: line.inset) }
      row(bottomCaps)
    }
  }

  /// 26 键的三排字母取热力图的同一张表：第二排两侧各缩进本行宽度的 5%，第三排两端是 ⇧ 和 ⌫。
  private var twentySixKeyRows: [(caps: [Cap], inset: CGFloat)] {
    TypingKeyHeatmap.twentySixKeyLetterRows.enumerated().map { index, cells in
      (cells.map { Self.cap($0, separator: .symbol("shift")) },
       index == 1 ? Self.width * CGFloat(TypingKeyHeatmap.keyboardMiddleRowInset) : 0)
    }
  }

  /// 14 键：每排等宽，第二排不缩进；第三排两端是分词键（五笔是 符）和 ⌫，各固定 44pt。
  private var fourteenKeyRows: [(caps: [Cap], inset: CGFloat)] {
    FourteenKeyLayout.rows.enumerated().map { index, keys in
      let letters = keys.map { Cap(face: .text($0.face)) }
      guard index == FourteenKeyLayout.rows.count - 1 else { return (letters, 0) }
      return ([Cap(face: .text(gridSplitsSyllables ? "分词" : "符"), role: .function, fixed: Self.letterGridEdgeWidth)] + letters
        + [Cap(face: .symbol("delete.left"), role: .function, fixed: Self.letterGridEdgeWidth)], 0)
    }
  }

  /// 18 键：三排从同一左右边界铺开，第二排六个键平分整行（与键盘相同，A 与 Q 左对齐、L 与 P 右对齐），第三排两端的分词键和 ⌫ 与字母键同宽。
  private var eighteenKeyRows: [(caps: [Cap], inset: CGFloat)] {
    EighteenKeyLayout.rows.enumerated().map { index, keys in
      let letters = keys.map { Cap(face: .text($0.face)) }
      switch index {
      case EighteenKeyLayout.rows.count - 1:
        return ([Cap(face: .text("分词"), role: .function)] + letters + [Cap(face: .symbol("delete.left"), role: .function)], 0)
      default: return (letters, 0)
      }
    }
  }

  // MARK: - 9 键

  /// 九键外框：符号栏和右列各占整宽的 14%，九宫格平分其余部分；四行等高。「右侧大回车」的 符 在符号栏正下方，回车在右列从第三行伸到底行，九宫格下方是其余几个底行键。
  private var nineKeyKeyboard: some View {
    let side = Self.width * TouchBottomRowLayout.nineKeySideRatio
    let grid = Self.width - side * 2 - Self.nineKeyColumnGap * 2
    let rowHeight = (Self.height - Self.rowGap * 3) / 4
    let gridHeight = rowHeight * 3 + Self.rowGap * 2
    let marks = TouchKeyLayoutKind.previewSidebar(style)
    let sidebarHighlighted = kind.highlightsSidebar(style)
    // 右列的 ⌫、分词和 ！ 都画成功能键，与键盘一样。
    let controls = TypingKeyHeatmap.nineKeyControls(for: style.nineKey).map { cell in
      var cap = Self.cap(cell)
      cap.role = .function
      return cap
    }
    let gridRows = TypingKeyHeatmap.nineKeyGrid.map { $0.map { Self.cap($0) } }
    let bottom = bottomCaps
    return Group {
      if style.nineKey == .tallReturn, let symbols = bottom.first, let enter = bottom.last {
        HStack(alignment: .top, spacing: Self.nineKeyColumnGap) {
          VStack(spacing: Self.rowGap) {
            sidebar(marks, highlighted: sidebarHighlighted).frame(height: gridHeight)
            key(symbols).frame(height: rowHeight)
          }
          .frame(width: side)
          VStack(spacing: Self.rowGap) {
            ForEach(Array(gridRows.enumerated()), id: \.offset) { _, caps in row(caps, width: grid).frame(height: rowHeight) }
            row(Array(bottom.dropFirst().dropLast()), width: grid).frame(height: rowHeight)
          }
          .frame(width: grid)
          VStack(spacing: Self.rowGap) {
            ForEach(Array(controls.enumerated()), id: \.offset) { _, cap in key(cap).frame(height: rowHeight) }
            key(enter).frame(height: rowHeight * 2 + Self.rowGap)
          }
          .frame(width: side)
        }
      } else {
        VStack(spacing: Self.rowGap) {
          HStack(spacing: Self.nineKeyColumnGap) {
            sidebar(marks, highlighted: sidebarHighlighted).frame(width: side)
            VStack(spacing: Self.rowGap) {
              ForEach(Array(gridRows.enumerated()), id: \.offset) { _, caps in row(caps, width: grid) }
            }
            .frame(width: grid)
            VStack(spacing: Self.rowGap) {
              ForEach(Array(controls.enumerated()), id: \.offset) { _, cap in key(cap) }
            }
            .frame(width: side)
          }
          .frame(height: gridHeight)
          row(bottom).frame(height: rowHeight)
        }
      }
    }
    .frame(width: Self.width, height: Self.height, alignment: .top)
  }

  /// 九键左侧的标点栏：一整条半透明底，标点竖排、平分高度，与键盘的 `nineKeySidebar` 画法相同。`highlighted` 时（用自定义符号表）每一格在栏内描一圈强调色。
  private func sidebar(_ marks: [String], highlighted: Bool = false) -> some View {
    VStack(spacing: 0) {
      ForEach(Array(marks.enumerated()), id: \.offset) { _, mark in
        Text(mark).font(.system(size: 16, weight: .medium)).foregroundStyle(.primary)
          .lineLimit(1)
          .minimumScaleFactor(0.5)
          .padding(.horizontal, 2)
          .frame(maxWidth: .infinity, maxHeight: .infinity)
          .overlay {
            if highlighted {
              RoundedRectangle(cornerRadius: Self.keyRadius, style: .continuous)
                .inset(by: 2)
                .strokeBorder(MetasequoiaTheme.accent, lineWidth: 2)
            }
          }
      }
    }
    .background(MetasequoiaTheme.surface.opacity(0.5), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
  }

  // MARK: - 行与键

  /// 所选样式的底行，取自键盘用的同一张表；与「标准」不同的键标上高亮。
  private var bottomCaps: [Cap] {
    let highlighted = kind.highlightedSlots(style)
    return kind.bottomRow(style, language: language).map { key in
      Cap(face: Self.face(key.slot), role: Self.role(key.slot), weight: key.weight,
          highlighted: highlighted.contains(key.slot))
    }
  }

  /// 一排键：`width` 扣掉两侧缩进、键距和固定宽度的键之后，按份额分给其余的键。
  private func row(_ caps: [Cap], inset: CGFloat = 0, width: CGFloat = KeyLayoutKeyboardPreview.width) -> some View {
    let available = width - inset * 2 - Self.keyGap * CGFloat(max(caps.count - 1, 0))
    let fixed = caps.compactMap(\.fixed).reduce(0, +)
    let weights = caps.filter { $0.fixed == nil }.map(\.weight).reduce(0, +)
    let unit = weights > 0 ? max(available - fixed, 0) / weights : 0
    return HStack(spacing: Self.keyGap) {
      ForEach(Array(caps.enumerated()), id: \.offset) { _, cap in
        key(cap).frame(width: cap.fixed ?? unit * cap.weight)
      }
    }
    .frame(width: width)
    .frame(maxHeight: .infinity)
  }

  private func key(_ cap: Cap) -> some View {
    RoundedRectangle(cornerRadius: Self.keyRadius, style: .continuous)
      .fill(Self.fill(cap.role))
      .overlay {
        Self.label(cap.face)
          .font(.system(size: cap.role == .letter ? 16 : 14, weight: .medium))
          .foregroundStyle(cap.role == .action ? MetasequoiaTheme.onAccent : Color.primary)
          .lineLimit(1)
          .minimumScaleFactor(0.5)
          .padding(.horizontal, 2)
      }
      .overlay {
        // 描边画在键外侧、隔开一点：回车本身是强调色实底，贴边描在它上面看不出来。
        if cap.highlighted {
          RoundedRectangle(cornerRadius: Self.keyRadius, style: .continuous)
            .inset(by: -3.5)
            .strokeBorder(MetasequoiaTheme.accent, lineWidth: 2)
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
  }

  @ViewBuilder
  private static func label(_ face: Cap.Face) -> some View {
    switch face {
    case .text(let text): Text(text)
    case .symbol(let name): Image(systemName: name)
    }
  }

  /// 配色跟着应用当前的季节主题走：键盘底色是强调色淡淡地混进卡片色，字母键是卡片色，功能键混得更深一些，回车是强调色实底。
  private static let keyboardFill = MetasequoiaTheme.accentMix(10, 18)

  private static func fill(_ role: Cap.Role) -> Color {
    switch role {
    case .letter: MetasequoiaTheme.surface
    case .function: MetasequoiaTheme.accentMix(24, 32)
    case .action: MetasequoiaTheme.accent
    }
  }

  /// 热力图格子画成预览键：⌫ 画成图标，⌫ 和分词键是功能键，其余照印它的字。`separator` 是分词键那一格画什么：26 键第三排左端空闲时是 ⇧，九键右列照印「分词」。
  private static func cap(_ cell: TypingKeyHeatmap.Cell, separator: Cap.Face? = nil) -> Cap {
    let weight = CGFloat(cell.width)
    switch cell.id {
    case TypingKeyID.backspace: return Cap(face: .symbol("delete.left"), role: .function, weight: weight)
    case TypingKeyID.pinyinSeparator: return Cap(face: separator ?? .text(cell.face), role: .function, weight: weight)
    default: return Cap(face: .text(cell.face), weight: weight)
    }
  }

  private static func face(_ slot: TouchBottomRowLayout.Slot) -> Cap.Face {
    switch slot {
    case .symbols: .text("符")
    case .layer: .text("123")
    case .globe: .symbol("globe")
    case .emoji: .symbol("face.smiling")
    case .comma: .text("，")
    case .space: .text("空格")
    case .period: .text("。")
    case .zero: .text("0")
    case .language: .text("中")
    case .returnKey: .symbol("return")
    }
  }

  private static func role(_ slot: TouchBottomRowLayout.Slot) -> Cap.Role {
    switch slot {
    case .returnKey: .action
    case .comma, .period, .space, .zero: .letter
    case .symbols, .layer, .globe, .emoji, .language: .function
    }
  }
}
