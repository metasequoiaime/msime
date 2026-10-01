import UIKit

final class KeyboardSchemePickerView: UIView {
  private var glyphBorders: [(UILabel, Bool)] = []
  private let skin = KeyboardTheme.current
  private var accent: UIColor { skin.accent }

  init(selected: ChineseInputScheme, isChineseMode: Bool = true,
       onSelect: @escaping (ChineseInputScheme) -> Void,
       onSelectEnglish: (() -> Void)? = nil,
       onSettings: (() -> Void)? = nil,
       onClose: @escaping () -> Void) {
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardSchemePicker"
    backgroundColor = skin.background

    let close = UIButton(type: .system)
    close.setImage(UIImage(systemName: "chevron.left"), for: .normal)
    close.tintColor = accent
    close.accessibilityLabel = "返回键盘"
    close.accessibilityIdentifier = "closeSchemePicker"
    close.addAction(UIAction { _ in onClose() }, for: .primaryActionTriggered)

    let settings = UIButton(type: .system)
    settings.setImage(UIImage(systemName: "gearshape"), for: .normal)
    settings.tintColor = accent
    settings.accessibilityLabel = "键盘设置"
    settings.accessibilityIdentifier = "schemePickerSettings"
    settings.isEnabled = onSettings != nil
    settings.addAction(UIAction { _ in onSettings?() }, for: .primaryActionTriggered)

    let content = UIView()
    let scroll = UIScrollView()
    scroll.accessibilityIdentifier = "schemePickerScroll"
    scroll.alwaysBounceVertical = false
    scroll.delaysContentTouches = false
    scroll.disableEdgeEffects()
    let panel = UIStackView()
    panel.axis = .vertical
    panel.spacing = 4
    // 行平分整块面板的高度。The rows were pinned to 62pt while the picker fills the whole keyboard, so everything below the last row was bare backdrop -- a dead block under the cards.
    panel.distribution = .fillEqually
    panel.backgroundColor = skin.keyBackground
    panel.layer.cornerRadius = 18
    panel.isLayoutMarginsRelativeArrangement = true
    panel.layoutMargins = UIEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)

    // Cantonese or Zhuyin without its dictionary is left off: the Engine would run another scheme under its card.
    var cards: [UIView] = InputSchemePreference.offeredSchemes.map { scheme in
      let glyph: String
      let badge: String
      // 卡片全部走皮肤的强调色。此前每一族配一个系统色 —— 双拼蓝、五笔棕、日语粉、手写青、回复橙、英文靛 —— 理由是十二张卡片同色的话分辨要靠读字。
      //
      // 但那些是系统色,不跟皮肤走:换成深色或任何一套自定义皮肤,它们和背景、按键、候选栏就不是一套配色了,那时它们不是在分组,只是六个闯进来的颜色。而字形本来就两两不同(拼26/拼9/鹤双/自双/微双/S双/五86/あ26/あ9/한26/粤26/注大千/越26/写手/聊AI/EN26),要读的那一眼无论如何都得读 —— 颜色没有给出图例,也就没有省掉这一眼。
      switch scheme {
      case .quanpin: glyph = "拼"; badge = "26"
      case .nineKey: glyph = "拼"; badge = "9"
      case .shuangpin: glyph = "鹤"; badge = "双"
      case .ziranma: glyph = "自"; badge = "双"
      case .microsoft: glyph = "微"; badge = "双"
      case .shoudao: glyph = "S"; badge = "双"
      case .wubi: glyph = "五"; badge = "86"
      case .japanese: glyph = "あ"; badge = "26"
      case .japaneseNineKey: glyph = "あ"; badge = "9"
      case .korean: glyph = "한"; badge = "26"
      case .cantonese: glyph = "粤"; badge = "26"
      case .zhuyin: glyph = "注"; badge = "大千"
      case .vietnamese: glyph = "越"; badge = "26"
      case .handwriting: glyph = "写"; badge = "手"
      case .thoughtfulReply: glyph = "聊"; badge = "AI"
      }
      return makeCard(title: scheme.title, glyph: glyph, badge: badge,
        selected: isChineseMode && scheme == selected, identifier: "schemeCard-\(scheme.rawValue)") { onSelect(scheme) }
    }
    if let onSelectEnglish {
      // English is a platform text mode, not a second persisted Engine scheme.
      cards.insert(makeCard(title: "英文 26 键", glyph: "EN", badge: "26",
        selected: !isChineseMode, identifier: "schemeEnglishCard", action: onSelectEnglish), at: min(2, cards.count))
    }
    for start in stride(from: 0, to: cards.count, by: 4) {
      let row = UIStackView()
      row.spacing = 4
      row.distribution = .fillEqually
      for card in cards[start..<min(start + 4, cards.count)] { row.addArrangedSubview(card) }
      while row.arrangedSubviews.count < 4 { row.addArrangedSubview(UIView()) }
      panel.addArrangedSubview(row)
      row.heightAnchor.constraint(greaterThanOrEqualToConstant: 62).isActive = true
    }

