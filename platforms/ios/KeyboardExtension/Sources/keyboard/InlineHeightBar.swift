import UIKit

/// 设计稿的高度调整模式（dc.html L2172-2179），移植自 Android 的 `InlineHeightBar`：它占据工具栏那一行，下面的按键照常可用，布局为 取消 | 拖动区（44×5 的把手，下面是 '上下拖动调整 · N%'）| 重置 | 完成。
///
/// 向上拖升高、向下拖降低，每移动 `referenceHeight` 的百分之一就改变一个百分点，并限制在创建时给定的范围内。VoiceOver 以 5% 为步长调节拖动区。这个条的背景保持透明，自己不存任何数据：键盘在 `onChange` 里把百分比换算成自己的高度设置，在 `onCancel` 里恢复，在 `onDone` 里保存。
///
/// 它的按钮是普通控件而不是 `UIButton`：键盘的皮肤处理会把每个 configuration 带底色的 `UIButton` 画成键帽，这样它就没法把 完成 变成一个按键。
final class InlineHeightBar: UIView {
  static let defaultPercent = 100
  static let accessibilityStep = 5
  /// 设计稿的按键区，四排 42pt：在键盘传入自己的按键区高度之前，每拖动 1.68pt 改变一个百分点。
  static let designReferenceHeight: CGFloat = 4 * 42
  private static let rowSpacing: CGFloat = 2
  private static let rowPadding: CGFloat = 2
  private static let dragHeight: CGFloat = 44
  private static let handleSize = CGSize(width: 44, height: 5)
  private static let handleRadius: CGFloat = 3
  private static let handleAlpha: CGFloat = 0.35
  private static let handleGap: CGFloat = 5

  /// 把手下面那行 '上下拖动调整 · N%'。
  static func label(_ percent: Int) -> String { "上下拖动调整 · \(percent)%" }

  /// 从 `start` 开始纵向拖动 `dy` 点（向下为正）之后的百分比，`referenceHeight` 为 100% 时按键区的高度。
  static func percent(start: Int, dy: CGFloat, referenceHeight: CGFloat, range: ClosedRange<Int>) -> Int {
    guard referenceHeight > 0 else { return clamp(start, to: range) }
    return clamp(Int((CGFloat(start) - dy / referenceHeight * 100).rounded()), to: range)
  }

  private static func clamp(_ value: Int, to range: ClosedRange<Int>) -> Int {
    min(max(value, range.lowerBound), range.upperBound)
  }

  let range: ClosedRange<Int>
  /// 100% 时按键区的高度，拖动距离以它为基准；键盘把它设为自己按键区的高度。
  var referenceHeight: CGFloat = InlineHeightBar.designReferenceHeight
  private(set) var percent: Int

  private let onChange: (Int) -> Void
  private let onCancel: () -> Void
  private let onReset: () -> Void
  private let onDone: () -> Void

  private let cancelButton = BarButton(title: "取消", font: .systemFont(ofSize: 15), height: 36, padding: 12, pressedAlpha: 0.55)
  private let resetButton = BarButton(title: "重置", font: .systemFont(ofSize: 15), height: 36, padding: 10, pressedAlpha: 0.55)
  private let doneButton = BarButton(title: "完成", font: .systemFont(ofSize: 14, weight: .semibold), height: 32, padding: 14, pressedAlpha: 0.7)
  private let dragArea = DragArea()
  private let handle = UIView()
  private let valueLabel = UILabel()
  private var dragStart: Int?
  private var dragStartHeight: CGFloat = 0

