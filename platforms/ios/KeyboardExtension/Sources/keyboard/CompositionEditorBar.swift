import UIKit

/// 放大拼写编辑条：候选栏下面一行，左边是放大的拼写和强调色光标，右边是「完成」。与 Android 的 `ImeCompositionEditor` 是同一个界面（#10591，三端对齐）：点读音或读音位置的笔打开它，键盘为它高一行；组字和光标仍由引擎持有，这里只发移光标的请求，删除和插入照旧用键盘本身。
final class CompositionEditorBar: UIView {
  /// 编辑条的高度：放大拼写 44pt 加上下各 4pt 内边距，与 Android 的一行（约 52dp）相同。
  static let height: CGFloat = 52

  let spelling: CompositionSpellingView
  let doneButton = UIButton(type: .system)

  /// - Parameters:
  ///   - onMove: 用户在拼写上点或拖出的新光标位置（`editing_text` 里的下标）。
  ///   - onDone: 点「完成」。
  init(onMove: @escaping (Int) -> Void, onDone: @escaping () -> Void) {
    spelling = CompositionSpellingView(move: onMove)
    super.init(frame: .zero)
    accessibilityIdentifier = "compositionEditorBar"
    accessibilityLabel = "拼写编辑面板"
    spelling.translatesAutoresizingMaskIntoConstraints = false
    addSubview(spelling)
    var configuration = UIButton.Configuration.plain()
    configuration.title = "完成"
    configuration.contentInsets = .zero
    doneButton.configuration = configuration
    doneButton.accessibilityIdentifier = "compositionEditorDone"
    doneButton.accessibilityLabel = "完成拼写编辑"
    doneButton.translatesAutoresizingMaskIntoConstraints = false
    doneButton.addAction(UIAction { _ in onDone() }, for: .primaryActionTriggered)
    addSubview(doneButton)
    // 钉死一行的高度：按键行的高度都是从键盘剩下的高度里分出来的，编辑条只靠拼写的固有高度撑起时，根栈视图会把键盘长出来的那一行之外再多分给它几十 pt，四排键跟着变矮、往下挪。比必需低一档，隐藏时让给栈视图收起它的那条约束。
    let rowHeight = heightAnchor.constraint(equalToConstant: Self.height)
    rowHeight.priority = .init(999)
    NSLayoutConstraint.activate([
      rowHeight,
      spelling.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      spelling.topAnchor.constraint(equalTo: topAnchor, constant: 4),
      spelling.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
      doneButton.leadingAnchor.constraint(equalTo: spelling.trailingAnchor, constant: 8),
      doneButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      doneButton.centerYAnchor.constraint(equalTo: centerYAnchor),
      doneButton.widthAnchor.constraint(equalToConstant: 64),
      doneButton.heightAnchor.constraint(equalToConstant: 40),
    ])
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

  /// 控制器开关编辑条只设这一层的 `isHidden`，子视图自己的 `isHidden` 不变、也还在窗口里，所以收起时在这里停掉拼写上进行中的拖动：否则组字结束时手指还按着，边缘计时器会一直走、重新打开时旧的拖动还能再挪光标。
  override var isHidden: Bool {
    didSet { if isHidden { spelling.stopDragging() } }
  }

  /// 按引擎这一刻的组字刷新。`display` 对不上 `editing` 时画原始按键，不猜位置（与 Android 相同）。`letterGroups` 是全拼 14 键、18 键的字母网格，读音的字母按它的组码对上按键（见 `CompositionCaretPolicy`）。
  /// 背景透明，透出键盘本身的底色或皮肤图，与 Android 编辑条用键盘底色是同一个样子。
  func update(display: String, editing: String, caret: Int, keyGrid: Bool, letterGroups: LetterKeyGrid?,
              foreground: UIColor, accent: UIColor) {
    let aligned = CompositionCaretPolicy.displayOffset(
      display: display, editing: editing, caret: caret, keyGrid: keyGrid, letterGroups: letterGroups) != nil
    spelling.update(display: aligned ? display : editing, editing: editing, caret: caret, keyGrid: keyGrid,
                    letterGroups: aligned ? letterGroups : nil, foreground: foreground, accent: accent)
    doneButton.configuration?.baseForegroundColor = accent
  }
}

/// 放大的拼写和闪动光标。点一下把光标放到点中的字母前；左右拖动超过阈值后按手指的位移移动光标（起点是按下时光标的位置，手指不用盖在光标上），拖到两端 28pt 内停住时每 90ms 再走一格并滚动长拼写。
final class CompositionSpellingView: UIView {
  static let fontSize: CGFloat = 22
  private static let inset: CGFloat = 8
  private static let edge: CGFloat = 28
  private static let touchSlop: CGFloat = 8

