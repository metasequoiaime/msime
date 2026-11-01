import UIKit

struct KeyboardCandidateAnnotation: Equatable {
  let text: String
  let accessibilityDescription: String

  static let none = KeyboardCandidateAnnotation(text: "", accessibilityDescription: "")

  /// 坐了首选的双拼纠错（`confident_correction`）在候选条和展开网格里的读屏标签都以它结尾，Android 与鸿蒙念同一句。
  static let confidentCorrectionSpoken = "，已纠正"
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
// 带标题栏时（默认）面板独自盖住整个键盘：顶部是品牌标志、拼写、候选数和收起箭头。不带标题栏时（`showsHeader: false`）面板用满候选条下方的按键区，拼写和收起箭头由候选条保留，不另放底部按钮。
//
// 传入 `columns` 时是全拼九键的三栏面板：左边拼音栏、中间候选网格、右边功能键，同样位于按键区；读音留在上面的候选栏里，所以不画标题栏，返回和退格在右栏里，也不画底栏。
final class KeyboardCandidatePanelView: CandidateCanvasHostView, UICollectionViewDataSource {
  static let minimumCellWidth: CGFloat = 64
  static let cellSpacing: CGFloat = 6
  static let cellHeight: CGFloat = 44
  static let gridPadding: CGFloat = 2
  /// 带注释的候选至少占的列数。
  private static let annotatedSpan = 2
  private static let cellInsets = NSDirectionalEdgeInsets(top: 6, leading: 8, bottom: 6, trailing: 8)

  private var candidates: [String]
  private let candidateScale: CGFloat
  private let candidateFamilies: [String]
  private var annotations: [KeyboardCandidateAnnotation]
  private var markers: [[CandidateMarker]]
  /// 在线占位行（AI 的「…」或云的「☁」）的下标：画成次要色，读屏念状态，不当成真候选（#6888）。
  private var aiPlaceholders: Set<Int>
  /// 坐了首选的双拼纠错（`confident_correction`）的下标：字和纠错注释用强调色，读屏在末尾念「已纠正」。
  private var confidentCorrections: Set<Int>
  private let display: (String) -> String
  private let onSelect: (Int) -> Void
  /// 长按候选块时提供的菜单。与候选条共用：在候选条里能管理词条的长按，展开列表后不应失效。
  private let menuElements: (Int) -> [UIMenuElement]
  /// 以选中样式绘制的候选（`accentSoft` 底、强调色半粗体文字），即候选条中高亮的那个，默认是第一个。
  private let selectedIndex: Int
  private let showsHeader: Bool
  private var skin: KeyboardTheme { KeyboardTheme.current }
  private var palette: CandidatePalette?
  private let gridLayout = CandidateGridLayout()
  private lazy var scrollView = UICollectionView(frame: .zero, collectionViewLayout: gridLayout)
  private let measurementChip = CandidateGridButton(frame: .zero)
  private let count = UILabel()
  private var laidOutWidth: CGFloat = 0
  private var pendingScrollOffset: CGPoint?
  private var headerGap: NSLayoutConstraint?

  /// 宽 `width` 的网格有几列：CSS auto-fill 的列数，即按 64pt 列宽、6pt 间隔能放下几列，至少一列。
  static func columns(width: CGFloat) -> Int {
    max(1, Int((width + cellSpacing) / (minimumCellWidth + cellSpacing)))
  }

  /// 一个候选占几列：足够容纳 `titleWidth`（第一行宽度加上格子内边距），带注释时至少两列，且不超过一行的总列数。
  static func span(titleWidth: CGFloat, annotated: Bool, columns: Int, columnWidth: CGFloat) -> Int {
    let needed = columnWidth > 0 ? Int(((titleWidth + cellSpacing) / (columnWidth + cellSpacing)).rounded(.up)) : 1
    return min(columns, max(needed, annotated ? annotatedSpan : 1, 1))
  }

  /// 按顺序把占 `spans` 列的格子装进 `columns` 列的各行，返回每格的行、起始列和实际占的列数。一行剩下的列放不下下一格时换行，与不开启 dense 填充的 CSS 网格一样；换行前那一格拉宽到行尾，网格右边不留空。最后一行不拉宽，一两个候选不会被撑成一整条。Android（`CandidateGridPolicy.place`）与鸿蒙（`ExpandedCandidateLayout.rows`）是同一条规则。
  static func place(spans: [Int], columns: Int) -> [(row: Int, column: Int, span: Int)] {
    var cells: [(row: Int, column: Int, span: Int)] = []
    var row = 0, column = 0
    for span in spans {
      if column > 0, column + span > columns {
        if let last = cells.indices.last { cells[last].span = columns - cells[last].column }
        row += 1
        column = 0
      }
      cells.append((row, column, span))
      column += span
    }
    return cells
  }

