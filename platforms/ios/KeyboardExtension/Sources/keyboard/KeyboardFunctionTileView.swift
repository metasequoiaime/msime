import UIKit

/// 功能面板（更多工具）里的一个图块，按设计稿的菜单项绘制：30pt 图标区在上、12.5pt 文字在下，相隔 7pt，在 52pt 的格子里居中，没有背景。
///
/// 这里刻意用普通的 UIControl。键盘的皮肤处理（`applyKeyboardSkin`）会把每个 configuration 带填充的 UIButton 重绘成键帽，KeyboardKeyButton 又是按按键样式绘制的，所以用这两者搭出来的图块都会变成实心色块。
///
/// 字形键面是一个 26pt 圆角框（圆角 5，1.8pt 当前色描边），框里是 14pt 粗体字；图标键面是 24pt 的 `KeyboardIcon` 线条图标。工具开启时当前色是皮肤强调色，否则是按键前景色。开启的工具还会把文字改为半粗体，并在图标区右下角（right -5，bottom -4）加一个 13pt 的强调色对勾角标，外圈是键盘背景色。
final class KeyboardFunctionTileView: UIControl {
  static let height: CGFloat = 52
  private static let iconArea: CGFloat = 30
  private static let iconSide: CGFloat = 24
  private static let glyphSide: CGFloat = 26
  private static let glyphRadius: CGFloat = 5
  private static let glyphBorder: CGFloat = 1.8
  private static let glyphFontSize: CGFloat = 14
  private static let labelFontSize: CGFloat = 12.5
  private static let labelGap: CGFloat = 7
  private static let badgeSide: CGFloat = 13
  private static let badgeRing: CGFloat = 2
  private static let badgeCheckSide: CGFloat = 9
  private static let badgeRight: CGFloat = -5
  private static let badgeBottom: CGFloat = -4
  private static let pressedAlpha: CGFloat = 0.55
  private static let disabledAlpha: CGFloat = 0.4

