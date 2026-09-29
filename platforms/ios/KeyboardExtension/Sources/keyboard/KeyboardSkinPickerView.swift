import UIKit

/// The keyboard's theme picker: the global themes of the catalog (the same ids the app and the desktop list), then the saved keyboard designs, which apply to the custom theme.
final class KeyboardSkinPickerView: UIView {
  /// `document` is the shared document the cards resolve against, so the custom card shows the custom theme as configured.
  init(selected: String, document: [String: Any]? = MetasequoiaInputSessionBridge.loadSharedPreferences(), showsHeader: Bool = true,
       onSelect: @escaping (String) -> Void, onSelectDesign: @escaping (CustomKeyboardSkin) -> Void, onClose: @escaping () -> Void) {
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardSkinPicker"
    backgroundColor = .secondarySystemBackground
    let header = UILabel()
    header.text = "选择皮肤"
    header.isHidden = !showsHeader
    header.font = .systemFont(ofSize: 17, weight: .semibold)
    let close = UIButton(type: .system)
    close.setTitle("完成", for: .normal)
    close.isHidden = !showsHeader
    close.accessibilityIdentifier = "closeSkinPicker"
    close.addAction(UIAction { _ in onClose() }, for: .primaryActionTriggered)
    let scroll = UIScrollView()
    scroll.disableEdgeEffects()
    let rows = UIStackView()
    rows.axis = .vertical
    rows.spacing = 10
    let saved = CustomSkinLibrary.designs
    let appliedDesign = GlobalThemePreference.design(in: document) ?? (document == nil ? CustomKeyboardSkinStore.stored : nil)
    if !saved.isEmpty {
      let label = UILabel()
      label.text = "我的设计"
      label.font = .systemFont(ofSize: 13, weight: .semibold)
      label.textColor = .secondaryLabel
      rows.addArrangedSubview(label)
      for index in stride(from: 0, to: saved.count, by: 2) {
        let row = UIStackView()
        row.spacing = 10; row.distribution = .fillEqually
        for item in saved[index..<min(index + 2, saved.count)] {
          let card = KeyboardKeyButton()
          var config = UIButton.Configuration.filled()
          config.title = item.name
          config.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 8, bottom: 74, trailing: 8)
          config.titleLineBreakMode = .byTruncatingTail
          config.baseForegroundColor = CustomKeyboardSkin.color(item.design.keyForeground)
          config.baseBackgroundColor = CustomKeyboardSkin.color(item.design.keyBackground)
          config.background.cornerRadius = item.design.cornerRadius
          config.background.strokeWidth = max(1, item.design.borderWidth)
          config.background.strokeColor = CustomKeyboardSkin.color(item.design.customBorderColor ?? item.design.accent)
          card.configuration = config
          card.heightAnchor.constraint(equalToConstant: 104).isActive = true
          let miniature = KeyboardSkinMiniature(skin: .designed(item.design))
          miniature.isUserInteractionEnabled = false
          miniature.translatesAutoresizingMaskIntoConstraints = false
          card.addSubview(miniature)
          NSLayoutConstraint.activate([
            miniature.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 6),
            miniature.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -6),
            miniature.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -6),
            miniature.heightAnchor.constraint(equalToConstant: 64)
          ])
          card.accessibilityIdentifier = "savedSkinCard-" + item.id.uuidString
          card.accessibilityLabel = item.name
          let active = selected == GlobalThemeCatalog.customId && item.design == appliedDesign
          card.accessibilityValue = active ? "已选中" : ""
          if active { card.accessibilityTraits.insert(.selected) }
          card.addAction(UIAction { _ in onSelectDesign(item.design) }, for: .primaryActionTriggered)
          row.addArrangedSubview(card)
        }
        if row.arrangedSubviews.count == 1 { row.addArrangedSubview(UIView()) }
        rows.addArrangedSubview(row)
      }
    }
    let themes = GlobalThemeCatalog.ids.map { KeyboardTheme.resolve($0, document: document) }
    for index in stride(from: 0, to: themes.count, by: 2) {
      let row = UIStackView()
      row.spacing = 10
      row.distribution = .fillEqually
      for skin in themes[index..<min(index + 2, themes.count)] {
        let chosen = skin.id == selected
        let card = KeyboardKeyButton()
        card.accessibilityIdentifier = "skinCard-\(skin.id)"
        card.accessibilityLabel = skin.title
        card.accessibilityValue = chosen ? "已选中" : ""
        if chosen { card.accessibilityTraits.insert(.selected) }
        // A theme with a fixed mode shows it in that mode whatever the keyboard's mode is.
        card.overrideUserInterfaceStyle = skin.appearance ?? .unspecified
        card.backgroundColor = skin.background
        card.layer.cornerRadius = 12
        card.layer.borderWidth = chosen ? 2 : 1
        card.layer.borderColor = (chosen ? UIColor.label : UIColor.separator).resolvedColor(with: traitCollection).cgColor
        card.clipsToBounds = true
        let title = UILabel()
        title.text = skin.title + (chosen ? "  ✓" : "")
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        title.textColor = skin.keyForeground
        let preview = KeyboardSkinMiniature(skin: skin)
        for child in [title, preview] {
          child.isUserInteractionEnabled = false
          child.translatesAutoresizingMaskIntoConstraints = false
          card.addSubview(child)
        }
        let preferredWidth = preview.widthAnchor.constraint(equalTo: card.widthAnchor, constant: -14)
        preferredWidth.priority = .defaultHigh
        NSLayoutConstraint.activate([
          preferredWidth,
          preview.widthAnchor.constraint(lessThanOrEqualToConstant: 220),
          preview.heightAnchor.constraint(equalTo: preview.widthAnchor, multiplier: KeyboardSkinMiniature.heightToWidthRatio),
          preview.centerXAnchor.constraint(equalTo: card.centerXAnchor),
          title.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 10),
          title.topAnchor.constraint(equalTo: card.topAnchor, constant: 7),
          title.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -8),
          preview.leadingAnchor.constraint(greaterThanOrEqualTo: card.leadingAnchor, constant: 7),
          preview.trailingAnchor.constraint(lessThanOrEqualTo: card.trailingAnchor, constant: -7),
          preview.topAnchor.constraint(equalTo: card.topAnchor, constant: 29),
          preview.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -7),
        ])
        card.addAction(UIAction { _ in onSelect(skin.id) }, for: .primaryActionTriggered)
        row.addArrangedSubview(card)
      }
      if row.arrangedSubviews.count == 1 { row.addArrangedSubview(UIView()) }
      rows.addArrangedSubview(row)
    }
    for child in [header, close, scroll] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    rows.translatesAutoresizingMaskIntoConstraints = false
    scroll.addSubview(rows)
    NSLayoutConstraint.activate([
      header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
      header.topAnchor.constraint(equalTo: topAnchor),
      header.heightAnchor.constraint(equalToConstant: showsHeader ? 40 : 0),
      close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
      close.topAnchor.constraint(equalTo: topAnchor),
      close.heightAnchor.constraint(equalToConstant: 40),
      close.widthAnchor.constraint(equalToConstant: 56),
      scroll.topAnchor.constraint(equalTo: header.bottomAnchor),
      scroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
      scroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
      scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
      rows.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
      rows.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
      rows.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
      rows.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -10),
      rows.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
    ])
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}

