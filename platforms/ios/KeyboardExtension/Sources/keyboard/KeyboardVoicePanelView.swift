import UIKit

/// 聆听面板的纯规则：动作行以上区域（按住说话时没有动作行，就是整个面板）的排版、麦克风电平的平滑和实时文字的截取。Android 与 HarmonyOS 用同一组数字，单位是 pt。
enum KeyboardVoicePanelPolicy {
  static let orbMax: CGFloat = 64
  static let orbMin: CGFloat = 40
  /// 光晕外扩是圆球直径的四分之一，所以指示区边长是圆球的 1.5 倍。
  static let haloRatio: CGFloat = 0.25
  /// 圆球小于这个尺寸时不再给标题留两行。
  static let twoLineMinOrb: CGFloat = 52
  static let padTop: CGFloat = 4
  static let gapIndicatorTitle: CGFloat = 6
  static let gapTitleHint: CGFloat = 4
  static let padBottom: CGFloat = 8
  /// 电平上升和回落的时间常数（毫秒）：说话时光晕立刻跟上，停下时慢慢收回。
  static let attackMs: Double = 60
  static let releaseMs: Double = 320
  /// 一帧最多按这么久推进，掉帧或刚恢复时不会一下跳到目标。
  static let maximumStepMs: Double = 100
  static let settleEpsilon: Float = 0.01
  /// 没有声音时光晕仍露出这一部分，看得出在听。
  static let idleHalo: CGFloat = 0.3
  static let outerHaloOpacity: Float = 0.16
  static let innerHaloOpacity: Float = 0.26
  static let ellipsis = "…"

  /// 一次排版的结果。`height` 是动作行以上的区域，`top` 起把指示区、标题和提示作为一组垂直居中；按计划的行数留位，实时文字换行时圆球不会跟着变大变小。
  struct Plan: Equatable {
    let orb: CGFloat
    let halo: CGFloat
    let titleLines: Int
    let showsHint: Bool
    let top: CGFloat
    let titleLine: CGFloat

    /// 指示区（圆球加两侧光晕）的边长，光晕在最大电平时也画在这个方框里。
    var indicatorSide: CGFloat { orb + 2 * halo }
    var indicatorCenterY: CGFloat { top + padTop + halo + orb / 2 }
    var titleTop: CGFloat { top + padTop + indicatorSide + gapIndicatorTitle }

    /// 提示行紧跟标题实际用到的行数；标题只用一行时，多留的那一行落在提示下面。
    func hintTop(usedTitleLines: Int) -> CGFloat {
      titleTop + CGFloat(min(max(usedTitleLines, 1), titleLines)) * titleLine + gapTitleHint
    }
  }

  static func fixed(lines: Int, hint: Bool, titleLine: CGFloat, hintLine: CGFloat) -> CGFloat {
    padTop + gapIndicatorTitle + CGFloat(lines) * titleLine + (hint ? gapTitleHint + hintLine : 0) + padBottom
  }

  /// 依次尝试两行标题加提示、一行标题加提示、只有一行标题，取第一个圆球不小于 `orbMin` 的；都放不下时圆球固定为 `orbMin`，再矮就连光晕一起缩。
  static func plan(height: CGFloat, titleLine: CGFloat, hintLine: CGFloat) -> Plan {
    func make(orb: CGFloat, halo: CGFloat, lines: Int, hint: Bool) -> Plan {
      let used = orb + 2 * halo + fixed(lines: lines, hint: hint, titleLine: titleLine, hintLine: hintLine)
      return Plan(orb: orb, halo: halo, titleLines: lines, showsHint: hint, top: max(0, (height - used) / 2), titleLine: titleLine)
    }
    for (lines, hint) in [(2, true), (1, true), (1, false)] {
      let box = height - fixed(lines: lines, hint: hint, titleLine: titleLine, hintLine: hintLine)
      let orb = min(orbMax, box / (1 + 2 * haloRatio))
      if lines == 2 && orb < twoLineMinOrb { continue }
      if orb >= orbMin { return make(orb: orb, halo: orb * haloRatio, lines: lines, hint: hint) }
    }
    let box = max(0, height - fixed(lines: 1, hint: false, titleLine: titleLine, hintLine: hintLine))
    guard box >= orbMin else { return make(orb: box, halo: 0, lines: 1, hint: false) }
    return make(orb: orbMin, halo: min((box - orbMin) / 2, orbMin * haloRatio), lines: 1, hint: false)
  }

