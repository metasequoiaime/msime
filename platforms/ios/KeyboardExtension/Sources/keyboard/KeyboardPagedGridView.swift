import UIKit

/// 横向分页的图块网格，下方带页码圆点，键盘的分页面板共用（功能面板以及沿用它布局的各个选择面板）。
///
/// 图块按行填满每一页，每页 `columns × rows` 个；行高为 `rowHeight`，行间隔 `rowGap`，等宽的列之间隔 `columnGap`，每页左右各留 `sidePadding`。页面贴着滚动区顶部，显示圆点时圆点在其下方 8pt、位于视图底部，与设计稿的 flex 纵向布局一致。
///
/// 选择面板（功能面板、输入方式）用 `init(columns:pickerRows:rowHeight:columnGap:sidePadding:)` 建网格：行数、行高和行间隔不固定，每次布局按滚动区的真实高度由 `pickerLayout` 重新决定，网格永远不超出滚动区；皮肤面板等其余网格保持初始化时的固定行数和行高。
///
/// 页面和滚动内容手动布局而不用 Auto Layout，这样设置内容尺寸的同一轮里就能重新套用页偏移：替换图块后仍停在用户原来那一页，而不是被 UIScrollView 把偏移夹回第一页。
final class KeyboardPagedGridView: UIView, UIScrollViewDelegate {
  static let dotsGap: CGFloat = 8

  let columns: Int
  /// 每页的行数；选择面板的网格在布局时按 `pickerLayout` 改它。
  private(set) var rows: Int
  private var rowHeight: CGFloat
  private var rowGap: CGFloat
  /// 初始化时给的行高，即格子的满尺寸行高；选择面板缩小时 `rowHeight` 是它乘以缩放。
  private let fullRowHeight: CGFloat
  /// 选择面板的首选行数（`pickerRows`）；nil 表示行数、行高和行间隔固定不变。
  private var preferredPickerRows: Int?
  /// 每一行的高度约束，缩放变化而行数不变时就地改它们的常量，不必重新分页。
  private var rowHeightConstraints: [NSLayoutConstraint] = []
  private let columnGap: CGFloat
  private let sidePadding: CGFloat
  private let showsDots: Bool
  private let scrollView = KeyboardPagingScrollView()
  private let dots = KeyboardPagerDotsView()
  private var pages: [UIStackView] = []
  private(set) var tiles: [UIView] = []
  /// 当前显示的页，从 0 起，随用户的滑动和 `scrollToPage` 变化。
  private(set) var currentPage = 0
  /// `currentPage` 每次变化时以新页码调用。
  var onPageChange: ((Int) -> Void)?
  /// 本视图自己写入内容尺寸和偏移期间置位，这样由此触发的滚动回调不算翻页。
  private var isApplyingLayout = false
  /// 带动画的 `scrollToPage` 期间置位，免得中途的一次布局把偏移弹回去。
  private var isScrollAnimating = false

  init(columns: Int, rows: Int = 2, rowHeight: CGFloat, rowGap: CGFloat, columnGap: CGFloat, sidePadding: CGFloat, showsDots: Bool = true) {
    self.columns = max(1, columns)
    self.rows = max(1, rows)
    self.rowHeight = rowHeight
    self.fullRowHeight = rowHeight
    self.rowGap = rowGap
    self.columnGap = columnGap
    self.sidePadding = sidePadding
    self.showsDots = showsDots
    super.init(frame: .zero)
    scrollView.delegate = self
    scrollView.isPagingEnabled = true
    scrollView.showsHorizontalScrollIndicator = false
    scrollView.showsVerticalScrollIndicator = false
    scrollView.alwaysBounceVertical = false
    scrollView.alwaysBounceHorizontal = false
    scrollView.scrollsToTop = false
    scrollView.contentInsetAdjustmentBehavior = .never
    // 翻页交给滚动视图自己的分页：抬手时按速度和距离停到整页。图块像按键一样在按下时高亮；从图块上开始的横向拖动仍能翻页，因为滚动视图可以取消它的触摸（见 `KeyboardPagingScrollView`）。
    scrollView.delaysContentTouches = false
    scrollView.canCancelContentTouches = true
    // 面板只有两三行高，系统的边缘渐隐会把两边的图块冲淡。
    scrollView.disableEdgeEffects()
    scrollView.onAccessibilityScroll = { [weak self] direction in self?.accessibilityTurnPage(direction) ?? false }
    addSubview(scrollView)
    if showsDots { addSubview(dots) }
    dots.setCount(1)
  }