  init(
    percent: Int, range: ClosedRange<Int>, onChange: @escaping (Int) -> Void, onCancel: @escaping () -> Void,
    onReset: @escaping () -> Void, onDone: @escaping () -> Void
  ) {
    self.range = range
    self.percent = min(max(percent, range.lowerBound), range.upperBound)
    self.onChange = onChange
    self.onCancel = onCancel
    self.onReset = onReset
    self.onDone = onDone
    super.init(frame: .zero)
    backgroundColor = .clear
    accessibilityIdentifier = "inlineHeightBar"

    handle.isUserInteractionEnabled = false
    handle.layer.cornerRadius = Self.handleRadius
    handle.alpha = Self.handleAlpha
    handle.translatesAutoresizingMaskIntoConstraints = false
    valueLabel.font = .systemFont(ofSize: 12)
    valueLabel.textAlignment = .center
    valueLabel.lineBreakMode = .byClipping
    valueLabel.adjustsFontSizeToFitWidth = true
    valueLabel.minimumScaleFactor = 0.75
    let dragStack = UIStackView(arrangedSubviews: [handle, valueLabel])
    dragStack.axis = .vertical
    dragStack.alignment = .center
    dragStack.spacing = Self.handleGap
    dragStack.isUserInteractionEnabled = false
    dragStack.translatesAutoresizingMaskIntoConstraints = false
    dragArea.addSubview(dragStack)
    dragArea.translatesAutoresizingMaskIntoConstraints = false
    dragArea.isAccessibilityElement = true
    dragArea.accessibilityIdentifier = "inlineHeightHandle"
    dragArea.accessibilityLabel = "键盘高度"
    dragArea.accessibilityHint = "上下拖动调整"
    dragArea.accessibilityTraits = .adjustable
    dragArea.onIncrement = { [weak self] in self?.step(by: Self.accessibilityStep) }
    dragArea.onDecrement = { [weak self] in self?.step(by: -Self.accessibilityStep) }
    dragArea.addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(handlePan(_:))))

    cancelButton.accessibilityIdentifier = "inlineHeightCancel"
    resetButton.accessibilityIdentifier = "inlineHeightReset"
    doneButton.accessibilityIdentifier = "inlineHeightDone"
    cancelButton.addAction(UIAction { [weak self] _ in self?.onCancel() }, for: .primaryActionTriggered)
    resetButton.addAction(UIAction { [weak self] _ in self?.reset() }, for: .primaryActionTriggered)
    doneButton.addAction(UIAction { [weak self] _ in self?.onDone() }, for: .primaryActionTriggered)

    let row = UIStackView(arrangedSubviews: [cancelButton, dragArea, resetButton, doneButton])
    row.axis = .horizontal
    row.alignment = .center
    row.spacing = Self.rowSpacing
    row.translatesAutoresizingMaskIntoConstraints = false
    addSubview(row)
    for button in [cancelButton, resetButton, doneButton] {
      button.setContentHuggingPriority(.required, for: .horizontal)
      button.setContentCompressionResistancePriority(.required, for: .horizontal)
    }
    dragArea.setContentHuggingPriority(.defaultLow, for: .horizontal)
    dragArea.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    valueLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    NSLayoutConstraint.activate([
      row.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Self.rowPadding),
      row.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Self.rowPadding),
      row.topAnchor.constraint(equalTo: topAnchor),
      row.bottomAnchor.constraint(equalTo: bottomAnchor),
      dragArea.heightAnchor.constraint(equalToConstant: Self.dragHeight),
      handle.widthAnchor.constraint(equalToConstant: Self.handleSize.width),
      handle.heightAnchor.constraint(equalToConstant: Self.handleSize.height),
      dragStack.centerXAnchor.constraint(equalTo: dragArea.centerXAnchor),
      dragStack.centerYAnchor.constraint(equalTo: dragArea.centerYAnchor),
      dragStack.leadingAnchor.constraint(greaterThanOrEqualTo: dragArea.leadingAnchor),
      dragStack.trailingAnchor.constraint(lessThanOrEqualTo: dragArea.trailingAnchor),
      valueLabel.widthAnchor.constraint(lessThanOrEqualTo: dragArea.widthAnchor),
    ])
    updateValue()
    apply(skin: KeyboardTheme.current)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 显示限制在范围内的 `value`，不调用 `onChange`：供键盘把这个条与自己的设置同步。
  func setPercent(_ value: Int) {
    let next = Self.clamp(value, to: range)
    guard next != percent else { return }
    percent = next
    updateValue()
  }

  /// 取消 和 重置 用皮肤的次要文字色，把手用按键前景色的 .35，完成 画成操作色的胶囊：带调色板的主题一律是强调色配白字，深色强调色上用 Rust 给出的加深文字色，键盘设计则用它自己的操作色和可读的文字色。
  func apply(skin: KeyboardTheme) {
    cancelButton.setColors(text: skin.secondary, fill: nil)
    resetButton.setColors(text: skin.secondary, fill: nil)
    doneButton.setColors(text: skin.actionForeground, fill: skin.actionBackground)
    handle.backgroundColor = skin.keyForeground
    valueLabel.textColor = skin.secondary
  }

  private func reset() {
    setPercent(Self.defaultPercent)
    onReset()
  }

  private func step(by delta: Int) {
    change(to: percent + delta)
  }

  private func change(to value: Int) {
    let next = Self.clamp(value, to: range)
    guard next != percent else { return }
    percent = next
    updateValue()
    onChange(next)
  }

  private func updateValue() {
    let text = Self.label(percent)
    valueLabel.text = text
    dragArea.accessibilityValue = "\(percent)%"
  }

  /// 手指到键盘窗口底边的高度。键盘以在屏幕上保持不动的底边为基准向上长高，而这个条随顶边移动，所以用这个量，而不是条自身坐标系里的位移，就不会追着拖动本身引起的尺寸变化跑。
  private func fingerHeight(_ recognizer: UIPanGestureRecognizer) -> CGFloat {
    guard let window else { return -recognizer.location(in: self).y }
    return window.bounds.maxY - recognizer.location(in: window).y
  }

  @objc private func handlePan(_ recognizer: UIPanGestureRecognizer) {
    switch recognizer.state {
    case .began:
      dragStart = percent
      dragStartHeight = fingerHeight(recognizer)
    case .changed:
      guard let start = dragStart else { return }
      let dy = dragStartHeight - fingerHeight(recognizer)
      change(to: Self.percent(start: start, dy: dy, referenceHeight: referenceHeight, range: range))
    default:
      dragStart = nil
    }
  }

  /// 拖动区：把 VoiceOver 的可调节增减转给这个条。
  private final class DragArea: UIView {
    var onIncrement: (() -> Void)?
    var onDecrement: (() -> Void)?

    override func accessibilityIncrement() { onIncrement?() }
    override func accessibilityDecrement() { onDecrement?() }
  }

  /// 这个条上的文字按钮：标签左右各留 `padding`，高 `height`，可选胶囊底色，按住时淡到 `pressedAlpha`。
  private final class BarButton: UIControl {
    private let label = UILabel()
    private let height: CGFloat
    private let padding: CGFloat
    private let pressedAlpha: CGFloat

    init(title: String, font: UIFont, height: CGFloat, padding: CGFloat, pressedAlpha: CGFloat) {
      self.height = height
      self.padding = padding
      self.pressedAlpha = pressedAlpha
      super.init(frame: .zero)
      label.text = title
      label.font = font
      label.textAlignment = .center
      label.isUserInteractionEnabled = false
      addSubview(label)
      layer.cornerCurve = .continuous
      isAccessibilityElement = true
      accessibilityLabel = title
      accessibilityTraits = .button
      // `UIControl` 只报告触摸；完成的点按会变成 primary action，所以点按和 `sendActions(for: .primaryActionTriggered)` 走同一个处理函数。
      addAction(UIAction { [weak self] _ in self?.sendActions(for: .primaryActionTriggered) }, for: .touchUpInside)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func setColors(text: UIColor, fill: UIColor?) {
      label.textColor = text
      backgroundColor = fill ?? .clear
    }

    override var isHighlighted: Bool {
      didSet { alpha = isHighlighted ? pressedAlpha : 1 }
    }

    override var intrinsicContentSize: CGSize {
      CGSize(width: ceil(label.intrinsicContentSize.width) + padding * 2, height: height)
    }

    override func layoutSubviews() {
      super.layoutSubviews()
      label.frame = bounds.insetBy(dx: padding, dy: 0)
      layer.cornerRadius = bounds.height / 2
    }
  }
}
