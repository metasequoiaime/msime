import UIKit

struct KeyboardCandidateAnnotation: Equatable {
  let text: String
  let accessibilityDescription: String

  static let none = KeyboardCandidateAnnotation(text: "", accessibilityDescription: "")
}

extension UIColor {
  /// 按下态表面的 CSS `filter: brightness(factor)`：每个颜色通道乘以 `factor`，alpha 不变，并针对每种 trait 重新解析，动态主题色因此仍是动态的。
  func keyboardPressedBrightness(_ factor: CGFloat) -> UIColor {
    UIColor { traits in
      var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
      self.resolvedColor(with: traits).getRed(&red, green: &green, blue: &blue, alpha: &alpha)
      return UIColor(red: red * factor, green: green * factor, blue: blue * factor, alpha: alpha)
    }
  }
}

// 引擎返回的全部候选，排成网格。
//
// 候选条一次显示九个，箭头一次翻九个，所以像 `yi` 这样返回 351 个候选的查询，末尾要点三十九下才能到。没人会翻到那里，看起来就像词库里没有这个词。这里改为一次显示整个列表。
//
// 网格就是设计稿的 `repeat(auto-fill, minmax(64px, 1fr))`，间隔 6pt、内边距 2pt：在不窄于 64pt 的前提下放尽可能多的等宽列，44pt 的格子画得像按键。比一列宽的候选按自身文字所需占多列，带注释的候选至少占两列，让翻译或标记行保持可读（Android 出于同样原因把这个列表排成换行布局）。占几列只取决于候选本身，与注释文字无关，所以晚到的注释不会改变页面尺寸。
//
// 带标题栏时（默认）面板独自盖住整个键盘：顶部是品牌标志、拼写、候选数和收起箭头。不带标题栏时（`showsHeader: false`）面板位于候选条下方的按键区，拼写和收起箭头由候选条保留，底部改为一行「返回」和 ⌫，代替标题栏。
//
// 传入 `columns` 时是全拼九键的三栏面板：左边拼音栏、中间候选网格、右边功能键，同样位于按键区；读音留在上面的候选栏里，所以不画标题栏，返回和退格在右栏里，也不画底栏。
final class KeyboardCandidatePanelView: UIView {
  static let minimumCellWidth: CGFloat = 64
  static let cellSpacing: CGFloat = 6
  static let cellHeight: CGFloat = 44
  static let gridPadding: CGFloat = 2
  /// 无标题栏面板底部按键的高度，以及它们上方的间隔。
  static let footerHeight: CGFloat = 40
  static let footerGap: CGFloat = 8
  /// 带注释的候选至少占的列数。
  private static let annotatedSpan = 2
  private static let cellInsets = NSDirectionalEdgeInsets(top: 6, leading: 8, bottom: 6, trailing: 8)

  private var candidates: [String]
  private let candidateScale: CGFloat
  private let candidateFamilies: [String]
  private var annotations: [KeyboardCandidateAnnotation]
  private var markers: [[CandidateMarker]]
  private let display: (String) -> String
  private let onSelect: (Int) -> Void
  /// 长按候选块时提供的菜单。与候选条共用：在候选条里能管理词条的长按，展开列表后不应失效。
  private let menuElements: (Int) -> [UIMenuElement]
  /// 以选中样式绘制的候选（`accentSoft` 底、强调色半粗体文字），即候选条中高亮的那个，默认是第一个。
  private let selectedIndex: Int
  private let showsHeader: Bool
  private let skin = KeyboardTheme.current
  private let scrollView = UIScrollView()
  private var chips: [CandidateGridButton] = []
  private let count = UILabel()
  private var laidOutWidth: CGFloat = 0

  /// 宽 `width` 的网格有几列：CSS auto-fill 的列数，即按 64pt 列宽、6pt 间隔能放下几列，至少一列。
  static func columns(width: CGFloat) -> Int {
    max(1, Int((width + cellSpacing) / (minimumCellWidth + cellSpacing)))
  }

  /// 一个候选占几列：足够容纳 `titleWidth`（第一行宽度加上格子内边距），带注释时至少两列，且不超过一行的总列数。
  static func span(titleWidth: CGFloat, annotated: Bool, columns: Int, columnWidth: CGFloat) -> Int {
    let needed = columnWidth > 0 ? Int(((titleWidth + cellSpacing) / (columnWidth + cellSpacing)).rounded(.up)) : 1
    return min(columns, max(needed, annotated ? annotatedSpan : 1, 1))
  }