  /// 选择面板（功能面板、输入方式）的网格：`pickerRows` 是首选行数（见 `pickerRows(formFactor:landscape:)`），`rowHeight` 是格子的满尺寸行高。实际的行数、行高和行间隔在每次布局时按滚动区的真实高度由 `pickerLayout` 决定。
  convenience init(columns: Int, pickerRows: Int, rowHeight: CGFloat, columnGap: CGFloat, sidePadding: CGFloat) {
    self.init(
      columns: columns, rows: pickerRows, rowHeight: rowHeight, rowGap: KeyboardPagedGridView.pickerRowGap(rows: pickerRows),
      columnGap: columnGap, sidePadding: sidePadding)
    preferredPickerRows = max(1, pickerRows)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  var tilesPerPage: Int { columns * rows }

  /// 输入方式和功能面板每页的首选行数：手机竖屏和 iPad 三行；手机横屏的按键区只有约 165pt（34pt 的按键四排），首选两行。实际排几行、缩放多少由 `pickerLayout` 按真实高度决定。
  static func pickerRows(formFactor: KeyboardFormFactor, landscape: Bool) -> Int {
    formFactor == .phone && landscape ? 2 : 3
  }

  /// 面板满尺寸时的行间隔：三行 12pt，两行 16pt。格子缩小时行间隔按同一比例缩小（`pickerLayout`）。
  static func pickerRowGap(rows: Int) -> CGFloat { rows >= 3 ? 12 : 16 }

  /// 选择面板格子的最小高度（pt），与 44pt 的触控目标下限一致：三行缩到比它还矮时改排两行。
  static let pickerMinTileHeight: CGFloat = 44

  /// 选择面板按真实高度排布的结果：行数、相对满尺寸的缩放，以及缩放后的行高和行间隔。
  struct PickerLayout: Equatable {
    let rows: Int
    let scale: CGFloat
    /// 满尺寸行高乘以 `scale`。
    let rowHeight: CGFloat
    /// `pickerRowGap(rows:)` 乘以 `scale`。
    let rowGap: CGFloat
  }

  /// 选择面板的排布规则，与 Android `PagedTileGrid.pickerGrid`、HarmonyOS `FunctionPanelPolicy.pickerGrid` 是同一条规则、同一组常量（行间隔 12 / 16，最小格子 44）。`height` 是格子各行能占的高度（滚动区的真实高度，不含页码圆点），`rowHeight` 是格子的满尺寸行高。
  ///
  /// 首选三行时先按三行算：满尺寸高度为 3 行加 2 个 12pt 间隔，缩放取 `min(1, height / 满尺寸)`，缩放后的行高不低于 `pickerMinTileHeight` 就排三行。否则排两行：满尺寸为 2 行加一个 16pt 间隔，缩放同样取 `min(1, height / 满尺寸)`，但不低于 `pickerMinTileHeight / rowHeight`（行高本来就不超过 44 时不缩）。高度够时缩放为 1，与原来的排法完全一样：满尺寸的行贴着顶部、用原来的行间隔。
  static func pickerLayout(preferredRows: Int, height: CGFloat, rowHeight: CGFloat) -> PickerLayout {
    // 浮点误差容差：正好缩到 44pt 的三行仍算放得下。
    let epsilon: CGFloat = 0.0001
    let available = max(0, height)
    if preferredRows >= 3 {
      let full = 3 * rowHeight + 2 * pickerRowGap(rows: 3)
      let scale = min(1, available / full)
      if rowHeight * scale >= pickerMinTileHeight - epsilon {
        return PickerLayout(rows: 3, scale: scale, rowHeight: rowHeight * scale, rowGap: pickerRowGap(rows: 3) * scale)
      }
    }
    let full = 2 * rowHeight + pickerRowGap(rows: 2)
    let floor = rowHeight > pickerMinTileHeight ? pickerMinTileHeight / rowHeight : 1
    let scale = max(floor, min(1, available / full))
    return PickerLayout(rows: 2, scale: scale, rowHeight: rowHeight * scale, rowGap: pickerRowGap(rows: 2) * scale)
  }

  /// 至少一页，没有图块时也是。
  var pageCount: Int { max(1, (tiles.count + tilesPerPage - 1) / tilesPerPage) }

  /// 一页网格的高度：`rows` 行加上行间隔。
  var gridHeight: CGFloat { CGFloat(rows) * rowHeight + CGFloat(rows - 1) * rowGap }

  override var intrinsicContentSize: CGSize {
    CGSize(width: UIView.noIntrinsicMetric, height: gridHeight + (showsDots ? Self.dotsGap + KeyboardPagerDotsView.dotSide : 0))
  }

  /// 把 `tiles` 按顺序逐页排好。已显示的视图只移动、不重建。传 `preservingPage` 时停在当前页（页数变少时停在最后一页），否则回到第一页。
  func setTiles(_ tiles: [UIView], preservingPage: Bool) {
    self.tiles = tiles
    rebuildPages(showing: preservingPage ? currentPage : 0)
  }

  /// 按当前的行数、行高和行间隔重新分页，停在 `page`（页数变少时停在最后一页），页码圆点跟着换数量。
  private func rebuildPages(showing page: Int) {
    pages.forEach { $0.removeFromSuperview() }
    rowHeightConstraints = []
    pages = stride(from: 0, to: max(tiles.count, 1), by: tilesPerPage).map { start in
      makePage(Array(tiles[min(start, tiles.count)..<min(start + tilesPerPage, tiles.count)]))
    }
    pages.forEach { scrollView.addSubview($0) }
    let target = min(max(page, 0), pageCount - 1)
    dots.setCount(pageCount)
    dots.setActive(target, animated: false)
    setPage(target)
    setNeedsLayout()
  }

  /// 换成 `layout` 的行数、行高和行间隔。行数变了就重新分页，停在含有原来那一页第一个图块的页上；只有缩放变了时就地改行高约束和行间隔，图块不挪窝，正按着的图块也不会被打断。
  private func applyPickerLayout(_ layout: PickerLayout) {
    if layout.rows != rows {
      let firstTile = currentPage * tilesPerPage
      rows = layout.rows
      rowHeight = layout.rowHeight
      rowGap = layout.rowGap
      rebuildPages(showing: firstTile / tilesPerPage)
      return
    }
    guard layout.rowHeight != rowHeight || layout.rowGap != rowGap else { return }
    rowHeight = layout.rowHeight
    rowGap = layout.rowGap
    pages.forEach { $0.spacing = rowGap }
    rowHeightConstraints.forEach { $0.constant = rowHeight }
  }

  /// 显示 `page`（限制在已有页数范围内），`animated` 时滑过去。
  func scrollToPage(_ page: Int, animated: Bool) {
    let target = min(max(page, 0), pageCount - 1)
    let width = scrollView.bounds.width
    guard animated, width > 0, window != nil, target != currentPage else {
      isScrollAnimating = false
      dots.setActive(target, animated: false)
      setPage(target)
      setNeedsLayout()
      layoutIfNeeded()
      return
    }
    // 滑动越过一半时，页码和对应圆点在 `scrollViewDidScroll` 里跟着偏移更新，与手指滑动时一样。
    isScrollAnimating = true
    scrollView.setContentOffset(CGPoint(x: CGFloat(target) * width, y: 0), animated: true)
  }

  /// 按 `skin` 重新给页码圆点上色：当前页用强调色，其余用细线色。
  func apply(skin: KeyboardTheme) {
    dots.setColors(active: skin.accent, inactive: skin.hairline)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let width = bounds.width
    let dotsBlock = showsDots ? Self.dotsGap + KeyboardPagerDotsView.dotSide : 0
    let scrollHeight = max(0, bounds.height - dotsBlock)
    // 选择面板按滚动区的真实高度排布，格子各行永远放得进滚动区，不会上下拖动，也不会被不均匀地压扁。
    if let preferred = preferredPickerRows, scrollHeight > 0 {
      applyPickerLayout(Self.pickerLayout(preferredRows: preferred, height: scrollHeight, rowHeight: fullRowHeight))
    }
    isApplyingLayout = true
    scrollView.frame = CGRect(x: 0, y: 0, width: width, height: scrollHeight)
    if showsDots {
      dots.frame = CGRect(x: 0, y: scrollHeight + Self.dotsGap, width: width, height: KeyboardPagerDotsView.dotSide)
    }
    for (index, page) in pages.enumerated() {
      page.frame = CGRect(
        x: CGFloat(index) * width + sidePadding, y: 0,
        width: max(0, width - sidePadding * 2), height: min(gridHeight, scrollHeight))
    }
    scrollView.contentSize = CGSize(width: width * CGFloat(pages.count), height: scrollHeight)
    if !scrollView.isDragging && !scrollView.isDecelerating && !isScrollAnimating {
      let offset = CGPoint(x: CGFloat(currentPage) * width, y: 0)
      if scrollView.contentOffset != offset { scrollView.contentOffset = offset }
    }
    isApplyingLayout = false
  }

  func scrollViewDidScroll(_ scrollView: UIScrollView) {
    let width = scrollView.bounds.width
    guard !isApplyingLayout, width > 0 else { return }
    let page = min(max(Int((scrollView.contentOffset.x / width).rounded()), 0), pageCount - 1)
    guard page != currentPage else { return }
    dots.setActive(page, animated: true)
    setPage(page)
  }

  func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) {
    isScrollAnimating = false
    settlePageOffset()
  }

