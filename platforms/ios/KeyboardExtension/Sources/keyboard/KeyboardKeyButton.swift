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

  /// 按住按钮时给手指的反馈，取自设计稿的 `style-active` 规则。
  enum PressFeedback: Equatable {
    /// 按键：缩放到 .96、亮度降到 .86，亮度用叠在按键底色上的 .14 黑色画出。
    case key
    /// 只缩放，例如工具栏的 .92。
    case scale(CGFloat)
    /// 淡到这个不透明度，例如候选胶囊的 .6。
    case opacity(CGFloat)
    /// 把底色压暗到这个亮度，例如网格单元的 .88。
    case brightness(CGFloat)
  }

  /// 默认是按键的反馈；把这个按钮用作胶囊、工具栏项或网格单元的调用方各自选用自己的。
  var pressFeedback: PressFeedback = .key {
    didSet {
      guard pressFeedback != oldValue else { return }
      resetPressFeedback()
      updatePressFeedback()
    }
  }

  static let keyPressScale: CGFloat = 0.96
  /// 以这个透明度叠在底色上的黑色，看起来就是设计稿的 `brightness(.86)`。
  static let keyPressShade: CGFloat = 0.14
  static let pressDuration: TimeInterval = 0.06

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      updatePressFeedback()
      updatePressPreview()
    }
  }

  /// 按住时叠在按键底色上的黑色。它是按钮的子图层，不属于 configuration 的背景，所以逐键改写背景的皮肤处理（`applyKeyboardSkin`）不会把它丢掉。
  private var pressShade: CAShapeLayer?
  /// `.opacity` 调暗之前按钮的 alpha，松手时恢复。
  private var restingAlpha: CGFloat?

  private func updatePressFeedback() {
    let pressed = isHighlighted && isEnabled
    guard window != nil else {
      resetPressFeedback()
      return
    }
    let scale: CGFloat, shade: CGFloat
    var opacity: CGFloat?
    switch pressFeedback {
    case .key:
      scale = pressed ? Self.keyPressScale : 1
      shade = pressed ? Self.keyPressShade : 0
    case .scale(let value):
      scale = pressed ? value : 1
      shade = 0
    case .opacity(let value):
      scale = 1
      shade = 0
      if pressed {
        if restingAlpha == nil { restingAlpha = alpha }
        opacity = (restingAlpha ?? 1) * value
      } else if let resting = restingAlpha {
        opacity = resting
        restingAlpha = nil
      }
    case .brightness(let value):
      scale = 1
      shade = pressed ? 1 - value : 0
    }
    let duration = KeyPressMotion.duration(for: pressFeedback)
    KeyPressMotion.animate(self, scale: scale, duration: duration)
    setPressShade(shade, duration: duration)
    if let opacity {
      UIView.animate(withDuration: duration, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState, .curveLinear]) {
        self.alpha = opacity
      }
    }
  }

  private func setPressShade(_ value: CGFloat, duration: TimeInterval) {
    guard value > 0 || (pressShade?.opacity ?? 0) > 0 else { return }
    // 自身没有底色的按键无从压暗，盖一块黑色上去反而像是底色。
    guard value == 0 || hasVisibleFill else { return }
    let shade = pressShade ?? {
      let layer = CAShapeLayer()
      layer.fillColor = UIColor.black.cgColor
      layer.opacity = 0
      pressShade = layer
      return layer
    }()
    if value > 0 {
      // 保持在背景和标题之上，UIKit 可能在上次按下之后又插入了它们。
      if layer.sublayers?.last !== shade { layer.addSublayer(shade) }
      layoutPressShade()
    }
    CATransaction.begin()
    CATransaction.setAnimationDuration(duration)
    CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .linear))
    shade.opacity = Float(value)
    CATransaction.commit()
  }

  /// configuration 是否画了底色：皮肤设计的按键表面，或者不透明的背景色。
  private var hasVisibleFill: Bool {
    guard let background = configuration?.background else { return false }
    return background.customView is SkinKeySurfaceView || (background.backgroundColor?.cgColor.alpha ?? 0) > 0
  }

  /// 按键底色的轮廓：皮肤设计自己的键形，或者 configuration 的圆角背景。
  private func layoutPressShade() {
    guard let shade = pressShade else { return }
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    shade.frame = bounds
    let background = configuration?.background
    let insets = background?.backgroundInsets ?? .zero
    let rect = bounds.inset(by: UIEdgeInsets(top: insets.top, left: insets.leading, bottom: insets.bottom, right: insets.trailing))
    if let surface = background?.customView as? SkinKeySurfaceView {
      // 与 `SkinKeySurfaceView.draw` 填充的键面相同：按它的缩放内缩，再按凸起材质的厚度上移。
      let design = surface.design.normalized
      let depth: CGFloat = design.keyMaterial == .raised ? 3 * surface.scale : 0
      let face = rect.insetBy(dx: surface.scale, dy: surface.scale).inset(by: UIEdgeInsets(top: 0, left: 0, bottom: depth, right: 0))
      shade.path = SkinKeySurfaceView.path(in: face, shape: design.keyShape ?? .rounded, radius: design.cornerRadius * surface.scale).cgPath
    } else {
      let radius = min(background?.cornerRadius ?? KeyboardTheme.current.cornerRadius, min(rect.width, rect.height) / 2)
      shade.path = UIBezierPath(roundedRect: rect, cornerRadius: max(radius, 0)).cgPath
    }
    CATransaction.commit()
  }

  /// 一次清掉按下留下的所有痕迹：变换、遮罩和调暗的 alpha。
  private func resetPressFeedback() {
    layer.removeAllAnimations()
    transform = .identity
    if let shade = pressShade {
      shade.removeAllAnimations()
      CATransaction.begin()
      CATransaction.setDisableActions(true)
      shade.opacity = 0
      CATransaction.commit()
    }
    if let resting = restingAlpha {
      alpha = resting
      restingAlpha = nil
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      resetPressFeedback()
      removePressPreview()
      resetCornerHint()
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

  /// 预览气泡代替按键标题显示的内容，例如在这个键上下滑会输入的字符；为 nil 时显示标题。
  var previewTitleOverride: String? {
    didSet {
      guard previewTitleOverride != oldValue, pressPreview != nil else { return }
      updatePressPreview()
    }
  }

  /// 手指下滑（`SwipeHintPolicy`）或按住 `cornerHintHoldDelay` 时，这个键代替自身动作输入的字符，即字母键的角标（`LetterHintTable`）。为 nil 时按键只响应普通点按。与 Android 的 `ImeLetterRows.bindLetterGestures` 一致：两种手势都会把预览切到角标，松手时输入它。
  var cornerHint: String? {
    didSet {
      if cornerHint == nil { resetCornerHint() }
    }
  }

  /// 松手时输入角标。触发它的触摸被取消而不是正常结束，所以按键自身的动作不会再执行一次。
  var onCornerHint: ((String) -> Void)?

  /// 下滑是否输入角标：「滑动输入符号」（`KeyboardLayoutPreference.swipeSymbols`）关掉时为假。长按不受它影响，与 Android 的 `bindLetterGestures` 一致。
  var swipesCornerHint = true

  /// UIKit 默认的长按时长，与九键长按用的半秒相同；Android 等的是系统的长按超时。
  static let cornerHintHoldDelay: TimeInterval = 0.5

  /// 被跟踪的手指按下的位置，用窗口坐标，免得按下缩放扭曲滑动距离。
  private var cornerHintDownY: CGFloat = 0
  private var cornerHintTriggered = false
  private var cornerHintHold: Timer?

  override func beginTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
    let began = super.beginTracking(touch, with: event)
    resetCornerHint()
    guard began, cornerHint != nil else { return began }
    cornerHintDownY = touch.location(in: nil).y
    cornerHintHold = Timer.scheduledTimer(withTimeInterval: Self.cornerHintHoldDelay, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated {
        // 滑出按键的手指不再算按住，对应 Android 检查 `isPressed()`。
        guard let self, self.isTracking, self.isHighlighted, !self.cornerHintTriggered, let hint = self.cornerHint else { return }
        self.triggerCornerHint(hint)
      }
    }
    return began
  }

  override func continueTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
    if !cornerHintTriggered, swipesCornerHint, let hint = cornerHint,
       SwipeHintPolicy.swiped(downY: cornerHintDownY, currentY: touch.location(in: nil).y) {
      triggerCornerHint(hint)
    }
    return super.continueTracking(touch, with: event)
  }

  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    guard cornerHintTriggered, let hint = cornerHint else {
      resetCornerHint()
      super.touchesEnded(touches, with: event)
      return
    }
    // 取消而不是结束这次触摸，与 Android 给按键发 `ACTION_CANCEL` 一样：按下反馈和预览消失，`touchUpInside` 不会触发。
    super.touchesCancelled(touches, with: event)
    resetCornerHint()
    onCornerHint?(hint)
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
    // 触摸被滑行输入接管或被系统取消时，什么也不输入。
    resetCornerHint()
    super.touchesCancelled(touches, with: event)
  }

  private func triggerCornerHint(_ hint: String) {
    cornerHintTriggered = true
    cornerHintHold?.invalidate()
    cornerHintHold = nil
    previewTitleOverride = hint
  }

  private func resetCornerHint() {
    cornerHintHold?.invalidate()
    cornerHintHold = nil
    cornerHintTriggered = false
    previewTitleOverride = nil
  }

  private func updatePressPreview() {
    guard showsPressPreview, isHighlighted, isEnabled, traitCollection.userInterfaceIdiom == .phone,
          let title = previewTitleOverride ?? configuration?.title, !title.isEmpty, let host = pressPreviewHost else {
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
    // 取未经变换的 frame：按键可能已经缩放到一半。
    let key = superview?.convert(untransformedFrame, to: host) ?? convert(bounds, to: host)
    preview.show(title, key: key, wide: KeyPressPreviewView.isWide(key: bounds.width, letter: rowLetterWidth),
                 fill: skin.keyBackground.resolvedColor(with: traitCollection).withAlphaComponent(1),
                 text: skin.keyForeground.resolvedColor(with: traitCollection),
                 monospaced: skin.usesMonospacedFont)
    pressPreview = preview
  }

  /// 按键静止时在父视图坐标系里的 frame，不受按下变换影响。
  private var untransformedFrame: CGRect {
    CGRect(x: center.x - bounds.width / 2, y: center.y - bounds.height / 2, width: bounds.width, height: bounds.height)
  }

  /// 这个键所在行里普通字母键的宽度，即显示预览的可见按键里最窄的那个。设计稿给伸展超过 1.2 个字母宽的按键加宽气泡，这个伸展是相对本行而言的。
  private var rowLetterWidth: CGFloat {
    let widths = (superview?.subviews ?? []).compactMap { view -> CGFloat? in
      guard let key = view as? KeyboardKeyButton, key.showsPressPreview, !key.isHidden, key.bounds.width > 0 else { return nil }
      return key.bounds.width
    }
    return widths.min() ?? bounds.width
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
    if let shade = pressShade, shade.opacity > 0 { layoutPressShade() }
    guard let lines = titleLineCount else { return }
    titleLabel?.numberOfLines = lines
  }
}

