import UIKit

/// 手写笔迹下面的书写卡片，真机面板和模拟器构建的替身共用，两边画出的书写板一样。
///
/// 卡片是一块按键表面：皮肤的按键填充色、按键圆角，下面带按键的投影。设计稿的辅助线用细线色画在上面：一个从边缘内缩 `guideInset`、圆角 8pt 的虚线框，以及穿过框中心的虚线十字。笔迹由画布自己画在最上层。
enum HandwritingCanvasCard {
  /// 虚线辅助框相对卡片边缘的内缩距离（dc.html：inset 14）。
  static let guideInset: CGFloat = 14
  /// 辅助框的圆角半径（dc.html：r8）。
  static let guideRadius: CGFloat = 8
  /// 辅助线的虚线样式。
  static let guideDash: [CGFloat] = [4, 4]

  /// `bounds` 里的卡片：整个区域减去底部的投影偏移，让按键投影留在绘制它的视图之内。
  static func cardRect(in bounds: CGRect, skin: KeyboardTheme) -> CGRect {
    guard skin.hasShadow else { return bounds }
    return CGRect(x: bounds.minX, y: bounds.minY, width: bounds.width, height: max(0, bounds.height - skin.shadowOffset))
  }

  /// `card` 的虚线辅助框；卡片太小放不下时为 nil。
  static func guideRect(for card: CGRect) -> CGRect? {
    let box = card.insetBy(dx: guideInset, dy: guideInset)
    return box.width > 0 && box.height > 0 ? box : nil
  }

  /// 按 `traits` 把卡片和辅助线画进 `context`。
  static func draw(_ card: CGRect, in context: CGContext, skin: KeyboardTheme, traits: UITraitCollection) {
    let path = UIBezierPath(roundedRect: card, cornerRadius: skin.cornerRadius)
    context.saveGState()
    if skin.hasShadow {
      context.setShadow(offset: CGSize(width: 0, height: skin.shadowOffset), blur: skin.shadowRadius,
                        color: skin.shadowColor.resolvedColor(with: traits).cgColor)
    }
    context.setFillColor(skin.keyBackground.resolvedColor(with: traits).cgColor)
    context.addPath(path.cgPath)
    context.fillPath()
    context.restoreGState()
    guard let box = guideRect(for: card) else { return }
    context.saveGState()
    context.setStrokeColor(skin.hairline.resolvedColor(with: traits).cgColor)
    context.setLineWidth(1)
    context.setLineDash(phase: 0, lengths: guideDash)
    context.addPath(UIBezierPath(roundedRect: box, cornerRadius: guideRadius).cgPath)
    context.move(to: CGPoint(x: box.midX, y: box.minY))
    context.addLine(to: CGPoint(x: box.midX, y: box.maxY))
    context.move(to: CGPoint(x: box.minX, y: box.midY))
    context.addLine(to: CGPoint(x: box.maxX, y: box.midY))
    context.strokePath()
    context.restoreGState()
  }
}