    for child in [close, settings, content] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    scroll.translatesAutoresizingMaskIntoConstraints = false
    panel.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(scroll)
    scroll.addSubview(panel)
    NSLayoutConstraint.activate([
      close.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      close.topAnchor.constraint(equalTo: topAnchor),
      close.widthAnchor.constraint(equalToConstant: 44), close.heightAnchor.constraint(equalToConstant: 44),
      settings.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      settings.topAnchor.constraint(equalTo: topAnchor),
      settings.widthAnchor.constraint(equalToConstant: 44), settings.heightAnchor.constraint(equalToConstant: 44),
      content.topAnchor.constraint(equalTo: close.bottomAnchor),
      content.leadingAnchor.constraint(equalTo: leadingAnchor), content.trailingAnchor.constraint(equalTo: trailingAnchor),
      content.bottomAnchor.constraint(equalTo: bottomAnchor),
      scroll.topAnchor.constraint(equalTo: content.topAnchor), scroll.bottomAnchor.constraint(equalTo: content.bottomAnchor),
      scroll.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 14),
      scroll.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -14),
      panel.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
      panel.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -10),
      panel.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
      panel.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
      panel.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
    ])
    // 有富余就撑满,内容比键盘高就照常滚动。
    let fill = panel.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor, constant: -10)
    fill.priority = .defaultHigh
    fill.isActive = true
  }

  private func makeCard(title: String, glyph: String, badge: String, selected: Bool,
                        identifier: String, action: @escaping () -> Void) -> UIButton {
    let card = KeyboardKeyButton()
    card.accessibilityIdentifier = identifier
    card.accessibilityLabel = title
    card.accessibilityValue = selected ? "已选中" : ""
    if selected { card.accessibilityTraits.insert(.selected) }
    card.layer.cornerRadius = 13
    card.backgroundColor = selected ? accent.withAlphaComponent(0.12) : .clear
    // 选中与未选中的差别落在底色和这一档透明度上,不再落在色相上。
    let color: UIColor = selected ? accent : accent.withAlphaComponent(0.78)
    let symbol = UILabel()
    symbol.text = glyph
    symbol.textAlignment = .center
    symbol.font = .systemFont(ofSize: glyph.count > 1 ? 15 : 20, weight: .semibold)
    symbol.textColor = color
    symbol.layer.cornerRadius = 4
    symbol.layer.borderWidth = 1.7
    symbol.layer.borderColor = color.resolvedColor(with: traitCollection).cgColor
    glyphBorders.append((symbol, selected))
    let suffix = UILabel()
    suffix.text = badge
    suffix.font = .systemFont(ofSize: 9, weight: .bold)
    suffix.textAlignment = .center
    suffix.textColor = color
    suffix.backgroundColor = skin.keyBackground
    let label = UILabel()
    label.text = title
    label.textAlignment = .center
    label.numberOfLines = 1
    label.font = .systemFont(ofSize: 12)
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.65
    label.textColor = color
    let check = UIImageView(image: UIImage(systemName: "checkmark.circle.fill"))
    check.tintColor = accent
    check.backgroundColor = skin.keyBackground
    check.layer.cornerRadius = 6
    check.isHidden = !selected
    for child in [symbol, suffix, label, check] {
      child.translatesAutoresizingMaskIntoConstraints = false
      child.isUserInteractionEnabled = false
      child.isAccessibilityElement = false
      card.addSubview(child)
    }
    NSLayoutConstraint.activate([
      symbol.topAnchor.constraint(equalTo: card.topAnchor, constant: 8), symbol.centerXAnchor.constraint(equalTo: card.centerXAnchor),
      symbol.widthAnchor.constraint(equalToConstant: 27), symbol.heightAnchor.constraint(equalToConstant: 27),
      suffix.trailingAnchor.constraint(equalTo: symbol.trailingAnchor, constant: 4), suffix.bottomAnchor.constraint(equalTo: symbol.bottomAnchor, constant: 3),
      suffix.widthAnchor.constraint(equalToConstant: 16), suffix.heightAnchor.constraint(equalToConstant: 12),
      label.topAnchor.constraint(equalTo: symbol.bottomAnchor, constant: 6),
      label.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 2), label.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -2),
      label.bottomAnchor.constraint(lessThanOrEqualTo: card.bottomAnchor, constant: -4),
      check.leadingAnchor.constraint(equalTo: symbol.trailingAnchor, constant: 1), check.topAnchor.constraint(equalTo: symbol.topAnchor, constant: -3),
      check.widthAnchor.constraint(equalToConstant: 12), check.heightAnchor.constraint(equalToConstant: 12),
    ])
    card.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    return card
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
    super.traitCollectionDidChange(previousTraitCollection)
    for (label, selected) in glyphBorders {
      label.layer.borderColor = (selected ? accent : skin.keyForeground).resolvedColor(with: traitCollection).cgColor
    }
  }
}
