import UIKit

/// 「键盘调整」的遮罩：铺满整个键盘视图，接住所有触摸，下面的键盘照常画成实时预览。三端同一套界面：
///
/// - 键盘外框（工具栏或候选行加按键区）上盖约 45% 不透明的黑色，四角画皮肤强调色的直角括号。
/// - 外框以内：上沿正中、下沿正中各一个横向把手（三道横线），左右两沿垂直居中各一个竖向把手（两道竖线）。每个柄只动它那条边，换算在 `KeyboardAdjustSession`。
/// - 正中一排三个白色图标加文字的按钮：「重置」「移动」（按住拖动整块键盘）「确认」。没有「取消」。
///
/// 不生效的项不显示对应的柄（`KeyboardAdjustCapabilities`）。这个视图自己不存任何数据：拖动、读屏调节和按钮都交给键盘，键盘改完预览后把新的外框和读屏数值设回来。
///
/// 按钮是普通控件而不是 `UIButton`：键盘的皮肤处理会把带底色配置的 `UIButton` 画成键帽。
final class KeyboardAdjustOverlay: UIView {
  typealias Part = KeyboardAdjustSession.Part

  static let dimAlpha: CGFloat = 0.45
  private static let bracketLength: CGFloat = 18
  private static let bracketWidth: CGFloat = 3
  /// 横向把手的触摸区，竖向把手转过来用。
  private static let handleTouchSize = CGSize(width: 96, height: 32)
  private static let buttonSize = CGSize(width: 72, height: 58)
  private static let buttonSpacing: CGFloat = 28

  /// 键盘外框在这个视图里的位置，由键盘在每次布局之后设置。
  var keyboardFrame: CGRect = .zero {
    didSet { if keyboardFrame != oldValue { setNeedsLayout() } }
  }

  var capabilities = KeyboardAdjustCapabilities.all {
    didSet { if capabilities != oldValue { applyCapabilities() } }
  }

  /// 拖动 `part`：`dx` 向右为正、`rise` 向上为正，都是从按下到现在、在屏幕上不动的坐标里的位移。
  var onDrag: ((Part, UIGestureRecognizer.State, _ dx: CGFloat, _ rise: CGFloat) -> Void)?
  /// 读屏对 `part` 的增减。
  var onStep: ((Part, _ increment: Bool) -> Void)?
  var onReset: (() -> Void)?
  var onConfirm: (() -> Void)?

  private let dim = UIView()
  private let brackets = CAShapeLayer()
  private let topHandle = Handle(part: .top, label: "键盘高度", hint: "上下拖动调整键盘高度")
  private let bottomHandle = Handle(part: .bottom, label: "底部高度", hint: "上下拖动抬高或放低键盘，上沿不动")
  private let leftHandle = Handle(part: .left, label: "键盘宽度，左边", hint: "左右拖动调整宽度，右边不动")
  private let rightHandle = Handle(part: .right, label: "键盘宽度，右边", hint: "左右拖动调整宽度，左边不动")
  private let resetButton = ActionButton(title: "重置", symbol: "arrow.counterclockwise")
  private let moveButton = ActionButton(title: "移动", symbol: "arrow.up.and.down.and.arrow.left.and.right")
  private let confirmButton = ActionButton(title: "确认", symbol: "checkmark.circle")