  init(candidates: [String], preedit: String, annotations: [KeyboardCandidateAnnotation] = [],
       markers: [[CandidateMarker]] = [],
       candidateScale: CGFloat = 1, preeditScale: CGFloat = 1, candidateFamilies: [String] = [],
       selectedIndex: Int = 0, showsHeader: Bool = true,
       display: @escaping (String) -> String,
       menuElements: @escaping (Int) -> [UIMenuElement] = { _ in [] },
       onSelect: @escaping (Int) -> Void, onClose: @escaping () -> Void,
       onBackspace: (() -> Void)? = nil,
       columns: (leading: UIView, trailing: UIView)? = nil) {
    self.candidates = candidates
    self.candidateScale = candidateScale
    self.candidateFamilies = candidateFamilies
    self.annotations = annotations
    self.markers = markers
    self.display = display
    self.menuElements = menuElements
    self.onSelect = onSelect
    self.selectedIndex = selectedIndex
    self.showsHeader = showsHeader
    super.init(frame: .zero)
    accessibilityIdentifier = "candidatePanel"
    // 在按键区里透出键盘自己的背景，与它所替代的按键下方一样。
    backgroundColor = showsHeader && columns == nil ? skin.keyBackground.withAlphaComponent(0.98) : .clear

    scrollView.translatesAutoresizingMaskIntoConstraints = false
    scrollView.alwaysBounceVertical = false
    scrollView.disableEdgeEffects()
    scrollView.contentInsetAdjustmentBehavior = .never
    addSubview(scrollView)
    if let columns {
      // 两栏各占面板宽度的 15%，贴着按键区的左右边；面板底色透明，留给后面的键盘皮肤，被盖住的按键由键盘控制器藏起来（`installPanel`）。
      for column in [columns.leading, columns.trailing] {
        column.translatesAutoresizingMaskIntoConstraints = false
        addSubview(column)
        NSLayoutConstraint.activate([
          column.topAnchor.constraint(equalTo: topAnchor),
          column.bottomAnchor.constraint(equalTo: bottomAnchor),
          column.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.15),
        ])
      }
      NSLayoutConstraint.activate([
        columns.leading.leadingAnchor.constraint(equalTo: leadingAnchor),
        columns.trailing.trailingAnchor.constraint(equalTo: trailingAnchor),
        scrollView.leadingAnchor.constraint(equalTo: columns.leading.trailingAnchor, constant: Self.cellSpacing),
        scrollView.trailingAnchor.constraint(equalTo: columns.trailing.leadingAnchor, constant: -Self.cellSpacing),
        scrollView.topAnchor.constraint(equalTo: topAnchor),
        scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
      ])
      rebuildChips()
      return
    }
    NSLayoutConstraint.activate([
      scrollView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: showsHeader ? 12 : 0),
      scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: showsHeader ? -12 : 0),
    ])
    if showsHeader {
      let header = makeHeader(preedit: preedit, preeditScale: preeditScale, onClose: onClose)
      NSLayoutConstraint.activate([
        scrollView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 4),
        scrollView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
      ])
    } else {
      let footer = makeFooter(onClose: onClose, onBackspace: onBackspace)
      NSLayoutConstraint.activate([
        scrollView.topAnchor.constraint(equalTo: topAnchor),
        scrollView.bottomAnchor.constraint(equalTo: footer.topAnchor, constant: -Self.footerGap),
      ])
    }
    rebuildChips()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  // 品牌标志位于标题栏开头，与 macOS 候选窗顶行的做法一样：16pt，与拼写之间隔 6pt。它只是装饰（图片视图不接收触摸，也不是无障碍元素），并像快捷栏的品牌按钮一样用皮肤强调色着色。
  private func makeHeader(preedit: String, preeditScale: CGFloat, onClose: @escaping () -> Void) -> UIView {
    let brandMark = UIImageView(
      image: KeyboardViewController.brandTemplate()
        ?? UIImage(systemName: "leaf.fill")?.withRenderingMode(.alwaysTemplate))
    brandMark.contentMode = .scaleAspectFit
    brandMark.tintColor = skin.accent
    brandMark.isUserInteractionEnabled = false
    brandMark.isAccessibilityElement = false
    brandMark.accessibilityIdentifier = "candidatePanelBrandIcon"
    brandMark.translatesAutoresizingMaskIntoConstraints = false
    brandMark.setContentHuggingPriority(.required, for: .horizontal)
    brandMark.setContentCompressionResistancePriority(.required, for: .horizontal)

    let spelling = UILabel()
    spelling.text = preedit
    spelling.font = CandidateFontPreference.font(.subheadline, scale: preeditScale)
    spelling.adjustsFontForContentSizeCategory = true
    spelling.textColor = skin.accent
    spelling.accessibilityIdentifier = "candidatePanelSpelling"

    count.text = "\(candidates.count) 个候选"
    count.font = .preferredFont(forTextStyle: .footnote)
    count.adjustsFontForContentSizeCategory = true
    count.textColor = skin.keyForeground.withAlphaComponent(0.6)

    var closeConfiguration = UIButton.Configuration.plain()
    closeConfiguration.image = UIImage(systemName: "chevron.up")
    closeConfiguration.baseForegroundColor = skin.keyForeground
    let close = UIButton(
      configuration: closeConfiguration,
      primaryAction: UIAction { _ in onClose() })
    close.accessibilityIdentifier = "closeCandidatePanel"
    close.accessibilityLabel = "收起候选"
    close.setContentHuggingPriority(.required, for: .horizontal)

    let header = UIStackView(arrangedSubviews: [brandMark, spelling, count, UIView(), close])
    header.axis = .horizontal
    header.alignment = .center
    header.spacing = 8
    header.setCustomSpacing(6, after: brandMark)
    header.translatesAutoresizingMaskIntoConstraints = false
    addSubview(header)
    NSLayoutConstraint.activate([
      header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
      header.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
      header.topAnchor.constraint(equalTo: topAnchor, constant: 6),
      header.heightAnchor.constraint(equalToConstant: 32),
      brandMark.widthAnchor.constraint(equalToConstant: 16),
      brandMark.heightAnchor.constraint(equalToConstant: 16),
    ])
    return header
  }

  /// 「返回」和 ⌫ 各占一半宽度，高 40pt，用功能键底色。「返回」沿用标题栏收起按钮的标识符，因为是同一个操作；没有处理方时不放 ⌫。
  private func makeFooter(onClose: @escaping () -> Void, onBackspace: (() -> Void)?) -> UIView {
    let back = footerKey(id: "closeCandidatePanel", label: "收起候选") { onClose() }
    var backConfiguration = back.configuration
    backConfiguration?.attributedTitle = AttributedString("返回", attributes: AttributeContainer([
      .font: UIFont.systemFont(ofSize: 15, weight: .medium),
    ]))
    back.configuration = backConfiguration
    var keys: [UIView] = [back]
    if let onBackspace {
      let delete = footerKey(id: "candidatePanelBackspace", label: "删除") { onBackspace() }
      var deleteConfiguration = delete.configuration
      deleteConfiguration?.image = KeyboardIcon.backspace.image(pointSize: 22)
      delete.configuration = deleteConfiguration
      keys.append(delete)
    }
    let footer = UIStackView(arrangedSubviews: keys)
    footer.axis = .horizontal
    footer.distribution = .fillEqually
    footer.spacing = Self.cellSpacing
    footer.translatesAutoresizingMaskIntoConstraints = false
    addSubview(footer)
    NSLayoutConstraint.activate([
      footer.leadingAnchor.constraint(equalTo: leadingAnchor),
      footer.trailingAnchor.constraint(equalTo: trailingAnchor),
      footer.bottomAnchor.constraint(equalTo: bottomAnchor),
      footer.heightAnchor.constraint(equalToConstant: Self.footerHeight),
    ])
    return footer
  }

  private func footerKey(id: String, label: String, action: @escaping () -> Void) -> KeyboardKeyButton {
    var configuration = UIButton.Configuration.plain()
    configuration.baseForegroundColor = skin.keyForeground
    configuration.background.backgroundColor = skin.functionKeyBackground
    configuration.background.cornerRadius = skin.cornerRadius
    configuration.contentInsets = .zero
    let key = KeyboardKeyButton(configuration: configuration, primaryAction: UIAction { _ in action() })
    key.isFunctionKey = true
    key.accessibilityIdentifier = id
    key.accessibilityLabel = label
    return key
  }

  func updateAnnotations(_ annotations: [KeyboardCandidateAnnotation]) {
    self.annotations = annotations
    rebuildChips()
    // 候选块刚被替换，重新出现的那些还没有 frame。主动请求一次布局，而不是等下一次碰巧让面板变脏的操作。
    setNeedsLayout()
  }

  /// 换成另一代候选（九键面板里选拼音、退格或筛选之后），面板不关，回到列表开头。
  func reload(candidates: [String], annotations: [KeyboardCandidateAnnotation], markers: [[CandidateMarker]]) {
    self.candidates = candidates
    self.annotations = annotations
    self.markers = markers
    count.text = "\(candidates.count) 个候选"
    scrollView.setContentOffset(.zero, animated: false)
    rebuildChips()
    setNeedsLayout()
  }

  // 格子要在已知宽度下摆放，所以在这里定位；候选块本身每组注释只构建一次。
  override func layoutSubviews() {
    super.layoutSubviews()
    let available = scrollView.bounds.width
    guard available > 0, available != laidOutWidth else { return }
    laidOutWidth = available
    layoutGrid(width: available)
  }

  private func rebuildChips() {
    for chip in chips { chip.removeFromSuperview() }
    chips = candidates.enumerated().map { makeChip(candidate: $1, index: $0) }
    for chip in chips { scrollView.addSubview(chip) }
    if laidOutWidth > 0 { layoutGrid(width: laidOutWidth) }
  }

  private func layoutGrid(width: CGFloat) {
    let padding = Self.gridPadding, spacing = Self.cellSpacing
    let content = max(0, width - padding * 2)
    let columns = Self.columns(width: content)
    let columnWidth = max(0, (content - spacing * CGFloat(columns - 1)) / CGFloat(columns))
    // 逐行排列：一行剩余空间放不下的候选从下一行开始，与不开启 dense 填充的 CSS 网格一样。
    var rows: [[(chip: CandidateGridButton, column: Int, span: Int)]] = [[]]
    var column = 0
    for chip in chips {
      let span = Self.span(titleWidth: chip.titleWidth, annotated: chip.glossLines > 0,
                           columns: columns, columnWidth: columnWidth)
      if column > 0, column + span > columns {
        rows.append([])
        column = 0
      }
      rows[rows.count - 1].append((chip, column, span))
      column += span
    }
    var y = padding
    for row in rows where !row.isEmpty {
      let height = row.map { cellHeight(for: $0.chip, width: cellWidth(span: $0.span, columnWidth: columnWidth)) }.max()
        ?? Self.cellHeight
      for cell in row {
        cell.chip.frame = CGRect(x: padding + CGFloat(cell.column) * (columnWidth + spacing), y: y,
                                 width: cellWidth(span: cell.span, columnWidth: columnWidth), height: height)
      }
      y += height + spacing
    }
    scrollView.contentSize = CGSize(width: width, height: max(0, y - spacing) + padding)
  }

  private func cellWidth(span: Int, columnWidth: CGFloat) -> CGFloat {
    columnWidth * CGFloat(span) + Self.cellSpacing * CGFloat(span - 1)
  }

  /// 44pt，或者标题加注释行需要更高时取所需高度：一页中带注释的候选块无论注释到没到，高度都一致。
  private func cellHeight(for chip: CandidateGridButton, width: CGFloat) -> CGFloat {
    let insets = Self.cellInsets.top + Self.cellInsets.bottom
    let lines = chip.titleLineHeight + CGFloat(chip.glossLines) * UIFont.preferredFont(forTextStyle: .caption2).lineHeight
    let measured = chip.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
    return ceil(max(Self.cellHeight, insets + lines + 2, chip.glossLines > 0 ? measured : 0))
  }

  private func makeChip(candidate: String, index: Int) -> CandidateGridButton {
    let number = index + 1
    let text = display(candidate)
    let annotation = annotations.indices.contains(index) ? annotations[index] : .none
    let marks = markers.indices.contains(index) ? markers[index] : []
    let selected = index == selectedIndex
    let secondary = skin.secondary
    let paragraph = NSMutableParagraphStyle()
    paragraph.lineBreakMode = .byTruncatingTail
    paragraph.alignment = .center
    var font = CandidateFontPreference.font(.body, scale: candidateScale, families: candidateFamilies)
    if selected { font = Self.semibold(font) }
    var title = AttributedString(text, attributes: AttributeContainer([.font: font, .paragraphStyle: paragraph]))
    title += KeyboardViewController.markerRun(marks, color: secondary, scale: candidateScale)
    let titleWidth = ceil(NSAttributedString(title).size().width) + Self.cellInsets.leading + Self.cellInsets.trailing
    let glossLines = annotation.text.isEmpty
      ? 0 : annotation.text.split(separator: "\n", omittingEmptySubsequences: false).count
    if glossLines > 0 {
      for line in annotation.text.split(separator: "\n", omittingEmptySubsequences: false) {
        title += AttributedString("\n" + String(line), attributes: AttributeContainer([
          .font: UIFont.preferredFont(forTextStyle: .caption2), .paragraphStyle: paragraph,
          .foregroundColor: secondary,
        ]))
      }
    }
    var configuration = UIButton.Configuration.plain()
    configuration.attributedTitle = title
    configuration.baseForegroundColor = selected || marks.contains { $0.symbol == "pin.fill" }
      ? skin.accent : skin.keyForeground
    configuration.contentInsets = Self.cellInsets
    configuration.background.cornerRadius = skin.cornerRadius
    let fill = selected ? skin.accentSoft : skin.keyBackground
    configuration.background.backgroundColor = fill
    let chip = CandidateGridButton(
      configuration: configuration,
      primaryAction: UIAction { [weak self] _ in self?.onSelect(index) })
    chip.configurationUpdateHandler = { button in
      button.configuration?.background.backgroundColor = button.isHighlighted ? fill.keyboardPressedBrightness(0.88) : fill
    }
    chip.titleWidth = titleWidth
    chip.titleLineHeight = font.lineHeight
    chip.glossLines = glossLines
    chip.titleLineCount = 1 + glossLines
    // 选中的格子平铺在它的底色上；其余格子都画得像按键，包括阴影。
    chip.drawsKeyShadow = !selected && skin.hasShadow
    chip.accessibilityIdentifier = "panelCandidate-\(number)"
    chip.accessibilityLabel = annotation.accessibilityDescription.isEmpty
      ? "候选词 \(number)：\(text)"
      : "候选词 \(number)：\(text)，\(annotation.accessibilityDescription)"
    for marker in marks { chip.accessibilityLabel? += "，\(marker.spoken)" }
    if selected { chip.accessibilityTraits.insert(.selected) }
    // 在长按打开菜单时才构建，而不是一开始就给每个候选块建好：这个面板会排出整个列表，像 `yi` 这样的查询有几百个。
    chip.menu = UIMenu(children: [
      UIDeferredMenuElement.uncached { [weak self] completion in
        completion(self?.menuElements(index) ?? [])
      }
    ])
    return chip
  }

  private static func semibold(_ font: UIFont) -> UIFont {
    let descriptor = font.fontDescriptor.addingAttributes([
      .traits: [UIFontDescriptor.TraitKey.weight: UIFont.Weight.semibold.rawValue],
    ])
    return UIFont(descriptor: descriptor, size: font.pointSize)
  }
}

