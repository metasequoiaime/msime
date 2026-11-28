import SwiftUI
import UIKit

// One renderer for the keyboard extension, the editor and community previews.
// Layer-backed on purpose: a `draw(_:)` override gives every key its own CPU backing store (bounds × screen scale × 4 bytes), and a full custom-skin keyboard of those pushed the keyboard extension past its memory limit on iPad (#11830). Shape and gradient layers are composited by the render server instead.
final class SkinKeySurfaceView: UIView {
  var design = CustomKeyboardSkin() { didSet { if design != oldValue { applyDesign() } } }
  var fillColor = UIColor.white { didSet { applyDesign() } }
  var scale: CGFloat = 1 { didSet { if scale != oldValue { applyDesign() } } }

  private let baseLayer = CAShapeLayer()
  private let baseShadeLayer = CAShapeLayer()
  private let faceLayer = CAShapeLayer()
  private let finishLayer = CALayer()
  private let finishMask = CAShapeLayer()
  private let gradientLayer = CAGradientLayer()
  private let shineLayer = CAShapeLayer()
  private let fiberLayer = CAShapeLayer()
  private let borderLayer = CAShapeLayer()
  private var appliedBounds = CGRect.null

  override init(frame: CGRect) {
    super.init(frame: frame)
    isOpaque = false
    isUserInteractionEnabled = false
    backgroundColor = .clear
    finishLayer.mask = finishMask
    finishLayer.addSublayer(gradientLayer)
    finishLayer.addSublayer(shineLayer)
    finishLayer.addSublayer(fiberLayer)
    gradientLayer.locations = [0, 0.48, 1]
    fiberLayer.fillColor = nil
    fiberLayer.lineCap = .butt
    borderLayer.fillColor = nil
    for sublayer in [baseLayer, baseShadeLayer, faceLayer, finishLayer, borderLayer] { layer.addSublayer(sublayer) }
    // Layer colours are CGColors, so dynamic colours are resolved here and again whenever the appearance changes.
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (view: SkinKeySurfaceView, _: UITraitCollection) in view.applyDesign() }
    applyDesign()
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func layoutSubviews() {
    super.layoutSubviews()
    if bounds != appliedBounds { applyDesign() }
  }

  static func path(in rect: CGRect, shape: SkinKeyShape, radius: CGFloat) -> UIBezierPath {
    switch shape {
    case .rounded:
      return UIBezierPath(roundedRect: rect, cornerRadius: min(radius, min(rect.width, rect.height) / 2))
    case .capsule:
      return UIBezierPath(roundedRect: rect, cornerRadius: min(rect.width, rect.height) / 2)
    case .pebble:
      let p = UIBezierPath()
      let x = rect.minX, y = rect.minY, w = rect.width, h = rect.height
      p.move(to: CGPoint(x: x + w * 0.35, y: y))
      p.addCurve(to: CGPoint(x: x + w, y: y + h * 0.3), controlPoint1: CGPoint(x: x + w * 0.83, y: y), controlPoint2: CGPoint(x: x + w, y: y + h * 0.04))
      p.addCurve(to: CGPoint(x: x + w * 0.68, y: y + h), controlPoint1: CGPoint(x: x + w, y: y + h * 0.85), controlPoint2: CGPoint(x: x + w * 0.9, y: y + h))
      p.addCurve(to: CGPoint(x: x, y: y + h * 0.7), controlPoint1: CGPoint(x: x + w * 0.18, y: y + h), controlPoint2: CGPoint(x: x, y: y + h * 0.97))
      p.addCurve(to: CGPoint(x: x + w * 0.35, y: y), controlPoint1: CGPoint(x: x, y: y + h * 0.2), controlPoint2: CGPoint(x: x + w * 0.06, y: y))
      p.close()
      return p
    case .ticket:
      let p = UIBezierPath()
      let r = min(rect.width, rect.height) * 0.12
      p.move(to: CGPoint(x: rect.minX, y: rect.minY))
      p.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
      p.addLine(to: CGPoint(x: rect.maxX, y: rect.midY - r))
      p.addArc(withCenter: CGPoint(x: rect.maxX, y: rect.midY), radius: r, startAngle: -.pi / 2, endAngle: -.pi * 1.5, clockwise: false)
      p.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
      p.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
      p.addLine(to: CGPoint(x: rect.minX, y: rect.midY + r))
      p.addArc(withCenter: CGPoint(x: rect.minX, y: rect.midY), radius: r, startAngle: .pi / 2, endAngle: -.pi / 2, clockwise: false)
      p.close()
      return p
    }
  }