  init(candidates: [String], preedit: String, annotations: [KeyboardCandidateAnnotation] = [],
       markers: [[CandidateMarker]] = [], aiPlaceholders: Set<Int> = [], confidentCorrections: Set<Int> = [],
       candidateScale: CGFloat = 1, preeditScale: CGFloat = 1, candidateFamilies: [String] = [],
       selectedIndex: Int = 0, showsHeader: Bool = true, palette: CandidatePalette? = nil,
       canvas: ExternalCandidateSkin.Canvas? = nil,
       display: @escaping (String) -> String,
       menuElements: @escaping (Int) -> [UIMenuElement] = { _ in [] },
       onSelect: @escaping (Int) -> Void, onClose: @escaping () -> Void,
       columns: (leading: UIView, trailing: UIView)? = nil) {
    self.candidates = candidates
    self.candidateScale = candidateScale
    self.candidateFamilies = candidateFamilies
    self.annotations = annotations
    self.markers = markers
    self.aiPlaceholders = aiPlaceholders
    self.confidentCorrections = confidentCorrections
    self.display = display
    self.menuElements = menuElements
    self.onSelect = onSelect
    self.selectedIndex = selectedIndex
    self.showsHeader = showsHeader
    self.palette = palette
    super.init(frame: .zero)
    self.canvas = canvas
    minimumContentHeight = Self.cellHeight
    accessibilityIdentifier = "candidatePanel"
    // 在按键区里透出键盘自己的背景，与它所替代的按键下方一样。
    backgroundColor = canvas?.panelLoaded == true ? .clear : palette?.surface
      ?? (showsHeader && columns == nil ? skin.keyBackground.withAlphaComponent(0.98) : .clear)

    scrollView.translatesAutoresizingMaskIntoConstraints = false
    scrollView.alwaysBounceVertical = false
    scrollView.backgroundColor = .clear
    scrollView.dataSource = self
    scrollView.register(CandidateGridCell.self, forCellWithReuseIdentifier: "candidate")
    scrollView.disableEdgeEffects()
    scrollView.contentInsetAdjustmentBehavior = .never
    addSubview(scrollView)
    if let columns {
      // 两栏各占面板宽度的 15%，贴着按键区的左右边；面板底色透明，留给后面的键盘皮肤，被盖住的按键由键盘控制器藏起来（`installPanel`）。
      for column in [columns.leading, columns.trailing] {
        column.translatesAutoresizingMaskIntoConstraints = false
        addSubview(column)
        NSLayoutConstraint.activate([
          column.topAnchor.constraint(equalTo: candidateContentGuide.topAnchor),
          column.bottomAnchor.constraint(equalTo: candidateContentGuide.bottomAnchor),
          column.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.15),
        ])
      }
      NSLayoutConstraint.activate([
        columns.leading.leadingAnchor.constraint(equalTo: candidateContentGuide.leadingAnchor),
        columns.trailing.trailingAnchor.constraint(equalTo: candidateContentGuide.trailingAnchor),
        scrollView.leadingAnchor.constraint(equalTo: columns.leading.trailingAnchor, constant: Self.cellSpacing),
        scrollView.trailingAnchor.constraint(equalTo: columns.trailing.leadingAnchor, constant: -Self.cellSpacing),
        scrollView.topAnchor.constraint(equalTo: candidateContentGuide.topAnchor),
        scrollView.bottomAnchor.constraint(equalTo: candidateContentGuide.bottomAnchor),
      ])
      rebuildChips()
      return
    }
    NSLayoutConstraint.activate([
      scrollView.leadingAnchor.constraint(equalTo: candidateContentGuide.leadingAnchor, constant: showsHeader ? 12 : 0),
      scrollView.trailingAnchor.constraint(equalTo: candidateContentGuide.trailingAnchor, constant: showsHeader ? -12 : 0),
    ])
    if showsHeader {
      let header = makeHeader(preedit: preedit, preeditScale: preeditScale, onClose: onClose)
      let gap = scrollView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: canvas?.lineGap ?? 4)
      headerGap = gap
      NSLayoutConstraint.activate([
        gap,
        scrollView.bottomAnchor.constraint(equalTo: candidateContentGuide.bottomAnchor, constant: -8),
      ])
    } else {
      NSLayoutConstraint.activate([
        scrollView.topAnchor.constraint(equalTo: candidateContentGuide.topAnchor, constant: 0),
        scrollView.bottomAnchor.constraint(equalTo: candidateContentGuide.bottomAnchor, constant: 0),
      ])
    }
    rebuildChips()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  // 品牌标志位于标题栏开头，与 macOS 候选窗顶行的做法一样：16pt，与拼写之间隔 6pt。它只是装饰（图片视图不接收触摸，也不是无障碍元素），和快捷栏的品牌按钮一样是固定配色的完整标志，不随皮肤着色。
  private func makeHeader(preedit: String, preeditScale: CGFloat, onClose: @escaping () -> Void) -> UIView {
    let brandMark = UIImageView(image: KeyboardViewController.brandMark(side: 16))
    brandMark.contentMode = .scaleAspectFit
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
      header.leadingAnchor.constraint(equalTo: candidateContentGuide.leadingAnchor, constant: 12),
      header.trailingAnchor.constraint(equalTo: candidateContentGuide.trailingAnchor, constant: -12),
      header.topAnchor.constraint(equalTo: candidateContentGuide.topAnchor, constant: 6),
      header.heightAnchor.constraint(equalToConstant: 32),
      brandMark.widthAnchor.constraint(equalToConstant: 16),
      brandMark.heightAnchor.constraint(equalToConstant: 16),
    ])
    return header
  }

  func applyAppearance(palette: CandidatePalette?, canvas: ExternalCandidateSkin.Canvas?) {
    self.palette = palette
    self.canvas = canvas
    headerGap?.constant = canvas?.lineGap ?? 4
    backgroundColor = canvas?.panelLoaded == true ? .clear : palette?.surface ?? (showsHeader ? skin.keyBackground : .clear)
    rebuildChips()
    setNeedsLayout()
  }

  func updateAnnotations(_ annotations: [KeyboardCandidateAnnotation]) {
    self.annotations = annotations
    rebuildChips()
    // 候选块刚被替换，重新出现的那些还没有 frame。主动请求一次布局，而不是等下一次碰巧让面板变脏的操作。
    setNeedsLayout()
  }

  /// 换成另一代候选，面板不关。九键面板里选拼音、退格或筛选之后回到列表开头；只是 AI 行变了（回答插进第一页最后一格、多出来的接在末尾）时 `keepingScrollPosition` 留在原处，不跳回开头（#6888）。
  func reload(candidates: [String], annotations: [KeyboardCandidateAnnotation], markers: [[CandidateMarker]],
              aiPlaceholders: Set<Int> = [], confidentCorrections: Set<Int> = [], keepingScrollPosition: Bool = false) {
    self.candidates = candidates
    self.annotations = annotations
    self.markers = markers
    self.aiPlaceholders = aiPlaceholders
    self.confidentCorrections = confidentCorrections
    count.text = "\(candidates.count) 个候选"
    if !keepingScrollPosition { scrollView.setContentOffset(.zero, animated: false) }
    rebuildChips()
    setNeedsLayout()
  }

  // 布局只保存几何；UICollectionView 负责可见按钮、滚动、菜单和无障碍的生命周期。
  override func layoutSubviews() {
    super.layoutSubviews()
    let available = scrollView.bounds.width
    if available > 0, available != laidOutWidth {
      laidOutWidth = available
      layoutGrid(width: available)
    }
    scrollView.layoutIfNeeded()
  }

  private func rebuildChips() {
    pendingScrollOffset = scrollView.contentOffset
    laidOutWidth = 0
    gridLayout.replace(frames: [], size: .zero)
    scrollView.reloadData()
    setNeedsLayout()
  }

  func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
    candidates.count
  }

  func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
    let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "candidate", for: indexPath) as! CandidateGridCell
    _ = makeChip(candidate: candidates[indexPath.item], index: indexPath.item, reusing: cell.chip)
    return cell
  }

  private func layoutGrid(width: CGFloat) {
    let padding = Self.gridPadding
    let spacing = palette?.layout?.itemGap?.dip(fontSize: CandidateFontPreference.font(.body, scale: candidateScale).pointSize) ?? Self.cellSpacing
    let content = max(0, width - padding * 2)
    let columns = Self.columns(width: content)
    let columnWidth = max(0, (content - spacing * CGFloat(columns - 1)) / CGFloat(columns))
    let spans = candidates.indices.map { index in
      let chip = makeChip(candidate: candidates[index], index: index, reusing: measurementChip, interactive: false)
      return Self.span(titleWidth: chip.titleWidth, annotated: chip.glossLines > 0,
                       columns: columns, columnWidth: columnWidth)
    }
    let cells = Self.place(spans: spans, columns: columns)
    var frames = [CGRect](repeating: .zero, count: candidates.count)
    var y = padding, rowStart = 0
    while rowStart < cells.count {
      var rowEnd = rowStart + 1
      while rowEnd < cells.count, cells[rowEnd].row == cells[rowStart].row { rowEnd += 1 }
      var height = Self.cellHeight
      for index in rowStart..<rowEnd {
        let chip = makeChip(candidate: candidates[index], index: index, reusing: measurementChip, interactive: false)
        height = max(height, cellHeight(for: chip, width: cellWidth(span: cells[index].span, columnWidth: columnWidth)))
      }
      for index in rowStart..<rowEnd {
        let cell = cells[index]
        frames[index] = CGRect(x: padding + CGFloat(cell.column) * (columnWidth + spacing), y: y,
                               width: cellWidth(span: cell.span, columnWidth: columnWidth), height: height)
      }
      y += height + spacing
      rowStart = rowEnd
    }
    gridLayout.replace(frames: frames, size: CGSize(width: width, height: max(0, y - spacing) + padding))
    scrollView.layoutIfNeeded()
    // 旋转或换代后收窄偏移，避免旧列表末尾的偏移把较短的新列表留在空白处。
    let offset = min(max(0, (pendingScrollOffset ?? scrollView.contentOffset).y),
                     max(0, gridLayout.collectionViewContentSize.height - scrollView.bounds.height))
    pendingScrollOffset = nil
    scrollView.setContentOffset(CGPoint(x: 0, y: offset), animated: false)
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

  private func makeChip(candidate: String, index: Int, reusing chip: CandidateGridButton,
                        interactive: Bool = true) -> CandidateGridButton {
    let number = index + 1
    let text = display(candidate)
    let annotation = annotations.indices.contains(index) ? annotations[index] : .none
    let marks = markers.indices.contains(index) ? markers[index] : []
    let selected = index == selectedIndex
    let secondary = (selected ? palette?.selectedTranslation : nil) ?? palette?.secondary ?? skin.secondary
    // 坐了首选的双拼纠错：字按固定词的规则取强调色，注释行（纠错前的拼写和释义）也用强调色，和没坐首选的纠错区分开。选中格画在调色板的选中底色上，调色板给了那里的翻译色或文字色（选中翻译色，其次选中文字色）就照用，都没给才用强调色：微信外观的选中底色就是强调色，强调色的注释会看不见。与候选条同一条规则。
    let confidentCorrection = confidentCorrections.contains(index)
    let annotationColor = confidentCorrection
      ? (selected ? palette?.selectedTranslation ?? palette?.selectedText : nil) ?? palette?.accent ?? skin.accent : secondary
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
          .foregroundColor: annotationColor,
        ]))
      }
    }
    var configuration = UIButton.Configuration.plain()
    configuration.attributedTitle = title
    configuration.baseForegroundColor = selected || marks.contains(where: \.pinned) || confidentCorrection
      ? (selected ? palette?.selectedText : nil) ?? palette?.accent ?? skin.accent : palette?.text ?? skin.keyForeground
    let aiPlaceholder = aiPlaceholders.contains(index)
    if aiPlaceholder { configuration.baseForegroundColor = secondary }
    configuration.contentInsets = palette?.layout?.chipInsets(fontSize: font.pointSize, fallback: Self.cellInsets) ?? Self.cellInsets
    configuration.background.cornerRadius = skin.cornerRadius
    let fill = selected ? palette?.selected ?? skin.accentSoft : palette != nil ? UIColor.clear : skin.keyBackground
    configuration.background.backgroundColor = fill
    if selected, let palette, palette.showSelectedBar {
      configuration.background.customView = CandidateSelectionBarView(color: palette.selectedBar ?? palette.accent, layout: palette.layout, fontSize: font.pointSize)
    }
    configuration.background.strokeWidth = palette == nil ? 0 : palette?.layout?.borderWidth ?? 1
    configuration.background.strokeColor = palette?.border ?? .clear
    configuration.background.cornerRadius = palette?.layout?.itemRadius ?? canvas?.itemCornerRadius ?? skin.cornerRadius
    chip.configuration = configuration
    chip.configurationUpdateHandler = { button in
      button.configuration?.background.backgroundColor = button.isHighlighted ? fill.keyboardPressedBrightness(0.88) : fill
    }
    chip.titleWidth = titleWidth
    chip.titleLineHeight = font.lineHeight
    chip.glossLines = glossLines
    chip.titleLineCount = 1 + glossLines
    // 选中的格子平铺在它的底色上；其余格子都画得像按键，包括阴影。
    chip.drawsKeyShadow = palette == nil && !selected && skin.hasShadow
    chip.accessibilityIdentifier = "panelCandidate-\(number)"
    chip.accessibilityLabel = annotation.accessibilityDescription.isEmpty
      ? "候选词 \(number)：\(text)"
      : "候选词 \(number)：\(text)，\(annotation.accessibilityDescription)"
    for marker in marks { chip.accessibilityLabel? += "，\(marker.spoken)" }
    if confidentCorrection { chip.accessibilityLabel? += KeyboardCandidateAnnotation.confidentCorrectionSpoken }
    if aiPlaceholder { chip.accessibilityLabel = "候选词 \(number)：\(CandidateMarker.pendingSpoken(candidate, markers: marks))" }
    chip.accessibilityTraits = selected ? [.button, .selected] : [.button]
    // 测量复用同一个按钮，不创建交互闭包；复用可见格子时先移除上一条候选的选择动作。
    if interactive {
      let action = UIAction.Identifier("candidateSelect")
      chip.removeAction(identifiedBy: action, for: .primaryActionTriggered)
      chip.addAction(UIAction(identifier: action) { [weak self] _ in self?.onSelect(index) }, for: .primaryActionTriggered)
      chip.menu = UIMenu(children: [
        UIDeferredMenuElement.uncached { [weak self] completion in
          completion(self?.menuElements(index) ?? [])
        }
      ])
    }
    return chip
  }

  private static func semibold(_ font: UIFont) -> UIFont {
    let descriptor = font.fontDescriptor.addingAttributes([
      .traits: [UIFontDescriptor.TraitKey.weight: UIFont.Weight.semibold.rawValue],
    ])
    return UIFont(descriptor: descriptor, size: font.pointSize)
  }
}