  static func clampedLevel(_ level: Float) -> Float { level.isNaN ? 0 : min(max(level, 0), 1) }

  /// 显示的电平向目标推进一帧：上升用 `attackMs`，回落用 `releaseMs`，离目标不到 `settleEpsilon` 时直接落到目标。
  static func step(shown: Float, target: Float, elapsedMs: Double) -> Float {
    let goal = clampedLevel(target)
    let elapsed = elapsedMs.isNaN ? 0 : min(max(elapsedMs, 0), maximumStepMs)
    let tau = goal > shown ? attackMs : releaseMs
    let next = shown + (goal - shown) * Float(1 - exp(-elapsed / tau))
    return abs(goal - next) < settleEpsilon ? goal : next
  }

  static func settled(shown: Float, target: Float) -> Bool { abs(clampedLevel(target) - shown) < settleEpsilon }

  // ---- 水杉娘指示区（#10313）----

  /// 点按开始（功能菜单「语音输入」磁贴、读屏操作等 `held: false` 的入口）的会话把圆球换成水杉娘与耳边红线；按住空格说话保留圆球和光晕。Android 工具栏入口与 HarmonyOS 非按住的会话按同一条规则。
  static func showsMascot(held: Bool) -> Bool { !held }

  /// 人物、耳部和红线画在 320×250 的视口里，数值与 Android 的 `VoiceMascotDrawing` 相同。
  static let mascotViewport = CGSize(width: 320, height: 250)
  static let mascotCharacter = CGRect(x: 20, y: 12, width: 228, height: 228)
  static let mascotEarCenter = CGPoint(x: 218, y: 111)
  /// 红线从水平向右往上下各张开这么多度。
  static let mascotArcSweepDegrees: CGFloat = 48
  static let mascotEarLineWidth: CGFloat = 2.5

  /// 一道红线：半径和线宽是视口单位，`opacity` 是 0…1。
  struct MascotArc: Equatable {
    let radius: CGFloat
    let opacity: CGFloat
    let lineWidth: CGFloat
  }

  /// 按显示电平给出要画的红线，由内向外：电平不到 `settleEpsilon` 时一道也不画（不沿用圆球的 `idleHalo`，安静时耳边没有线），第 i 道在电平达到 0.3 × i 时出现，半径 33 + 15i + 12 × 电平，不透明度 (90 + 165 × 电平) / 255，线宽 3 + 2 × 电平。
  static func mascotArcs(level: Float) -> [MascotArc] {
    let value = CGFloat(clampedLevel(level))
    guard value >= CGFloat(settleEpsilon) else { return [] }
    return (0..<3).compactMap { index in
      guard value >= 0.3 * CGFloat(index) else { return nil }
      return MascotArc(radius: 33 + 15 * CGFloat(index) + 12 * value, opacity: (90 + 165 * value) / 255,
                       lineWidth: 3 + 2 * value)
    }
  }

  /// 人物指示区的宽度：高度仍是 `Plan.indicatorSide`（圆球的 1.5 倍），宽度按视口比例放宽，不超过面板宽度。
  static func mascotWidth(side: CGFloat, available: CGFloat) -> CGFloat {
    max(0, min(available, side * mascotViewport.width / mascotViewport.height))
  }

  /// 把视口等比缩放后居中放进 `size` 的变换。
  static func mascotTransform(in size: CGSize) -> CGAffineTransform {
    let scale = max(0, min(size.width / mascotViewport.width, size.height / mascotViewport.height))
    return CGAffineTransform(translationX: (size.width - mascotViewport.width * scale) / 2,
                             y: (size.height - mascotViewport.height * scale) / 2).scaledBy(x: scale, y: scale)
  }

  /// 两圈光晕的半径：外圈铺满外扩，内圈只到一半；电平为 0 时仍露出 `idleHalo`。
  static func haloRadii(orb: CGFloat, halo: CGFloat, level: Float) -> (outer: CGFloat, inner: CGFloat) {
    let spread = idleHalo + (1 - idleHalo) * CGFloat(clampedLevel(level))
    return (orb / 2 + halo * spread, orb / 2 + 0.5 * halo * spread)
  }

