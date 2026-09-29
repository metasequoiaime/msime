import UIKit

final class KeyboardMorePickerView: UIView {
  private let rows = UIStackView()

  init(sections: [KeyboardToolSection], title: String = "工具", onClose: @escaping () -> Void) {
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardMorePicker"
    let skin = KeyboardTheme.current
    // 这个面板和方案选择器都盖在同一块键盘上,所以底色取当前皮肤,不是主题里那个固定值 —— 里面的按键早就走皮肤了,只有外面这一层没有。
    backgroundColor = skin.background
    let header = UILabel()
    header.text = title
    header.font = .systemFont(ofSize: 14, weight: .semibold)
    // `.secondaryLabel` 跟随系统外观而不是皮肤:浅色系统配深色皮肤时,它是深灰压在深底上。
    header.textColor = skin.keyForeground.withAlphaComponent(0.6)
    let close = UIButton(type: .system)
    var back = UIButton.Configuration.plain()
    back.title = "返回"
    back.image = UIImage(systemName: "chevron.left")
    back.preferredSymbolConfigurationForImage = .init(pointSize: 14, weight: .semibold)
    back.imagePadding = 5
    back.baseForegroundColor = skin.accent
    back.contentInsets = .init(top: 0, leading: 6, bottom: 0, trailing: 10)
    close.configuration = back
    close.accessibilityLabel = "返回键盘"
    close.accessibilityIdentifier = "closeMorePicker"
    close.addAction(UIAction { _ in onClose() }, for: .primaryActionTriggered)
    let scroll = UIScrollView()
    rows.axis = .vertical
    rows.spacing = 6
    scroll.showsVerticalScrollIndicator = false
    scroll.alwaysBounceVertical = false
    // This panel already sits below its own header; a system edge veil obscures the first tool.
    scroll.disableEdgeEffects()
    for child in [header, close, scroll] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    rows.translatesAutoresizingMaskIntoConstraints = false
    scroll.addSubview(rows)
    NSLayoutConstraint.activate([
      header.centerXAnchor.constraint(equalTo: centerXAnchor),
      header.topAnchor.constraint(equalTo: topAnchor),
      header.heightAnchor.constraint(equalToConstant: 44),
      close.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
      close.topAnchor.constraint(equalTo: topAnchor),
      close.heightAnchor.constraint(equalToConstant: 44),
      close.widthAnchor.constraint(greaterThanOrEqualToConstant: 76),
      scroll.topAnchor.constraint(equalTo: header.bottomAnchor),
      scroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
      scroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
      scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
      rows.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
      rows.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
      rows.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
      rows.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -10),
      rows.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
    ])
    update(sections: sections)
  }

  func update(sections: [KeyboardToolSection]) {
    rows.arrangedSubviews.forEach { rows.removeArrangedSubview($0); $0.removeFromSuperview() }
    sections.forEach { append($0) }
  }

  private func append(_ section: KeyboardToolSection) {
    let skin = KeyboardTheme.current
    if let title = section.title {
      let label = UILabel()
      label.text = title
      label.font = .systemFont(ofSize: 11, weight: .medium)
      label.textColor = skin.keyForeground.withAlphaComponent(0.6)
      label.heightAnchor.constraint(equalToConstant: 18).isActive = true
      rows.addArrangedSubview(label)
    }
    let columns = max(1, section.columns)
    for index in stride(from: 0, to: section.tools.count, by: columns) {
      let row = UIStackView()
      row.spacing = 8
      row.distribution = .fillEqually
      for tool in section.tools[index..<min(index + columns, section.tools.count)] {
        row.addArrangedSubview(makeCard(tool, in: section, fullWidth: columns == 1))
      }
      while row.arrangedSubviews.count < columns { row.addArrangedSubview(UIView()) }
      rows.addArrangedSubview(row)
    }
  }

  private func makeCard(_ tool: KeyboardTool, in section: KeyboardToolSection,
                        fullWidth: Bool) -> UIButton {
    let skin = KeyboardTheme.current
    let caption = section.caption(for: tool)
    let card = KeyboardKeyButton()
    var configuration = UIButton.Configuration.filled()
    configuration.title = tool.title
    configuration.subtitle = caption
    configuration.image = tool.symbol.flatMap { UIImage(systemName: $0) }
    configuration.imagePlacement = .leading
    configuration.imagePadding = 10
    configuration.preferredSymbolConfigurationForImage = .init(pointSize: 17, weight: .medium)
    configuration.contentInsets = .init(top: 6, leading: 12, bottom: 6, trailing: fullWidth ? 32 : 12)
    configuration.titleLineBreakMode = .byTruncatingTail
    configuration.titleAlignment = .leading
    // The design's more-panel tiles: a switched-on tool fills with the platform `accentSoft` and writes in the platform accent, the rest sit on the key fill, all at the iOS tile radius (dc.html L1446, `tileOn`).
    configuration.baseForegroundColor = tool.selected ? skin.toggleForeground : skin.keyForeground
    configuration.baseBackgroundColor = tool.selected ? skin.toggleBackground : skin.keyBackground
    configuration.background.cornerRadius = 14
    configuration.background.strokeWidth = 0
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { value in
      var value = value; value.font = .systemFont(ofSize: 14, weight: .medium); return value
    }
    configuration.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { value in
      var value = value; value.font = .systemFont(ofSize: 11); value.foregroundColor = skin.keyForeground.withAlphaComponent(0.7); return value
    }
    card.configuration = configuration
    card.contentHorizontalAlignment = fullWidth ? .leading : .center
    card.heightAnchor.constraint(equalToConstant: 48).isActive = true
    if fullWidth {
      let chevron = UIImageView(image: UIImage(systemName: "chevron.right"))
      chevron.preferredSymbolConfiguration = .init(pointSize: 11, weight: .semibold)
      chevron.tintColor = skin.keyForeground.withAlphaComponent(0.35)
      chevron.contentMode = .scaleAspectFit
      chevron.translatesAutoresizingMaskIntoConstraints = false
      card.addSubview(chevron)
      NSLayoutConstraint.activate([
        chevron.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -14),
        chevron.centerYAnchor.constraint(equalTo: card.centerYAnchor),
        chevron.widthAnchor.constraint(equalToConstant: 10),
        chevron.heightAnchor.constraint(equalToConstant: 14),
      ])
    }
    card.accessibilityIdentifier = "moreCard-" + tool.title
    card.accessibilityLabel = tool.title
    card.accessibilityValue = caption ?? "点击打开"
    card.isEnabled = tool.enabled
    if tool.selected { card.accessibilityTraits.insert(.selected) }
    card.addAction(UIAction { _ in tool.run() }, for: .primaryActionTriggered)
    return card
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}