/// 单元格只持有一个可复用按钮；候选身份在配置时绑定到完整列表的位置。
private final class CandidateGridCell: UICollectionViewCell {
  let chip = CandidateGridButton(frame: .zero)

  override init(frame: CGRect) {
    super.init(frame: frame)
    contentView.addSubview(chip)
    chip.autoresizingMask = [.flexibleWidth, .flexibleHeight]
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  override func layoutSubviews() {
    super.layoutSubviews()
    chip.frame = contentView.bounds
  }
}

/// 布局保留全部几何，按纵向有序的行二分查找可见范围，不在每次滚动时遍历整个候选列表。
private final class CandidateGridLayout: UICollectionViewLayout {
  private var frames: [CGRect] = []
  private var size = CGSize.zero

  func replace(frames: [CGRect], size: CGSize) {
    self.frames = frames
    self.size = size
    invalidateLayout()
  }

  override var collectionViewContentSize: CGSize { size }

  override func layoutAttributesForItem(at indexPath: IndexPath) -> UICollectionViewLayoutAttributes? {
    guard frames.indices.contains(indexPath.item) else { return nil }
    let attributes = UICollectionViewLayoutAttributes(forCellWith: indexPath)
    attributes.frame = frames[indexPath.item]
    return attributes
  }

  override func layoutAttributesForElements(in rect: CGRect) -> [UICollectionViewLayoutAttributes]? {
    var lower = 0, upper = frames.count
    while lower < upper {
      let middle = lower + (upper - lower) / 2
      if frames[middle].maxY < rect.minY { lower = middle + 1 } else { upper = middle }
    }
    var attributes: [UICollectionViewLayoutAttributes] = []
    while lower < frames.count, frames[lower].minY <= rect.maxY {
      if frames[lower].intersects(rect), let item = layoutAttributesForItem(at: IndexPath(item: lower, section: 0)) {
        attributes.append(item)
      }
      lower += 1
    }
    return attributes
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
