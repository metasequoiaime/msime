import UIKit

/// Keyboard spacing and height are adjusted over the live keyboard rather than in an opaque form.
///
/// The keyboard remains visible below the small toolbar: drag the height control, drag
/// sideways on the keyboard to change key spacing, and drag vertically to change row spacing.
///
/// 高度仍然靠拖,但拖的是数值本身而不是一根 46×5pt 的细条:目标大得多,而且数值常驻 —— 原先它只在拖动
/// 过程中出现,想知道现在是多少就得先改一下。
final class KeyboardLayoutPickerView: UIView {
  private static let spacingDragScale: Double = 18
  // 一行:恢复默认 / 高度 / 语音 / 完成。说明和拖动时的数值不在这里 —— 它们浮在键盘中央,见 hint。
  private static let barHeight: CGFloat = 52
  private static let heightRange: (lower: Double, upper: Double) = (-12, 48)

  private enum Axis { case vertical, horizontal }

  private let onKeySpacing: (Double) -> Void
  private let onRowSpacing: (Double) -> Void
  private let onHeight: (Double) -> Void
  /// Called when a drag ends, so the value it settled on can be written somewhere slower than the
  /// live keyboard. The drags themselves report on every gesture frame.
  private let onCommit: () -> Void
  private var keySpacing: Double
  private var rowSpacing: Double
  private var height: Double
  private let hint = PaddedLabel()
  private let heightValue = UILabel()
  private weak var heightControl: HeightGripView?
  private var axis: Axis?
  private var base: (key: Double, row: Double, height: Double) = (0, 0, 0)

