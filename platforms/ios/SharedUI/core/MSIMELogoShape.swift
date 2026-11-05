import SwiftUI
import UIKit

/// 官方水杉标志，就是 packages/ui/src/assets/msime.svg（110 x 110 viewBox，PC 端应用图标从它渲染）整体放大 1.2 倍：一个填充的圆角正方形外框（120 x 120，圆角半径为边长的 22%，放在原点在 x = -8 的 132 x 132 viewBox 里），上面叠白色折线描边，折线、线宽、圆头端点和尖角连接都与 msime.svg 相同，与 Android 的 KeyboardBrandButton 和 splash_mark.xml 画的是同一组路径。两条路径都按比例适配并居中到请求的矩形里，所以画进同一矩形的外框和描边能对齐。
enum MSIMELogo {
  static let viewBox = CGRect(x: -8, y: 0, width: 132, height: 132)
  /// `M-2.15686 5.8335H117.843V125.833H-2.15686Z`.
  static let frameRect = CGRect(x: -2.15686, y: 5.8335, width: 120, height: 120)
  /// 外框圆角半径，以 viewBox 单位计：边长的 22%，与 packages/ui/src/assets/msime.svg 相同。
  static let frameCornerRadius: CGFloat = 26.4
  /// 描边宽度，以 viewBox 单位计：msime.svg 的 8 放大 1.2 倍，用圆头端点和尖角连接绘制；使用时乘以 `scale(for:)`。
  static let strokeWidth: CGFloat = 9.6
  /// Miter limit of the stroke's joins. SVG's default is 4 while CoreGraphics and SwiftUI default to 10; all three vertices of the mark (about 35 to 42 degrees) stay under either limit, so this only pins the value to msime.svg's explicitly.
  static let strokeMiterLimit: CGFloat = 4
  /// 外框的填充色，所有平台的标志都是这一块深色圆角正方形，不跟随主题或皮肤的强调色（与 packages/ui/src/assets/msime.svg 相同）。
  static let fieldColor = UIColor(red: 0x25 / 255.0, green: 0x25 / 255.0, blue: 0x25 / 255.0, alpha: 1)
  /// 外框的绿边颜色。
  static let edgeColor = UIColor(red: 0xA8 / 255.0, green: 0xDF / 255.0, blue: 0x8E / 255.0, alpha: 1)
  /// 绿边宽度，以 viewBox 单位计，描在外框路径的中线上；使用时乘以 `scale(for:)`。边长 120 的 5%，与 msime.svg 里边长 100、宽 5 的绿边同比例。
  static let edgeWidth: CGFloat = 6

  /// 标志按比例适配进 `rect` 时，从 viewBox 单位换算到点的比例。
  static func scale(for rect: CGRect) -> CGFloat {
    min(rect.width / viewBox.width, rect.height / viewBox.height)
  }

  /// `M76.7775 26.4735L38.7872 41.0393L76.7775 51.9637L38.7872 70.1708C74.7026 76.2399 108.462 81.2165 36.7231 104.2335`: msime.svg's `M70.7787 22.2L39.1201 34.3382L70.7787 43.4418L39.1201 58.6144C69.0496 63.672 97.1824 67.8192 37.4 87` scaled by 1.2 so its `x5 y5` frame lands on `frameRect`'s origin.
  static func strokePath(in rect: CGRect) -> CGPath {
    let point = mapper(for: rect)
    let path = CGMutablePath()
    path.move(to: point(76.7775, 26.4735))
    path.addLine(to: point(38.7872, 41.0393))
    path.addLine(to: point(76.7775, 51.9637))
    path.addLine(to: point(38.7872, 70.1708))
    path.addCurve(to: point(36.7231, 104.2335), control1: point(74.7026, 76.2399), control2: point(108.462, 81.2165))
    return path
  }

  static func framePath(in rect: CGRect) -> CGPath {
    let point = mapper(for: rect)
    let origin = point(frameRect.minX, frameRect.minY)
    let scale = scale(for: rect)
    let radius = frameCornerRadius * scale
    return CGPath(roundedRect: CGRect(x: origin.x, y: origin.y, width: frameRect.width * scale, height: frameRect.height * scale),
                  cornerWidth: radius, cornerHeight: radius, transform: nil)
  }

  /// 在当前 UIKit 图形上下文里把完整标志（深色圆角正方形、绿边、白色折线）按比例画进 `rect` 中央，供 UIKit 绘制的键盘工具栏和皮肤缩略图使用，与 SwiftUI 的 `MSIMELogoMark` 画法相同。
  static func draw(in rect: CGRect) {
    let scale = scale(for: rect)
    let frame = UIBezierPath(cgPath: framePath(in: rect))
    fieldColor.setFill()
    frame.fill()
    frame.lineWidth = edgeWidth * scale
    edgeColor.setStroke()
    frame.stroke()
    let stroke = UIBezierPath(cgPath: strokePath(in: rect))
    stroke.lineWidth = strokeWidth * scale
    stroke.lineCapStyle = .round
    stroke.lineJoinStyle = .miter
    stroke.miterLimit = strokeMiterLimit
    UIColor.white.setStroke()
    stroke.stroke()
  }

  private static func mapper(for rect: CGRect) -> (CGFloat, CGFloat) -> CGPoint {
    let scale = scale(for: rect)
    let originX = rect.minX + (rect.width - viewBox.width * scale) / 2 - viewBox.minX * scale
    let originY = rect.minY + (rect.height - viewBox.height * scale) / 2 - viewBox.minY * scale
    return { x, y in CGPoint(x: originX + x * scale, y: originY + y * scale) }
  }
}

/// 标志的白色描边；用 `MSIMELogo.strokeWidth * MSIMELogo.scale(for:)` 的线宽、圆头端点和尖角连接（miter limit 为 `MSIMELogo.strokeMiterLimit`）来描。裁剪它会从起点开始画出描边，与启动页的效果一致。
struct MSIMELogoStroke: Shape {
  func path(in rect: CGRect) -> Path { Path(MSIMELogo.strokePath(in: rect)) }
}

/// 标志的填充外框。
struct MSIMELogoFrame: Shape {
  func path(in rect: CGRect) -> Path { Path(MSIMELogo.framePath(in: rect)) }
}

/// 完整的水杉标志：深色圆角正方形、绿边和白色折线，配色固定，各处画出来都一样。`strokeDrawn` 是折线画出来的比例（0 到 1），启动页用它从起点开始描出折线。
struct MSIMELogoMark: View {
  var strokeDrawn: CGFloat = 1

  var body: some View {
    GeometryReader { geometry in
      let scale = MSIMELogo.scale(for: CGRect(origin: .zero, size: geometry.size))
      ZStack {
        MSIMELogoFrame().fill(Color(uiColor: MSIMELogo.fieldColor))
        MSIMELogoFrame().stroke(Color(uiColor: MSIMELogo.edgeColor), lineWidth: MSIMELogo.edgeWidth * scale)
        MSIMELogoStroke()
          .trim(from: 0, to: strokeDrawn)
          .stroke(.white, style: StrokeStyle(lineWidth: MSIMELogo.strokeWidth * scale, lineCap: .round, lineJoin: .miter, miterLimit: MSIMELogo.strokeMiterLimit))
      }
    }
  }
}