  /// 实时文字放不下时保留最新的部分：先把空白（含换行）压成一个空格，整段放得下就原样返回，否则在字形边界上找放得下的最靠前的起点，前面加省略号；一个字也放不下时只留最后一个字。
  static func fitTail(_ text: String, fits: (String) -> Bool) -> String {
    let normalized = text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    guard !normalized.isEmpty else { return "" }
    if fits(normalized) { return normalized }
    let characters = Array(normalized)
    // 起点越靠后剩下的越短，越放得下，所以可以二分。
    var low = 1, high = characters.count - 1, best = characters.count - 1
    while low <= high {
      let middle = (low + high) / 2
      if fits(ellipsis + String(characters[middle...])) {
        best = middle
        high = middle - 1
      } else {
        low = middle + 1
      }
    }
    return ellipsis + String(characters[best...])
  }
}

/// 手机键盘的聆听面板：结束采集后直接上屏，取消只丢弃本次听写。按住空格时由松手结束、上滑取消，面板上没有按钮；点按开始时有「说完了」和「取消」。
///
/// 点按开始时动作行贴着面板底边（留 `actionsBottomMargin`），它以上的区域按 `KeyboardVoicePanelPolicy.plan` 排指示区、标题和提示；按住说话时没有动作行，整个面板都给这三样。光晕的外扩算进指示区，不会画出面板。
final class KeyboardVoicePanelView: UIView {
  static let titleSize: CGFloat = 16
  static let hintSize: CGFloat = 13
  /// 跟随系统字号放大时的上限：再大就只剩一行标题和最小的圆球了。
  static let titleMaximumSize: CGFloat = 24
  static let hintMaximumSize: CGFloat = 19
  static let horizontalInset: CGFloat = 16
  static let actionsBottomMargin: CGFloat = 10
  static let actionMinimumSide: CGFloat = 44
  static let actionSpacing: CGFloat = 24
  /// 麦克风图标相对圆球的大小：72pt 的圆球里是 30pt，按同一比例缩放。
  static let micRatio: CGFloat = 30 / 72
  static let micLineWidth: CGFloat = 1.8
  static let listeningTitle = "正在聆听…"
  static let recognizingTitle = "正在识别…"
  static let cancellingTitle = "松开取消"
  static let heldCaption = "松开结束，上滑取消"
  static let tappedCaption = "说完后点「说完了」，识别后自动上屏"
  static let recognizingCaption = "识别完成后自动上屏"

  let orb: KeyboardVoiceOrbButton
  let statusLabel = UILabel()
  let captionLabel = UILabel()
  let finishButton = UIButton(type: .system)
  let cancelButton = UIButton(type: .system)
  var onFinish: (() -> Void)?
  var onCancel: (() -> Void)?
  private let actions: UIStackView
  private let idleCaption: String
  private(set) var isCancelling = false
  private(set) var isRecognizing = false
  /// 最近一次的实时文字，按当前宽度截取后才显示。
  private var partial = ""
  private var message = ""
  /// 最近一次布局的排版。
  private(set) var plan: KeyboardVoicePanelPolicy.Plan?