  /// 拖动打断带动画的 `scrollToPage` 时由拖动接管，偏移重新归用户控制。
  func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
    isScrollAnimating = false
  }

  func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
    settlePageOffset()
  }

  func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
    if !decelerate { settlePageOffset() }
  }

  /// 结束拖动或动画时把偏移对齐到整页。面板重排、转屏或手势反向时可能在减速中触发布局；只依赖 `layoutSubviews` 会把滚动位置留在半页，下一次反向滑动就从错误的基准继续。
  private func settlePageOffset() {
    let width = scrollView.bounds.width
    guard width > 0 else { return }
    let target = min(max(Int((scrollView.contentOffset.x / width).rounded()), 0), pageCount - 1)
    dots.setActive(target, animated: false)
    setPage(target)
    let offset = CGPoint(x: CGFloat(target) * width, y: 0)
    guard scrollView.contentOffset != offset else { return }
    isApplyingLayout = true
    scrollView.setContentOffset(offset, animated: false)
    isApplyingLayout = false
  }

  private func setPage(_ page: Int) {
    guard page != currentPage else { return }
    currentPage = page
    onPageChange?(page)
  }

  /// 一页：`rows` 行高为 `rowHeight`、每行 `columns` 个等宽格子，最后一个图块之后用空格子补齐。
  private func makePage(_ pageTiles: [UIView]) -> UIStackView {
    let grid = UIStackView()
    grid.axis = .vertical
    grid.alignment = .fill
    grid.distribution = .fill
    grid.spacing = rowGap
    for row in 0..<rows {
      let line = UIStackView()
      line.axis = .horizontal
      line.alignment = .fill
      line.distribution = .fillEqually
      line.spacing = columnGap
      for column in 0..<columns {
        let index = row * columns + column
        line.addArrangedSubview(index < pageTiles.count ? pageTiles[index] : UIView())
      }
      let height = line.heightAnchor.constraint(equalToConstant: rowHeight)
      // 优先级低于 required，键盘被压得比网格还矮时压缩行高，而不是打印约束冲突。选择面板的行高已按真实高度缩放，只有两行缩到最小格子仍放不下时才会走到这里。
      height.priority = .required - 1
      height.isActive = true
      rowHeightConstraints.append(height)
      grid.addArrangedSubview(line)
    }
    return grid
  }

  /// VoiceOver 三指滑动翻页：向左显示下一页，向右显示上一页。
  private func accessibilityTurnPage(_ direction: UIAccessibilityScrollDirection) -> Bool {
    let target: Int
    switch direction {
    case .left, .next: target = currentPage + 1
    case .right, .previous: target = currentPage - 1
    default: return false
    }
    guard (0..<pageCount).contains(target) else { return false }
    scrollToPage(target, animated: true)
    UIAccessibility.post(notification: .pageScrolled, argument: "第 \(target + 1) 页，共 \(pageCount) 页")
    return true
  }
}