  private let move: (Int) -> Void
  private let font = UIFont.systemFont(ofSize: CompositionSpellingView.fontSize)
  private let caretLayer = CALayer()
  private var foreground: UIColor = .label
  private(set) var display = ""
  private var editing = ""
  /// 画出来的读音按这张全拼字母网格对上按键；画的就是按键本身或 26 键时为 nil。
  private var letterGroups: LetterKeyGrid?
  /// 光标画在 `display` 的这个下标前。
  private(set) var offset = 0
  private var scroll: CGFloat = 0
  private var downX: CGFloat = 0
  private var touchX: CGFloat = 0
  private var dragOrigin: CGFloat = 0
  private(set) var dragging = false
  private var edgeTimer: Timer?

  init(move: @escaping (Int) -> Void) {
    self.move = move
    super.init(frame: .zero)
    isOpaque = false
    contentMode = .redraw
    clipsToBounds = true
    isMultipleTouchEnabled = false
    accessibilityIdentifier = "compositionEditorSpelling"
    isAccessibilityElement = true
    accessibilityLabel = "放大拼写"
    accessibilityHint = "左右拖动移动光标"
    accessibilityTraits = .adjustable
    caretLayer.cornerRadius = 1
    layer.addSublayer(caretLayer)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

  override var intrinsicContentSize: CGSize {
    CGSize(width: UIView.noIntrinsicMetric, height: max(44, ceil(font.lineHeight) + 10))
  }

  func update(display: String, editing: String, caret: Int, keyGrid: Bool, letterGroups: LetterKeyGrid?,
              foreground: UIColor, accent: UIColor) {
    let textChanged = self.display != display
    let previousOffset = offset
    self.display = display
    self.editing = editing
    self.letterGroups = letterGroups
    self.foreground = foreground
    offset = max(0, CompositionCaretPolicy.displayOffset(
      display: display, editing: editing, caret: caret, keyGrid: keyGrid, letterGroups: letterGroups) ?? 0)
    caretLayer.backgroundColor = accent.cgColor
    accessibilityValue = display
    place()
    if textChanged { setNeedsDisplay() }
    if textChanged || previousOffset != offset { UIAccessibility.post(notification: .layoutChanged, argument: nil) }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    place()
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil { stopDragging() }
    updateBlinking()
  }

  override func draw(_ rect: CGRect) {
    let text = display as NSString
    let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: foreground]
    let y = (bounds.height - font.lineHeight) / 2
    UIGraphicsGetCurrentContext()?.clip(to: bounds.insetBy(dx: 4, dy: 0))
    text.draw(at: CGPoint(x: Self.inset - scroll, y: y), withAttributes: attributes)
  }

