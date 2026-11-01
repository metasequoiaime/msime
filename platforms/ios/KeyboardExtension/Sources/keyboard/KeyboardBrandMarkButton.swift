import UIKit

/// 工具栏品牌按钮：24pt 官方水杉标志（固定配色的深色圆角正方形、绿边和白色折线，本身就是外框，不垫圆底，也不跟随皮肤），使用原生按钮图像保留系统键盘命中。按钮仍按原来 36pt 圆底的占位随行高缩放。
final class KeyboardBrandMarkButton: UIButton {
  static let discSide: CGFloat = 36
  static let markSide: CGFloat = 24
  static let pressScale: CGFloat = 0.92
  private static let disabledAlpha: CGFloat = 0.4

  private var renderedSize = CGSize.zero

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
    apply(skin: KeyboardTheme.current)
    imageView?.accessibilityIdentifier = "keyboardBrandIcon"
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 标志配色固定，不取皮肤的颜色；保留这个入口是为了和工具栏其他按钮一样在换皮肤时重新绘制。
  func apply(skin _: KeyboardTheme) {
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

  /// 图像覆盖整个按钮，官方标志在中央绘制，透明部分保留整格点击范围。
  private func renderMark(size: CGSize) {
    func glyph(scale: CGFloat) -> UIImage {
      // 和工具栏其他按钮一样随行高缩小（`KeyboardToolbarButton.contentScale`），「候选栏大小」调小时标志不伸出这一行。
      let slot = min(Self.discSide * KeyboardToolbarButton.contentScale(rowHeight: size.height), size.width, size.height) * scale
      let mark = slot * Self.markSide / Self.discSide
      let markRect = CGRect(x: (size.width - mark) / 2, y: (size.height - mark) / 2, width: mark, height: mark)
      return UIGraphicsImageRenderer(size: size).image { _ in
        MSIMELogo.draw(in: markRect)
      }.withRenderingMode(.alwaysOriginal)
    }
    renderedSize = size
    let normal = glyph(scale: 1)
    setImage(normal, for: .normal)
    setImage(normal, for: .disabled)
    setImage(glyph(scale: Self.pressScale), for: .highlighted)
  }
}