/// `KeyboardPagedGridView` 内部的分页滚动视图。
///
/// 它让从图块（UIControl）上开始的拖动取消图块的触摸并翻页，UIScrollView 默认不对控件这样做；它还把 VoiceOver 的滚动手势交给掌握页面信息的网格处理。
private final class KeyboardPagingScrollView: UIScrollView {
  var onAccessibilityScroll: ((UIAccessibilityScrollDirection) -> Bool)?

  override func touchesShouldCancel(in view: UIView) -> Bool { true }

  override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
    onAccessibilityScroll?(direction) ?? super.accessibilityScroll(direction)
  }
}

/// 分页面板下方的页码圆点：6pt 高的胶囊，间隔 6pt，当前页 16pt 宽、用强调色，其余 6pt 宽、用细线色。宽度和颜色在 0.2s 内过渡，只有一页时整行隐藏。
final class KeyboardPagerDotsView: UIView {
  static let dotSide: CGFloat = 6
  static let activeWidth: CGFloat = 16
  static let gap: CGFloat = 6
  private static let transition: TimeInterval = 0.2

  private var dots: [UIView] = []
  private(set) var count = 0
  private(set) var active = 0
  private var activeColor: UIColor = .label
  private var inactiveColor: UIColor = .tertiaryLabel