/// 设计稿的按键气泡：与按住的键分开的圆角矩形，比键宽并压住它的上边缘，里面是放大的字符。它放在键盘的根视图里，才能伸到按键所在行的上方。
final class KeyPressPreviewView: UIView {
  struct Geometry: Equatable {
    /// 气泡。
    var head: CGRect
    /// 它所属的按键。
    var key: CGRect
  }

  static let height: CGFloat = 54
  /// 气泡下边缘向下压过按键上边缘的距离。
  static let overlap: CGFloat = 6
  static let topRadius: CGFloat = 12
  static let bottomRadius: CGFloat = 8
  static let fontSize: CGFloat = 30
  static let widthFactor: CGFloat = 1.38
  static let wideWidthFactor: CGFloat = 1.5
  /// 伸展宽度超过这么多个字母的按键用 `wideWidthFactor`。
  static let wideKeyThreshold: CGFloat = 1.2

  /// 在普通字母键宽 `letter` 点的行里，宽 `key` 点的按键是否算宽键。
  static func isWide(key: CGFloat, letter: CGFloat) -> Bool {
    letter > 0 && key > letter * wideKeyThreshold
  }

  /// `key` 的气泡位置，两者都在 `bounds` 的坐标系里：与按键居中对齐，在键盘左右边缘处向内收，下边缘比按键上边缘低 `overlap`。按键上方空间不够时气泡变矮：扩展的窗口止于 `bounds`，画到外面的部分会被系统裁掉。
  static func geometry(key: CGRect, in bounds: CGRect, wide: Bool = false) -> Geometry {
    let width = min(key.width * (wide ? wideWidthFactor : widthFactor), bounds.width)
    let x = min(max(key.midX - width / 2, bounds.minX), bounds.maxX - width)
    let bottom = key.minY + overlap
    let height = max(min(Self.height, bottom - bounds.minY), 0)
    return Geometry(head: CGRect(x: x, y: bottom - height, width: width, height: height), key: key)
  }

