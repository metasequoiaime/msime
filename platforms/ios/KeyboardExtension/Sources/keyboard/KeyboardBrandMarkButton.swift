import UIKit

/// 工具栏品牌按钮：36pt 圆底与 24pt 官方水杉标志，使用原生按钮图像保留系统键盘命中。
final class KeyboardBrandMarkButton: UIButton {
  static let discSide: CGFloat = 36
  static let markSide: CGFloat = 24
  static let pressScale: CGFloat = 0.92
  private static let disabledAlpha: CGFloat = 0.4

  private var renderedSize = CGSize.zero
  private var skin = KeyboardTheme.current

  var isActive = false {
    didSet {
      guard isActive != oldValue else { return }
      accessibilityTraits = isActive ? [.button, .selected] : .button
    }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    backgroundColor = .clear
    accessibilityLabel = "功能"
    accessibilityTraits = .button
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: KeyboardBrandMarkButton, _: UITraitCollection) in
      button.renderMark(size: button.renderedSize)
    }
    apply(skin: skin)
    imageView?.accessibilityIdentifier = "keyboardBrandIcon"
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  func apply(skin: KeyboardTheme) {
    self.skin = skin
    let size = bounds.width > 0 && bounds.height > 0 ? bounds.size : CGSize(width: Self.discSide, height: Self.discSide)
    renderMark(size: size)
  }

  override var isEnabled: Bool {
    didSet {
      alpha = isEnabled ? 1 : Self.disabledAlpha
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      imageView?.resetKeyboardPressAnimationAfterWindowRemoval()
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    if bounds.width > 0, bounds.height > 0, renderedSize != bounds.size {
      renderMark(size: bounds.size)
      super.layoutSubviews()
    }
  }

  /// 图像覆盖整个按钮，圆底和官方标志在中央绘制，透明部分保留整格点击范围。
  private func renderMark(size: CGSize) {
    func glyph(scale: CGFloat) -> UIImage {
      let disc = min(Self.discSide, size.width, size.height) * scale
      let discRect = CGRect(x: (size.width - disc) / 2, y: (size.height - disc) / 2, width: disc, height: disc)
      let mark = disc * Self.markSide / Self.discSide
      let markRect = CGRect(x: (size.width - mark) / 2, y: (size.height - mark) / 2, width: mark, height: mark)
      return UIGraphicsImageRenderer(size: size).image { _ in
        skin.logoCircle.resolvedColor(with: traitCollection).setFill()
        UIBezierPath(ovalIn: discRect).fill()
        skin.logoMark.resolvedColor(with: traitCollection).setFill()
        UIBezierPath(cgPath: MSIMELogo.framePath(in: markRect)).fill()
        let stroke = UIBezierPath(cgPath: MSIMELogo.strokePath(in: markRect))
        stroke.lineWidth = MSIMELogo.strokeWidth * MSIMELogo.scale(for: markRect)
        stroke.lineCapStyle = .round
        stroke.lineJoinStyle = .round
        UIColor.white.setStroke()
        stroke.stroke()
      }.withRenderingMode(.alwaysOriginal)
    }
    renderedSize = size
    let normal = glyph(scale: 1)
    setImage(normal, for: .normal)
    setImage(normal, for: .disabled)
    setImage(glyph(scale: Self.pressScale), for: .highlighted)
  }
}
