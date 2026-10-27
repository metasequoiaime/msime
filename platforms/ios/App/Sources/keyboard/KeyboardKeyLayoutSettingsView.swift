import SwiftUI

/// 「键盘 › 按键布局」：26 键、14 键与 18 键、9 键三类键面各选一种底行样式，外加顶部数字行开关，对应共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 与 `touch_number_row`（`TouchKeyLayoutStyle`）。三端同一套选项和画法，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
///
/// 每个选项下面画一条那种底行的缩略图，键的顺序和宽度取自键盘用的同一张表（`TouchBottomRowLayout`）。本版本不提供的键面类型不列出它那一组，既没有中文 26 键也没有 14/18 键时不列顶部数字行开关（`TouchKeyLayoutOffering`）。改动只写刚选的那一项，键盘下次出现或重载文档时当场换上，不打断正在组的字。
struct KeyboardKeyLayoutSettingsView: View {
  @State private var style = TouchKeyLayoutStyle.mirrored
  @State private var saveFailed = false
  /// 本版本提供哪几类键面，决定列哪几组和要不要数字行开关。
  private let offering = TouchKeyLayoutOffering.edition

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        if offering.twentySixKey {
          choiceGroup(title: "26 键", footer: "作用于中文 26 键（全拼、双拼、五笔等），英文键盘不变。长空格把符号入口移到工具栏，逗号下滑或长按输入句号；表情键样式长按 123 打开符号。",
                      options: TouchTwentySixKeyStyle.allCases, selected: style.twentySixKey, label: \.title,
                      identifier: { "keyLayout26_\($0.rawValue)" },
                      preview: { KeyLayoutRowPreview(keys: TouchBottomRowLayout.twentySixKey($0, globe: false)) }) { next in
            save(TouchKeyLayoutStyle.save(twentySixKey: next))
          }
        }
        if offering.letterGrid {
          choiceGroup(title: "14 键与 18 键", footer: nil,
                      options: TouchFourteenKeyStyle.allCases, selected: style.fourteenKey, label: \.title,
                      identifier: { "keyLayout14_\($0.rawValue)" },
                      preview: { KeyLayoutRowPreview(keys: TouchBottomRowLayout.letterGrid($0, globe: false)) }) { next in
            save(TouchKeyLayoutStyle.save(fourteenKey: next))
          }
        }
        if offering.nineKey {
          choiceGroup(title: "9 键", footer: "右侧大回车把回车加高到两排，符 移到符号栏下方，右列的 ！ 并进符号栏，数字页也这样排；其他样式的左侧符号栏、右侧功能键和数字页不变。",
                      options: TouchNineKeyStyle.allCases, selected: style.nineKey, label: \.title,
                      identifier: { "keyLayout9_\($0.rawValue)" },
                      preview: { KeyLayoutRowPreview(keys: TouchBottomRowLayout.nineKey($0, globe: false), tallReturn: $0 == .tallReturn) }) { next in
            save(TouchKeyLayoutStyle.save(nineKey: next))
          }
        }
        if offering.numberRow {
          DesignGroup(footer: saveFailed ? SettingsSaveFailure.message : nil) {
            DesignToggleRow(title: "顶部数字行", subtitle: "在 26 键、14 键和 18 键的字母上方多一排 1 到 0，手机横屏时不显示",
                            isOn: Binding(get: { style.numberRow }, set: { save(TouchKeyLayoutStyle.save(numberRow: $0)) }))
              .accessibilityIdentifier("keyLayoutNumberRowToggle")
          }
        } else if saveFailed {
          // 没有数字行开关时，保存失败的提示单独放在最后一组下面。
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

  private func choiceGroup<Value: Hashable>(
    title: String, footer: String?, options: [Value], selected: Value, label optionTitle: @escaping (Value) -> String,
    identifier: @escaping (Value) -> String, preview: @escaping (Value) -> KeyLayoutRowPreview,
    onSelect: @escaping (Value) -> Void
  ) -> some View {
    DesignGroup(title: title, footer: footer) {
      ForEach(Array(options.enumerated()), id: \.offset) { index, option in
        if index > 0 { DesignDivider() }
        Button {
          guard option != selected else { return }
          onSelect(option)
        } label: {
          VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 12) {
              Text(optionTitle(option)).font(.system(size: 17)).foregroundStyle(.primary)
              Spacer(minLength: 8)
              if option == selected {
                Image(systemName: "checkmark").font(.system(size: 17, weight: .semibold))
                  .foregroundStyle(MetasequoiaTheme.accent).accessibilityHidden(true)
              }
            }
            preview(option).accessibilityHidden(true)
          }
          .padding(.vertical, 10)
          .padding(.horizontal, 20)
          .frame(maxWidth: .infinity, alignment: .leading)
          .contentShape(Rectangle())
        }
        .buttonStyle(PressFillButtonStyle())
        .accessibilityIdentifier(identifier(option))
        .accessibilityValue(option == selected ? "已选择" : "未选择")
        .accessibilityAddTraits(option == selected ? .isSelected : [])
      }
    }
  }

  /// 写进文档成功时页面换成新值；写不进去时提示并回到文档里存着的值。
  private func save(_ saved: Bool) {
    saveFailed = !saved
    readPreferences()
  }

  private func readPreferences() {
    // 文档是权威：先抄进 App Group，再从镜像读回页面，两边才是同一个值。
    TouchKeyLayoutStyle.mirror(MetasequoiaInputSessionBridge.loadSharedPreferences())
    style = TouchKeyLayoutStyle.mirrored
  }
}

