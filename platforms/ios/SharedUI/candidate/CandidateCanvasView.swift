import UIKit

/// 候选内容的承载视图。三个绘图层位于内容的前后，不参与命中测试。
class CandidateCanvasHostView: UIView {
  let candidateContentGuide = UILayoutGuide()
  private let back = CandidateCanvasView(level: -1)
  private let panel = CandidateCanvasView(level: 0)
  private let front = CandidateCanvasView(level: 1)
  private var insetConstraints: [NSLayoutConstraint] = []
  var minimumContentHeight: CGFloat = 24
  var minimumContentWidth: CGFloat = 44
  var canvasVisible = true {
    didSet { if oldValue != canvasVisible { refreshCanvasVisibility() } }
  }
  var canvas: ExternalCandidateSkin.Canvas? {
    didSet {
      guard oldValue != canvas else { return }
      for node in [back, panel, front] { node.canvas = canvas }
      refreshCanvasVisibility()
      setNeedsLayout()
    }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    addLayoutGuide(candidateContentGuide)
    insetConstraints = [
      candidateContentGuide.topAnchor.constraint(equalTo: topAnchor),
      candidateContentGuide.leadingAnchor.constraint(equalTo: leadingAnchor),
      candidateContentGuide.bottomAnchor.constraint(equalTo: bottomAnchor),
      candidateContentGuide.trailingAnchor.constraint(equalTo: trailingAnchor),
    ]
    NSLayoutConstraint.activate(insetConstraints)
    for node in [back, panel, front] { addSubview(node) }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  override var isHidden: Bool {
    didSet { refreshCanvasVisibility() }
  }

  private func refreshCanvasVisibility() {
    for node in [back, panel, front] { node.isHidden = !canvasVisible || isHidden }
    setNeedsLayout()
  }

  override func layoutSubviews() {
    let insets = effectiveContentInsets()
    let constants = [insets.top, insets.left, -insets.bottom, -insets.right]
    for (constraint, value) in zip(insetConstraints, constants) where constraint.constant != value {
      constraint.constant = value
    }
    super.layoutSubviews()
    for node in [back, panel, front] {
      node.geometryScale = geometryScale()
      node.frame = bounds
      node.layoutIfNeeded()
    }
    sendSubviewToBack(panel)
    sendSubviewToBack(back)
    bringSubviewToFront(front)
  }

  /// 移动候选条高度固定；所有皮肤几何等比压缩，优先保留可读的内容区。
  private func geometryScale() -> CGFloat {
    guard let canvas, canvasVisible else { return 1 }
    let a = canvas.contentInsets, b = canvas.outerInsets
    let vertical = a.top + a.bottom + b.top + b.bottom
    let horizontal = a.left + a.right + b.left + b.right
    let sy = vertical > 0 ? max(0, bounds.height - minimumContentHeight) / vertical : 1
    let sx = horizontal > 0 ? max(0, bounds.width - minimumContentWidth) / horizontal : 1
    return min(1, sy, sx)
  }

  func effectiveContentInsets() -> UIEdgeInsets {
    guard let canvas, canvasVisible else { return .zero }
    let a = canvas.contentInsets, b = canvas.outerInsets, scale = geometryScale()
    return UIEdgeInsets(top: (a.top + b.top) * scale, left: (a.left + b.left) * scale,
                        bottom: (a.bottom + b.bottom) * scale, right: (a.right + b.right) * scale)
  }
}

/// 九宫格与挂件的一个绘图层。位图在后台预裁切；布局只移动图层，动画交给 Core Animation。
final class CandidateCanvasView: UIView {
  private let level: Int
  var geometryScale: CGFloat = 1 { didSet { if oldValue != geometryScale { setNeedsLayout() } } }
  private var sprites: [(CALayer, ExternalCandidateSkin.Canvas.Frames)] = []
  private var suspended = false
  var reduceMotion: () -> Bool = { UIAccessibility.isReduceMotionEnabled }
  var canvas: ExternalCandidateSkin.Canvas? {
    didSet {
      for (sprite, _) in sprites { sprite.removeFromSuperlayer() }
      sprites.removeAll()
      if let canvas {
        if level == 0 {
          for (index, frames) in canvas.tiles.enumerated() {
            if index == 4 && !canvas.fill { continue }
            if let frames { add(frames, identifier: "tile-\(index)") }
          }
        } else {
          for (index, decoration) in canvas.decorations.enumerated()
            where (decoration.zIndex < 1 ? -1 : 1) == level {
            add(decoration.frames, identifier: "decoration-\(index)")
          }
        }
      }
      refreshPlayback()
      setNeedsLayout()
    }
  }

