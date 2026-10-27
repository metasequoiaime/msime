import SwiftUI

/// 「键盘 › 按键布局」：26 键、14 键与 18 键、9 键三类键面各选一种底行样式，外加顶部数字行开关，对应共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 与 `touch_number_row`（`TouchKeyLayoutStyle`）。三端同一套选项和画法，见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md。
///
/// 每个选项下面画一条那种底行的缩略图，键的顺序和宽度取自键盘用的同一张表（`TouchBottomRowLayout`）。本版本不提供的键面类型不列出它那一组。改动只写刚选的那一项，键盘下次出现或重载文档时当场换上，不打断正在组的字。
struct KeyboardKeyLayoutSettingsView: View {
  @State private var style = TouchKeyLayoutStyle.mirrored
  @State private var saveFailed = false

  var body: some View {
    ScrollView {
      VStack(spacing: 28) {
        if Self.offersTwentySixKey {
          choiceGroup(title: "26 键", footer: "作用于中文 26 键（全拼、双拼、五笔等），英文键盘不变。长空格把符号入口移到工具栏，逗号下滑或长按输入句号；表情键样式长按 123 打开符号。",
                      options: TouchTwentySixKeyStyle.allCases, selected: style.twentySixKey, label: \.title,
                      identifier: { "keyLayout26_\($0.rawValue)" },
                      preview: { TouchBottomRowLayout.twentySixKey($0, globe: false) }) { next in
            save(TouchKeyLayoutStyle.save(twentySixKey: next))
          }
        }
        if Self.offersLetterGrid {
          choiceGroup(title: "14 键与 18 键", footer: nil,
                      options: TouchFourteenKeyStyle.allCases, selected: style.fourteenKey, label: \.title,
                      identifier: { "keyLayout14_\($0.rawValue)" },
                      preview: { TouchBottomRowLayout.letterGrid($0, globe: false) }) { next in
            save(TouchKeyLayoutStyle.save(fourteenKey: next))
          }
        }
        if Self.offersNineKey {
          choiceGroup(title: "9 键", footer: "左侧符号栏、右侧功能键和数字页不变。",
                      options: TouchNineKeyStyle.allCases, selected: style.nineKey, label: \.title,
                      identifier: { "keyLayout9_\($0.rawValue)" },
                      preview: { TouchBottomRowLayout.nineKey($0, globe: false) }) { next in
            save(TouchKeyLayoutStyle.save(nineKey: next))
          }
        }
        DesignGroup(footer: saveFailed ? SettingsSaveFailure.message : nil) {
          DesignToggleRow(title: "顶部数字行", subtitle: "在 26 键、14 键和 18 键的字母上方多一排 1 到 0，手机横屏时不显示",
                          isOn: Binding(get: { style.numberRow }, set: { save(TouchKeyLayoutStyle.save(numberRow: $0)) }))
            .accessibilityIdentifier("keyLayoutNumberRowToggle")
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

  /// 本版本有没有画中文 26 键的方案。
  static var offersTwentySixKey: Bool {
    ChineseInputScheme.allCases.contains { scheme in
      scheme.isOfferedByEdition && !scheme.isMandarinNineKey && scheme != .japaneseNineKey && !scheme.isZhuyin
        && !scheme.isStroke && scheme != .handwriting && scheme != .fourteenKey && scheme != .eighteenKey
    }
  }

  /// 本版本有没有 14 键或 18 键：全拼的两个入口，以及双拼和五笔可选的字母键布局。
  static var offersLetterGrid: Bool {
    [ChineseInputScheme.fourteenKey, .eighteenKey, .shuangpin, .wubi].contains { $0.isOfferedByEdition }
  }

  /// 本版本有没有九键框架：拼音九键、九键双拼和笔画。
  static var offersNineKey: Bool {
    [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke].contains { $0.isOfferedByEdition }
  }

  private func choiceGroup<Value: Hashable>(
    title: String, footer: String?, options: [Value], selected: Value, label optionTitle: @escaping (Value) -> String,
    identifier: @escaping (Value) -> String, preview: @escaping (Value) -> [TouchBottomRowLayout.Key],
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
            KeyLayoutRowPreview(keys: preview(option)).accessibilityHidden(true)
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

/// 一种底行的缩略图：照键盘的宽度份额排出圆角键块，键上印这个键的字。
private struct KeyLayoutRowPreview: View {
  let keys: [TouchBottomRowLayout.Key]
  private static let gap: CGFloat = 3

  var body: some View {
    GeometryReader { geometry in
      let total = keys.reduce(0) { $0 + $1.weight }
      let unit = (geometry.size.width - Self.gap * CGFloat(max(keys.count - 1, 0))) / max(total, 1)
      HStack(spacing: Self.gap) {
        ForEach(Array(keys.enumerated()), id: \.offset) { _, key in
          RoundedRectangle(cornerRadius: 4, style: .continuous)
            .fill(key.slot == .returnKey ? MetasequoiaTheme.accent.opacity(0.85) : MetasequoiaTheme.sub.opacity(0.16))
            .overlay {
              face(key.slot)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(key.slot == .returnKey ? Color.white : Color.primary)
                .lineLimit(1).minimumScaleFactor(0.6)
            }
            .frame(width: unit * key.weight)
        }
      }
    }
    .frame(height: 24)
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
