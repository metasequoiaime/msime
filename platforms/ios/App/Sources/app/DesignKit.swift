import SwiftUI
import UIKit

// iOS 设计稿共用的构件：卡片、行、底部选项面板、toast 和分段控件。这里只读取 `MetasequoiaTheme`、`AppThemePalette` 和 `MSIMELogo`，因为键盘单元测试会把这个文件和候选设置页放在一起编译，不带应用的其余部分。

private func dynamicColor(light: UIColor, dark: UIColor) -> Color {
  Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? dark : light })
}

private extension View {
  /// 只在有 id 时才设置无障碍标识符，这样没有 id 的行能保留调用方可能给它加上的标识符。
  @ViewBuilder func designIdentifier(_ identifier: String?) -> some View {
    if let identifier { accessibilityIdentifier(identifier) } else { self }
  }
}

extension View {
  /// Applies the app canvas colour through the safe-area edges.
  func designCanvasBackground() -> some View {
    background(MetasequoiaTheme.canvas.ignoresSafeArea())
  }

  /// Applies the standard compact secondary text treatment used by design-kit surfaces.
  func designSubtext() -> some View {
    font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
  }
}

// MARK: - 页头标志

/// 页头的应用标志（「设置」「关于」和首次引导）：固定配色的水杉标志（`MSIMELogoMark`）本身就是外框，不再垫圆盘。`box` 是它在版面里占的方块，标志取其 5/6 居中，与原来圆盘的占位相同。
struct AppMark: View {
  var box: CGFloat = 52

  var body: some View {
    MSIMELogoMark()
      .frame(width: box * 5 / 6, height: box * 5 / 6)
      .frame(width: box, height: box)
      .accessibilityHidden(true)
  }
}

/// Accent-colored initial tile shared by compact list rows and community cards.
struct DesignInitialTile: View {
  enum TileShape { case rounded, circle }
  let text: String
  var size: CGFloat = 40
  var fontSize: CGFloat = 16
  var fontWeight: Font.Weight = .bold
  var cornerRadius: CGFloat = MetasequoiaTheme.tabCardRadius
  var shape: TileShape = .rounded

  var body: some View {
    Text(text)
      .font(.system(size: fontSize, weight: fontWeight))
      .foregroundStyle(MetasequoiaTheme.accent)
      .frame(width: size, height: size)
      .background(MetasequoiaTheme.accentSoft, in: tileShape)
      .accessibilityHidden(true)
  }

  private var tileShape: AnyShape {
    switch shape {
    case .rounded: AnyShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
    case .circle: AnyShape(Circle())
    }
  }
}

// MARK: - 卡片与分组

/// Section header shared by settings lists and design cards.
struct SettingsGroupHeader: View {
  let title: String
  var body: some View {
    Text(title).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle).textCase(nil)
      .accessibilityAddTraits(.isHeader)
  }
}

/// 分组卡片：季节卡片色的连续圆角矩形，无阴影、无边框。卡片里的行之间没有间距，行与行之间放一个 `DesignDivider`。
struct DesignCard<Content: View>: View {
  var radius: CGFloat
  private let content: Content

  init(radius: CGFloat = MetasequoiaTheme.cardRadius, @ViewBuilder content: () -> Content) {
    self.radius = radius
    self.content = content()
  }

  var body: some View {
    let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
    VStack(spacing: 0) { content }
      .frame(maxWidth: .infinity)
      .background(MetasequoiaTheme.surface, in: shape)
      .clipShape(shape)
      .accessibilityElement(children: .contain)
  }
}

/// 带标题的分组：13pt 的分组标题，其下 7pt 是卡片，另有可选的 13pt 脚注。页面上的分组之间相隔 28pt。
struct DesignGroup<Content: View>: View {
  var title: String?
  var footer: String?
  var radius: CGFloat
  private let content: Content

  init(title: String? = nil, footer: String? = nil, radius: CGFloat = MetasequoiaTheme.cardRadius,
       @ViewBuilder content: () -> Content) {
    self.title = title
    self.footer = footer
    self.radius = radius
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 7) {
      if let title {
        Text(title).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.groupTitle)
          .padding(.horizontal, 20)
          .accessibilityAddTraits(.isHeader)
      }
      DesignCard(radius: radius) { content }
      if let footer {
        Text(footer).designSubtext()
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 20)
      }
    }
  }
}

/// 季节分隔线色的一像素细线，默认从前缘缩进 16。
struct DesignDivider: View {
  var leading: CGFloat = 16
  var trailing: CGFloat = 0
  @Environment(\.displayScale) private var displayScale

  var body: some View {
    Rectangle().fill(MetasequoiaTheme.hair)
      .frame(height: 1 / displayScale)
      .padding(.leading, leading)
      .padding(.trailing, trailing)
      .accessibilityHidden(true)
  }
}

// MARK: - 行

