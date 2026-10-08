import UIKit

/// 工具栏的「功能」按钮：30pt 圆盘上的官方水杉标志，与设计稿移动端工具栏的画法（dc.html L2197-2198）和 Android KeyboardBrandButton 的绘制一致。
///
/// 圆盘用 `logoCircle`，标志的框用 `logoMark`，两者都取 App 主题的季节而不是皮肤，最上面是白色的折线描边。功能菜单、任一面板或候选网格打开时（`isActive`），40pt 的格子会垫上一块圆角 12 的 `toolbarActiveBackground` 底板。它没有 configuration，所以键盘的皮肤处理（会把每个 configuration 带填充的 UIButton 画成键帽）不会动它。
final class KeyboardBrandMarkButton: UIButton {
  static let padSide: CGFloat = 40
  static let padRadius: CGFloat = 12
  static let discSide: CGFloat = 30
  static let markSide: CGFloat = 18
  static let pressScale: CGFloat = 0.92
  static let pressDuration: TimeInterval = 0.1
  static let activeDuration: TimeInterval = 0.15
  private static let disabledAlpha: CGFloat = 0.4

  /// 有面板打开时垫在标志后面的底板。
  private let pad = UIView()
  /// 圆盘和标志；测试在 `moreShortcut` 下按 `keyboardBrandIcon` 找到它。
  let markView = UIView()
  private let discLayer = CAShapeLayer()
  private let frameLayer = CAShapeLayer()
  private let strokeLayer = CAShapeLayer()
  private var skin = KeyboardTheme.current

  /// 功能菜单、某个面板或候选网格是否打开；打开时画出底板。
  var isActive = false {
    didSet {
      guard isActive != oldValue else { return }
      updatePad(animated: window != nil)
      accessibilityTraits = isActive ? [.button, .selected] : .button
    }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    backgroundColor = .clear
    pad.isUserInteractionEnabled = false
    pad.layer.cornerRadius = Self.padRadius
    pad.layer.cornerCurve = .continuous
    addSubview(pad)
    markView.isUserInteractionEnabled = false
    markView.accessibilityIdentifier = "keyboardBrandIcon"
    strokeLayer.fillColor = nil
    strokeLayer.strokeColor = UIColor.white.cgColor
    strokeLayer.lineCap = .round
    strokeLayer.lineJoin = .round
    for shape in [discLayer, frameLayer, strokeLayer] { markView.layer.addSublayer(shape) }
    addSubview(markView)
    accessibilityLabel = "功能"
    accessibilityTraits = .button
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: KeyboardBrandMarkButton, _: UITraitCollection) in
      button.applyColors()
    }
    apply(skin: skin)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 按 `skin` 重绘：底板取它的 `toolbarActiveBackground`；圆盘和标志无论什么皮肤都跟随 App 主题的季节。
  func apply(skin: KeyboardTheme) {
    self.skin = skin
    applyColors()
    updatePad(animated: false)
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
    let shorter = min(bounds.width, bounds.height)
    let padSide = min(Self.padSide, shorter)
    pad.bounds = CGRect(x: 0, y: 0, width: padSide, height: padSide)
    pad.center = center
    pad.layer.cornerRadius = min(Self.padRadius, padSide / 2)
    let disc = min(Self.discSide, shorter)
    markView.bounds = CGRect(x: 0, y: 0, width: disc, height: disc)
    markView.center = center
    let markSide = disc * Self.markSide / Self.discSide
    let mark = CGRect(x: (disc - markSide) / 2, y: (disc - markSide) / 2, width: markSide, height: markSide)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    discLayer.path = UIBezierPath(ovalIn: markView.bounds).cgPath
    frameLayer.path = MSIMELogo.framePath(in: mark)
    strokeLayer.path = MSIMELogo.strokePath(in: mark)
    strokeLayer.lineWidth = MSIMELogo.strokeWidth * MSIMELogo.scale(for: mark)
    CATransaction.commit()
  }

  private func applyColors() {
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    discLayer.fillColor = skin.logoCircle.resolvedColor(with: traitCollection).cgColor
    frameLayer.fillColor = skin.logoMark.resolvedColor(with: traitCollection).cgColor
    CATransaction.commit()
  }

  private func updatePad(animated: Bool) {
    let color = isActive ? skin.toolbarActiveBackground : .clear
    guard animated else {
      pad.backgroundColor = color
      return
    }
    UIView.animate(withDuration: Self.activeDuration, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
      self.pad.backgroundColor = color
    }
  }
}
