import UIKit

/// 键盘工具栏上的一个图标，按设计稿移动端工具栏的画法（dc.html L2200-2205）：22pt 的 `KeyboardIcon` 线框（「收起」箭头为 21pt），在所在单元格里居中。平时图标用 kbSub（皮肤的 `secondary`），与 Android 的 `toolbarIcon` 一致；「收起」箭头与 Android 的收起键一样用按键前景色。
///
/// 对应面板打开时（`isActive`），图标后面出现一块 40pt、圆角 12 的 `toolbarActiveBackground` 底板，图标变成强调色，两者都在 .15s 内过渡；按下时按钮缩放到 .92。它没有 configuration，所以键盘的皮肤处理（会把 configuration 带填充的每个 UIButton 画成键帽）不会动它。
final class KeyboardToolbarButton: UIButton {
  static let iconSide: CGFloat = 22
  static let collapseIconSide: CGFloat = 21
  static let padSide: CGFloat = 40
  static let padRadius: CGFloat = 12
  static let pressScale: CGFloat = 0.92
  static let pressDuration: TimeInterval = 0.1
  static let activeDuration: TimeInterval = 0.15
  private static let disabledAlpha: CGFloat = 0.4

  let icon: KeyboardIcon
  private let pad = UIView()
  private let iconLayer = CAShapeLayer()
  private var skin = KeyboardTheme.current

  /// 这个按钮对应的面板是否打开：打开时显示底板，图标变成强调色。
  var isActive = false {
    didSet {
      guard isActive != oldValue else { return }
      updateActive(animated: window != nil)
      accessibilityTraits = isActive ? [.button, .selected] : .button
    }
  }

  init(icon: KeyboardIcon, accessibilityLabel: String) {
    self.icon = icon
    super.init(frame: .zero)
    backgroundColor = .clear
    pad.isUserInteractionEnabled = false
    pad.layer.cornerRadius = Self.padRadius
    pad.layer.cornerCurve = .continuous
    addSubview(pad)
    iconLayer.lineCap = .round
    iconLayer.lineJoin = .round
    layer.addSublayer(iconLayer)
    self.accessibilityLabel = accessibilityLabel
    accessibilityTraits = .button
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: KeyboardToolbarButton, _: UITraitCollection) in
      button.applyIconColor()
    }
    apply(skin: skin)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 按 `skin` 重画：图标平时用它的 `secondary`，「收起」箭头用 `keyForeground`，激活时用它的 `accent`，底板用它的 `toolbarActiveBackground`。
  func apply(skin: KeyboardTheme) {
    self.skin = skin
    updateActive(animated: false)
  }

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      KeyPressMotion.animate(self, scale: isHighlighted && isEnabled ? Self.pressScale : 1, duration: Self.pressDuration)
    }
  }

  override var isEnabled: Bool {
    didSet {
      alpha = isEnabled ? 1 : Self.disabledAlpha
      if !isEnabled { KeyPressMotion.animate(self, scale: 1, duration: Self.pressDuration) }
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      layer.removeAllAnimations()
      transform = .identity
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let center = CGPoint(x: bounds.midX, y: bounds.midY)
    let padSide = min(Self.padSide, bounds.width, bounds.height)
    pad.bounds = CGRect(x: 0, y: 0, width: padSide, height: padSide)
    pad.center = center
    pad.layer.cornerRadius = min(Self.padRadius, padSide / 2)
    let side = icon == .collapse ? Self.collapseIconSide : Self.iconSide
    let rect = CGRect(x: center.x - side / 2, y: center.y - side / 2, width: side, height: side)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    iconLayer.frame = bounds
    iconLayer.path = icon.path(in: rect).cgPath
    // 描边宽度以 viewBox 单位给出：1.7，箭头为 1.8，按图标自身的缩放换算。
    iconLayer.lineWidth = icon.isFilled ? 0 : icon.lineWidth * side / KeyboardIcon.viewBox
    CATransaction.commit()
  }

  private var iconColor: UIColor {
    if isActive { return skin.accent }
    // 表情、常用语、剪贴板、皮肤、输入方式平时用 kbSub（与 Android `toolbarIcon` 一致）；收起箭头与 Android 的收起键一样用键面文字色。
    return icon == .collapse ? skin.keyForeground : skin.secondary
  }

  private func applyIconColor() {
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    setIconColor(iconColor.resolvedColor(with: traitCollection).cgColor)
    CATransaction.commit()
  }

  private func setIconColor(_ color: CGColor) {
    if icon.isFilled {
      iconLayer.fillColor = color
      iconLayer.strokeColor = nil
    } else {
      iconLayer.fillColor = nil
      iconLayer.strokeColor = color
    }
  }

  private func updateActive(animated: Bool) {
    let background = isActive ? skin.toolbarActiveBackground : .clear
    guard animated else {
      pad.backgroundColor = background
      applyIconColor()
      return
    }
    UIView.animate(withDuration: Self.activeDuration, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
      self.pad.backgroundColor = background
    }
    // 图标是独立的 layer，颜色变化会隐式动画；给它与底板相同的时长。
    CATransaction.begin()
    CATransaction.setAnimationDuration(Self.activeDuration)
    setIconColor(iconColor.resolvedColor(with: traitCollection).cgColor)
    CATransaction.commit()
  }
}
