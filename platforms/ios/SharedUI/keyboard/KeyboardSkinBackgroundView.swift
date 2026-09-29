import UIKit
import SwiftUI

final class KeyboardSkinBackgroundView: UIView {
  private var photoData: Data?
  private var photoImage: UIImage?
  var skin: KeyboardTheme = .system {
    didSet {
      backgroundColor = skin.background
      let data = skin.design?.photo
      if data != photoData { photoData = data; photoImage = data.flatMap { SkinPhotoData.image(from: $0) } }
      setNeedsDisplay()
    }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    isUserInteractionEnabled = false
    isAccessibilityElement = false
    contentMode = .redraw
    backgroundColor = skin.background
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func draw(_ rect: CGRect) {
    guard let context = UIGraphicsGetCurrentContext() else { return }
    let design = skin.design
    if let end = design?.gradientEnd,
       let gradient = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: [CustomKeyboardSkin.color(design!.background).cgColor, CustomKeyboardSkin.color(end).cgColor] as CFArray, locations: [0, 1]) {
      let endPoint = design?.gradientHorizontal == true ? CGPoint(x: bounds.width, y: 0) : CGPoint(x: 0, y: bounds.height)
      context.drawLinearGradient(gradient, start: .zero, end: endPoint, options: [])
    }
    if let photoImage, photoImage.size.width > 0, photoImage.size.height > 0 {
      let scale = max(bounds.width / photoImage.size.width, bounds.height / photoImage.size.height)
      let size = CGSize(width: photoImage.size.width * scale, height: photoImage.size.height * scale)
      let position = CGFloat(design?.photoPosition ?? 0.5)
      photoImage.draw(in: CGRect(x: (bounds.width - size.width) * position, y: (bounds.height - size.height) * position, width: size.width, height: size.height))
      UIColor.black.withAlphaComponent(CGFloat(design?.photoShade ?? 0.25)).setFill()
      context.fill(bounds)
    }
    let pattern = design?.pattern ?? skin.pattern
    guard pattern != 0 else { return }
    let tint = (design.map { CustomKeyboardSkin.color($0.accent) } ?? skin.accent).resolvedColor(with: traitCollection).withAlphaComponent(CGFloat(design?.patternOpacity ?? 0.15))
    context.setStrokeColor(tint.cgColor)
    context.setFillColor(tint.cgColor)
    context.setLineWidth(0.5)
    switch pattern {
    case 1:
      for y in stride(from: CGFloat(8), to: bounds.height, by: 16) {
        for x in stride(from: CGFloat(8), to: bounds.width, by: 16) {
          context.fillEllipse(in: CGRect(x: x, y: y, width: 1.5, height: 1.5))
        }
      }
    case 2:
      for x in stride(from: CGFloat(0), to: bounds.width, by: 20) {
        context.move(to: CGPoint(x: x, y: 0)); context.addLine(to: CGPoint(x: x, y: bounds.height))
      }
      for y in stride(from: CGFloat(0), to: bounds.height, by: 20) {
        context.move(to: CGPoint(x: 0, y: y)); context.addLine(to: CGPoint(x: bounds.width, y: y))
      }
      context.strokePath()
    default:
      context.setLineWidth(2)
      for offset in stride(from: CGFloat(-100), to: bounds.height + bounds.width, by: 24) {
        context.move(to: CGPoint(x: 0, y: offset))
        context.addCurve(to: CGPoint(x: bounds.width, y: offset - 70),
          control1: CGPoint(x: bounds.width * 0.35, y: offset - 90),
          control2: CGPoint(x: bounds.width * 0.65, y: offset + 20))
      }
      context.strokePath()
    }
  }

  override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
    super.traitCollectionDidChange(previousTraitCollection)
    setNeedsDisplay()
  }
}

struct KeyboardSkinBackdrop: UIViewRepresentable {
  let skin: KeyboardTheme
  @Environment(\.colorScheme) private var colorScheme
  func makeUIView(context: Context) -> KeyboardSkinBackgroundView { KeyboardSkinBackgroundView() }
  func updateUIView(_ view: KeyboardSkinBackgroundView, context: Context) {
    view.overrideUserInterfaceStyle = colorScheme == .dark ? .dark : .light
    view.skin = skin
  }
}