  init(level: Int) {
    self.level = level
    super.init(frame: .zero)
    isUserInteractionEnabled = false
    isAccessibilityElement = false
    backgroundColor = .clear
    let center = NotificationCenter.default
    center.addObserver(self, selector: #selector(motionChanged), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
    for name in [UIApplication.didEnterBackgroundNotification, NSNotification.Name("NSExtensionHostWillResignActiveNotification")] {
      center.addObserver(self, selector: #selector(pausePlayback), name: name, object: nil)
    }
    for name in [UIApplication.didBecomeActiveNotification, NSNotification.Name("NSExtensionHostDidBecomeActiveNotification")] {
      center.addObserver(self, selector: #selector(resumePlayback), name: name, object: nil)
    }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }
  deinit { NotificationCenter.default.removeObserver(self) }

  private func add(_ frames: ExternalCandidateSkin.Canvas.Frames, identifier: String) {
    let sprite = CALayer()
    sprite.name = identifier
    sprite.contents = frames.images.first
    sprite.contentsGravity = .resize
    sprite.minificationFilter = .linear
    sprite.magnificationFilter = .linear
    layer.addSublayer(sprite)
    sprites.append((sprite, frames))
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    refreshPlayback()
  }

  override var isHidden: Bool {
    didSet { if oldValue != isHidden { refreshPlayback() } }
  }

  @objc private func motionChanged() { refreshPlayback() }
  @objc private func pausePlayback() { suspended = true; refreshPlayback() }
  @objc private func resumePlayback() { suspended = false; refreshPlayback() }

  func refreshPlayback() {
    let play = window != nil && !isHidden && !suspended && !reduceMotion()
    let anchor = CACurrentMediaTime()
    for (sprite, frames) in sprites {
      sprite.removeAnimation(forKey: "canvas-frames")
      sprite.contents = frames.images.first
      guard play, frames.images.count > 1, let last = frames.images.last else { continue }
      let animation = CAKeyframeAnimation(keyPath: "contents")
      animation.values = frames.images + [last]
      animation.keyTimes = frames.times
      animation.duration = frames.duration
      animation.calculationMode = .discrete
      animation.repeatCount = frames.loops == 0 ? .infinity : Float(frames.loops)
      animation.beginTime = sprite.convertTime(anchor, from: nil)
      animation.fillMode = .forwards
      animation.isRemovedOnCompletion = false
      sprite.add(animation, forKey: "canvas-frames")
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    guard let canvas else { return }
    let scale = geometryScale
    let outer = canvas.outerInsets
    let card = Self.cardRect(in: bounds, outer: UIEdgeInsets(top: outer.top * scale, left: outer.left * scale, bottom: outer.bottom * scale, right: outer.right * scale))
    let x = ExternalCandidateSkin.Canvas.cuts(card.width, start: canvas.widths[3] * scale, end: canvas.widths[1] * scale, origin: card.minX)
    let y = ExternalCandidateSkin.Canvas.cuts(card.height, start: canvas.widths[0] * scale, end: canvas.widths[2] * scale, origin: card.minY)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    for (sprite, _) in sprites {
      guard let name = sprite.name else { continue }
      if name.hasPrefix("tile-"), let index = Int(name.dropFirst(5)) {
        let row = index / 3, col = index % 3
        sprite.frame = CGRect(x: x[col], y: y[row], width: x[col + 1] - x[col], height: y[row + 1] - y[row])
      } else if let index = Int(name.dropFirst(11)), canvas.decorations.indices.contains(index) {
        sprite.frame = Self.decorationRect(canvas.decorations[index], card: card, scale: scale)
      }
    }
    CATransaction.commit()
  }

  static func cardRect(in bounds: CGRect, outer: UIEdgeInsets) -> CGRect {
    let x = ExternalCandidateSkin.Canvas.cuts(bounds.width, start: outer.left, end: outer.right, origin: bounds.minX)
    let y = ExternalCandidateSkin.Canvas.cuts(bounds.height, start: outer.top, end: outer.bottom, origin: bounds.minY)
    return CGRect(x: x[1], y: y[1], width: max(0, x[2] - x[1]), height: max(0, y[2] - y[1]))
  }

  static func decorationRect(_ decoration: ExternalCandidateSkin.Canvas.Decoration, card: CGRect, scale: CGFloat = 1) -> CGRect {
    let width = decoration.width * scale, height = decoration.height * scale
    let anchor = decoration.anchor
    let x = anchor.contains("left") ? card.minX : anchor.contains("right") ? card.maxX - width : card.midX - width / 2
    let y = anchor.contains("top") ? card.minY : anchor.contains("bottom") ? card.maxY - height : card.midY - height / 2
    guard let image = decoration.frames.images.first else { return .zero }
    let fit = min(width / CGFloat(image.width), height / CGFloat(image.height))
    let fitted = CGSize(width: CGFloat(image.width) * fit, height: CGFloat(image.height) * fit)
    return CGRect(x: x + decoration.offsetX * scale + (width - fitted.width) / 2,
                  y: y + decoration.offsetY * scale + (height - fitted.height) / 2, width: fitted.width, height: fitted.height)
  }
}

/// 高亮候选的竖条属于皮肤，不截获候选按钮的点击。
final class CandidateSelectionBarView: UIView {
  private let color: UIColor

  init(color: UIColor) {
    self.color = color
    super.init(frame: .zero)
    backgroundColor = .clear
    isUserInteractionEnabled = false
    isAccessibilityElement = false
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  override func draw(_ rect: CGRect) {
    color.setFill()
    UIBezierPath(roundedRect: CGRect(x: 1, y: 6, width: 3, height: max(0, bounds.height - 12)), cornerRadius: 1.5).fill()
  }
}
