import UIKit

/// 键盘功能菜单里的语音结果面板（dc.html `showVoice`）：一个 72pt 的强调色圆球，里面是 30pt 的麦克风，外面一圈脉冲环，下面是 15pt 的状态行和 12.5pt 的提示「点任意处取消」，居中排列，间距 14pt。
///
/// 它有两种用法。按住说话时（`init(dictationHeld:skin:)`）是聆听界面，照 Android 的 `VoiceListeningView`：状态行「正在聆听…」，提示行先写怎么结束，开始识别后换成实时文字（太长时从开头截断，最新的字留在眼前）；上滑到取消区时状态行变成「松开取消」，圆球换成次要色。录音和识别都在后台的主 App 里，见 `KeyboardVoiceDictation`。另一种是语音交接的入口（`init(entry:skin:)`）：没有待取的结果时状态行显示「轻点开启语音」，点圆球去 App 开启语音会话；有待取的结果时显示「已识别：<text>」，点圆球把它上屏。点其他任何地方都取消。脉冲环照 Android 的 `VoiceListeningView`：在 1.2s 内从圆球向外扩 18pt，同时从 35% 淡出，颜色用皮肤的强调色而不是原型里固定的绿色，开了「减弱动态效果」时保持静止。
final class KeyboardVoicePanelView: UIControl {
  static let orbSide: CGFloat = 72
  static let micSide: CGFloat = 30
  static let micLineWidth: CGFloat = 1.8
  static let pulseSpread: CGFloat = 18
  static let pulseOpacity: Float = 0.35
  static let pulseDuration: CFTimeInterval = 1.2
  static let spacing: CGFloat = 14
  static let cancelCaption = "点任意处取消"
  static let listeningTitle = "正在聆听…"
  static let recognizingTitle = "正在识别…"
  static let cancellingTitle = "松开取消"
  /// 按住空格开始的听写：松开结束，上滑取消（Android 的 `ImeVoiceEntry` 同一句）。
  static let heldCaption = "松开结束，上滑取消"
  /// 读屏操作开始的听写：再点圆球结束，点其他地方取消。
  static let tappedCaption = "点圆球结束，点其他地方取消"

  /// 状态行：待取的结果，没有结果时说明圆球的作用。
  static func status(for entry: VoiceTextHandoff?) -> String {
    entry.map { "已识别：\($0.text)" } ?? "轻点开启语音"
  }

  /// 圆球的作用，作为 VoiceOver 读出的名称。
  static func orbLabel(for entry: VoiceTextHandoff?) -> String {
    entry == nil ? "去水杉 App 开启语音" : "插入语音结果"
  }

  let orb: KeyboardVoiceOrbButton
  let statusLabel = UILabel()
  let captionLabel = UILabel()
  var onOrb: (() -> Void)?
  var onCancel: (() -> Void)?

  /// 听写的提示行：没有文字时说明怎么结束。
  private var idleCaption = cancelCaption
  /// 是否在上滑取消区里。
  private(set) var isCancelling = false

  /// 听写中的聆听界面。`held` 为真时手指还按在空格上，松开和上滑由空格的手势处理；否则点圆球结束。两种情况下点面板其他地方都取消。
  convenience init(dictationHeld held: Bool, skin: KeyboardTheme) {
    self.init(entry: nil, skin: skin)
    accessibilityIdentifier = "keyboardVoiceListening"
    idleCaption = held ? Self.heldCaption : Self.tappedCaption
    orb.accessibilityLabel = "结束语音输入"
    showListening("")
  }

  /// 听写中的实时文字。
  func showListening(_ text: String) {
    guard !isCancelling else { return }
    statusLabel.text = Self.listeningTitle
    showCaption(text)
  }

  /// 松开以后在识别或润色。
  func showRecognizing(text: String, message: String) {
    isCancelling = false
    orb.isCancelling = false
    statusLabel.text = message.isEmpty ? Self.recognizingTitle : message
    showCaption(text)
  }

  /// 手指上滑进出取消区。
  func setCancelling(_ cancelling: Bool) {
    guard cancelling != isCancelling else { return }
    isCancelling = cancelling
    orb.isCancelling = cancelling
    statusLabel.text = cancelling ? Self.cancellingTitle : Self.listeningTitle
  }

  private func showCaption(_ text: String) {
    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    captionLabel.text = trimmed.isEmpty ? idleCaption : trimmed
    captionLabel.lineBreakMode = trimmed.isEmpty ? .byTruncatingTail : .byTruncatingHead
    captionLabel.accessibilityLabel = captionLabel.text
  }