/// 网格中的一个格子：类似按键的按钮，标题保持行数，阴影跟随键盘的按键阴影。
private final class CandidateGridButton: UIButton {
  /// 第一行加上格子内边距后的宽度，决定占几列。
  var titleWidth: CGFloat = 0
  var titleLineHeight: CGFloat = 0
  var glossLines = 0
  var drawsKeyShadow = false {
    didSet { updateShadow() }
  }

  /// 刚设置 configuration 后立即给 `titleLabel?.numberOfLines` 赋值不会生效：UIKit 在套用 configuration 时会重建标题标签，所以每次布局都重新设置这个上限（与 `KeyboardKeyButton.titleLineCount` 的做法相同）。
  var titleLineCount = 1 {
    didSet { setNeedsLayout() }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: CandidateGridButton, _: UITraitCollection) in
      button.updateShadow()
    }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  private func updateShadow() {
    let skin = KeyboardTheme.current
    layer.shadowOpacity = drawsKeyShadow ? 1 : 0
    layer.shadowColor = skin.keyShadowColor.resolvedColor(with: traitCollection).cgColor
    layer.shadowRadius = skin.shadowRadius
    layer.shadowOffset = CGSize(width: 0, height: skin.shadowOffset)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    titleLabel?.numberOfLines = titleLineCount
    if layer.shadowOpacity > 0 {
      layer.shadowPath = UIBezierPath(roundedRect: bounds, cornerRadius: KeyboardTheme.current.cornerRadius).cgPath
    }
  }
}