  override init(frame: CGRect) {
    super.init(frame: frame)
    accessibilityIdentifier = "keyboardAdjustOverlay"
    accessibilityViewIsModal = true
    backgroundColor = .clear

    dim.backgroundColor = UIColor.black.withAlphaComponent(Self.dimAlpha)
    dim.isUserInteractionEnabled = false
    addSubview(dim)
    brackets.fillColor = nil
    brackets.lineWidth = Self.bracketWidth
    brackets.lineCap = .butt
    brackets.lineJoin = .miter
    layer.addSublayer(brackets)

    topHandle.accessibilityIdentifier = "keyboardAdjustTop"
    bottomHandle.accessibilityIdentifier = "keyboardAdjustBottom"
    leftHandle.accessibilityIdentifier = "keyboardAdjustLeft"
    rightHandle.accessibilityIdentifier = "keyboardAdjustRight"
    resetButton.accessibilityIdentifier = "keyboardAdjustReset"
    moveButton.accessibilityIdentifier = "keyboardAdjustMove"
    confirmButton.accessibilityIdentifier = "keyboardAdjustConfirm"
    moveButton.accessibilityHint = "按住拖动整块键盘"
    moveButton.accessibilityTraits = [.button, .adjustable]
    for handle in [topHandle, bottomHandle, leftHandle, rightHandle] {
      addSubview(handle)
      track(handle, part: handle.part)
      handle.onStep = { [weak self] part, increment in self?.onStep?(part, increment) }
    }
    for button in [resetButton, moveButton, confirmButton] { addSubview(button) }
    track(moveButton, part: .move)
    moveButton.onStep = { [weak self] increment in self?.onStep?(.move, increment) }
    resetButton.addAction(UIAction { [weak self] _ in self?.onReset?() }, for: .primaryActionTriggered)
    confirmButton.addAction(UIAction { [weak self] _ in self?.onConfirm?() }, for: .primaryActionTriggered)
    accessibilityElements = [topHandle, leftHandle, resetButton, moveButton, confirmButton, rightHandle, bottomHandle]
    apply(skin: KeyboardTheme.current)
    applyCapabilities()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 四角的括号用皮肤的强调色。
  func apply(skin: KeyboardTheme) {
    brackets.strokeColor = skin.accent.resolvedColor(with: traitCollection).cgColor
  }

  /// 各柄朗读的当前值，由键盘按预览算好设进来。
  func setAccessibilityValues(height: String, bottom: String, width: String, move: String) {
    topHandle.accessibilityValue = height
    bottomHandle.accessibilityValue = bottom
    leftHandle.accessibilityValue = width
    rightHandle.accessibilityValue = width
    moveButton.accessibilityValue = move
  }

  private func applyCapabilities() {
    bottomHandle.isHidden = !capabilities.showsBottomHandle
    leftHandle.isHidden = !capabilities.showsSideHandles
    rightHandle.isHidden = !capabilities.showsSideHandles
    moveButton.isHidden = !capabilities.showsMove
    setNeedsLayout()
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let box = keyboardFrame
    dim.frame = box
    brackets.frame = bounds
    brackets.path = Self.bracketPath(box)
    let horizontal = Self.handleTouchSize
    let vertical = CGSize(width: horizontal.height, height: horizontal.width)
    topHandle.frame = CGRect(x: box.midX - horizontal.width / 2, y: box.minY, width: horizontal.width, height: horizontal.height)
    bottomHandle.frame = CGRect(x: box.midX - horizontal.width / 2, y: box.maxY - horizontal.height,
                                width: horizontal.width, height: horizontal.height)
    leftHandle.frame = CGRect(x: box.minX, y: box.midY - vertical.height / 2, width: vertical.width, height: vertical.height)
    rightHandle.frame = CGRect(x: box.maxX - vertical.width, y: box.midY - vertical.height / 2,
                               width: vertical.width, height: vertical.height)
    let buttons = [resetButton, moveButton, confirmButton].filter { !$0.isHidden }
    let size = Self.buttonSize
    let rowWidth = CGFloat(buttons.count) * size.width + CGFloat(max(buttons.count - 1, 0)) * Self.buttonSpacing
    var x = box.midX - rowWidth / 2
    for button in buttons {
      button.frame = CGRect(x: x, y: box.midY - size.height / 2, width: size.width, height: size.height)
      x += size.width + Self.buttonSpacing
    }
  }

  override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
    super.traitCollectionDidChange(previousTraitCollection)
    apply(skin: KeyboardTheme.current)
  }

  /// 外框四角向内的直角括号。
  private static func bracketPath(_ box: CGRect) -> CGPath {
    let path = UIBezierPath()
    guard box.width > 0, box.height > 0 else { return path.cgPath }
    let inset = bracketWidth / 2
    let rect = box.insetBy(dx: inset, dy: inset)
    let arm = min(bracketLength, rect.width / 2, rect.height / 2)
    let corners: [(CGPoint, CGFloat, CGFloat)] = [
      (CGPoint(x: rect.minX, y: rect.minY), 1, 1), (CGPoint(x: rect.maxX, y: rect.minY), -1, 1),
      (CGPoint(x: rect.minX, y: rect.maxY), 1, -1), (CGPoint(x: rect.maxX, y: rect.maxY), -1, -1),
    ]
    for (corner, dx, dy) in corners {
      path.move(to: CGPoint(x: corner.x + dx * arm, y: corner.y))
      path.addLine(to: corner)
      path.addLine(to: CGPoint(x: corner.x, y: corner.y + dy * arm))
    }
    return path.cgPath
  }