  init(dictationHeld held: Bool, skin: KeyboardTheme) {
    orb = KeyboardVoiceOrbButton(skin: skin, mascot: KeyboardVoicePanelPolicy.showsMascot(held: held))
    idleCaption = held ? Self.heldCaption : Self.tappedCaption
    actions = UIStackView(arrangedSubviews: [finishButton, cancelButton])
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardVoiceListening"
    backgroundColor = .clear
    orb.isUserInteractionEnabled = false
    orb.isAccessibilityElement = false
    statusLabel.accessibilityIdentifier = "keyboardVoicePanelStatus"
    statusLabel.textAlignment = .center
    statusLabel.lineBreakMode = .byTruncatingTail
    captionLabel.textAlignment = .center
    captionLabel.numberOfLines = 1
    captionLabel.lineBreakMode = .byTruncatingTail
    finishButton.setTitle("说完了", for: .normal)
    finishButton.accessibilityIdentifier = "keyboardVoiceFinish"
    finishButton.accessibilityLabel = "结束聆听并识别，文字自动上屏"
    finishButton.isHidden = held
    finishButton.addAction(UIAction { [weak self] _ in self?.onFinish?() }, for: .primaryActionTriggered)
    cancelButton.setTitle("取消", for: .normal)
    cancelButton.accessibilityIdentifier = "keyboardVoiceCancel"
    cancelButton.addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .primaryActionTriggered)
    // 按住说话时手指还按在空格上，松手结束、上滑取消，面板上不放按钮；读屏的退出手势仍然取消。
    cancelButton.isHidden = held
    actions.isHidden = held
    actions.spacing = Self.actionSpacing
    actions.alignment = .center
    for view in [orb, statusLabel, captionLabel, actions] as [UIView] { addSubview(view) }
    var minimums: [NSLayoutConstraint] = []
    for button in [finishButton, cancelButton] {
      minimums.append(button.heightAnchor.constraint(greaterThanOrEqualToConstant: Self.actionMinimumSide))
      minimums.append(button.widthAnchor.constraint(greaterThanOrEqualToConstant: Self.actionMinimumSide))
    }
    NSLayoutConstraint.activate(minimums)
    registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (panel: KeyboardVoicePanelView, _: UITraitCollection) in
      panel.setNeedsLayout()
    }
    apply(skin: skin)
    refreshText()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  func showListening(_ text: String) {
    partial = text
    refreshText()
    setNeedsLayout()
  }

  func showRecognizing(text: String, message: String) {
    isCancelling = false
    isRecognizing = true
    orb.isCancelling = false
    orb.isRecognizing = true
    orb.setLevel(0)
    finishButton.isEnabled = false
    partial = text
    self.message = message
    refreshText()
    setNeedsLayout()
  }

  func setCancelling(_ cancelling: Bool) {
    guard cancelling != isCancelling else { return }
    isCancelling = cancelling
    orb.isCancelling = cancelling
    refreshText()
    setNeedsLayout()
  }

  /// App 报来的麦克风电平（0…1）；识别阶段已经不在录音，光晕收回到静止。
  func setLevel(_ level: Float) {
    guard !isRecognizing else { return }
    orb.setLevel(level)
  }

  func apply(skin: KeyboardTheme) {
    statusLabel.textColor = skin.keyForeground
    captionLabel.textColor = skin.secondary
    finishButton.tintColor = skin.accent
    cancelButton.tintColor = skin.keyForeground
    orb.apply(skin: skin)
  }

  override func accessibilityPerformEscape() -> Bool {
    onCancel?()
    return true
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let inset = Self.horizontalInset
    var region = bounds.height
    if !actions.isHidden {
      let row = actions.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
      let rowWidth = min(row.width, max(0, bounds.width - 2 * inset))
      let rowHeight = max(Self.actionMinimumSide, row.height)
      actions.frame = CGRect(x: bounds.midX - rowWidth / 2, y: bounds.height - Self.actionsBottomMargin - rowHeight,
                             width: rowWidth, height: rowHeight)
      region = actions.frame.minY
    }
    let titleFont = Self.titleFont(for: traitCollection), hintFont = Self.hintFont(for: traitCollection)
    let plan = KeyboardVoicePanelPolicy.plan(height: max(0, region), titleLine: titleFont.lineHeight,
                                             hintLine: hintFont.lineHeight)
    self.plan = plan
    refreshText()
    let side = plan.indicatorSide
    // 人物模式的指示区同样高，宽度按视口比例放宽；素材还没到时圆球画在这个方框的中间。
    let indicatorWidth = orb.showsMascot ? KeyboardVoicePanelPolicy.mascotWidth(side: side, available: bounds.width) : side
    orb.frame = CGRect(x: bounds.midX - indicatorWidth / 2, y: plan.indicatorCenterY - side / 2,
                       width: indicatorWidth, height: side)
    orb.setGeometry(orb: plan.orb, halo: plan.halo)
    let width = labelWidth
    let used = min(plan.titleLines, Self.lineCount(statusLabel.text ?? "", font: titleFont, width: width))
    statusLabel.frame = CGRect(x: inset, y: plan.titleTop, width: width, height: ceil(CGFloat(used) * titleFont.lineHeight))
    captionLabel.isHidden = !plan.showsHint
    captionLabel.frame = CGRect(x: inset, y: plan.hintTop(usedTitleLines: used), width: width, height: ceil(hintFont.lineHeight))
  }

  private var labelWidth: CGFloat { max(0, bounds.width - 2 * Self.horizontalInset) }

  /// 按当前状态和排版填标题与提示：听写中有实时文字时标题显示它（截取到计划的行数），提示保留操作说明；识别中提示显示截成一行的实时文字。标题占的行数可能变，调用方在布局之外调用时要再 `setNeedsLayout`。
  private func refreshText() {
    let titleFont = Self.titleFont(for: traitCollection), hintFont = Self.hintFont(for: traitCollection)
    let lines = plan?.titleLines ?? 2
    statusLabel.font = titleFont
    captionLabel.font = hintFont
    statusLabel.numberOfLines = lines
    if isRecognizing {
      statusLabel.text = message.isEmpty ? Self.recognizingTitle : message
      let fitted = fit(partial, font: hintFont, lines: 1)
      captionLabel.text = fitted.isEmpty ? Self.recognizingCaption : fitted
    } else {
      let fitted = isCancelling ? "" : fit(partial, font: titleFont, lines: lines)
      statusLabel.text = isCancelling ? Self.cancellingTitle : fitted.isEmpty ? Self.listeningTitle : fitted
      captionLabel.text = idleCaption
    }
    // 读屏读到的就是屏幕上显示的这一段。
    statusLabel.accessibilityLabel = statusLabel.text
    captionLabel.accessibilityLabel = captionLabel.text
  }

  /// 还没布局（宽度为 0）时不截取，布局后按实际宽度重新截。
  private func fit(_ text: String, font: UIFont, lines: Int) -> String {
    let width = labelWidth
    return KeyboardVoicePanelPolicy.fitTail(text) { candidate in
      width <= 0 || Self.lineCount(candidate, font: font, width: width) <= lines
    }
  }

  /// `text` 用 `font` 在 `width` 宽里折成几行，和标签的折行一致。
  static func lineCount(_ text: String, font: UIFont, width: CGFloat) -> Int {
    guard !text.isEmpty, width > 0 else { return 1 }
    let height = (text as NSString).boundingRect(
      with: CGSize(width: width, height: .greatestFiniteMagnitude), options: [.usesLineFragmentOrigin],
      attributes: [.font: font], context: nil).height
    return max(1, Int(ceil(height / font.lineHeight - 0.01)))
  }

  /// 标题和提示跟随系统字号，与键盘里的提示文字一样，但设了上限，免得把圆球挤没。
  static func titleFont(for traits: UITraitCollection) -> UIFont {
    UIFontMetrics(forTextStyle: .body).scaledFont(for: .systemFont(ofSize: titleSize), maximumPointSize: titleMaximumSize,
                                                  compatibleWith: traits)
  }

  static func hintFont(for traits: UITraitCollection) -> UIFont {
    UIFontMetrics(forTextStyle: .footnote).scaledFont(for: .systemFont(ofSize: hintSize), maximumPointSize: hintMaximumSize,
                                                      compatibleWith: traits)
  }
}