  override init(frame: CGRect) {
    super.init(frame: frame)
    isUserInteractionEnabled = false
    // 纯装饰：翻页由网格自己播报。
    accessibilityElementsHidden = true
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// `count` 个圆点这一行的宽度：一个当前页圆点、其余圆点和间隔。
  static func totalWidth(count: Int) -> CGFloat {
    guard count > 0 else { return 0 }
    return activeWidth + CGFloat(count - 1) * (dotSide + gap)
  }

  override var intrinsicContentSize: CGSize {
    CGSize(width: Self.totalWidth(count: count), height: Self.dotSide)
  }

  func setCount(_ value: Int) {
    let next = max(0, value)
    isHidden = next <= 1
    guard next != count else { return }
    count = next
    active = min(active, max(0, count - 1))
    while dots.count < count {
      let dot = UIView()
      dot.layer.cornerRadius = Self.dotSide / 2
      dot.layer.cornerCurve = .circular
      addSubview(dot)
      dots.append(dot)
    }
    while dots.count > count { dots.removeLast().removeFromSuperview() }
    invalidateIntrinsicContentSize()
    layoutDots()
  }

  func setActive(_ value: Int, animated: Bool) {
    let next = min(max(value, 0), max(0, count - 1))
    guard next != active else { return }
    active = next
    guard animated, window != nil else {
      layoutDots()
      return
    }
    UIView.animate(withDuration: Self.transition, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
      self.layoutDots()
    }
  }

  func setColors(active: UIColor, inactive: UIColor) {
    activeColor = active
    inactiveColor = inactive
    layoutDots()
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    layoutDots()
  }

  private func layoutDots() {
    var x = ((bounds.width - Self.totalWidth(count: count)) / 2).rounded(.down)
    let y = ((bounds.height - Self.dotSide) / 2).rounded(.down)
    for (index, dot) in dots.enumerated() {
      let width = index == active ? Self.activeWidth : Self.dotSide
      dot.frame = CGRect(x: x, y: y, width: width, height: Self.dotSide)
      dot.backgroundColor = index == active ? activeColor : inactiveColor
      x += width + Self.gap
    }
  }
}