  /// 给 `view` 装上拖动：位移按手指到窗口底边的高度和窗口里的横坐标算。键盘长高、抬高和收窄都会挪动柄本身，窗口的底边和左右却不动，所以不会追着拖动自己引起的变化跑。
  private func track(_ view: UIView, part: Part) {
    let pan = PartPan(target: self, action: #selector(handlePan(_:)))
    pan.part = part
    pan.maximumNumberOfTouches = 1
    view.addGestureRecognizer(pan)
  }

  private func stablePoint(_ recognizer: UIGestureRecognizer) -> CGPoint {
    guard let window else {
      let point = recognizer.location(in: self)
      return CGPoint(x: point.x, y: -point.y)
    }
    let point = recognizer.location(in: window)
    return CGPoint(x: point.x, y: window.bounds.maxY - point.y)
  }

  @objc private func handlePan(_ recognizer: UIPanGestureRecognizer) {
    guard let pan = recognizer as? PartPan, let part = pan.part else { return }
    let point = stablePoint(recognizer)
    if recognizer.state == .began { pan.start = point }
    let start = pan.start ?? point
    onDrag?(part, recognizer.state, point.x - start.x, point.y - start.y)
    if recognizer.state != .began && recognizer.state != .changed { pan.start = nil }
  }

  private final class PartPan: UIPanGestureRecognizer {
    var part: Part?
    var start: CGPoint?
  }

  /// 拖动柄：横向的画三道横线，竖向的画两道竖线，都是半透明白色。读屏里是可调节元素。
  private final class Handle: UIView {
    let part: Part
    var onStep: ((Part, Bool) -> Void)?
    private let lines: [UIView]

    init(part: Part, label: String, hint: String) {
      self.part = part
      lines = (0..<(part == .left || part == .right ? 2 : 3)).map { _ in UIView() }
      super.init(frame: .zero)
      for line in lines {
        line.backgroundColor = UIColor.white.withAlphaComponent(0.85)
        line.isUserInteractionEnabled = false
        line.layer.cornerRadius = 0.75
        addSubview(line)
      }
      isAccessibilityElement = true
      accessibilityLabel = label
      accessibilityHint = hint
      accessibilityTraits = .adjustable
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layoutSubviews() {
      super.layoutSubviews()
      let thickness: CGFloat = 1.5, gap: CGFloat = 3.5, length: CGFloat = 22
      let vertical = part == .left || part == .right
      let span = CGFloat(lines.count) * thickness + CGFloat(lines.count - 1) * gap
      for (index, line) in lines.enumerated() {
        let offset = CGFloat(index) * (thickness + gap) - span / 2
        line.frame = vertical
          ? CGRect(x: bounds.midX + offset, y: bounds.midY - length / 2, width: thickness, height: length)
          : CGRect(x: bounds.midX - length / 2, y: bounds.midY + offset, width: length, height: thickness)
      }
    }

    override func accessibilityIncrement() { onStep?(part, true) }
    override func accessibilityDecrement() { onStep?(part, false) }
  }

  /// 中间一排的按钮：白色 SF Symbol 在上，白色文字在下；按住时变淡。
  private final class ActionButton: UIControl {
    var onStep: ((Bool) -> Void)?
    private let icon = UIImageView()
    private let label = UILabel()

    init(title: String, symbol: String) {
      super.init(frame: .zero)
      icon.image = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: 24, weight: .regular))
      icon.tintColor = .white
      icon.contentMode = .center
      icon.isUserInteractionEnabled = false
      label.text = title
      label.font = .systemFont(ofSize: 13)
      label.textColor = .white
      label.textAlignment = .center
      label.isUserInteractionEnabled = false
      addSubview(icon)
      addSubview(label)
      isAccessibilityElement = true
      accessibilityLabel = title
      accessibilityTraits = .button
      // `UIControl` 只报告触摸；点按变成 primary action，所以点按和 `sendActions(for: .primaryActionTriggered)` 走同一个处理函数。
      addAction(UIAction { [weak self] _ in self?.sendActions(for: .primaryActionTriggered) }, for: .touchUpInside)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var isHighlighted: Bool {
      didSet { alpha = isHighlighted ? 0.55 : 1 }
    }

    override func layoutSubviews() {
      super.layoutSubviews()
      let labelHeight: CGFloat = 18
      icon.frame = CGRect(x: 0, y: 0, width: bounds.width, height: bounds.height - labelHeight - 4)
      label.frame = CGRect(x: 0, y: bounds.height - labelHeight, width: bounds.width, height: labelHeight)
    }

    override func accessibilityIncrement() { onStep?(true) }
    override func accessibilityDecrement() { onStep?(false) }
  }
}