/// Shared tinted capsule label used for compact status and action pills.
struct DesignPillLabel: View {
  enum Style { case filled, tonal, neutral }
  enum Size { case micro, compact, regular, page }

  let title: String
  var style: Style = .tonal
  var size: Size = .compact

  var body: some View {
    Text(title)
      .font(.system(size: size == .micro ? 10 : size == .compact ? 12 : 13,
        weight: size == .micro ? .medium : .semibold))
      .foregroundStyle(foreground)
      .lineLimit(1)
      .fixedSize()
      .padding(.vertical, size == .micro ? 2 : size == .compact ? 4 : size == .regular ? 5 : 6)
      .padding(.horizontal, size == .micro ? 6 : size == .compact ? 12 : 14)
      .background(background, in: Capsule())
      .contentShape(Capsule())
  }

  private var foreground: Color {
    switch style {
    case .filled: MetasequoiaTheme.onAccent
    case .tonal: MetasequoiaTheme.accent
    case .neutral: MetasequoiaTheme.sub
    }
  }

  private var background: Color {
    switch style {
    case .filled: MetasequoiaTheme.accent
    case .tonal: MetasequoiaTheme.accentSoft
    case .neutral: MetasequoiaTheme.segBg
    }
  }
}

/// 所有行共用的布局：最小高度 52pt，内边距 8/20，17pt 标题，其下 2pt 处是可选的 13pt 副标题，再加上行尾的内容。
private struct DesignRowLayout<Leading: View, Trailing: View>: View {
  let title: String
  let subtitle: String?
  private let leading: Leading
  private let trailing: Trailing

  init(title: String, subtitle: String?, @ViewBuilder leading: () -> Leading, @ViewBuilder trailing: () -> Trailing) {
    self.title = title
    self.subtitle = subtitle
    self.leading = leading()
    self.trailing = trailing()
  }

  var body: some View {
    HStack(spacing: 12) {
      leading
      DesignRowLabel(title: title, subtitle: subtitle)
        .multilineTextAlignment(.leading)
      Spacer(minLength: 8)
      trailing
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
    .contentShape(Rectangle())
  }
}

private extension DesignRowLayout where Leading == EmptyView {
  init(title: String, subtitle: String?, @ViewBuilder trailing: () -> Trailing) {
    self.init(title: title, subtitle: subtitle, leading: { EmptyView() }, trailing: trailing)
  }
}

/// 导航行和选择行行尾的箭头。
/// Shared trailing chevron for compact navigation rows.
struct DesignChevron: View {
  var fontSize: CGFloat = 13
  var opacity: Double = 0.55

  var body: some View {
    Image(systemName: "chevron.right").font(.system(size: fontSize, weight: .semibold))
      .foregroundStyle(MetasequoiaTheme.sub.opacity(opacity))
      .accessibilityHidden(true)
  }
}

/// Shared trailing icon for links that leave the current settings surface.
struct DesignExternalLinkIcon: View {
  var body: some View {
    Image(systemName: "arrow.up.right")
      .font(.system(size: 13, weight: .semibold))
      .foregroundStyle(MetasequoiaTheme.sub.opacity(0.55))
      .accessibilityHidden(true)
  }
}

/// 推入新页面的行的标签：可选的 29pt 图标框、标题和副标题、可选的值，以及箭头。放在 `NavigationLink` 里并加上 `.buttonStyle(PressFillButtonStyle())`。
struct DesignNavRowLabel: View {
  let title: String
  var subtitle: String? = nil
  var value: String? = nil
  var symbol: String? = nil

  var body: some View {
    DesignRowLayout(title: title, subtitle: subtitle) {
      if let symbol {
        Image(systemName: symbol).font(.system(size: 20)).foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 29, height: 29).accessibilityHidden(true)
      }
    } trailing: {
      HStack(spacing: 8) {
        if let value {
          Text(value).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
        }
        DesignChevron()
      }
    }
  }
}

/// 开关行；开关打开时的颜色是季节强调色。
struct DesignToggleRow: View {
  let title: String
  var subtitle: String? = nil
  @Binding var isOn: Bool

  init(title: String, subtitle: String? = nil, isOn: Binding<Bool>) {
    self.title = title
    self.subtitle = subtitle
    _isOn = isOn
  }

  var body: some View {
    Toggle(isOn: $isOn) {
      DesignRowLabel(title: title, subtitle: subtitle)
    }
    .toggleStyle(.switch)
    .tint(MetasequoiaTheme.switchOn)
    .padding(.vertical, 8)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
  }
}

/// Shared title and optional subtitle layout for settings rows.
private struct DesignRowLabel: View {
  let title: String
  let subtitle: String?

  var body: some View {
    VStack(alignment: .leading, spacing: 2) {
      Text(title).font(.system(size: 17)).foregroundStyle(.primary)
      if let subtitle {
        Text(subtitle).designSubtext()
          .fixedSize(horizontal: false, vertical: true)
      }
    }
  }
}

