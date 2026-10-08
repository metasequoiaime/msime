import UIKit

/// 单手模式的几何与规则（dc.html `ohDir`，占 15% 的侧栏），键盘和它的测试共用。
///
/// 按键区变成一行：侧栏占整行的 15%，再留 6pt 间距，剩下的给按键，侧栏放在按键的另一侧。只对手机键盘生效，设计稿把它限定在 `mob && !pad`；全宽的 iPad 键盘（也只有它会分离）保留存下的值，但画的时候不带侧栏，和 Android 的 `SplitKeyboardPolicy.effectiveOneHanded` 在画分离式键盘时的做法一致。
enum KeyboardOneHandLayout {
  /// 侧栏占按键行宽度的比例。
  static let gutterRatio: CGFloat = 0.15
  /// 侧栏和按键之间的间距。
  static let gap: CGFloat = 6

  /// 键盘实际画出的模式：手机键盘用存下的值，全宽的 iPad 键盘一律关闭。
  static func effective(_ stored: KeyboardOneHandedMode, formFactor: KeyboardFormFactor) -> KeyboardOneHandedMode {
    formFactor == .phone ? stored : .off
  }

  /// 按键行宽 `available` 时按键分到的宽度。
  static func keysWidth(available: CGFloat, mode: KeyboardOneHandedMode) -> CGFloat {
    let width = max(0, available)
    guard mode != .off else { return width }
    return max(0, width - width * gutterRatio - gap)
  }

  /// 侧栏是否排在行首，也就是在左边：按键靠右时如此。
  static func gutterLeads(_ mode: KeyboardOneHandedMode) -> Bool { mode == .right }
}

/// 单手模式的侧栏（dc.html 的 `oneHand` 栏）：「换到另一侧」是一个 40pt 的按键色圆，带按键阴影，里面是指向另一侧的 20pt 箭头；它下方隔 14pt 是「退出单手」，一个 40pt 的无底按钮，画次要颜色的最大化轮廓。两者按下时都缩放到 .9。
///
/// 按钮只上报按下，模式变成什么由键盘决定，和 Android 的 `OneHandGutterView` 交给 `toggleOneHanded` 处理一样。
final class KeyboardOneHandGutterView: UIView {
  static let buttonSide: CGFloat = 40
  static let buttonGap: CGFloat = 14

  let swapButton = KeyboardOneHandGutterButton(icon: .oneHandSwapLeft, round: true)
  let exitButton = KeyboardOneHandGutterButton(icon: .oneHandExit, round: false)
  var onSwap: (() -> Void)?
  var onExit: (() -> Void)?

  override init(frame: CGRect) {
    super.init(frame: frame)
    accessibilityIdentifier = "oneHandGutter"
    swapButton.accessibilityLabel = "单手键盘换到另一侧"
    swapButton.accessibilityIdentifier = "oneHandSwap"
    exitButton.accessibilityLabel = "退出单手模式"
    exitButton.accessibilityIdentifier = "oneHandExit"
    swapButton.addAction(UIAction { [weak self] _ in self?.onSwap?() }, for: .primaryActionTriggered)
    exitButton.addAction(UIAction { [weak self] _ in self?.onExit?() }, for: .primaryActionTriggered)
    let column = UIStackView(arrangedSubviews: [swapButton, exitButton])
    column.axis = .vertical
    column.alignment = .center
    column.spacing = Self.buttonGap
    column.translatesAutoresizingMaskIntoConstraints = false
    addSubview(column)
    NSLayoutConstraint.activate([
      column.centerXAnchor.constraint(equalTo: centerXAnchor),
      column.centerYAnchor.constraint(equalTo: centerYAnchor),
      column.topAnchor.constraint(greaterThanOrEqualTo: topAnchor),
      column.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor),
      swapButton.widthAnchor.constraint(equalToConstant: Self.buttonSide),
      swapButton.heightAnchor.constraint(equalToConstant: Self.buttonSide),
      exitButton.widthAnchor.constraint(equalToConstant: Self.buttonSide),
      exitButton.heightAnchor.constraint(equalToConstant: Self.buttonSide),
    ])
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 让换边箭头指向按键将要移去的一侧：按键在右时指向左，在左时指向右。
  func setKeysOnRight(_ onRight: Bool) {
    swapButton.icon = onRight ? .oneHandSwapLeft : .oneHandSwapRight
  }