/// 语音面板的圆球：强调色的圆，麦克风用在它上面看得清的颜色，后面是随麦克风电平张缩的两圈光晕。按钮本身就是指示区，光晕最大时也不出它的边界。它没有 configuration，所以键盘套皮肤时不会碰它。
///
/// `mascot` 为真（点按开始的会话）时，素材解码出来以后改画水杉娘、放大的耳部和随电平出现的三道红线，与 Android 的 `VoiceMascotDrawing` 同一套几何；素材还没到或读不到时仍画圆球。人物是装饰，不进读屏。
final class KeyboardVoiceOrbButton: UIButton {
  private let outerHalo = CAShapeLayer()
  private let innerHalo = CAShapeLayer()
  private let circleLayer = CAShapeLayer()
  private let micLayer = CAShapeLayer()
  private let mascotLayer = CALayer()
  private let earLayer = CAShapeLayer()
  private let earInnerLayer = CAShapeLayer()
  private let arcLayers: [CAShapeLayer] = (0..<3).map { _ in CAShapeLayer() }
  /// 这个会话是否用人物指示区，创建后不变。
  let showsMascot: Bool
  private(set) var mascotImage: UIImage?
  private var skin: KeyboardTheme
  private(set) var orbSide = KeyboardVoicePanelPolicy.orbMax
  private(set) var haloSpread = KeyboardVoicePanelPolicy.orbMax * KeyboardVoicePanelPolicy.haloRatio
  /// 正在画的电平，按 `KeyboardVoicePanelPolicy.step` 每帧向 `targetLevel` 推进。
  private(set) var shownLevel: Float = 0
  private(set) var targetLevel: Float = 0
  private var displayLink: CADisplayLink?
  private var lastFrame: CFTimeInterval?

