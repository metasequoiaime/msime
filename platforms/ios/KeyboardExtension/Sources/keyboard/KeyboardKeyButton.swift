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

  /// 带释义的词块按内容取宽：候选字和最宽那行释义里更宽的那个，释义最多把词块撑到一栏（`column`，候选栏的三分之一），再长的释义由 `fittedGloss` 缩小或截断。原先带释义的词块一律至少一栏宽，「你好 / hello」这样的短候选也占去三分之一个候选栏。
  static func chipContentWidth(titleLine: CGFloat, glossLine: CGFloat, glossLines: Int, column: CGFloat) -> CGFloat {
    glossLines > 0 ? max(titleLine, min(glossLine, column)) : titleLine
  }

  static func chipWidth(
    titleLine: CGFloat, glossLine: CGFloat, glossLines: Int, column: CGFloat, insets: NSDirectionalEdgeInsets
  ) -> CGFloat {
    ceil(chipContentWidth(titleLine: titleLine, glossLine: glossLine, glossLines: glossLines, column: column)
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
    // 整段在最小字号下也放不下：留下最长的、接上「…」还放得下的前缀（0 到少一个字符），连「…」都放不下时为空。按长度二分，每次只量一次。原来从末尾一个字一个字地删、每删一个量一次，单字候选的 Unihan 释义常要删掉几十个字符，每次按键每个词块都要量几十次。只记量过确实放得下的结果，所以字宽偶尔不随长度单调增长时也不会画出超宽的一行。
    let floorFont = font.withSize(minimumGlossFontSize)
    let characters = Array(text)
    var fitted = ""
    var low = 0
    var high = characters.count - 1
    while low <= high {
      let length = (low + high) / 2
      let shortened = String(characters[..<length]) + "…"
      if measure(shortened, floorFont) <= width {
        fitted = shortened
        low = length + 1
      } else {
        high = length - 1
      }
    }
    return (fitted, floorFont)
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

  /// 连续输入的九键关闭过渡，其他按钮保留原有动画。
  var animatesPressFeedback = true

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
    let duration = animatesPressFeedback ? KeyPressMotion.duration(for: pressFeedback) : 0
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
    if duration == 0 {
      CATransaction.setDisableActions(true)
      shade.removeAnimation(forKey: "opacity")
    }
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
      _ = nineKeySwipe.finish()
      restoreNineKeySwipeFace()
      touchDownLocation = nil
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

  /// 手指沿设定方向滑动（`SwipeHintPolicy`）或按住 `cornerHintHoldDelay` 时，这个键代替自身动作输入的字符，即字母键的角标（`LetterHintTable`）。为 nil 时按键只响应普通点按。与 Android 的 `ImeLetterRows.bindLetterGestures` 一致：两种手势都会把预览切到角标，松手时输入它。
  var cornerHint: String? {
    didSet {
      if cornerHint == nil { resetCornerHint() }
    }
  }

  /// 松手时输入角标。触发它的触摸被取消而不是正常结束，所以按键自身的动作不会再执行一次。
  var onCornerHint: ((String) -> Void)?

  /// 滑动是否输入角标：「滑动输入符号」（`KeyboardLayoutPreference.swipeSymbols`）关掉时为假。长按不受它影响，与 Android 的 `bindLetterGestures` 一致。
  var swipesCornerHint = true
  var cornerHintSwipeDirection: KeyboardLayoutPreference.SwipeDirection = .up
  /// 为真时上滑、下滑都输入角标，不看 `cornerHintSwipeDirection`，纵向位移还要大于横向，按满 `cornerHintHoldDelay` 还没滑出去就不再起滑动（`SwipeHintPolicy.swipedEitherWay`）。「长空格」的逗号用它，与 Android 的 `ImeBottomRow.bindCommaSwipe` 一致。
  var cornerHintSwipesEitherWay = false
  /// 按住是否输入角标。为假时角标只靠滑动输入，按住留给按键自己的长按菜单（`menu`），例如 14 键「长空格」的逗号：上滑或下滑输入句号，长按仍打开常用标点。
  var holdsCornerHint = true

  /// UIKit 默认的长按时长，与九键长按用的半秒相同；Android 等的是系统的长按超时。
  static let cornerHintHoldDelay: TimeInterval = 0.5

  /// 是否记下按下点（#10018）：26 键的字母键和微软双拼的 `;`。其余按钮不记，按下时不必去找键区。
  var recordsTouchDown = false

  /// 这一次触摸按下时手指在键区（`KeyAreaStackView`）坐标里的位置：落在键距里被 `KeyGapRouting` 归给这个键的触摸也是手指的真实位置，不受按下缩放影响。松手输入时由按键的动作取走（`takeTouchDownLocation`）；触摸取消、改输角标或结束后都清掉，不会留给之后没有触摸的激活（`sendActions`、读屏）。
  var touchDownLocation: CGPoint?

  /// 取走并清掉这一次的按下点。
  func takeTouchDownLocation() -> CGPoint? {
    defer { touchDownLocation = nil }
    return touchDownLocation
  }

  /// 「九键滑动输入数字」（#10229）：九宫格 1–9 键的数字，为 nil 时这个键不接九键滑动。控制器建九宫格时设好；同一个键在数字层、九键双拼里也是它，那时 `nineKeySwipeMode` 返回关闭。
  var nineKeySwipeDigit: Int?
  /// 这个键有没有字母可弹：2–9 有，1 键是「@#」没有，反方向滑动什么也不做。
  var nineKeySwipeHasLetters = false
  /// 右上角的数字角标，输入数字时临时藏起来，松手或取消后还原。
  weak var nineKeySwipeHint: UIView?
  /// 此刻的设置，每次判定时现读：不在拼音九键字母层、长按菜单已经弹出、横滑移光标接管了触摸或读屏开着时由控制器返回关闭。
  var nineKeySwipeMode: (() -> NineKeySwipeDigits)?
  /// 判出反方向时立刻以 `.letters` 回调（弹出数字和字母）；沿设定方向的在松手时以 `.digit` 回调（输入数字）。触摸被取消时不回调。两种都不再执行按键自己的点按。
  var onNineKeySwipe: ((NineKeySwipePolicy.Gesture) -> Void)?
  private var nineKeySwipe = NineKeySwipeTracker()
  /// 输入数字期间被换下的键面标题和角标的显隐，松手或取消时原样还原（1 键的键面可能随组字状态变过）。
  private var nineKeySwipeRestingFace: (title: String?, hintHidden: Bool)?

  /// 当前这次触摸已经判出九键滑动。
  var nineKeySwipeGesture: NineKeySwipePolicy.Gesture { nineKeySwipe.gesture }

  /// 被跟踪的手指按下的位置，用窗口坐标，免得按下缩放扭曲滑动距离。
  private var cornerHintDown: CGPoint = .zero
  /// 按下时的触摸时间戳（`UITouch.timestamp`），`cornerHintSwipesEitherWay` 用它判断这次触摸是否已经归长按。
  private var cornerHintDownTime: TimeInterval = 0
  private var cornerHintTriggered = false
  private var cornerHintHold: Timer?

  override func beginTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
    let began = super.beginTracking(touch, with: event)
    resetCornerHint()
    touchDownLocation = began && recordsTouchDown ? keyArea.map { touch.location(in: $0) } : nil
    restoreNineKeySwipeFace()
    _ = nineKeySwipe.finish()
    if began, nineKeySwipeDigit != nil { nineKeySwipe.begin(y: touch.location(in: nil).y) }
    guard began, cornerHint != nil else { return began }
    cornerHintDown = touch.location(in: nil)
    cornerHintDownTime = touch.timestamp
    guard holdsCornerHint else { return began }
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
    if let digit = nineKeySwipeDigit, nineKeySwipe.gesture == .none, let mode = nineKeySwipeMode?(),
       let decided = nineKeySwipe.move(y: touch.location(in: nil).y, mode: mode, hasLetters: nineKeySwipeHasLetters) {
      beginNineKeySwipe(decided, digit: digit)
      return true
    }
    if !cornerHintTriggered, swipesCornerHint, let hint = cornerHint, cornerHintSwiped(touch) {
      triggerCornerHint(hint)
    }
    return super.continueTracking(touch, with: event)
  }

  private func cornerHintSwiped(_ touch: UITouch) -> Bool {
    let current = touch.location(in: nil)
    guard cornerHintSwipesEitherWay else {
      return SwipeHintPolicy.swiped(direction: cornerHintSwipeDirection, downY: cornerHintDown.y, currentY: current.y)
    }
    return SwipeHintPolicy.swipedEitherWay(down: cornerHintDown, current: current,
                                           elapsed: touch.timestamp - cornerHintDownTime, holdDelay: Self.cornerHintHoldDelay)
  }

  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    // 判出九键滑动之后这次触摸不再是点按：键停在判出时的样子（输入数字时保持按下，弹出字母时松开），不随手指进出按键变化。
    guard nineKeySwipe.gesture == .none else { return }
    super.touchesMoved(touches, with: event)
    if nineKeySwipe.gesture != .none { isHighlighted = nineKeySwipe.gesture == .digit }
  }

  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    let swiped = nineKeySwipe.finish()
    if swiped != .none {
      // 和角标一样取消而不是结束这次触摸，按键自己的点按不会再触发；还原键面之后再上屏，上屏引起的重画看到的是原来的键面。
      super.touchesCancelled(touches, with: event)
      restoreNineKeySwipeFace()
      resetCornerHint()
      touchDownLocation = nil
      if swiped == .digit { onNineKeySwipe?(.digit) }
      return
    }
    guard cornerHintTriggered, let hint = cornerHint else {
      resetCornerHint()
      // 松手输入时按键的动作在这里面同步执行并取走按下点；手指滑出按键后松开不输入，按下点也不留下。
      super.touchesEnded(touches, with: event)
      touchDownLocation = nil
      return
    }
    // 取消而不是结束这次触摸，与 Android 给按键发 `ACTION_CANCEL` 一样：按下反馈和预览消失，`touchUpInside` 不会触发。
    super.touchesCancelled(touches, with: event)
    resetCornerHint()
    touchDownLocation = nil
    onCornerHint?(hint)
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
    // 触摸被滑行输入、横滑移光标或长按接管，或被系统取消时，什么也不输入；已经换成数字的键面还原。
    _ = nineKeySwipe.finish()
    restoreNineKeySwipeFace()
    resetCornerHint()
    touchDownLocation = nil
    super.touchesCancelled(touches, with: event)
  }

  /// 判出九键滑动的那一刻：取消这个键的长按（之后不会再弹长按菜单），点按由 `touchesEnded` 改成取消。输入数字时键保持按下、键面换成数字、角标藏起；弹出字母时立刻回调。
  private func beginNineKeySwipe(_ gesture: NineKeySwipePolicy.Gesture, digit: Int) {
    touchDownLocation = nil
    for hold in gestureRecognizers ?? [] where hold is UILongPressGestureRecognizer {
      let enabled = hold.isEnabled
      hold.isEnabled = false
      hold.isEnabled = enabled
    }
    switch gesture {
    case .digit:
      if nineKeySwipeRestingFace == nil {
        nineKeySwipeRestingFace = (configuration?.title, nineKeySwipeHint?.isHidden ?? true)
      }
      configuration?.title = String(digit)
      nineKeySwipeHint?.isHidden = true
      previewTitleOverride = String(digit)
      isHighlighted = true
    case .letters:
      isHighlighted = false
      onNineKeySwipe?(.letters)
    case .none:
      break
    }
  }

  private func restoreNineKeySwipeFace() {
    guard let face = nineKeySwipeRestingFace else { return }
    nineKeySwipeRestingFace = nil
    configuration?.title = face.title
    nineKeySwipeHint?.isHidden = face.hintHidden
    previewTitleOverride = nil
  }

  /// 这个键所在的键区。按键的祖先里最近的 `KeyAreaStackView`；不在键区里（面板、候选栏）时为 nil。
  private var keyArea: KeyAreaStackView? {
    var node = superview
    while let current = node {
      if let area = current as? KeyAreaStackView { return area }
      node = current.superview
    }
    return nil
  }

  private func triggerCornerHint(_ hint: String) {
    // 这一下输入的是角标，不是字母，按下点不再有用。
    touchDownLocation = nil
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

  private var appliedShadowBounds: CGRect?
  private var appliedShadowRadius: CGFloat?

  override func layoutSubviews() {
    super.layoutSubviews()
    // 按钮自己的布局才能看到最终尺寸。控制器不再遍历整棵视图树重复更新阴影；
    // 按钮尺寸和圆角不变时，候选更新引发的布局也不必重建路径。
    if layer.shadowOpacity > 0 {
      let radius = KeyboardTheme.current.cornerRadius
      if appliedShadowBounds != bounds || appliedShadowRadius != radius || layer.shadowPath == nil {
        layer.shadowPath = UIBezierPath(roundedRect: bounds, cornerRadius: radius).cgPath
        appliedShadowBounds = bounds
        appliedShadowRadius = radius
      }
    }
    if let shade = pressShade, shade.opacity > 0 { layoutPressShade() }
    guard let lines = titleLineCount else { return }
    if titleLabel?.numberOfLines != lines { titleLabel?.numberOfLines = lines }
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
    if duration == 0 {
      view.layer.removeAnimation(forKey: "transform")
      UIView.performWithoutAnimation { view.transform = target }
      return
    }
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

/// 九键拼音列只在固定宽度内竖排按钮；内容变化不向整套键盘传播排列约束。
final class KeyboardSpellingScrollView: UIScrollView {
  var buttons: [UIButton] = [] { didSet { setNeedsLayout() } }

  override func layoutSubviews() {
    super.layoutSubviews()
    var y: CGFloat = 0
    for button in buttons where !button.isHidden {
      let height = ceil(button.intrinsicContentSize.height)
      let frame = CGRect(x: 0, y: y, width: bounds.width, height: height)
      if button.frame != frame { button.frame = frame }
      y += height + 6
    }
    let size = CGSize(width: bounds.width, height: max(0, y - 6))
    if contentSize != size { contentSize = size }
  }
}

/// 九键字母层用自定义符号表时的左列：每项等高，不超过一屏（`visibleRowLimit`，与九宫格三行对齐）时按实际项数均分左列高度，超过时一屏放满、其余上下滚动；每项会矮于 28pt（`NineKeySidebar.minRowHeight`）时少放几项。按帧排列，表变了不向整套键盘传播约束。
final class NineKeySidebarScrollView: UIScrollView {
  var buttons: [UIButton] = [] { didSet { setNeedsLayout() } }
  var visibleRowLimit = NineKeySidebar.letterVisibleRows { didSet { setNeedsLayout() } }

  override init(frame: CGRect) {
    super.init(frame: frame)
    configure()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    configure()
  }

  private func configure() {
    // 与 `CandidateScrollView` 一样：按下当场有反馈，手指一滑就交给滚动。
    delaysContentTouches = false
    showsVerticalScrollIndicator = true
    showsHorizontalScrollIndicator = false
    disableEdgeEffects()
  }

  override func touchesShouldCancel(in view: UIView) -> Bool {
    if view is KeyboardKeyButton { return true }
    return super.touchesShouldCancel(in: view)
  }

  /// 此刻一屏放几项。
  var visibleRows: Int {
    NineKeySidebar.visibleRows(count: buttons.count, limit: visibleRowLimit, height: bounds.height)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let rows = visibleRows
    let height = rows > 0 ? bounds.height / CGFloat(rows) : 0
    for (index, button) in buttons.enumerated() {
      let frame = CGRect(x: 0, y: CGFloat(index) * height, width: bounds.width, height: height)
      if button.frame != frame { button.frame = frame }
    }
    let size = CGSize(width: bounds.width, height: height * CGFloat(buttons.count))
    if contentSize != size { contentSize = size }
  }
}