/// 一种底行的缩略图：照键盘的宽度份额排出圆角键块，键上印这个键的字。九键「右侧大回车」（`tallReturn`）画两排：九宫格最下一排和底行，左列是符号栏和它下方的 符，右列是跨两排的回车。
private struct KeyLayoutRowPreview: View {
  let keys: [TouchBottomRowLayout.Key]
  var tallReturn = false
  private static let gap: CGFloat = 3
  private static let rowHeight: CGFloat = 24

  var body: some View {
    if tallReturn { tallReturnFrame } else { row }
  }

  private var row: some View {
    GeometryReader { geometry in
      HStack(spacing: Self.gap) { blocks(keys, width: geometry.size.width) }
    }
    .frame(height: Self.rowHeight)
  }

  /// 第一格 符 和最后一格回车各占一个侧列宽，中间几格排在九宫格下方；九宫格最下一排是三个等宽的字母键。
  private var tallReturnFrame: some View {
    GeometryReader { geometry in
      let side = geometry.size.width * TouchBottomRowLayout.nineKeySideRatio
      let grid = geometry.size.width - side * 2 - Self.gap * 2
      let cell = (grid - Self.gap * 2) / 3
      HStack(alignment: .top, spacing: Self.gap) {
        VStack(spacing: Self.gap) {
          block(fill: MetasequoiaTheme.sub.opacity(0.08)) { Text("，。？！") }.frame(width: side, height: Self.rowHeight)
          if let symbols = keys.first { block(symbols.slot).frame(width: side, height: Self.rowHeight) }
        }
        VStack(spacing: Self.gap) {
          HStack(spacing: Self.gap) {
            ForEach(["PQRS", "TUV", "WXYZ"], id: \.self) { letters in
              block(fill: MetasequoiaTheme.sub.opacity(0.16)) { Text(letters) }.frame(width: cell, height: Self.rowHeight)
            }
          }
          HStack(spacing: Self.gap) { blocks(Array(keys.dropFirst().dropLast()), width: grid) }
            .frame(width: grid, height: Self.rowHeight)
        }
        if let enter = keys.last { block(enter.slot).frame(width: side, height: Self.rowHeight * 2 + Self.gap) }
      }
    }
    .frame(height: Self.rowHeight * 2 + Self.gap)
  }

  /// 一排键块，按份额分摊扣掉键缝之后的 `width`。
  private func blocks(_ keys: [TouchBottomRowLayout.Key], width: CGFloat) -> some View {
    let total = keys.reduce(0) { $0 + $1.weight }
    let unit = (width - Self.gap * CGFloat(max(keys.count - 1, 0))) / max(total, 1)
    return ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
      block(key.slot).frame(width: unit * key.weight)
    }
  }

  private func block(_ slot: TouchBottomRowLayout.Slot) -> some View {
    block(fill: slot == .returnKey ? MetasequoiaTheme.accent.opacity(0.85) : MetasequoiaTheme.sub.opacity(0.16),
          ink: slot == .returnKey ? Color.white : Color.primary) { face(slot) }
  }

  private func block<Face: View>(fill: Color, ink: Color = .primary, @ViewBuilder face: () -> Face) -> some View {
    RoundedRectangle(cornerRadius: 4, style: .continuous)
      .fill(fill)
      .overlay {
        face()
          .font(.system(size: 11, weight: .medium))
          .foregroundStyle(ink)
          .lineLimit(1).minimumScaleFactor(0.6)
      }
  }

  @ViewBuilder
  private func face(_ slot: TouchBottomRowLayout.Slot) -> some View {
    switch slot {
    case .symbols: Text("符")
    case .layer: Text("123")
    case .globe: Image(systemName: "globe")
    case .emoji: Image(systemName: "face.smiling")
    case .comma: Text("，")
    case .space: Text("空格")
    case .period: Text("。")
    case .zero: Text("0")
    case .language: Text("中")
    case .returnKey: Image(systemName: "return")
    }
  }
}