  init(keySpacing: Double, rowSpacing: Double, height: Double,
       onKeySpacing: @escaping (Double) -> Void,
       onRowSpacing: @escaping (Double) -> Void,
       onHeight: @escaping (Double) -> Void,
       onCommit: @escaping () -> Void,
       onReset: @escaping () -> Void,
       onClose: @escaping () -> Void) {
    self.keySpacing = keySpacing
    self.rowSpacing = rowSpacing
    self.height = height
    self.onKeySpacing = onKeySpacing
    self.onRowSpacing = onRowSpacing
    self.onHeight = onHeight
    self.onCommit = onCommit
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardLayoutPicker"
    let skin = KeyboardSkinPreference.selected
    backgroundColor = .clear

    let bar = UIView()
    bar.backgroundColor = skin.keyBackground
    bar.layer.cornerRadius = 10

    let close = UIButton(type: .system)
    close.setTitle("完成", for: .normal)
    close.titleLabel?.font = .systemFont(ofSize: 15, weight: .medium)
    close.setTitleColor(skin.accent, for: .normal)
    close.accessibilityIdentifier = "closeLayoutPicker"
    close.accessibilityLabel = "返回键盘"
    close.addAction(UIAction { _ in onClose() }, for: .primaryActionTriggered)

    let reset = UIButton(type: .system)
    reset.setTitle("恢复默认", for: .normal)
    reset.titleLabel?.font = .systemFont(ofSize: 14, weight: .medium)
    reset.setTitleColor(.systemRed, for: .normal)
    reset.accessibilityIdentifier = "resetKeyboardSettings"
    reset.accessibilityLabel = "恢复默认"
    reset.accessibilityHint = "把间距和高度恢复成默认值"
    reset.addAction(UIAction { _ in onReset() }, for: .primaryActionTriggered)

    // 说明和拖动时的数值浮在键盘中央,不占工具条。
    //
    // 放回条里要多一行,而条里已经有四组控件 —— 那正是上一版把第一行压到说明文字上的原因。挪出来之后
    // 条退回一行,省下的高度还给键盘;更要紧的是数值出现在视线中央、压在正在改的东西上,而不是逼着
    // 眼睛在键盘和顶栏之间来回切。
    hint.font = .systemFont(ofSize: 13)
    hint.textColor = .white
    hint.textAlignment = .center
    hint.numberOfLines = 2
    hint.adjustsFontSizeToFitWidth = true
    hint.minimumScaleFactor = 0.7
    hint.backgroundColor = UIColor(red: 20 / 255, green: 35 / 255, blue: 29 / 255, alpha: 0.82)
    hint.layer.cornerRadius = 12
    hint.layer.masksToBounds = true
    // 浮层压在键盘上,不能吃掉落在它下面的拖动。
    hint.isUserInteractionEnabled = false
    hint.accessibilityIdentifier = "layoutAdjustHint"
    updateHint()

    heightValue.font = .monospacedDigitSystemFont(ofSize: 14, weight: .medium)
    heightValue.textColor = skin.keyForeground
    heightValue.textAlignment = .center
    heightValue.adjustsFontSizeToFitWidth = true
    heightValue.minimumScaleFactor = 0.7
    heightValue.accessibilityIdentifier = "keyboardHeightValue"

    let grip = UIView()
    grip.backgroundColor = skin.accent.withAlphaComponent(0.45)
    grip.layer.cornerRadius = 2.5
    grip.isUserInteractionEnabled = false
    grip.translatesAutoresizingMaskIntoConstraints = false

    let heightControl = HeightGripView()
    self.heightControl = heightControl
    heightControl.backgroundColor = .clear
    heightControl.accessibilityIdentifier = "keyboardHeightGrip"
    heightControl.accessibilityLabel = "键盘高度"
    heightControl.isAccessibilityElement = true
    heightControl.accessibilityTraits = .adjustable
    heightControl.onAdjust = { [weak self] delta in self?.adjustHeight(by: delta) }
    heightControl.addGestureRecognizer(
      UIPanGestureRecognizer(target: self, action: #selector(dragHeight(_:))))

    // 数值本身就是把手:整块可拖,比原先那根 46×5pt 的细条好抓得多,而且不用先改一下才知道现在是多少。
    // 下面那道横条只是在说「这里可以拖」—— 光一行文字看不出它是个控件。
    let heightStack = UIStackView(arrangedSubviews: [heightValue, grip])
    heightStack.axis = .vertical
    heightStack.alignment = .center
    heightStack.spacing = 5
    heightStack.translatesAutoresizingMaskIntoConstraints = false
    heightControl.addSubview(heightStack)
    NSLayoutConstraint.activate([
      heightStack.leadingAnchor.constraint(equalTo: heightControl.leadingAnchor),
      heightStack.trailingAnchor.constraint(equalTo: heightControl.trailingAnchor),
      heightStack.centerYAnchor.constraint(equalTo: heightControl.centerYAnchor),
      grip.widthAnchor.constraint(equalToConstant: 46),
      grip.heightAnchor.constraint(equalToConstant: 5),
    ])

    addGestureRecognizer(UIPanGestureRecognizer(target: self, action: #selector(dragSpacing(_:))))

    for item in [bar, close, reset, hint, heightControl] {
      item.translatesAutoresizingMaskIntoConstraints = false
      addSubview(item)
    }
    NSLayoutConstraint.activate([
      bar.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      bar.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      bar.topAnchor.constraint(equalTo: topAnchor, constant: 4),
      bar.heightAnchor.constraint(equalToConstant: Self.barHeight),
      close.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -12),
      reset.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 12),
      // 高度占第一行的中段,间距说明退到第二行 —— 高度的数值现在常驻,说明不再需要和它抢同一行。
      heightControl.centerXAnchor.constraint(equalTo: bar.centerXAnchor),
      heightControl.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
      heightControl.heightAnchor.constraint(equalToConstant: 32),
      heightControl.leadingAnchor.constraint(greaterThanOrEqualTo: reset.trailingAnchor, constant: 6),
      heightControl.trailingAnchor.constraint(lessThanOrEqualTo: close.leadingAnchor, constant: -8),
      close.centerYAnchor.constraint(equalTo: heightControl.centerYAnchor),
      reset.centerYAnchor.constraint(equalTo: heightControl.centerYAnchor),
      hint.centerXAnchor.constraint(equalTo: centerXAnchor),
      hint.centerYAnchor.constraint(equalTo: centerYAnchor),
      hint.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, multiplier: 0.78),
    ])
    updateHeightValue()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  override func accessibilityIncrement() { adjustHeight(by: 2) }
  override func accessibilityDecrement() { adjustHeight(by: -2) }

  private func adjustHeight(by delta: Double) {
    height = KeyboardGeometry.clamped(height + delta, Self.heightRange.lower, Self.heightRange.upper)
    onHeight(height)
    onCommit()
    updateHeightValue()
  }

  private func updateHeightValue() {
    heightValue.text = "高度 " + Self.format(height)
    // 可调节元素读出来的就是这一个数,不再需要用户先改一下才知道现在是多少。
    heightControl?.accessibilityValue = Self.format(height)
  }

  private static func format(_ value: Double) -> String {
    value > 0 ? "+\(Int(value))" : "\(Int(value))"
  }

  @objc private func dragHeight(_ gesture: UIPanGestureRecognizer) {
    switch gesture.state {
    case .began:
      base = (keySpacing, rowSpacing, height)
    case .changed:
      height = KeyboardGeometry.clamped(base.height - Double(gesture.translation(in: self).y),
                                        Self.heightRange.lower, Self.heightRange.upper)
      onHeight(height)
      updateHeightValue()
    default:
      axis = nil
      onCommit()
      updateHint()
    }
  }

  @objc private func dragSpacing(_ gesture: UIPanGestureRecognizer) {
    let translation = gesture.translation(in: self)
    switch gesture.state {
    case .began:
      base = (keySpacing, rowSpacing, height)
      axis = nil
    case .changed:
      let direction = axis ?? (abs(translation.y) >= abs(translation.x) ? .vertical : .horizontal)
      axis = direction
      switch direction {
      case .vertical:
        rowSpacing = KeyboardGeometry.clamped(base.row + Double(translation.y) / Self.spacingDragScale, 4, 10)
        onRowSpacing(rowSpacing)
        updateHint(String(format: "行间距 %.1f", rowSpacing))
      case .horizontal:
        keySpacing = KeyboardGeometry.clamped(base.key + Double(translation.x) / Self.spacingDragScale, 3, 6)
        onKeySpacing(keySpacing)
        updateHint(String(format: "按键间距 %.1f", keySpacing))
      }
    default:
      axis = nil
      onCommit()
      updateHint()
    }
  }

  private func updateHint(_ value: String? = nil) {
    hint.text = value ?? String(format: "键距 %.1f · 行距 %.1f\n左右拖改键距，上下拖改行间距",
                                keySpacing, rowSpacing)
  }

}

/// 浮层要的是一圈内边距,而 UILabel 只会把文字贴着自己的边。
private final class PaddedLabel: UILabel {
  private static let inset = UIEdgeInsets(top: 9, left: 16, bottom: 9, right: 16)

  override func drawText(in rect: CGRect) {
    super.drawText(in: rect.inset(by: Self.inset))
  }

  override var intrinsicContentSize: CGSize {
    let size = super.intrinsicContentSize
    return CGSize(width: size.width + Self.inset.left + Self.inset.right,
                  height: size.height + Self.inset.top + Self.inset.bottom)
  }

  override func textRect(forBounds bounds: CGRect, limitedToNumberOfLines lines: Int) -> CGRect {
    let rect = super.textRect(forBounds: bounds.inset(by: Self.inset), limitedToNumberOfLines: lines)
    return rect.inset(by: UIEdgeInsets(top: -Self.inset.top, left: -Self.inset.left,
                                       bottom: -Self.inset.bottom, right: -Self.inset.right))
  }
}

private final class HeightGripView: UIView {
  var onAdjust: ((Double) -> Void)?

  override func accessibilityIncrement() { onAdjust?(2) }
  override func accessibilityDecrement() { onAdjust?(-2) }
}