  init(skin: KeyboardTheme, mascot: Bool = false) {
    self.skin = skin
    showsMascot = mascot
    super.init(frame: .zero)
    backgroundColor = .clear
    accessibilityTraits = .button
    for shape in [outerHalo, innerHalo, circleLayer, micLayer] { layer.addSublayer(shape) }
    outerHalo.opacity = KeyboardVoicePanelPolicy.outerHaloOpacity
    innerHalo.opacity = KeyboardVoicePanelPolicy.innerHaloOpacity
    micLayer.fillColor = nil
    micLayer.lineCap = .round
    micLayer.lineJoin = .round
    if mascot {
      for sublayer in [mascotLayer, earLayer, earInnerLayer] + arcLayers { layer.addSublayer(sublayer) }
      mascotLayer.contentsGravity = .resizeAspect
      // 耳部与红线的颜色和 Android 的 `VoiceMascotDrawing` 一样固定，不随皮肤变。
      earLayer.fillColor = UIColor(red: 255 / 255, green: 220 / 255, blue: 196 / 255, alpha: 1).cgColor
      earLayer.strokeColor = Self.earStroke
      earInnerLayer.fillColor = nil
      earInnerLayer.strokeColor = Self.earStroke
      for shape in [earLayer, earInnerLayer] + arcLayers { shape.lineCap = .round }
      for arc in arcLayers {
        arc.fillColor = nil
        arc.strokeColor = UIColor(red: 211 / 255, green: 47 / 255, blue: 47 / 255, alpha: 1).cgColor
      }
      updateMascotVisibility()
    }
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (orb: KeyboardVoiceOrbButton, _: UITraitCollection) in
      orb.applyColors()
    }
    NotificationCenter.default.addObserver(self, selector: #selector(reduceMotionChanged),
                                           name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
    applyColors()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 光晕是否正在逐帧追电平；追上、离开窗口或开了「减弱动态效果」时停下。
  var isAnimatingLevel: Bool { displayLink != nil }

  /// 外圈光晕此刻在按钮坐标里占的方框。
  var outerHaloFrame: CGRect { outerHalo.path?.boundingBox ?? .zero }

  /// 上滑到取消区时圆球换成次要色，和 Android 一样。
  var isCancelling = false {
    didSet { if isCancelling != oldValue { applyColors() } }
  }

  /// 已结束录音、在识别或润色：人物的红线立即收起，之后的电平也不再画；圆球的光晕照旧按平滑回落。
  var isRecognizing = false {
    didSet { if isRecognizing != oldValue { drawHalo() } }
  }

  /// 此刻画的是人物而不是圆球。
  var isShowingMascot: Bool { showsMascot && mascotImage != nil }

  /// 此刻画出的红线道数，供测试检查。
  var visibleMascotArcs: Int { arcLayers.filter { !$0.isHidden && $0.path != nil }.count }

  /// 人物在按钮坐标里占的方框。
  var mascotFrame: CGRect { mascotLayer.frame }

  /// 解码好的人物素材；`nil` 时退回圆球。
  func setMascot(_ image: UIImage?) {
    guard showsMascot else { return }
    mascotImage = image
    mascotLayer.contents = image?.cgImage
    mascotLayer.contentsScale = image?.scale ?? 1
    updateMascotVisibility()
    setNeedsLayout()
  }

  func apply(skin: KeyboardTheme) {
    self.skin = skin
    applyColors()
  }

  /// 圆球直径和光晕外扩，由面板的排版给出。
  func setGeometry(orb: CGFloat, halo: CGFloat) {
    guard orb != orbSide || halo != haloSpread else { return }
    orbSide = orb
    haloSpread = halo
    setNeedsLayout()
  }

  func setLevel(_ level: Float) {
    targetLevel = KeyboardVoicePanelPolicy.clampedLevel(level)
    updateDisplayLink()
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let mic = orbSide * KeyboardVoicePanelView.micRatio
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    for shape in [outerHalo, innerHalo, circleLayer, micLayer] { shape.frame = bounds }
    circleLayer.path = UIBezierPath(ovalIn: circle(radius: orbSide / 2)).cgPath
    micLayer.path = KeyboardIcon.mic.path(in: CGRect(x: bounds.midX - mic / 2, y: bounds.midY - mic / 2, width: mic, height: mic)).cgPath
    micLayer.lineWidth = KeyboardVoicePanelView.micLineWidth * mic / KeyboardIcon.viewBox
    if showsMascot { layoutMascot() }
    drawHalo()
    CATransaction.commit()
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    updateDisplayLink()
    if showsMascot, mascotImage == nil, let window {
      // 回调时按钮可能已经随面板关掉；弱引用到的只会是发起解码的这一个，不会把素材交给别的面板。
      KeyboardVoiceMascot.load(scale: window.traitCollection.displayScale) { [weak self] image in
        guard let image else { return }
        self?.setMascot(image)
      }
    }
  }

  private static let earStroke = UIColor(red: 157 / 255, green: 92 / 255, blue: 77 / 255, alpha: 1).cgColor

  /// 人物和耳部跟着按钮大小摆到视口里；线宽是视口单位，跟着同一个缩放。
  private func layoutMascot() {
    var transform = KeyboardVoicePanelPolicy.mascotTransform(in: bounds.size)
    let scale = transform.a
    for sublayer in [mascotLayer, earLayer, earInnerLayer] + arcLayers as [CALayer] { sublayer.frame = bounds }
    mascotLayer.frame = KeyboardVoicePanelPolicy.mascotCharacter.applying(transform)
    earLayer.path = Self.earOutline.copy(using: &transform)
    earInnerLayer.path = Self.earInner.copy(using: &transform)
    earLayer.lineWidth = KeyboardVoicePanelPolicy.mascotEarLineWidth * scale
    earInnerLayer.lineWidth = KeyboardVoicePanelPolicy.mascotEarLineWidth * scale
  }

  /// 在原素材右耳的位置画放大的卡通耳部，路径与 Android 的 `VoiceMascotDrawing` 相同。
  private static let earOutline: CGPath = {
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 187, y: 109))
    path.addCurve(to: CGPoint(x: 234, y: 109), control1: CGPoint(x: 189, y: 76), control2: CGPoint(x: 235, y: 77))
    path.addCurve(to: CGPoint(x: 213, y: 145), control1: CGPoint(x: 235, y: 127), control2: CGPoint(x: 215, y: 132))
    path.addCurve(to: CGPoint(x: 190, y: 130), control1: CGPoint(x: 207, y: 158), control2: CGPoint(x: 186, y: 145))
    path.closeSubpath()
    return path
  }()

  private static let earInner: CGPath = {
    let path = CGMutablePath()
    path.move(to: CGPoint(x: 199, y: 111))
    path.addCurve(to: CGPoint(x: 222, y: 109), control1: CGPoint(x: 198, y: 90), control2: CGPoint(x: 224, y: 91))
    path.addCurve(to: CGPoint(x: 206, y: 131), control1: CGPoint(x: 222, y: 118), control2: CGPoint(x: 205, y: 117))
    return path
  }()

  private func updateMascotVisibility() {
    let mascot = isShowingMascot
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    for shape in [outerHalo, innerHalo, circleLayer, micLayer] { shape.isHidden = mascot }
    for sublayer in [mascotLayer, earLayer, earInnerLayer] + arcLayers as [CALayer] { sublayer.isHidden = !mascot }
    CATransaction.commit()
    drawHalo()
  }

  /// 按当前显示电平画三道红线；识别阶段一律按 0 画。
  private func drawMascotArcs() {
    let arcs = KeyboardVoicePanelPolicy.mascotArcs(level: isRecognizing ? 0 : shownLevel)
    let transform = KeyboardVoicePanelPolicy.mascotTransform(in: bounds.size)
    let center = KeyboardVoicePanelPolicy.mascotEarCenter.applying(transform)
    let sweep = KeyboardVoicePanelPolicy.mascotArcSweepDegrees * .pi / 180
    for (index, shape) in arcLayers.enumerated() {
      guard index < arcs.count else {
        shape.path = nil
        continue
      }
      let arc = arcs[index]
      shape.path = UIBezierPath(arcCenter: center, radius: arc.radius * transform.a, startAngle: -sweep,
                                endAngle: sweep, clockwise: true).cgPath
      shape.lineWidth = arc.lineWidth * transform.a
      shape.opacity = Float(arc.opacity)
    }
  }

  private func circle(radius: CGFloat) -> CGRect {
    CGRect(x: bounds.midX - radius, y: bounds.midY - radius, width: 2 * radius, height: 2 * radius)
  }

  private func drawHalo() {
    let radii = KeyboardVoicePanelPolicy.haloRadii(orb: orbSide, halo: haloSpread, level: shownLevel)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    outerHalo.path = UIBezierPath(ovalIn: circle(radius: radii.outer)).cgPath
    innerHalo.path = UIBezierPath(ovalIn: circle(radius: radii.inner)).cgPath
    if isShowingMascot { drawMascotArcs() }
    CATransaction.commit()
  }

  /// 只在圆球在屏幕上、电平还没追上、没开「减弱动态效果」时逐帧刷新；减弱动态效果时光晕固定在静止的两圈。
  private func updateDisplayLink() {
    let reduceMotion = UIAccessibility.isReduceMotionEnabled
    if reduceMotion && shownLevel != 0 {
      shownLevel = 0
      drawHalo()
    }
    let animates = window != nil && !reduceMotion
      && !KeyboardVoicePanelPolicy.settled(shown: shownLevel, target: targetLevel)
    if animates, displayLink == nil {
      let link = CADisplayLink(target: self, selector: #selector(advanceLevel(_:)))
      link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)
      link.add(to: .main, forMode: .common)
      displayLink = link
      lastFrame = nil
    } else if !animates, let link = displayLink {
      link.invalidate()
      displayLink = nil
    }
  }

  @objc private func advanceLevel(_ link: CADisplayLink) {
    // 第一帧没有上一帧可比，按这一帧的时长推进。
    let elapsed = lastFrame.map { link.timestamp - $0 } ?? (link.targetTimestamp - link.timestamp)
    lastFrame = link.timestamp
    shownLevel = KeyboardVoicePanelPolicy.step(shown: shownLevel, target: targetLevel, elapsedMs: elapsed * 1000)
    drawHalo()
    if KeyboardVoicePanelPolicy.settled(shown: shownLevel, target: targetLevel) { updateDisplayLink() }
  }

  @objc private func reduceMotionChanged() {
    updateDisplayLink()
  }

  private func applyColors() {
    let traits = traitCollection
    let accent = (isCancelling ? skin.secondary : skin.accent).resolvedColor(with: traits).cgColor
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    outerHalo.fillColor = accent
    innerHalo.fillColor = accent
    circleLayer.fillColor = accent
    micLayer.strokeColor = skin.onAccent.resolvedColor(with: traits).cgColor
    CATransaction.commit()
  }
}