/// 只读行：标题，右侧是 16pt 的值。
struct DesignValueRow: View {
  let title: String
  let value: String

  var body: some View {
    DesignRowLayout(title: title, subtitle: nil) {
      Text(value).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
    }
    .accessibilityElement(children: .combine)
  }
}

/// 右侧带一个 13pt 强调色纯文字按钮的行，例如「试一下振动」。给了 `doneTitle` 时，点按后按钮会显示它 1.6 秒。
struct DesignInlineButtonRow: View {
  let title: String
  var subtitle: String? = nil
  let buttonTitle: String
  var doneTitle: String? = nil
  var identifier: String? = nil
  let action: () -> Void
  @State private var showsDone = false
  @State private var resetTask: Task<Void, Never>?

  init(title: String, subtitle: String? = nil, buttonTitle: String, doneTitle: String? = nil, identifier: String? = nil,
       action: @escaping () -> Void) {
    self.title = title
    self.subtitle = subtitle
    self.buttonTitle = buttonTitle
    self.doneTitle = doneTitle
    self.identifier = identifier
    self.action = action
  }

  var body: some View {
    DesignRowLayout(title: title, subtitle: subtitle) {
      Button {
        action()
        guard doneTitle != nil else { return }
        showsDone = true
        ViewRequestTask.cancel(task: $resetTask)
        resetTask = Task { @MainActor in
          try? await Task.sleep(nanoseconds: 1_600_000_000)
          guard !Task.isCancelled else { return }
          showsDone = false
        }
      } label: {
        Text(showsDone ? doneTitle ?? buttonTitle : buttonTitle)
          .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.accent)
          .padding(.vertical, 6)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityLabel(showsDone ? doneTitle ?? buttonTitle : buttonTitle)
      .designIdentifier(identifier)
    }
    .onDisappear { ViewRequestTask.cancel(task: $resetTask) }
  }
}

/// 右侧带短滑块的行（紧凑宽度下 110pt，常规宽度下 160pt），格式化后的值放在 46pt 宽的一列里。
struct DesignSliderRow: View {
  let title: String
  /// 标题下的说明行；缺省不画。
  var subtitle: String? = nil
  @Binding var value: Double
  let range: ClosedRange<Double>
  let step: Double
  let format: (Double) -> String
  var identifier: String? = nil
  @Environment(\.horizontalSizeClass) private var widthClass

  init(title: String, subtitle: String? = nil, value: Binding<Double>, range: ClosedRange<Double>, step: Double,
       identifier: String? = nil, format: @escaping (Double) -> String) {
    self.title = title
    self.subtitle = subtitle
    _value = value
    self.range = range
    self.step = step
    self.identifier = identifier
    self.format = format
  }

  var body: some View {
    DesignRowLayout(title: title, subtitle: subtitle) {
      HStack(spacing: 8) {
        Slider(value: $value, in: range, step: step) { Text(title) }
          .tint(MetasequoiaTheme.accent)
          .frame(width: widthClass == .regular ? 160 : 110)
          .accessibilityValue(format(value))
          .designIdentifier(identifier)
        Text(format(value)).designSubtext()
          .monospacedDigit().lineLimit(1).minimumScaleFactor(0.8)
          .frame(width: 46, alignment: .trailing)
          .accessibilityHidden(true)
      }
    }
  }
}

/// 从底部选项面板中选一个值的行：右侧是 16pt 的当前值和箭头。带子项的选项会打开第二层面板。
struct DesignSelectRow<Value: Hashable>: View {
  let title: String
  var subtitle: String? = nil
  let options: [DesignOption<Value>]
  @Binding var selection: Value
  var sheetTitle: String? = nil
  var sheetMessage: String? = nil
  var identifier: String? = nil
  var onSelect: ((Value) -> Void)? = nil
  @State private var presented = false

  init(title: String, subtitle: String? = nil, options: [DesignOption<Value>], selection: Binding<Value>,
       sheetTitle: String? = nil, sheetMessage: String? = nil, identifier: String? = nil,
       onSelect: ((Value) -> Void)? = nil) {
    self.title = title
    self.subtitle = subtitle
    self.options = options
    _selection = selection
    self.sheetTitle = sheetTitle
    self.sheetMessage = sheetMessage
    self.identifier = identifier
    self.onSelect = onSelect
  }

  private var valueTitle: String { DesignOption.title(of: selection, in: options) ?? "" }

  var body: some View {
    Button { presented = true } label: {
      DesignRowLayout(title: title, subtitle: subtitle) {
        HStack(spacing: 8) {
          Text(valueTitle).font(.system(size: 16)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
          DesignChevron()
        }
      }
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityValue(valueTitle)
    .designIdentifier(identifier)
    .designOptionSheet(isPresented: $presented, title: sheetTitle, message: sheetMessage, options: options,
                       selected: selection) { value in
      selection = value
      onSelect?(value)
    }
  }
}

/// 行在整行宽度上绘制按下填充（指针悬停时绘制悬停填充），与设计稿的列表行一致。
struct PressFillButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    PressFillBody(configuration: configuration)
  }

