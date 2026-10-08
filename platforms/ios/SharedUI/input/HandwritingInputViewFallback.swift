import UIKit

final class HandwritingCanvas: UIView {
  var acceptsInk = false
  var onStrokeBegan: (() -> Void)?
  var onChange: (() -> Void)?
  private(set) var strokes: [[CGPoint]] = []
  var hasInk: Bool { !strokes.isEmpty }

  func clear() { strokes.removeAll(); setNeedsDisplay(); onChange?() }
  func undo() { _ = strokes.popLast(); setNeedsDisplay(); onChange?() }
  func setTestStrokes(_ values: [[CGPoint]]) { strokes = values; setNeedsDisplay(); onChange?() }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard acceptsInk, let point = touches.first?.location(in: self) else { return }
    strokes.append([point]); onStrokeBegan?(); setNeedsDisplay()
  }
  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard acceptsInk, let point = touches.first?.location(in: self), !strokes.isEmpty else { return }
    strokes[strokes.index(before: strokes.endIndex)].append(point); onChange?(); setNeedsDisplay()
  }
  override func draw(_ rect: CGRect) {
    let skin = KeyboardTheme.current
    if let context = UIGraphicsGetCurrentContext() {
      HandwritingCanvasCard.draw(HandwritingCanvasCard.cardRect(in: bounds, skin: skin), in: context, skin: skin, traits: traitCollection)
    }
    skin.keyForeground.setStroke()
    for stroke in strokes where stroke.count > 1 {
      let path = UIBezierPath(); path.move(to: stroke[0]);
      for point in stroke.dropFirst() { path.addLine(to: point) }
      path.lineWidth = 3; path.lineCapStyle = .round; path.stroke()
    }
  }
}

/// The simulator build of the handwriting panel.
///
/// ML Kit Digital Ink ships an arm64 slice for device, not for simulator, so this target compiles
/// the same panel without it. It draws ink and recognises nothing, which is the honest answer —
/// but it has to *say* so. Silence here is indistinguishable from a broken keyboard: the user
/// writes a character, nothing appears, and there is nothing on screen to explain why. The device
/// panel carries a status line for exactly this kind of message, so this one does too, and reads
/// the same as the upstream client's.
final class HandwritingInputView: UIView {
  let canvas = HandwritingCanvas()
  private let status = UILabel()
  static let unavailableMessage = "此版本不含手写识别，请使用真机版本"
  var onResults: (([String]) -> Void)?
  var canDownload: () -> Bool = { false }
  var onInsert: ((String) -> Void)?
  private(set) var results: [String] = []
  var hasInk: Bool { canvas.hasInk }

  override init(frame: CGRect) {
    super.init(frame: frame)
    accessibilityIdentifier = "handwritingInput"
    // 卡片以圆角画在画布内部，所以卡片外的四角保持透明。
    canvas.isOpaque = false
    canvas.backgroundColor = .clear
    canvas.contentMode = .redraw
    addSubview(canvas); canvas.translatesAutoresizingMaskIntoConstraints = false
    status.text = Self.unavailableMessage
    status.font = .systemFont(ofSize: 13)
    status.textAlignment = .center
    status.numberOfLines = 2
    status.accessibilityIdentifier = "handwritingStatus"
    status.textColor = KeyboardTheme.current.secondary
    addSubview(status); status.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      canvas.leadingAnchor.constraint(equalTo: leadingAnchor), canvas.trailingAnchor.constraint(equalTo: trailingAnchor),
      canvas.topAnchor.constraint(equalTo: topAnchor), canvas.bottomAnchor.constraint(equalTo: bottomAnchor),
      heightAnchor.constraint(greaterThanOrEqualToConstant: 120),
      status.centerXAnchor.constraint(equalTo: centerXAnchor),
      status.centerYAnchor.constraint(equalTo: centerYAnchor),
      status.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: HandwritingCanvasCard.guideInset),
      status.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -HandwritingCanvasCard.guideInset),
    ])
    canvas.acceptsInk = true
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
  func activate() { canvas.acceptsInk = true }
  func deactivate() { clear() }
  func clear() { results = []; onResults?([]); canvas.clear() }
  @discardableResult func use(at index: Int) -> Bool { false }
  func commitFirst() -> Bool { false }
}