  private(set) var tool: KeyboardTool
  private var skin: KeyboardTheme
  private let iconLayer = CAShapeLayer()
  private let glyphBoxLayer = CAShapeLayer()
  private let glyphLabel = UILabel()
  private let badgeLayer = CAShapeLayer()
  private let badgeCheckLayer = CAShapeLayer()
  private let titleLabel = UILabel()
  private lazy var longPressRecognizer = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))

  init(tool: KeyboardTool, skin: KeyboardTheme) {
    self.tool = tool
    self.skin = skin
    super.init(frame: .zero)
    backgroundColor = .clear
    isAccessibilityElement = true
    for shape in [iconLayer, glyphBoxLayer, badgeCheckLayer] {
      shape.fillColor = nil
      shape.lineCap = .round
      shape.lineJoin = .round
    }
    for layer in [iconLayer, glyphBoxLayer, badgeLayer] { self.layer.addSublayer(layer) }
    badgeLayer.addSublayer(badgeCheckLayer)
    glyphLabel.font = .systemFont(ofSize: Self.glyphFontSize, weight: .bold)
    glyphLabel.textAlignment = .center
    titleLabel.textAlignment = .center
    titleLabel.numberOfLines = 1
    titleLabel.lineBreakMode = .byTruncatingTail
    for label in [glyphLabel, titleLabel] {
      label.isUserInteractionEnabled = false
      addSubview(label)
    }
    // UIControl 只上报触摸事件；把一次完整的点按转成主操作，这样点按和 `sendActions(for: .primaryActionTriggered)` 执行的是同一个工具。
    addTarget(self, action: #selector(handleTap), for: .touchUpInside)
    addAction(UIAction { [weak self] _ in self?.tool.run() }, for: .primaryActionTriggered)
    addGestureRecognizer(longPressRecognizer)
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (tile: KeyboardFunctionTileView, _: UITraitCollection) in
      tile.applyColors()
    }
    configure(tool, skin: skin)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 按 `skin` 为 `tool` 重绘图块（键面、状态和操作），视图本身保留，面板更新时不会挪动它。
  func configure(_ tool: KeyboardTool, skin: KeyboardTheme) {
    self.tool = tool
    self.skin = skin
    titleLabel.text = tool.title
    titleLabel.font = .systemFont(ofSize: Self.labelFontSize, weight: tool.selected ? .semibold : .regular)
    switch tool.face {
    case .glyph(let glyph):
      glyphLabel.text = glyph
      glyphLabel.isHidden = false
      glyphBoxLayer.isHidden = false
      iconLayer.isHidden = true
    case .icon:
      glyphLabel.isHidden = true
      glyphBoxLayer.isHidden = true
      iconLayer.isHidden = false
    }
    badgeLayer.isHidden = !tool.selected
    isSelected = tool.selected
    isEnabled = tool.enabled
    longPressRecognizer.isEnabled = tool.longPress != nil
    accessibilityIdentifier = "moreCard-" + tool.title
    accessibilityLabel = tool.accessibilityLabel ?? tool.title
    var traits: UIAccessibilityTraits = .button
    if tool.selected { traits.insert(.selected) }
    if !tool.enabled { traits.insert(.notEnabled) }
    accessibilityTraits = traits
    if !tool.enabled {
      accessibilityValue = "不可用"
    } else if tool.isToggle {
      accessibilityValue = tool.selected ? "已开启" : "已关闭"
    } else {
      accessibilityValue = tool.caption
    }
    applyColors()
    updateAlpha()
    setNeedsLayout()
  }

  override var isHighlighted: Bool {
    didSet { updateAlpha() }
  }

  override var isEnabled: Bool {
    didSet { updateAlpha() }
  }

  override var intrinsicContentSize: CGSize {
    CGSize(width: UIView.noIntrinsicMetric, height: Self.height)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let font = titleLabel.font ?? .systemFont(ofSize: Self.labelFontSize)
    let labelHeight = ceil(font.lineHeight)
    let total = Self.iconArea + Self.labelGap + labelHeight
    let top = max(0, ((bounds.height - total) / 2).rounded(.down))
    let area = CGRect(x: bounds.midX - Self.iconArea / 2, y: top, width: Self.iconArea, height: Self.iconArea)
    titleLabel.frame = CGRect(x: 0, y: area.maxY + Self.labelGap, width: bounds.width, height: labelHeight)
    let glyphBox = CGRect(x: area.midX - Self.glyphSide / 2, y: area.midY - Self.glyphSide / 2, width: Self.glyphSide, height: Self.glyphSide)
    glyphLabel.frame = glyphBox
    let badge = CGRect(
      x: area.maxX - Self.badgeRight - Self.badgeSide, y: area.maxY - Self.badgeBottom - Self.badgeSide,
      width: Self.badgeSide, height: Self.badgeSide)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    // 描边画在 26pt 框的内侧，与设计稿 border-box 的尺寸算法一致。
    let inset = Self.glyphBorder / 2
    glyphBoxLayer.lineWidth = Self.glyphBorder
    glyphBoxLayer.path = UIBezierPath(roundedRect: glyphBox.insetBy(dx: inset, dy: inset), cornerRadius: Self.glyphRadius - inset).cgPath
    if case .icon(let icon) = tool.face {
      let iconRect = CGRect(x: area.midX - Self.iconSide / 2, y: area.midY - Self.iconSide / 2, width: Self.iconSide, height: Self.iconSide)
      iconLayer.path = icon.path(in: iconRect).cgPath
      iconLayer.lineWidth = icon.isFilled ? 0 : icon.lineWidth * Self.iconSide / KeyboardIcon.viewBox
    }
    badgeLayer.frame = badge
    // 外圈是一条 2pt 描边，骑在半径比 13pt 圆盘大 1pt 的圆上，这样强调色保持完整的 13pt，外圈落在它外面，与设计稿的 box-shadow 一样。
    let ringRadius = Self.badgeSide / 2 + Self.badgeRing / 2
    badgeLayer.lineWidth = Self.badgeRing
    badgeLayer.path = UIBezierPath(
      arcCenter: CGPoint(x: Self.badgeSide / 2, y: Self.badgeSide / 2), radius: ringRadius,
      startAngle: 0, endAngle: .pi * 2, clockwise: true
    ).cgPath
    let checkOrigin = (Self.badgeSide - Self.badgeCheckSide) / 2
    let checkRect = CGRect(x: checkOrigin, y: checkOrigin, width: Self.badgeCheckSide, height: Self.badgeCheckSide)
    badgeCheckLayer.frame = badgeLayer.bounds
    badgeCheckLayer.path = KeyboardIcon.check.path(in: checkRect).cgPath
    badgeCheckLayer.lineWidth = KeyboardIcon.check.lineWidth * Self.badgeCheckSide / KeyboardIcon.viewBox
    CATransaction.commit()
  }

  /// 按当前 trait 解析皮肤颜色：图层用的是 CGColor，不会自己跟随浅色和深色模式。
  private func applyColors() {
    let color = tool.selected ? skin.toggleForeground : skin.keyForeground
    let resolved = color.resolvedColor(with: traitCollection).cgColor
    let background = skin.background.resolvedColor(with: traitCollection).cgColor
    let accent = skin.toggleForeground.resolvedColor(with: traitCollection).cgColor
    titleLabel.textColor = color
    glyphLabel.textColor = color
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    glyphBoxLayer.strokeColor = resolved
    if case .icon(let icon) = tool.face, icon.isFilled {
      iconLayer.fillColor = resolved
      iconLayer.strokeColor = nil
    } else {
      iconLayer.fillColor = nil
      iconLayer.strokeColor = resolved
    }
    badgeLayer.fillColor = accent
    badgeLayer.strokeColor = background
    badgeCheckLayer.strokeColor = background
    CATransaction.commit()
  }

  private func updateAlpha() {
    alpha = !isEnabled ? Self.disabledAlpha : isHighlighted ? Self.pressedAlpha : 1
  }

  @objc private func handleTap() {
    sendActions(for: .primaryActionTriggered)
  }

  @objc private func handleLongPress(_ recognizer: UILongPressGestureRecognizer) {
    guard recognizer.state == .began else { return }
    isHighlighted = false
    tool.longPress?()
  }
}
