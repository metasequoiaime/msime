import UIKit

/// 装饰性的双拼底部提示由按键直接排版，不显式安装与按键相连的三条边约束。
final class KeyboardBottomHintLabel: UILabel {
  init() {
    super.init(frame: .zero)
    font = .systemFont(ofSize: 9, weight: .regular)
    textColor = KeyboardTheme.current.accent.withAlphaComponent(0.8)
    textAlignment = .center
    numberOfLines = 1
    adjustsFontSizeToFitWidth = true
    minimumScaleFactor = 0.6
    isHidden = true
    isAccessibilityElement = false
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func invalidateIntrinsicContentSize() {
    super.invalidateIntrinsicContentSize()
    // 提示可以在按键尺寸不变时从空文本变成韵母，父视图仍需重排。
    superview?.setNeedsLayout()
  }

  func layout(in keyBounds: CGRect) {
    guard keyBounds.width > 4, keyBounds.height > 2 else {
      frame = .zero
      return
    }
    let width = keyBounds.width - 4
    let height = min(keyBounds.height - 2, max(0, sizeThatFits(
      CGSize(width: width, height: .greatestFiniteMagnitude)).height))
    frame = CGRect(x: keyBounds.minX + 2, y: keyBounds.maxY - 2 - height, width: width, height: height)
  }
}