  init(entry: VoiceTextHandoff?, skin: KeyboardTheme) {
    orb = KeyboardVoiceOrbButton(skin: skin)
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardVoicePanel"
    backgroundColor = .clear
    orb.accessibilityIdentifier = "keyboardVoiceOrb"
    orb.accessibilityLabel = Self.orbLabel(for: entry)
    orb.addAction(UIAction { [weak self] _ in self?.onOrb?() }, for: .primaryActionTriggered)
    statusLabel.accessibilityIdentifier = "keyboardVoicePanelStatus"
    statusLabel.text = Self.status(for: entry)
    statusLabel.font = .systemFont(ofSize: 15)
    statusLabel.textAlignment = .center
    statusLabel.numberOfLines = 2
    statusLabel.lineBreakMode = .byTruncatingTail
    captionLabel.text = Self.cancelCaption
    captionLabel.font = .systemFont(ofSize: 12.5)
    captionLabel.textAlignment = .center
    captionLabel.numberOfLines = 2
    let column = UIStackView(arrangedSubviews: [orb, statusLabel, captionLabel])
    column.axis = .vertical
    column.alignment = .center
    column.spacing = Self.spacing
    column.translatesAutoresizingMaskIntoConstraints = false
    addSubview(column)
    NSLayoutConstraint.activate([
      column.centerXAnchor.constraint(equalTo: centerXAnchor),
      column.centerYAnchor.constraint(equalTo: centerYAnchor),
      column.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 16),
      column.topAnchor.constraint(greaterThanOrEqualTo: topAnchor),
      statusLabel.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, constant: -32),
      captionLabel.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, constant: -32),
      orb.widthAnchor.constraint(equalToConstant: Self.orbSide),
      orb.heightAnchor.constraint(equalToConstant: Self.orbSide),
    ])
    // 落在圆球以外任何地方的点按都取消，包括文字和间隙。
    addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .touchUpInside)
    accessibilityElements = [orb, statusLabel, captionLabel]
    apply(skin: skin)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 落在这一列的间隙和文字上的点按归面板处理，所以同样取消；只有圆球自己处理点按。
  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    guard let hit = super.hitTest(point, with: event) else { return nil }
    return hit.isDescendant(of: orb) ? hit : self
  }

  func apply(skin: KeyboardTheme) {
    statusLabel.textColor = skin.keyForeground
    captionLabel.textColor = skin.secondary
    orb.apply(skin: skin)
  }

  /// VoiceOver 的退出手势和点圆球旁边一样取消。
  override func accessibilityPerformEscape() -> Bool {
    onCancel?()
    return true
  }
}

/// 语音面板的圆球：强调色的圆，麦克风用在它上面看得清的颜色，后面是脉冲环。它没有 configuration，所以键盘套皮肤时不会碰它。
final class KeyboardVoiceOrbButton: UIButton {
  private let pulseLayer = CAShapeLayer()
  private let circleLayer = CAShapeLayer()
  private let micLayer = CAShapeLayer()
  private var skin: KeyboardTheme
  private static let pulseKey = "voicePulse"

  init(skin: KeyboardTheme) {
    self.skin = skin
    super.init(frame: .zero)
    backgroundColor = .clear
    accessibilityTraits = .button
    for shape in [pulseLayer, circleLayer, micLayer] { layer.addSublayer(shape) }
    pulseLayer.opacity = 0
    micLayer.fillColor = nil
    micLayer.lineCap = .round
    micLayer.lineJoin = .round
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (orb: KeyboardVoiceOrbButton, _: UITraitCollection) in
      orb.applyColors()
    }
    applyColors()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 脉冲环是否在运行。
  var isPulsing: Bool { pulseLayer.animation(forKey: Self.pulseKey) != nil }

  /// 上滑到取消区时圆球换成次要色，和 Android 一样。
  var isCancelling = false {
    didSet { if isCancelling != oldValue { applyColors() } }
  }

  func apply(skin: KeyboardTheme) {
    self.skin = skin
    applyColors()
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let side = min(bounds.width, bounds.height)
    let circle = CGRect(x: bounds.midX - side / 2, y: bounds.midY - side / 2, width: side, height: side)
    let mic = KeyboardVoicePanelView.micSide
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    for shape in [pulseLayer, circleLayer] {
      shape.frame = bounds
      shape.path = UIBezierPath(ovalIn: circle).cgPath
    }
    micLayer.frame = bounds
    micLayer.path = KeyboardIcon.mic.path(in: CGRect(x: bounds.midX - mic / 2, y: bounds.midY - mic / 2, width: mic, height: mic)).cgPath
    micLayer.lineWidth = KeyboardVoicePanelView.micLineWidth * mic / KeyboardIcon.viewBox
    CATransaction.commit()
    updatePulse()
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    updatePulse()
  }

  /// 圆球在屏幕上且已完成布局时运行脉冲环，开了「减弱动态效果」时除外。
  private func updatePulse() {
    guard window != nil, bounds.width > 0, !UIAccessibility.isReduceMotionEnabled else {
      pulseLayer.removeAnimation(forKey: Self.pulseKey)
      return
    }
    guard !isPulsing else { return }
    let side = min(bounds.width, bounds.height)
    let scale = CABasicAnimation(keyPath: "transform.scale")
    scale.fromValue = 1
    scale.toValue = (side + 2 * KeyboardVoicePanelView.pulseSpread) / side
    let fade = CABasicAnimation(keyPath: "opacity")
    fade.fromValue = KeyboardVoicePanelView.pulseOpacity
    fade.toValue = 0
    let group = CAAnimationGroup()
    group.animations = [scale, fade]
    group.duration = KeyboardVoicePanelView.pulseDuration
    group.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
    group.repeatCount = .infinity
    pulseLayer.add(group, forKey: Self.pulseKey)
  }

  private func applyColors() {
    let traits = traitCollection
    let accent = (isCancelling ? skin.secondary : skin.accent).resolvedColor(with: traits).cgColor
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    pulseLayer.fillColor = accent
    circleLayer.fillColor = accent
    micLayer.strokeColor = skin.onAccent.resolvedColor(with: traits).cgColor
    CATransaction.commit()
  }
}