final class KeyboardSkinMiniature: UIView {
  // Scale the whole four-row keyboard together, including in landscape cards.
  static let heightToWidthRatio: CGFloat = 0.6
  let skin: KeyboardTheme
  let nineKey: Bool
  init(skin: KeyboardTheme, nineKey: Bool = false) {
    self.nineKey = nineKey
    self.skin = skin
    super.init(frame: .zero)
    isOpaque = false
    contentMode = .redraw
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func draw(_ rect: CGRect) {
    guard let context = UIGraphicsGetCurrentContext() else { return }
    let canvas = CGRect(x: 0, y: 0, width: 390, height: 390 * Self.heightToWidthRatio)
    context.saveGState()
    defer { context.restoreGState() }
    context.scaleBy(x: bounds.width / canvas.width, y: bounds.height / canvas.height)
    let design = skin.design
    let backdrop = KeyboardSkinBackgroundView(frame: canvas)
    backdrop.skin = skin
    backdrop.overrideUserInterfaceStyle = traitCollection.userInterfaceStyle
    backdrop.draw(canvas)
    let top = "QWERTYUIOP".map { String($0) }
    let middle = "ASDFGHJKL".map { String($0) }
    let bottom = ["⇧"] + "ZXCVBNM".map { String($0) } + ["⌫"]
    let action = ["123", "空格", "↵"]
    let rows: [[String]] = nineKey
      ? [["1", "ABC", "DEF"], ["GHI", "JKL", "MNO"], ["PQRS", "TUV", "WXYZ"], action]
      : [top, middle, bottom, action]
    let gap: CGFloat = 4
    let height = (canvas.height - gap * 3) / 4
    for (rowIndex, row) in rows.enumerated() {
      let inset: CGFloat = !nineKey && rowIndex == 1 ? canvas.width * 0.04 : 0
      let width = (canvas.width - inset * 2 - gap * CGFloat(row.count - 1)) / CGFloat(row.count)
      for (index, title) in row.enumerated() {
        let key = CGRect(x: inset + CGFloat(index) * (width + gap), y: CGFloat(rowIndex) * (height + gap), width: width, height: height)
        if let design, let context = UIGraphicsGetCurrentContext() {
          let surface = SkinKeySurfaceView(frame: CGRect(origin: .zero, size: key.size))
          surface.design = design; surface.scale = 1
          surface.fillColor = CustomKeyboardSkin.color(title == "↵" ? design.actionBackground : design.keyBackground)
            .withAlphaComponent(title == "↵" ? 1 : design.keyOpacity ?? 1)
          context.saveGState(); context.translateBy(x: key.minX, y: key.minY)
          surface.draw(surface.bounds)
          context.restoreGState()
        } else {
        let path = UIBezierPath(roundedRect: key, cornerRadius: min(skin.cornerRadius * 0.3, height * 0.4))
        if skin.hasShadow {
          skin.shadowColor.resolvedColor(with: traitCollection).setFill()
          UIBezierPath(roundedRect: key.offsetBy(dx: 0, dy: 2), cornerRadius: 2).fill()
        }
        (title == "↵" ? skin.actionBackground : skin.keyBackground).setFill()
        path.fill()
        if skin.borderWidth > 0 { skin.borderColor.setStroke(); path.lineWidth = 1; path.stroke() }
        }
        let font = (design?.monospaced ?? skin.usesMonospacedFont) ? UIFont.monospacedSystemFont(ofSize: 14, weight: .medium) : UIFont.systemFont(ofSize: 14, weight: .medium)
        let foreground = design.map { CustomKeyboardSkin.color(title == "↵" ? CustomKeyboardSkin.readableText(on: $0.actionBackground) : $0.keyForeground) }
          ?? (title == "↵" ? skin.actionForeground : skin.keyForeground)
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: foreground]
        let size = (title as NSString).size(withAttributes: attributes)
        (title as NSString).draw(at: CGPoint(x: key.midX - size.width / 2, y: key.midY - size.height / 2), withAttributes: attributes)
      }
    }
  }
}
