import UIKit

/// 工具栏使用官方路径绘制 26pt 图标（收起箭头为 25pt），通过 UIButton 原生图像提供给系统键盘宿主。
/// 图像覆盖整个按钮，图标居中，透明边缘也属于点击区域；按压只缩小图标和底板，原生图像边界保持不变。
final class KeyboardToolbarButton: UIButton {
  static let iconSide: CGFloat = 26
  static let collapseIconSide: CGFloat = 25
  static let padSide: CGFloat = 40
  static let padRadius: CGFloat = 12
  static let pressScale: CGFloat = 0.92
  static let pressDuration: TimeInterval = 0.1
  static let activeDuration: TimeInterval = 0.15
  private static let disabledAlpha: CGFloat = 0.4

  let icon: KeyboardIcon
  private let pad = UIView()
  private var renderedSize = CGSize.zero
  private var skin = KeyboardTheme.current

  /// 对应面板打开时显示底板和强调色。
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
    self.accessibilityLabel = accessibilityLabel
    accessibilityTraits = .button
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: KeyboardToolbarButton, _: UITraitCollection) in
      button.tintColor = button.iconColor
    }
    renderIcon(size: CGSize(width: Self.padSide, height: Self.padSide))
    apply(skin: skin)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  func apply(skin: KeyboardTheme) {
    self.skin = skin
    updateActive(animated: false)
  }

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      animatePress(scale: isHighlighted && isEnabled ? Self.pressScale : 1)
    }
  }

  override var isEnabled: Bool {
    didSet {
      alpha = isEnabled ? 1 : Self.disabledAlpha
      if !isEnabled { animatePress(scale: 1) }
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      pad.resetKeyboardPressAnimationAfterWindowRemoval()
      imageView?.resetKeyboardPressAnimationAfterWindowRemoval()
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    if bounds.width > 0, bounds.height > 0, renderedSize != bounds.size {
      renderIcon(size: bounds.size)
      super.layoutSubviews()
    }
    let side = min(Self.padSide, bounds.width, bounds.height)
    pad.bounds = CGRect(x: 0, y: 0, width: side, height: side)
    pad.center = CGPoint(x: bounds.midX, y: bounds.midY)
    pad.layer.cornerRadius = min(Self.padRadius, side / 2)
  }

  /// UIKit 的跨进程键盘命中使用按钮的原生内容；仅挂 CAShapeLayer 的空按钮在真机上收不到触摸。
  private func renderIcon(size: CGSize) {
    func glyph(scale: CGFloat) -> UIImage {
      let side = (icon == .collapse ? Self.collapseIconSide : Self.iconSide) * scale
      let rect = CGRect(x: (size.width - side) / 2, y: (size.height - side) / 2, width: side, height: side)
      return UIGraphicsImageRenderer(size: size).image { _ in
        let path = icon.path(in: rect)
        UIColor.black.set()
        if icon.isFilled {
          path.fill()
        } else {
          path.lineWidth = icon.lineWidth * side / KeyboardIcon.viewBox
          path.lineCapStyle = .round
          path.lineJoinStyle = .round
          path.stroke()
        }
      }.withRenderingMode(.alwaysTemplate)
    }
    renderedSize = size
    let normal = glyph(scale: 1)
    setImage(normal, for: .normal)
    setImage(normal, for: .disabled)
    setImage(glyph(scale: Self.pressScale), for: .highlighted)
  }

  private func animatePress(scale: CGFloat) {
    // 高亮图像只缩小内部路径，原生 imageView 的边界始终覆盖整格。
    KeyPressMotion.animate(pad, scale: scale, duration: Self.pressDuration)
  }

  private var iconColor: UIColor {
    if isActive { return skin.accent }
    return icon == .collapse ? skin.keyForeground : skin.secondary
  }

  private func updateActive(animated: Bool) {
    let background = isActive ? skin.toolbarActiveBackground : .clear
    guard animated else {
      pad.backgroundColor = background
      tintColor = iconColor
      return
    }
    UIView.animate(withDuration: Self.activeDuration, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
      self.pad.backgroundColor = background
      self.tintColor = self.iconColor
    }
  }
}