/// 点按聆听面板上的水杉娘：随键盘打包的 `stickers/shuishan/01.webp`，与 Android 工具栏面板用的是同一张。按指示区最大时人物的边长下采样，在后台解码一次，进程内缓存，之后的面板直接用。
enum KeyboardVoiceMascot {
  static let file = "01.webp"
  /// 指示区最高是圆球上限的 1.5 倍，人物占视口高 250 里的 228。
  static var maximumSide: CGFloat {
    KeyboardVoicePanelPolicy.orbMax * (1 + 2 * KeyboardVoicePanelPolicy.haloRatio)
      * KeyboardVoicePanelPolicy.mascotCharacter.height / KeyboardVoicePanelPolicy.mascotViewport.height
  }

  private static var cached: (scale: CGFloat, image: UIImage)?
  private static let queue = DispatchQueue(label: "app.msime.keyboard.voice-mascot", qos: .userInitiated)

  /// 表情包里的那一张；清单读不到或没有这个文件时为空，面板就一直画圆球。
  static func sticker(in stickers: [KeyboardStickerCatalog.Sticker]) -> KeyboardStickerCatalog.Sticker? {
    stickers.first { $0.url.lastPathComponent == file }
  }

  /// 在主线程回调；读不到或解码失败时给 `nil`，不重试也不下载。
  static func load(scale: CGFloat, completion: @escaping (UIImage?) -> Void) {
    if let cached, cached.scale == scale {
      completion(cached.image)
      return
    }
    guard let sticker = sticker(in: KeyboardStickerCatalog.bundled) else {
      completion(nil)
      return
    }
    let side = maximumSide
    queue.async {
      let image = KeyboardStickerCatalog.thumbnail(sticker, side: side, scale: scale)
      DispatchQueue.main.async {
        if let image { cached = (scale, image) }
        completion(image)
      }
    }
  }
}