  // MARK: 触摸

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard let touch = touches.first else { return }
    let x = touch.location(in: self).x
    downX = x
    touchX = x
    dragOrigin = width(to: offset)
  }

  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard let touch = touches.first else { return }
    touchX = touch.location(in: self).x
    if !dragging && abs(touchX - downX) > Self.touchSlop {
      dragging = true
      updateBlinking()
      edgeTimer?.invalidate()
      edgeTimer = Timer.scheduledTimer(withTimeInterval: 0.09, repeats: true) { [weak self] _ in
        MainActor.assumeIsolated { self?.stepAtEdge() }
      }
    }
    if dragging { requestCaret(atDisplayOffset: nearest(dragOrigin + touchX - downX)) }
  }

  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    // 没拖动过就是一次点按：光标放到点中的字母前。
    if !dragging, let touch = touches.first {
      requestCaret(atDisplayOffset: nearest(touch.location(in: self).x - Self.inset + scroll))
    }
    stopDragging()
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
    stopDragging()
  }

  override func accessibilityIncrement() { requestStep(1) }

  override func accessibilityDecrement() { requestStep(-1) }

  // MARK: 光标

  /// 拖在两端时逐格走：音节分隔不占按键，所以按按键下标走，而不是按显示下标。
  private func stepAtEdge() {
    guard dragging, window != nil else { return stopDragging() }
    let direction = touchX < Self.edge ? -1 : touchX > bounds.width - Self.edge ? 1 : 0
    guard direction != 0 else { return }
    requestStep(direction)
    dragOrigin = width(to: offset)
    downX = touchX
  }

  private func requestStep(_ direction: Int) {
    guard let current = CompositionCaretPolicy.tapTarget(
      display: display, editing: editing, tapped: offset, letterGroups: letterGroups) else { return }
    let next = max(0, min(editing.count, current + direction))
    if next != current { move(next) }
  }

  /// 把光标放到 `display` 的这个下标前：换成按键下标，和现在的位置不同时才发请求。点按和拖动都走这里。
  func requestCaret(atDisplayOffset shownOffset: Int) {
    guard let target = CompositionCaretPolicy.tapTarget(
            display: display, editing: editing, tapped: shownOffset, letterGroups: letterGroups),
          let current = CompositionCaretPolicy.tapTarget(
            display: display, editing: editing, tapped: offset, letterGroups: letterGroups),
          target != current else { return }
    move(target)
  }

  fileprivate func stopDragging() {
    edgeTimer?.invalidate()
    edgeTimer = nil
    guard dragging else { return }
    dragging = false
    updateBlinking()
  }

  /// 拖动时光标常亮，平时按 1 秒一周闪动（亮 0.6 秒），与 Android 相同。
  private func updateBlinking() {
    caretLayer.removeAnimation(forKey: "blink")
    guard !dragging, window != nil, !UIAccessibility.isReduceMotionEnabled else { return }
    let blink = CAKeyframeAnimation(keyPath: "opacity")
    blink.values = [1, 1, 0, 0]
    blink.keyTimes = [0, 0.6, 0.6, 1]
    blink.duration = 1
    blink.calculationMode = .discrete
    blink.repeatCount = .infinity
    caretLayer.add(blink, forKey: "blink")
  }

  /// 让光标留在条内：右边留出边缘拖动区，左边留一点余量；再把光标竖条摆到位置上。
  private func place() {
    guard bounds.width > 0 else { return }
    let caret = width(to: offset)
    let right = max(20, bounds.width - 40)
    let previous = scroll
    scroll = max(0, min(scroll, caret))
    if caret - scroll > right { scroll = caret - right }
    if caret - scroll < 12 { scroll = max(0, caret - 12) }
    if scroll != previous { setNeedsDisplay() }
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    caretLayer.frame = CGRect(x: Self.inset + caret - scroll, y: 8, width: 2, height: max(0, bounds.height - 16))
    CATransaction.commit()
  }

  /// `display` 前 `index` 个字符画出来的宽度。
  func width(to index: Int) -> CGFloat {
    let prefix = String(display.prefix(max(0, index)))
    return (prefix as NSString).size(withAttributes: [.font: font]).width
  }

  /// 离 `x`（拼写自己的坐标，不含内边距和滚动）最近的字符缝。
  func nearest(_ x: CGFloat) -> Int {
    let count = display.count
    for index in 0..<count where x < (width(to: index) + width(to: index + 1)) / 2 {
      return index
    }
    return count
  }
}