  private struct PressFillBody: View {
    let configuration: ButtonStyleConfiguration
    @State private var hovered = false

    var body: some View {
      configuration.label
        .background(configuration.isPressed ? MetasequoiaTheme.pressFill : hovered ? MetasequoiaTheme.hoverFill : Color.clear)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
    }
  }
}

// MARK: - 选项面板

/// 选项面板中的一个选项。带 `children` 的选项在标题后显示 ›，点开后用这些子项打开第二层面板（双拼 ›、五笔 ›）；它自己的 `value` 永远不会被选中。
struct DesignOption<Value: Hashable>: Identifiable {
  var title: String
  var value: Value
  var isDestructive = false
  var identifier: String? = nil
  var children: [DesignOption]? = nil

  var id: String { identifier ?? title }

  /// `selected` 是否就是这个选项；对父选项来说，也包括它的某个后代。
  func contains(_ selected: Value?) -> Bool {
    guard let selected else { return false }
    if let children { return children.contains { $0.contains(selected) } }
    return value == selected
  }

  /// 值为 `value` 的叶子选项的标题，子项也会搜索。
  static func title(of value: Value, in options: [DesignOption]) -> String? {
    for option in options {
      if let children = option.children {
        if let title = title(of: value, in: children) { return title }
      } else if option.value == value {
        return option.title
      }
    }
    return nil
  }
}

extension View {
  /// 弹出设计稿的底部选项面板：一层遮罩、一张居中排列选项并在选中项上打勾的卡片，以及单独的「取消」卡片。常规宽度下改为弹出式列表。`onSelect` 在面板消失后才运行；不做选择就关闭时不会调用它。
  func designOptionSheet<Value: Hashable>(isPresented: Binding<Bool>, title: String? = nil, message: String? = nil,
                                          options: [DesignOption<Value>], selected: Value?,
                                          onSelect: @escaping (Value) -> Void) -> some View {
    modifier(DesignOptionSheetModifier(isPresented: isPresented, title: title, message: message, options: options,
                                       selected: selected, onSelect: onSelect))
  }
}

private struct DesignOptionSheetModifier<Value: Hashable>: ViewModifier {
  @Binding var isPresented: Bool
  let title: String?
  let message: String?
  let options: [DesignOption<Value>]
  let selected: Value?
  let onSelect: (Value) -> Void
  @Environment(\.horizontalSizeClass) private var widthClass
  @State private var coverShown = false

  private var isRegular: Bool { widthClass == .regular }

  func body(content: Content) -> some View {
    content
      .popover(isPresented: Binding(get: { isPresented && isRegular }, set: { if !$0 { isPresented = false } })) {
        DesignOptionPopover(title: title, message: message, options: options, selected: selected) { value in
          isPresented = false
          onSelect(value)
        }
      }
      // 覆盖层本身出现时不用 UIKit 的滑入动画，这样面板才能自己画出设计稿的遮罩淡入和 .28s 上滑。
      .fullScreenCover(isPresented: $coverShown) {
        DesignOptionSheetView(title: title, message: message, options: options, selected: selected) { value in
          setCover(false)
          isPresented = false
          if let value { onSelect(value) }
        }
        .presentationBackground(.clear)
      }
      .onChange(of: isPresented) { _, shown in setCover(shown && !isRegular) }
      .onAppear { if isPresented && !isRegular { setCover(true) } }
  }

  private func setCover(_ shown: Bool) {
    guard coverShown != shown else { return }
    var transaction = Transaction()
    transaction.disablesAnimations = true
    withTransaction(transaction) { coverShown = shown }
  }
}

/// 紧凑宽度下选项面板的尺寸。选项行的字号固定（不随动态字体变化）、只排一行，行高因此是定值，列表高度可以直接按行数算出来，不用量。
enum DesignOptionSheetMetrics {
  static let rowHeight: CGFloat = 56
  static let hairline: CGFloat = 0.5

  /// `rows` 行选项连同行间分隔线的高度，不超过 `limit`；超过时列表在这个高度里滚动。向上取整到整点，免得 3x 屏把 0.5pt 的分隔线对齐到像素后内容比框高出零点几、可以拖动一下。
  static func listHeight(rows: Int, limit: CGFloat) -> CGFloat {
    guard rows > 0 else { return 0 }
    let content = CGFloat(rows) * rowHeight + CGFloat(rows - 1) * hairline
    return min(content.rounded(.up), max(limit, 0))
  }
}