  /// The key face: inset by `scale`, then lifted by the raised material's depth. `KeyboardKeyButton` shades the same rect when pressed.
  static func faceRect(in rect: CGRect, design: CustomKeyboardSkin, scale: CGFloat) -> CGRect {
    let depth: CGFloat = design.normalized.keyMaterial == .raised ? 3 * scale : 0
    return rect.insetBy(dx: scale, dy: scale).inset(by: UIEdgeInsets(top: 0, left: 0, bottom: depth, right: 0))
  }

  private func applyDesign() {
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    defer { CATransaction.commit() }
    appliedBounds = bounds
    let visible = bounds.width > 2 && bounds.height > 2
    for sublayer in [baseLayer, baseShadeLayer, faceLayer, finishLayer, borderLayer] {
      sublayer.frame = bounds
      sublayer.isHidden = !visible
    }
    guard visible else { return }
    let value = design.normalized
    let material = value.keyMaterial ?? .flat
    let fill = fillColor.resolvedColor(with: traitCollection)
    let face = Self.faceRect(in: bounds, design: value, scale: scale)
    let path = Self.path(in: face, shape: value.keyShape ?? .rounded, radius: value.cornerRadius * scale).cgPath

    // Raised keys sit on a darker copy of the face, `depth` points lower.
    let depth: CGFloat = material == .raised ? 3 * scale : 0
    var lowered = CGAffineTransform(translationX: 0, y: depth)
    let basePath = path.copy(using: &lowered)
    baseLayer.isHidden = depth <= 0
    baseShadeLayer.isHidden = depth <= 0
    baseLayer.path = basePath
    baseLayer.fillColor = fill.cgColor
    baseShadeLayer.path = basePath
    baseShadeLayer.fillColor = UIColor.black.withAlphaComponent(0.28).cgColor

    faceLayer.path = path
    faceLayer.fillColor = fill.cgColor
    // A Core Graphics shadow is cast by the fill's own alpha, so a translucent key casts a fainter shadow.
    faceLayer.shadowPath = value.shadow > 0 ? path : nil
    faceLayer.shadowColor = UIColor.black.cgColor
    faceLayer.shadowOpacity = value.shadow > 0 ? Float(value.shadow * fill.cgColor.alpha) : 0
    faceLayer.shadowOffset = CGSize(width: 0, height: scale)
    faceLayer.shadowRadius = scale

    finishMask.frame = bounds
    finishMask.path = path
    gradientLayer.isHidden = !(material == .glass || material == .raised)
    shineLayer.isHidden = material != .glass
    fiberLayer.isHidden = material != .paper
    finishLayer.isHidden = material == .flat
    if material == .glass || material == .raised {
      let alpha: CGFloat = material == .glass ? 0.24 : 0.13
      gradientLayer.frame = face
      gradientLayer.colors = [UIColor.white.withAlphaComponent(alpha).cgColor, UIColor.white.withAlphaComponent(0).cgColor,
                              UIColor.black.withAlphaComponent(material == .glass ? 0.03 : 0.10).cgColor]
    }
    if material == .glass {
      let shine = UIBezierPath()
      shine.move(to: CGPoint(x: face.minX, y: face.minY))
      shine.addLine(to: CGPoint(x: face.maxX, y: face.minY))
      shine.addLine(to: CGPoint(x: face.minX, y: face.maxY * 0.6))
      shine.close()
      shineLayer.path = shine.cgPath
      shineLayer.fillColor = UIColor.white.withAlphaComponent(0.09).cgColor
    }
    if material == .paper {
      // Fixed fibers: stable while typing and independent of random state.
      let fibers = CGMutablePath()
      for row in stride(from: 3, to: Int(face.height), by: max(2, Int(4 * scale))) {
        let y = face.minY + CGFloat(row)
        fibers.move(to: CGPoint(x: face.minX, y: y))
        fibers.addLine(to: CGPoint(x: face.maxX, y: y - scale))
      }
      fiberLayer.path = fibers
      fiberLayer.strokeColor = UIColor.black.withAlphaComponent(0.08).cgColor
      fiberLayer.lineWidth = 0.5 * scale
    }

    borderLayer.isHidden = value.borderWidth <= 0
    borderLayer.path = path
    borderLayer.strokeColor = CustomKeyboardSkin.color(value.customBorderColor ?? value.accent).resolvedColor(with: traitCollection).cgColor
    borderLayer.lineWidth = value.borderWidth * scale
  }
}

struct SkinKeySurface: UIViewRepresentable {
  let design: CustomKeyboardSkin
  let action: Bool
  var scale: CGFloat = 1
  func makeUIView(context: Context) -> SkinKeySurfaceView { SkinKeySurfaceView() }
  func updateUIView(_ view: SkinKeySurfaceView, context: Context) {
    view.design = design
    view.scale = scale
    view.fillColor = CustomKeyboardSkin.color(action ? design.actionBackground : design.keyBackground)
      .withAlphaComponent(action ? 1 : design.keyOpacity ?? 1)
  }
}
