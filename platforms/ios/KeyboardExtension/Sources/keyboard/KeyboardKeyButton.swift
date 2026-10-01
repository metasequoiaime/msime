import UIKit

/// Animate the key itself without changing the stack view's layout or input timing.
final class KeyboardKeyButton: UIButton {
  static let glossColumns: CGFloat = 3

  static func glossColumnWidth(
    visible: CGFloat, spacing: CGFloat, insets: NSDirectionalEdgeInsets
  ) -> CGFloat {
    guard visible > 0 else { return 0 }
    return (visible - spacing * (glossColumns - 1)) / glossColumns
      - insets.leading - insets.trailing
  }

  static func chipContentWidth(titleLine: CGFloat, glossLines: Int, column: CGFloat) -> CGFloat {
    glossLines > 0 ? max(titleLine, column) : titleLine
  }

  static func chipWidth(
    titleLine: CGFloat, glossLines: Int, column: CGFloat, insets: NSDirectionalEdgeInsets
  ) -> CGFloat {
    ceil(chipContentWidth(titleLine: titleLine, glossLines: glossLines, column: column)
      + insets.leading + insets.trailing)
  }

  static let minimumGlossFontSize: CGFloat = 9

  static func fittedGloss(_ text: String, font: UIFont, width: CGFloat) -> (text: String, font: UIFont) {
    func measure(_ value: String, _ candidateFont: UIFont) -> CGFloat {
      (value as NSString).size(withAttributes: [.font: candidateFont]).width
    }
    guard width > 0, !text.isEmpty else { return (text, font) }
    if measure(text, font) <= width { return (text, font) }
    var size = font.pointSize
    while size > minimumGlossFontSize {
      size = max(size - 0.5, minimumGlossFontSize)
      let smaller = font.withSize(size)
      if measure(text, smaller) <= width { return (text, smaller) }
    }
    let floorFont = font.withSize(minimumGlossFontSize)
    var characters = Array(text)
    while !characters.isEmpty {
      characters.removeLast()
      let shortened = String(characters) + "…"
      if measure(shortened, floorFont) <= width { return (shortened, floorFont) }
    }
    return ("", floorFont)
  }

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      updatePressFeedback()
      updatePressPreview()
    }
  }

  private func updatePressFeedback() {
    let pressed = isHighlighted && isEnabled
    let target = pressed
      ? CGAffineTransform(translationX: 0, y: 1).scaledBy(x: 0.94, y: 0.94)
      : .identity
    guard !UIAccessibility.isReduceMotionEnabled, window != nil else {
      layer.removeAllAnimations()
      transform = .identity
      return
    }
    UIView.animate(
      withDuration: pressed ? 0.06 : 0.18,
      delay: 0,
      usingSpringWithDamping: pressed ? 1 : 0.72,
      initialSpringVelocity: 0,
      options: [.allowUserInteraction, .beginFromCurrentState]
    ) {
      self.transform = target
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      layer.removeAllAnimations()
      transform = .identity
      removePressPreview()
    }
  }

  override var isEnabled: Bool {
    didSet {
      if !isEnabled {
        updatePressFeedback()
        updatePressPreview()
      }
    }
  }

  /// Whether holding this key shows the iPhone system keyboard's magnified copy of it above the finger. Only keys that
  /// type their title turn it on: letters, the Microsoft Shuangpin `;` and the symbol layer. The iPad system keyboard
  /// shows none, so neither does this one there.
  var showsPressPreview = false
  private weak var pressPreview: KeyPressPreviewView?

  private func updatePressPreview() {
    guard showsPressPreview, isHighlighted, isEnabled, traitCollection.userInterfaceIdiom == .phone,
          let title = configuration?.title, !title.isEmpty, let host = pressPreviewHost else {
      removePressPreview()
      return
    }
    let preview = pressPreview ?? KeyPressPreviewView()
    if preview.superview !== host {
      preview.removeFromSuperview()
      host.addSubview(preview)
    }
    preview.frame = host.bounds
    let skin = KeyboardTheme.current
    preview.show(title, key: convert(bounds, to: host), cornerRadius: skin.cornerRadius,
                 fill: skin.keyBackground.resolvedColor(with: traitCollection).withAlphaComponent(1),
                 text: skin.keyForeground.resolvedColor(with: traitCollection),
                 monospaced: skin.usesMonospacedFont)
    pressPreview = preview
  }

  private func removePressPreview() {
    pressPreview?.removeFromSuperview()
    pressPreview = nil
  }

  /// The keyboard's root view: the callout reaches above the key's row, so it cannot live inside the row's stack view.
  private var pressPreviewHost: UIView? {
    var responder: UIResponder? = next
    while let current = responder {
      if let controller = current as? UIViewController { return controller.view }
      responder = current.next
    }
    return nil
  }

  /// Shift, delete, 123, the symbol, globe, language and Tab keys: they take the theme's function-key fill (`kb.spec`) rather than the letter-key fill whenever the keyboard is recoloured.
  var isFunctionKey = false

  /// How many lines the title is allowed to take, or nil to leave UIKit's own choice alone.
  ///
  /// Assigning `titleLabel?.numberOfLines` right after a configuration does not hold: UIKit
  /// applies the configuration on its own schedule and rebuilds the title label while doing it,
  /// so the value is back to 0 by the time the button lays out. For a candidate chip that means
  /// wrapping onto a second line the strip has no room for -- the chip grows downward instead of
  /// truncating. Re-applying it on every layout pass is what makes the limit stick.
  var titleLineCount: Int? {
    didSet {
      guard titleLineCount != oldValue else { return }
      setNeedsLayout()
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    // The key's own pass is the one that sees its final size. The keyboard controller's pass does not run again when the
    // system walks the window down to its real height while the keyboard appears, so a shadow path taken there kept a
    // key from the taller frame and hung a dark block below every key until something laid the keyboard out again.
    if layer.shadowOpacity > 0 {
      layer.shadowPath = UIBezierPath(roundedRect: bounds, cornerRadius: KeyboardTheme.current.cornerRadius).cgPath
    }
    guard let lines = titleLineCount else { return }
    titleLabel?.numberOfLines = lines
  }
}

