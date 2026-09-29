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
    }
  }

  override var isEnabled: Bool {
    didSet {
      if !isEnabled { updatePressFeedback() }
    }
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
    guard let lines = titleLineCount else { return }
    titleLabel?.numberOfLines = lines
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