/// 紧凑宽度下的面板。出现时自己做入场动画，调用 `finish` 前先做完退场动画，再由 `finish` 无动画地移除覆盖层。
private struct DesignOptionSheetView<Value: Hashable>: View {
  let title: String?
  let message: String?
  let options: [DesignOption<Value>]
  let selected: Value?
  let finish: (Value?) -> Void

  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var shown = false
  @State private var level: (title: String?, message: String?, options: [DesignOption<Value>])?
  @State private var closing = false

  private static var cardFill: Color {
    dynamicColor(light: UIColor(red: 250 / 255, green: 250 / 255, blue: 250 / 255, alpha: 0.96),
                 dark: UIColor(red: 44 / 255, green: 44 / 255, blue: 46 / 255, alpha: 0.96))
  }

  private var current: (title: String?, message: String?, options: [DesignOption<Value>]) {
    level ?? (title, message, options)
  }

  var body: some View {
    GeometryReader { proxy in
      ZStack(alignment: .bottom) {
        Color.black.opacity(shown ? 0.35 : 0)
          .ignoresSafeArea()
          .contentShape(Rectangle())
          .onTapGesture { close(nil) }
          .accessibilityHidden(true)
        if shown {
          VStack(spacing: 8) {
            optionsCard(maxListHeight: proxy.size.height * 0.6)
            Button { close(nil) } label: {
              Text("取消").font(.system(size: 19, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
                .frame(maxWidth: .infinity, minHeight: 56)
                .contentShape(Rectangle())
            }
            .buttonStyle(PressFillButtonStyle())
            .background(Self.cardFill)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .accessibilityIdentifier("designOptionSheetCancel")
          }
          .padding(.horizontal, 8)
          .padding(.bottom, 8)
          .transition(reduceMotion ? .opacity : .move(edge: .bottom))
          .accessibilityElement(children: .contain)
          .accessibilityAddTraits(.isModal)
          .accessibilityAction(.escape) { close(nil) }
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
    }
    .onAppear { withAnimation(.easeOut(duration: 0.28)) { shown = true } }
  }

  private func optionsCard(maxListHeight: CGFloat) -> some View {
    let level = current
    return VStack(spacing: 0) {
      if let title = level.title {
        VStack(spacing: 4) {
          Text(title).font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.sub)
          if let message = level.message {
            Text(message).designSubtext()
          }
        }
        .multilineTextAlignment(.center)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
        Rectangle().fill(MetasequoiaTheme.hair).frame(height: 0.5).accessibilityHidden(true)
      }
      // 列表的高度按选项行数算出来，卡片贴着选项；超过上限才滚动。原来用 `ViewThatFits` 加 `maxHeight`，卡片总是撑满上限，四个选项也画成六成屏高、选项居中、标题下空一大段。
      ScrollView { optionList(level.options) }
        .scrollBounceBehavior(.basedOnSize)
        .frame(height: DesignOptionSheetMetrics.listHeight(rows: level.options.count, limit: maxListHeight))
    }
    .background(Self.cardFill)
    .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
  }

  private func optionList(_ options: [DesignOption<Value>]) -> some View {
    VStack(spacing: 0) {
      ForEach(options.indices, id: \.self) { index in
        if index > 0 {
          Rectangle().fill(MetasequoiaTheme.hair).frame(height: DesignOptionSheetMetrics.hairline).accessibilityHidden(true)
        }
        optionRow(options[index])
      }
    }
  }

  private func optionRow(_ option: DesignOption<Value>) -> some View {
    let isSelected = option.contains(selected)
    return Button { pick(option) } label: {
      Text(option.children == nil ? option.title : option.title + " ›")
        .font(.system(size: 19, weight: isSelected ? .semibold : .regular))
        .foregroundStyle(option.isDestructive ? MetasequoiaTheme.danger : MetasequoiaTheme.accent)
        .lineLimit(1).minimumScaleFactor(0.7)
        .padding(.horizontal, 48)
        .frame(maxWidth: .infinity)
        .frame(height: DesignOptionSheetMetrics.rowHeight)
        .overlay(alignment: .trailing) {
          if isSelected {
            Image(systemName: "checkmark").font(.system(size: 18, weight: .semibold))
              .foregroundStyle(MetasequoiaTheme.accent)
              .padding(.trailing, 20)
              .accessibilityHidden(true)
          }
        }
        .contentShape(Rectangle())
    }
    .buttonStyle(PressFillButtonStyle())
    .accessibilityLabel(option.title)
    .accessibilityAddTraits(isSelected ? .isSelected : [])
    .designIdentifier(option.identifier)
  }

  private func pick(_ option: DesignOption<Value>) {
    guard !closing else { return }
    if let children = option.children {
      // 链式面板：先把这一层滑出，再把子项滑入，以父选项作为标题。
      closing = true
      withAnimation(.easeIn(duration: 0.2)) { shown = false } completion: {
        level = (option.title, nil, children)
        closing = false
        withAnimation(.easeOut(duration: 0.28)) { shown = true }
      }
    } else {
      close(option.value)
    }
  }

  private func close(_ value: Value?) {
    guard !closing else { return }
    closing = true
    withAnimation(.easeIn(duration: 0.2)) { shown = false } completion: { finish(value) }
  }
}

/// 常规宽度下的选项面板：弹出式列表，选中项行尾打勾。
private struct DesignOptionPopover<Value: Hashable>: View {
  let title: String?
  let message: String?
  let options: [DesignOption<Value>]
  let selected: Value?
  let pick: (Value) -> Void
  /// 正在显示的那一层列表量出的高度（含推入第二层后的导航栏）。量到之前，以及 iOS 17 量不到时，按行数估一个。
  @State private var fitted: CGFloat?

  private var estimated: CGFloat { CGFloat(options.count) * 52 + (title == nil ? 40 : 96) }

  var body: some View {
    NavigationStack {
      DesignOptionPopoverList(title: title, message: message, options: options, selected: selected, pick: pick) { fitted = $0 }
        .toolbar(.hidden, for: .navigationBar)
    }
    .frame(width: 340, height: min(fitted ?? estimated, 480))
    .accessibilityAddTraits(.isModal)
  }
}

private struct DesignOptionPopoverList<Value: Hashable>: View {
  let title: String?
  let message: String?
  let options: [DesignOption<Value>]
  let selected: Value?
  let pick: (Value) -> Void
  /// 报告这一层需要的高度。弹出框按正在显示的那一层定高：第二层（双拼 ›、五笔 ›）的选项数和第一层不同，还多一条导航栏，沿用第一层的高度会空一截或要滚动。
  let fit: (CGFloat) -> Void
  @State private var measured: CGFloat?

  var body: some View {
    measuring(list)
      // 从第二层退回来时第一层的内容没变，量高度的回调不会再来，用上次量到的值。
      .onAppear { if let measured { fit(measured) } }
  }

  @ViewBuilder private func measuring(_ list: some View) -> some View {
    if #available(iOS 18, *) {
      list.onScrollGeometryChange(for: CGFloat.self) { geometry in
        geometry.contentSize.height + geometry.contentInsets.top + geometry.contentInsets.bottom
      } action: { _, height in
        let height = height.rounded(.up)
        guard height > 0 else { return }
        measured = height
        fit(height)
      }
    } else {
      list
    }
  }

  private var list: some View {
    List {
      Section {
        ForEach(options.indices, id: \.self) { index in row(options[index]) }
      } header: {
        if let title {
          VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.system(size: 13, weight: .semibold))
            if let message { Text(message).font(.system(size: 13)) }
          }
          .foregroundStyle(MetasequoiaTheme.sub)
          .textCase(nil)
        }
      }
    }
    .listStyle(.insetGrouped)
  }