  /// 按 `skin` 重画：换边圆用按键填充色和按键前景色，退出轮廓用次要颜色。
  func apply(skin: KeyboardTheme) {
    swapButton.apply(skin: skin)
    exitButton.apply(skin: skin)
  }
}

/// 单手模式侧栏里的一个按钮。它没有 configuration，键盘套皮肤时只给 configuration 带填充色的 UIButton 画键帽，所以不会碰它；圆、阴影和图标都由它自己画。
final class KeyboardOneHandGutterButton: UIButton {
  static let iconSide: CGFloat = 20
  /// 设计稿在 24 单位网格上用 1.9 的线宽描这两个图标。
  static let iconLineWidth: CGFloat = 1.9
  static let pressScale: CGFloat = 0.9
  static let pressDuration: TimeInterval = 0.1

  var icon: KeyboardIcon {
    didSet { if icon != oldValue { setNeedsLayout() } }
  }
  /// 按钮是画带按键阴影的按键色圆（换到另一侧），还是图标后面什么都不画（退出单手）。
  let round: Bool
  private let circleLayer = CAShapeLayer()
  private let iconLayer = CAShapeLayer()
  private var skin = KeyboardTheme.current

  init(icon: KeyboardIcon, round: Bool) {
    self.icon = icon
    self.round = round
    super.init(frame: .zero)
    backgroundColor = .clear
    accessibilityTraits = .button
    layer.addSublayer(circleLayer)
    iconLayer.fillColor = nil
    iconLayer.lineCap = .round
    iconLayer.lineJoin = .round
    layer.addSublayer(iconLayer)
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (button: KeyboardOneHandGutterButton, _: UITraitCollection) in
      button.applyColors()
    }
    applyColors()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  func apply(skin: KeyboardTheme) {
    self.skin = skin
    applyColors()
  }

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      KeyPressMotion.animate(self, scale: isHighlighted ? Self.pressScale : 1, duration: Self.pressDuration)
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil {
      layer.removeAllAnimations()
      transform = .identity
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let side = min(bounds.width, bounds.height)
    let circle = CGRect(x: bounds.midX - side / 2, y: bounds.midY - side / 2, width: side, height: side)
    let iconRect = CGRect(x: bounds.midX - Self.iconSide / 2, y: bounds.midY - Self.iconSide / 2,
                          width: Self.iconSide, height: Self.iconSide)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    circleLayer.frame = bounds
    circleLayer.path = round ? UIBezierPath(ovalIn: circle).cgPath : nil
    circleLayer.shadowPath = circleLayer.path
    iconLayer.frame = bounds
    iconLayer.path = icon.path(in: iconRect).cgPath
    iconLayer.lineWidth = Self.iconLineWidth * Self.iconSide / KeyboardIcon.viewBox
    CATransaction.commit()
  }

  /// 圆用按键填充色，并且和 `decorateKey` 一样带按键阴影，除非键盘设计自己画表面；换边箭头用按键前景色，退出轮廓用次要颜色，与设计稿的配色一致。
  private func applyColors() {
    let traits = traitCollection
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    circleLayer.fillColor = round ? skin.keyBackground.resolvedColor(with: traits).cgColor : nil
    let shadow = round && skin.design == nil && skin.hasShadow
    circleLayer.shadowColor = skin.shadowColor.resolvedColor(with: traits).cgColor
    circleLayer.shadowOpacity = shadow ? 1 : 0
    circleLayer.shadowRadius = skin.shadowRadius
    circleLayer.shadowOffset = CGSize(width: 0, height: skin.shadowOffset)
    iconLayer.strokeColor = (round ? skin.keyForeground : skin.secondary).resolvedColor(with: traits).cgColor
    CATransaction.commit()
  }
}
