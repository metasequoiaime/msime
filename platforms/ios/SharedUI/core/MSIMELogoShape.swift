import SwiftUI
import UIKit

/// 官方水杉标志，坐标取自设计稿的 116 x 132 viewBox：一个填充的外框（去掉笔刷纹理的 msime_frame.svg），上面叠白色折线描边，与 Android 的 KeyboardBrandButton 和 splash_mark.xml 画的是同一组路径。两条路径都按比例适配并居中到请求的矩形里，所以画进同一矩形的外框和描边能对齐。
enum MSIMELogo {
  static let viewBox = CGSize(width: 116, height: 132)
  /// `M5.84314 5.8335H109.843V125.833H5.84314Z`.
  static let frameRect = CGRect(x: 5.84314, y: 5.8335, width: 104, height: 120)
  /// 描边宽度，以 viewBox 单位计，用圆头端点和圆角连接绘制；使用时乘以 `scale(for:)`。
  static let strokeWidth: CGFloat = 9

  /// 标志按比例适配进 `rect` 时，从 viewBox 单位换算到点的比例。
  static func scale(for rect: CGRect) -> CGFloat {
    min(rect.width / viewBox.width, rect.height / viewBox.height)
  }

  /// `M80.394 18.8335L34.3451 36.489L80.394 49.7306L34.3451 71.7999C77.8789 79.1564 118.8 85.1887 31.8431 113.088`.
  static func strokePath(in rect: CGRect) -> CGPath {
    let point = mapper(for: rect)
    let path = CGMutablePath()
    path.move(to: point(80.394, 18.8335))
    path.addLine(to: point(34.3451, 36.489))
    path.addLine(to: point(80.394, 49.7306))
    path.addLine(to: point(34.3451, 71.7999))
    path.addCurve(to: point(31.8431, 113.088), control1: point(77.8789, 79.1564), control2: point(118.8, 85.1887))
    return path
  }

  static func framePath(in rect: CGRect) -> CGPath {
    let point = mapper(for: rect)
    let origin = point(frameRect.minX, frameRect.minY)
    let scale = scale(for: rect)
    return CGPath(rect: CGRect(x: origin.x, y: origin.y, width: frameRect.width * scale, height: frameRect.height * scale),
                  transform: nil)
  }

  private static func mapper(for rect: CGRect) -> (CGFloat, CGFloat) -> CGPoint {
    let scale = scale(for: rect)
    let originX = rect.minX + (rect.width - viewBox.width * scale) / 2
    let originY = rect.minY + (rect.height - viewBox.height * scale) / 2
    return { x, y in CGPoint(x: originX + x * scale, y: originY + y * scale) }
  }
}

/// 标志的白色描边；用 `MSIMELogo.strokeWidth * MSIMELogo.scale(for:)` 的线宽和圆头端点、圆角连接来描。裁剪它会从起点开始画出描边，与启动页的效果一致。
struct MSIMELogoStroke: Shape {
  func path(in rect: CGRect) -> Path { Path(MSIMELogo.strokePath(in: rect)) }
}

/// 标志的填充外框。
struct MSIMELogoFrame: Shape {
  func path(in rect: CGRect) -> Path { Path(MSIMELogo.framePath(in: rect)) }
}