  @ViewBuilder private func row(_ option: DesignOption<Value>) -> some View {
    let isSelected = option.contains(selected)
    if let children = option.children {
      NavigationLink {
        DesignOptionPopoverList(title: nil, message: nil, options: children, selected: selected, pick: pick, fit: fit)
          .navigationTitle(option.title)
          .navigationBarTitleDisplayMode(.inline)
          .toolbar(.visible, for: .navigationBar)
      } label: {
        Text(option.title).fontWeight(isSelected ? .semibold : .regular)
      }
      .accessibilityAddTraits(isSelected ? .isSelected : [])
      .designIdentifier(option.identifier)
    } else {
      Button { pick(option.value) } label: {
        HStack {
          Text(option.title)
            .foregroundStyle(option.isDestructive ? MetasequoiaTheme.danger : Color.primary)
            .fontWeight(isSelected ? .semibold : .regular)
          Spacer(minLength: 8)
          if isSelected {
            Image(systemName: "checkmark").font(.system(size: 15, weight: .semibold))
              .foregroundStyle(MetasequoiaTheme.accent)
              .accessibilityHidden(true)
          }
        }
        .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityAddTraits(isSelected ? .isSelected : [])
      .designIdentifier(option.identifier)
    }
  }
}

// MARK: - Toast 提示

/// 全应用共用的 toast，用于轻量反馈（已添加、已复制、已使用…）。需要用户确认的错误仍用 alert。新消息会替换屏幕上的旧消息；每条停留 1.6 秒，并由 VoiceOver 播报。
final class ToastCenter: ObservableObject {
  static let shared = ToastCenter()
  @Published var message: String?
  private var hideWork: DispatchWorkItem?

  func show(_ text: String) {
    guard Thread.isMainThread else {
      DispatchQueue.main.async { self.show(text) }
      return
    }
    hideWork?.cancel()
    withAnimation(.easeInOut(duration: 0.2)) { message = text }
    AccessibilityNotification.Announcement(text).post()
    let work = DispatchWorkItem { [weak self] in
      withAnimation(.easeInOut(duration: 0.2)) { self?.message = nil }
    }
    hideWork = work
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.6, execute: work)
  }
}