  private let shape = CAShapeLayer()
  private let label = UILabel()

  override init(frame: CGRect) {
    super.init(frame: frame)
    isUserInteractionEnabled = false
    isAccessibilityElement = false
    accessibilityElementsHidden = true
    layer.addSublayer(shape)
    // 设计稿的 `0 6px 18px rgba(0,0,0,.22)` 阴影，加一圈 .5pt 的 `rgba(0,0,0,.08)` 描边。
    shape.shadowColor = UIColor.black.cgColor
    shape.shadowOpacity = 0.22
    shape.shadowRadius = 9
    shape.shadowOffset = CGSize(width: 0, height: 6)
    shape.strokeColor = UIColor.black.withAlphaComponent(0.08).cgColor
    shape.lineWidth = 0.5
    label.textAlignment = .center
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.5
    addSubview(label)
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  var text: String? { label.text }

  func show(_ title: String, key: CGRect, wide: Bool, fill: UIColor, text: UIColor, monospaced: Bool) {
    let geometry = Self.geometry(key: key, in: bounds, wide: wide)
    let path = Self.path(geometry.head)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    shape.frame = bounds
    shape.path = path
    shape.shadowPath = path
    shape.fillColor = fill.cgColor
    CATransaction.commit()
    // 被窗口顶部压矮的气泡，字符仍留在气泡内。
    let size = min(Self.fontSize, geometry.head.height * 0.62)
    label.font = monospaced ? .monospacedSystemFont(ofSize: size, weight: .regular) : .systemFont(ofSize: size, weight: .regular)
    label.textColor = text
    label.text = title
    label.frame = geometry.head.insetBy(dx: 4, dy: 2)
  }

  /// 上方圆角为 `topRadius`、下方圆角为 `bottomRadius` 的圆角矩形。
  private static func path(_ rect: CGRect) -> CGPath {
    let limit = min(rect.width, rect.height) / 2
    let top = min(topRadius, limit), bottom = min(bottomRadius, limit)
    let path = UIBezierPath()
    path.move(to: CGPoint(x: rect.minX + top, y: rect.minY))
    path.addLine(to: CGPoint(x: rect.maxX - top, y: rect.minY))
    path.addArc(withCenter: CGPoint(x: rect.maxX - top, y: rect.minY + top), radius: top,
                startAngle: -.pi / 2, endAngle: 0, clockwise: true)
    path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - bottom))
    path.addArc(withCenter: CGPoint(x: rect.maxX - bottom, y: rect.maxY - bottom), radius: bottom,
                startAngle: 0, endAngle: .pi / 2, clockwise: true)
    path.addLine(to: CGPoint(x: rect.minX + bottom, y: rect.maxY))
    path.addArc(withCenter: CGPoint(x: rect.minX + bottom, y: rect.maxY - bottom), radius: bottom,
                startAngle: .pi / 2, endAngle: .pi, clockwise: true)
    path.addLine(to: CGPoint(x: rect.minX, y: rect.minY + top))
    path.addArc(withCenter: CGPoint(x: rect.minX + top, y: rect.minY + top), radius: top,
                startAngle: .pi, endAngle: -.pi / 2, clockwise: true)
    path.close()
    return path.cgPath
  }
}

/// 所有键盘按钮共用的按下缩放：每次按下一段线性动画，开启「减弱动态效果」或不在屏幕上时跳过。
enum KeyPressMotion {
  /// 设计稿里按键按下的动画时长为 .06s，工具栏的 `transform` 为 .1s。
  static func duration(for feedback: KeyboardKeyButton.PressFeedback) -> TimeInterval {
    if case .scale = feedback { return 0.1 }
    return KeyboardKeyButton.pressDuration
  }

  /// 在 `duration` 内把 `view` 缩放到 `scale`（1 为复原）。开启「减弱动态效果」或视图不在窗口中时，直接回到 identity，不留动画。
  static func animate(_ view: UIView, scale: CGFloat, duration: TimeInterval) {
    guard !UIAccessibility.isReduceMotionEnabled, view.window != nil else {
      view.layer.removeAllAnimations()
      view.transform = .identity
      return
    }
    let target = scale == 1 ? CGAffineTransform.identity : CGAffineTransform(scaleX: scale, y: scale)
    guard view.transform != target else { return }
    UIView.animate(withDuration: duration, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState, .curveLinear]) {
      view.transform = target
    }
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