/// The iPhone system keyboard's key callout: a wider rounded head above a held key with a large copy of its character,
/// joined to the key's own outline, which it covers.
final class KeyPressPreviewView: UIView {
  struct Geometry: Equatable {
    var head: CGRect
    var key: CGRect
  }

  /// Where the callout goes for `key`, both in `bounds`' space. The head slides inward at the keyboard's side edges, and
  /// where there is too little room above the key it gets shorter: the extension's window ends at `bounds`, and the system
  /// clips anything drawn past it.
  static func geometry(key: CGRect, in bounds: CGRect) -> Geometry {
    let widen = min(max(key.width * 0.28, 8), 16)
    let width = key.width + widen * 2
    let x = min(max(key.midX - width / 2, bounds.minX), bounds.maxX - width)
    let height = min(max(key.height * 1.15, 44), key.minY - bounds.minY)
    return Geometry(head: CGRect(x: x, y: key.minY - height, width: width, height: max(height, 0)), key: key)
  }

  private let shape = CAShapeLayer()
  private let label = UILabel()

  override init(frame: CGRect) {
    super.init(frame: frame)
    isUserInteractionEnabled = false
    isAccessibilityElement = false
    accessibilityElementsHidden = true
    layer.addSublayer(shape)
    shape.shadowColor = UIColor.black.cgColor
    shape.shadowOpacity = 0.3
    shape.shadowRadius = 1.5
    shape.shadowOffset = CGSize(width: 0, height: 1)
    label.textAlignment = .center
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.5
    addSubview(label)
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  var text: String? { label.text }

  func show(_ title: String, key: CGRect, cornerRadius: CGFloat, fill: UIColor, text: UIColor, monospaced: Bool) {
    let geometry = Self.geometry(key: key, in: bounds)
    let path = Self.path(geometry, cornerRadius: cornerRadius)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    shape.frame = bounds
    shape.path = path
    shape.shadowPath = path
    shape.fillColor = fill.cgColor
    CATransaction.commit()
    let size = min(geometry.head.height * 0.62, 34)
    label.font = monospaced ? .monospacedSystemFont(ofSize: size, weight: .regular) : .systemFont(ofSize: size)
    label.textColor = text
    label.text = title
    label.frame = geometry.head.insetBy(dx: 4, dy: 2)
  }

  private static func path(_ geometry: Geometry, cornerRadius: CGFloat) -> CGPath {
    let head = geometry.head, key = geometry.key
    let headRadius = min(cornerRadius + 2, head.height / 2, head.width / 2)
    let keyRadius = min(cornerRadius, key.height / 2, key.width / 2)
    // The waist where the head narrows into the key, eased over this much height on either side of the key's top edge.
    let ease = max(min(head.width - key.width, head.height - headRadius, key.height / 2, 10), 0)
    let path = UIBezierPath()
    path.move(to: CGPoint(x: head.minX + headRadius, y: head.minY))
    path.addLine(to: CGPoint(x: head.maxX - headRadius, y: head.minY))
    path.addArc(withCenter: CGPoint(x: head.maxX - headRadius, y: head.minY + headRadius), radius: headRadius,
                startAngle: -.pi / 2, endAngle: 0, clockwise: true)
    path.addLine(to: CGPoint(x: head.maxX, y: key.minY - ease))
    path.addCurve(to: CGPoint(x: key.maxX, y: key.minY + ease),
                  controlPoint1: CGPoint(x: head.maxX, y: key.minY), controlPoint2: CGPoint(x: key.maxX, y: key.minY))
    path.addLine(to: CGPoint(x: key.maxX, y: key.maxY - keyRadius))
    path.addArc(withCenter: CGPoint(x: key.maxX - keyRadius, y: key.maxY - keyRadius), radius: keyRadius,
                startAngle: 0, endAngle: .pi / 2, clockwise: true)
    path.addLine(to: CGPoint(x: key.minX + keyRadius, y: key.maxY))
    path.addArc(withCenter: CGPoint(x: key.minX + keyRadius, y: key.maxY - keyRadius), radius: keyRadius,
                startAngle: .pi / 2, endAngle: .pi, clockwise: true)
    path.addLine(to: CGPoint(x: key.minX, y: key.minY + ease))
    path.addCurve(to: CGPoint(x: head.minX, y: key.minY - ease),
                  controlPoint1: CGPoint(x: key.minX, y: key.minY), controlPoint2: CGPoint(x: head.minX, y: key.minY))
    path.addLine(to: CGPoint(x: head.minX, y: head.minY + headRadius))
    path.addArc(withCenter: CGPoint(x: head.minX + headRadius, y: head.minY + headRadius), radius: headRadius,
                startAngle: .pi, endAngle: -.pi / 2, clockwise: true)
    path.close()
    return path.cgPath
  }
}

/// Candidate chips highlight immediately, but a drag still belongs to the strip.
final class CandidateScrollView: UIScrollView {
  override init(frame: CGRect) {
    super.init(frame: frame)
    delaysContentTouches = false
    disableEdgeEffects()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    delaysContentTouches = false
    disableEdgeEffects()
  }

  override func touchesShouldCancel(in view: UIView) -> Bool {
    if view is KeyboardKeyButton { return true }
    return super.touchesShouldCancel(in: view)
  }

}