/// 在距屏幕底部 110pt 处绘制 `ToastCenter.shared`；用 `.overlay(alignment: .bottom)` 在根视图上挂一次即可。
struct ToastOverlay: View {
  @ObservedObject private var center = ToastCenter.shared

  private static let fill = dynamicColor(light: UIColor(red: 28 / 255, green: 28 / 255, blue: 30 / 255, alpha: 1),
                                         dark: UIColor(red: 242 / 255, green: 242 / 255, blue: 242 / 255, alpha: 1))
  private static let text = dynamicColor(light: .white,
                                         dark: UIColor(red: 17 / 255, green: 17 / 255, blue: 17 / 255, alpha: 1))

  var body: some View {
    GeometryReader { proxy in
      VStack {
        Spacer(minLength: 0)
        if let message = center.message {
          Text(message)
            .font(.system(size: 14))
            .foregroundStyle(Self.text)
            .multilineTextAlignment(.center)
            .lineLimit(3)
            .padding(.vertical, 10)
            .padding(.horizontal, 18)
            .background(Self.fill, in: Capsule())
            .shadow(color: .black.opacity(0.18), radius: 10, y: 6)
            .frame(maxWidth: proxy.size.width * 0.8)
            .padding(.bottom, 110)
            .transition(.opacity)
            .accessibilityIdentifier("toastMessage")
        }
      }
      .frame(maxWidth: .infinity)
    }
    .ignoresSafeArea()
    .allowsHitTesting(false)
  }
}

// MARK: - 分段控件

/// 设计稿的分段控件。`.capsule` 是剪贴板和 AI 设计皮肤的切换器（选中的滑块上是强调色文字）；`.rounded` 是统计页、皮肤页和社区页的切换器（主文字色，着色轨道）。每一项都是一个按钮，id 为 "<identifierPrefix>-<index>"；给了 `itemIdentifier` 时按项的值取 id，列出的项随版本增减时 id 不跟着挪位。
///
/// iOS 26 起两种样式都画成系统分段控件的液态玻璃外形：胶囊轨道是 `glassEffect`，选中的滑块是一枚浮起的胶囊，切换时滑过去，与标签栏、导航栏同一种材质。更早的系统沿用设计稿的平面轨道。
struct DesignSegmentedControl<Value: Hashable>: View {
  enum Style { case capsule, rounded }

  let items: [(title: String, value: Value)]
  @Binding var selection: Value
  var style: Style = .capsule
  let identifierPrefix: String
  let itemIdentifier: ((Value) -> String)?
  @Namespace private var thumbSpace

  init(items: [(title: String, value: Value)], selection: Binding<Value>, style: Style = .capsule, identifierPrefix: String,
       itemIdentifier: ((Value) -> String)? = nil) {
    self.items = items
    _selection = selection
    self.style = style
    self.identifierPrefix = identifierPrefix
    self.itemIdentifier = itemIdentifier
  }

  private func identifier(_ index: Int) -> String {
    itemIdentifier?(items[index].value) ?? "\(identifierPrefix)-\(index)"
  }

  private static var thumbFill: Color {
    dynamicColor(light: .white, dark: UIColor(red: 99 / 255, green: 99 / 255, blue: 102 / 255, alpha: 1))
  }

  private static var roundedTrack: Color {
    Color(uiColor: UIColor { traits in
      traits.userInterfaceStyle == .dark
        ? UIColor(white: 1, alpha: 0.08)
        : MetasequoiaTheme.mixUIColor(9).resolvedColor(with: traits)
    })
  }

