import UIKit

/// 横向分页的图块网格，下方带页码圆点，键盘的分页面板共用（功能面板以及沿用它布局的各个选择面板）。
///
/// 图块按行填满每一页，每页 `columns × rows` 个；行高固定为 `rowHeight`，行间隔 `rowGap`，等宽的列之间隔 `columnGap`，每页左右各留 `sidePadding`。页面贴着滚动区顶部，显示圆点时圆点在其下方 8pt、位于视图底部，与设计稿的 flex 纵向布局一致。
///
/// 页面和滚动内容手动布局而不用 Auto Layout，这样设置内容尺寸的同一轮里就能重新套用页偏移：替换图块后仍停在用户原来那一页，而不是被 UIScrollView 把偏移夹回第一页。
final class KeyboardPagedGridView: UIView, UIScrollViewDelegate {
  static let dotsGap: CGFloat = 8

  let columns: Int
  let rows: Int
  private let rowHeight: CGFloat
  private let rowGap: CGFloat
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
    // 图块像按键一样在按下时高亮；从图块上开始的横向拖动仍能翻页，因为滚动视图可以取消它的触摸（见 `KeyboardPagingScrollView`）。
    scrollView.delaysContentTouches = false
    scrollView.canCancelContentTouches = true
    // 面板只有两行高，系统的边缘渐隐会把两边的图块冲淡。
    scrollView.disableEdgeEffects()
    scrollView.onAccessibilityScroll = { [weak self] direction in self?.accessibilityTurnPage(direction) ?? false }
    addSubview(scrollView)
    if showsDots { addSubview(dots) }
    dots.setCount(1)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  var tilesPerPage: Int { columns * rows }

  /// 至少一页，没有图块时也是。
  var pageCount: Int { max(1, (tiles.count + tilesPerPage - 1) / tilesPerPage) }

  /// 一页网格的高度：`rows` 行加上行间隔。
  var gridHeight: CGFloat { CGFloat(rows) * rowHeight + CGFloat(rows - 1) * rowGap }

  override var intrinsicContentSize: CGSize {
    CGSize(width: UIView.noIntrinsicMetric, height: gridHeight + (showsDots ? Self.dotsGap + KeyboardPagerDotsView.dotSide : 0))
  }

  /// 把 `tiles` 按顺序逐页排好。已显示的视图只移动、不重建。传 `preservingPage` 时停在当前页（页数变少时停在最后一页），否则回到第一页。
  func setTiles(_ tiles: [UIView], preservingPage: Bool) {
    pages.forEach { $0.removeFromSuperview() }
    self.tiles = tiles
    pages = stride(from: 0, to: max(tiles.count, 1), by: tilesPerPage).map { start in
      makePage(Array(tiles[min(start, tiles.count)..<min(start + tilesPerPage, tiles.count)]))
    }
    pages.forEach { scrollView.addSubview($0) }
    let target = preservingPage ? min(currentPage, pageCount - 1) : 0
    dots.setCount(pageCount)
    dots.setActive(target, animated: false)
    setPage(target)
    setNeedsLayout()
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
  }

  /// 拖动打断带动画的 `scrollToPage` 时由拖动接管，偏移重新归用户控制。
  func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
    isScrollAnimating = false
  }

  private func setPage(_ page: Int) {
    guard page != currentPage else { return }
    currentPage = page
    onPageChange?(page)
  }

  /// 一页：`rows` 行固定高度、每行 `columns` 个等宽格子，最后一个图块之后用空格子补齐。
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
      // 优先级低于 required，键盘被压得比网格还矮时压缩行高，而不是打印约束冲突。
      height.priority = .required - 1
      height.isActive = true
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