  var body: some View {
    if #available(iOS 26, *) {
      HStack(spacing: 0) {
        ForEach(items.indices, id: \.self) { index in glassItem(index) }
      }
      .padding(4)
      .glassEffect(.regular.interactive(), in: Capsule())
      .animation(.spring(response: 0.32, dampingFraction: 0.82), value: selection)
    } else {
      HStack(spacing: 2) {
        ForEach(items.indices, id: \.self) { index in item(index) }
      }
      .padding(2)
      .background {
        switch style {
        case .capsule: Capsule().fill(MetasequoiaTheme.segBg)
        case .rounded: RoundedRectangle(cornerRadius: 9, style: .continuous).fill(Self.roundedTrack)
        }
      }
      .animation(.easeInOut(duration: 0.2), value: selection)
    }
  }

  private func item(_ index: Int) -> some View {
    let item = items[index]
    let isSelected = item.value == selection
    return Button { selection = item.value } label: {
      Text(item.title)
        .font(.system(size: style == .capsule ? 13 : 14, weight: style == .rounded && isSelected ? .semibold : .regular))
        .foregroundStyle(foreground(isSelected))
        .lineLimit(1).minimumScaleFactor(0.8)
        .padding(.vertical, style == .capsule ? 6 : 0)
        .frame(maxWidth: .infinity)
        .frame(height: style == .rounded ? 32 : nil)
        .background { if isSelected { thumb } }
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier(identifier(index))
    .accessibilityAddTraits(isSelected ? .isSelected : [])
  }

  /// iOS 26 的一项：选中的滑块用 `matchedGeometryEffect` 在各项之间滑动，而不是在旧位置淡出、新位置淡入。
  @available(iOS 26, *)
  private func glassItem(_ index: Int) -> some View {
    let item = items[index]
    let isSelected = item.value == selection
    return Button { selection = item.value } label: {
      Text(item.title)
        .font(.system(size: style == .capsule ? 13 : 15, weight: isSelected ? .semibold : .medium))
        .foregroundStyle(foreground(isSelected))
        .lineLimit(1).minimumScaleFactor(0.8)
        .frame(maxWidth: .infinity)
        .frame(height: style == .capsule ? 28 : 34)
        .background {
          if isSelected {
            Capsule().fill(Self.thumbFill)
              .shadow(color: .black.opacity(0.12), radius: 6, y: 2)
              .matchedGeometryEffect(id: "thumb", in: thumbSpace)
          }
        }
        .contentShape(Capsule())
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier(identifier(index))
    .accessibilityAddTraits(isSelected ? .isSelected : [])
  }

  @ViewBuilder private var thumb: some View {
    switch style {
    case .capsule:
      Capsule().fill(Self.thumbFill).shadow(color: .black.opacity(0.12), radius: 1.5, y: 1)
    case .rounded:
      RoundedRectangle(cornerRadius: 7, style: .continuous).fill(Self.thumbFill).shadow(color: .black.opacity(0.12), radius: 1.5, y: 1)
    }
  }

  private func foreground(_ isSelected: Bool) -> Color {
    switch style {
    case .capsule: return isSelected ? MetasequoiaTheme.accent : MetasequoiaTheme.sub
    case .rounded: return .primary
    }
  }
}

// MARK: - 固定在页面顶部的分段

extension View {
  /// 皮肤页和社区页标题下固定的一排分段。iOS 26 起放进 `safeAreaBar`：分段浮在内容上，滚到它下面的内容与导航栏一样按滚动边缘效果渐隐，不再垫一条实色底把玻璃导航栏和内容切开。更早的系统仍放在安全区内边距里，垫页面色；背景限制在自己的 frame 内，颜色默认会延伸进安全区盖住大标题。两种方式里列表都仍是页面的滚动视图，大标题照样随滚动收进导航栏。
  @ViewBuilder func designPinnedSegments<Bar: View>(@ViewBuilder _ bar: () -> Bar) -> some View {
    if #available(iOS 26, *) {
      safeAreaBar(edge: .top, spacing: 0) { bar() }
    } else {
      safeAreaInset(edge: .top, spacing: 0) { bar().background(MetasequoiaTheme.canvas, ignoresSafeAreaEdges: []) }
    }
  }
}

extension View {
  /// 页面底部固定的操作栏（AI 设计皮肤的「生成皮肤」等）。iOS 26 起放进 `safeAreaBar`，按钮浮在内容上，滚到下面的内容按滚动边缘效果渐隐；更早的系统放在安全区内边距里，由操作栏自己画分隔线和底色。
  @ViewBuilder func designActionBar<Bar: View>(@ViewBuilder _ bar: () -> Bar) -> some View {
    if #available(iOS 26, *) {
      safeAreaBar(edge: .bottom, spacing: 0) { bar() }
    } else {
      safeAreaInset(edge: .bottom, spacing: 0) { bar() }
    }
  }

  /// 搜索框的外形，设置首页和社区列表共用：44pt 高的胶囊。iOS 26 起是可交互的玻璃胶囊，与分段控件、标签栏同一种材质；更早的系统填设计稿的分段底色。
  @ViewBuilder func designSearchFieldBackground() -> some View {
    if #available(iOS 26, *) {
      padding(.horizontal, 14).frame(minHeight: 44).glassEffect(.regular.interactive(), in: Capsule())
    } else {
      padding(.horizontal, 14).frame(minHeight: 44).background(MetasequoiaTheme.segBg, in: Capsule())
    }
  }
}

// MARK: - List 与 Form 页面

extension View {
  /// 供仍用 `List` 或 `Form` 的页面使用：行后面是季节页面色，分区之间相隔 28pt，行最小高度 52pt。
  func designPage() -> some View {
    scrollContentBackground(.hidden)
      .background(MetasequoiaTheme.canvas.ignoresSafeArea())
      .listSectionSpacing(28)
      .environment(\.defaultMinListRowHeight, 52)
  }

  /// 季节卡片色背景、带季节分隔线的 `List` 或 `Form` 行。
  func designRow() -> some View {
    listRowBackground(MetasequoiaTheme.surface)
      .listRowSeparatorTint(MetasequoiaTheme.hair)
  }
}
