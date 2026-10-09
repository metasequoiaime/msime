import UIKit

/// 键盘里的输入方式选择面板（输入方式）：列出可用的方案，英文排在第三位，排成横向分页、带页码圆点的方框字形网格，手机每页 4 × 2，iPad 键盘每页 6 × 2，布局与功能面板相同。
///
/// 在键盘里它以 `showsHeader: false` 在工具栏下方打开：没有「返回」和「键盘设置」按钮，背景透明，透出键盘自己的背景。带标题栏时保留全屏覆盖的形式，两个按钮放在皮肤背景上。传入 `onAddLanguage` 时末尾多一个「添加语言」格子。
final class KeyboardSchemePickerView: UIView {
  private static let topMargin: CGFloat = 10
  private static let bottomMargin: CGFloat = 4
  private static let headerHeight: CGFloat = 44
  private static let rowGap: CGFloat = 16
  private static let columnGap: CGFloat = 4

  private let skin = KeyboardTheme.current
  private var accent: UIColor { skin.accent }
  private let grid: KeyboardPagedGridView

  /// - Parameter shuangpinProfile: 共享文档里的 `shuangpin_profile`，面板里的双拼只列这一种（见 `InputSchemePreference.pickerSchemes`）；nil 按小鹤。
  init(selected: ChineseInputScheme, isChineseMode: Bool = true, showsHeader: Bool = true,
       formFactor: KeyboardFormFactor = .phone, shuangpinProfile: String? = nil,
       onSelect: @escaping (ChineseInputScheme) -> Void,
       onSelectEnglish: (() -> Void)? = nil,
       onSettings: (() -> Void)? = nil,
       onAddLanguage: (() -> Void)? = nil,
       onClose: @escaping () -> Void) {
    let tablet = formFactor == .tablet
    grid = KeyboardPagedGridView(
      columns: tablet ? 6 : 4, rows: 2, rowHeight: KeyboardSchemeTileView.height,
      rowGap: Self.rowGap, columnGap: Self.columnGap, sidePadding: tablet ? 48 : 4)
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardSchemePicker"
    accessibilityLabel = "输入方式"
    backgroundColor = showsHeader ? skin.background : .clear
    grid.accessibilityIdentifier = "schemePickerScroll"
    grid.apply(skin: skin)

    // 粤拼、注音或笔画缺少对应词库时不列出：否则点它的格子，Engine 跑的会是另一个方案。双拼只列用户设置的那一种。
    let schemes = InputSchemePreference.pickerSchemes(
      InputSchemePreference.offeredSchemes, selected: selected, shuangpinProfile: shuangpinProfile)
    var tiles: [KeyboardSchemeTileView] = schemes.map { scheme in
      let glyph: String
      let badge: String
      // 瓷砖全部走皮肤的颜色：未选中用按键文字色，选中用强调色。字形本来就两两不同(拼26/拼9/鹤双/自双/微双/S双/五86/あ26/あ9/한26/粤26/注大千/越26/藏26/笔5/写手/EN26)，分辨靠读字，不靠给每一族配一个不跟皮肤走的系统色。
      switch scheme {
      case .quanpin: glyph = "拼"; badge = "26"
      case .nineKey: glyph = "拼"; badge = "9"
      case .shuangpin: glyph = "鹤"; badge = "双"
      case .ziranma: glyph = "自"; badge = "双"
      case .microsoft: glyph = "微"; badge = "双"
      case .shoudao: glyph = "S"; badge = "双"
      case .wubi: glyph = "五"; badge = WubiProfilePreference.badge(WubiProfilePreference.profile)
      case .japanese: glyph = "あ"; badge = "26"
      case .japaneseNineKey: glyph = "あ"; badge = "9"
      case .korean: glyph = "한"; badge = "26"
      case .cantonese: glyph = "粤"; badge = "26"
      case .zhuyin: glyph = "注"; badge = "大千"
      case .vietnamese: glyph = "越"; badge = "26"
      case .tibetan: glyph = "藏"; badge = "26"
      // 角标是五个笔画键，与 Android、鸿蒙的卡片一致。
      case .stroke: glyph = "笔"; badge = "5"
      case .handwriting: glyph = "写"; badge = "手"
      }
      return KeyboardSchemeTileView(
        title: scheme.title, face: .glyph(glyph, badge: badge), selected: isChineseMode && scheme == selected,
        identifier: "schemeCard-\(scheme.rawValue)", skin: skin) { onSelect(scheme) }
    }
    if let onSelectEnglish {
      // English is a platform text mode, not a second persisted Engine scheme.
      tiles.insert(KeyboardSchemeTileView(
        title: "英文 26 键", face: .glyph("EN", badge: "26"), selected: !isChineseMode,
        identifier: "schemeEnglishCard", skin: skin, action: onSelectEnglish), at: min(2, tiles.count))
    }
    if let onAddLanguage {
      tiles.append(KeyboardSchemeTileView(
        title: "添加语言", face: .add, selected: false, identifier: "schemeAddLanguageCard", skin: skin, action: onAddLanguage))
    }
    grid.setTiles(tiles, preservingPage: false)
    // 打开时停在当前输入方式所在的那一页，让它的勾选标记在视野里。
    if let index = tiles.firstIndex(where: \.isSelected) {
      grid.scrollToPage(index / grid.tilesPerPage, animated: false)
    }

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

    // 没有标题栏时由上方的工具栏关闭面板，所以这两个按钮都不创建，它们的 id 也不在视图树里。
    let header = showsHeader ? [close, settings] : []
    for child in header + [grid] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    var constraints = [
      grid.topAnchor.constraint(equalTo: topAnchor, constant: (showsHeader ? Self.headerHeight : 0) + Self.topMargin),
      grid.leadingAnchor.constraint(equalTo: leadingAnchor),
      grid.trailingAnchor.constraint(equalTo: trailingAnchor),
      grid.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -Self.bottomMargin),
    ]
    if showsHeader {
      constraints += [
        close.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
        close.topAnchor.constraint(equalTo: topAnchor),
        close.widthAnchor.constraint(equalToConstant: Self.headerHeight), close.heightAnchor.constraint(equalToConstant: Self.headerHeight),
        settings.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
        settings.topAnchor.constraint(equalTo: topAnchor),
        settings.widthAnchor.constraint(equalToConstant: Self.headerHeight), settings.heightAnchor.constraint(equalToConstant: Self.headerHeight),
      ]
    }
    NSLayoutConstraint.activate(constraints)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 当前显示的页，从 0 起。
  var currentPage: Int { grid.currentPage }

  var pageCount: Int { grid.pageCount }
}

/// 输入方式面板里的一个格子，按设计稿的条目画：30pt 的图标区在上，12.5pt 的标签在下，相距 7pt，在 52pt 的单元格里居中，没有背景。
///
/// 方案格子显示一个 26pt 的方框（圆角 5，边线 1.8pt），框内是 14pt 粗体的字形，字形有两个字符或更多时用 10.5pt；另有一个 8.5pt heavy 字重的数字角标，以键盘背景为底压在方框右下角（相对图标区右 -3、下 -1）。「添加语言」格子改为显示一个 26pt 的加号。全部用按键前景色；当前输入方式用强调色，标签为半粗体，并用与功能面板里已打开格子相同的 13pt 勾选角标（右 -5、下 -4）代替数字角标。
///
/// 它是不带 configuration 的 UIButton，所以键盘的皮肤处理（`applyKeyboardSkin`）会把带 configuration 的按钮重画成键帽，却不会动它。
final class KeyboardSchemeTileView: UIButton {
  enum Face {
    case glyph(String, badge: String)
    case add
  }

  static let height: CGFloat = 52
  private static let iconArea: CGFloat = 30
  private static let glyphSide: CGFloat = 26
  private static let glyphRadius: CGFloat = 5
  private static let glyphBorder: CGFloat = 1.8
  private static let glyphFontSize: CGFloat = 14
  private static let wideGlyphFontSize: CGFloat = 10.5
  private static let numberFontSize: CGFloat = 8.5
  private static let numberRight: CGFloat = -3
  private static let numberBottom: CGFloat = -1
  /// 「添加语言」格子的加号：26pt，描边按 viewBox 单位为 1.6，与设计稿的画法一致。
  private static let addSide: CGFloat = 26
  private static let addStroke: CGFloat = 1.6
  private static let labelFontSize: CGFloat = 12.5
  private static let labelGap: CGFloat = 7
  private static let badgeSide: CGFloat = 13
  private static let badgeRing: CGFloat = 2
  private static let badgeCheckSide: CGFloat = 9
  private static let badgeRight: CGFloat = -5
  private static let badgeBottom: CGFloat = -4
  private static let pressedAlpha: CGFloat = 0.55

  private let face: Face
  private let skin: KeyboardTheme
  private let glyphBoxLayer = CAShapeLayer()
  private let addLayer = CAShapeLayer()
  private let badgeLayer = CAShapeLayer()
  private let badgeCheckLayer = CAShapeLayer()
  private let glyphLabel = UILabel()
  /// 数字角标的底板，用键盘背景色，把数字下方那段方框边线断开；数字本身是单独的标签，这样它的行高不会被裁到 8.5pt 的底板里。
  private let numberPlate = UIView()
  private let numberLabel = UILabel()
  private let nameLabel = UILabel()

  init(title: String, face: Face, selected: Bool, identifier: String, skin: KeyboardTheme, action: @escaping () -> Void) {
    self.face = face
    self.skin = skin
    super.init(frame: .zero)
    for shape in [glyphBoxLayer, addLayer, badgeCheckLayer] {
      shape.fillColor = nil
      shape.lineCap = .round
      shape.lineJoin = .round
    }
    for shape in [glyphBoxLayer, addLayer, badgeLayer] { layer.addSublayer(shape) }
    badgeLayer.addSublayer(badgeCheckLayer)
    glyphLabel.textAlignment = .center
    numberLabel.font = .systemFont(ofSize: Self.numberFontSize, weight: .heavy)
    numberLabel.textAlignment = .center
    nameLabel.text = title
    nameLabel.textAlignment = .center
    nameLabel.numberOfLines = 1
    nameLabel.lineBreakMode = .byTruncatingTail
    nameLabel.adjustsFontSizeToFitWidth = true
    nameLabel.minimumScaleFactor = 0.75
    nameLabel.font = .systemFont(ofSize: Self.labelFontSize, weight: selected ? .semibold : .regular)
    switch face {
    case .glyph(let glyph, let badge):
      glyphLabel.text = glyph
      glyphLabel.font = .systemFont(ofSize: glyph.count > 1 ? Self.wideGlyphFontSize : Self.glyphFontSize, weight: .bold)
      numberLabel.text = badge
      // 当前输入方式的格子在数字角标的位置显示勾选标记，与 Android 的网格格子一致。
      numberPlate.isHidden = selected || badge.isEmpty
      numberLabel.isHidden = numberPlate.isHidden
      addLayer.isHidden = true
    case .add:
      glyphLabel.isHidden = true
      glyphBoxLayer.isHidden = true
      numberPlate.isHidden = true
      numberLabel.isHidden = true
    }
    badgeLayer.isHidden = !selected
    for child in [glyphLabel, numberPlate, numberLabel, nameLabel] {
      child.isUserInteractionEnabled = false
      child.isAccessibilityElement = false
      addSubview(child)
    }
    isSelected = selected
    accessibilityIdentifier = identifier
    accessibilityLabel = title
    accessibilityValue = selected ? "已选中" : ""
    if selected { accessibilityTraits.insert(.selected) }
    addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (tile: KeyboardSchemeTileView, _: UITraitCollection) in
      tile.applyColors()
    }
    applyColors()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override var isHighlighted: Bool {
    didSet { alpha = isHighlighted ? Self.pressedAlpha : 1 }
  }

  override var intrinsicContentSize: CGSize {
    CGSize(width: UIView.noIntrinsicMetric, height: Self.height)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let font = nameLabel.font ?? .systemFont(ofSize: Self.labelFontSize)
    let labelHeight = ceil(font.lineHeight)
    let top = max(0, ((bounds.height - Self.iconArea - Self.labelGap - labelHeight) / 2).rounded(.down))
    let area = CGRect(x: (bounds.midX - Self.iconArea / 2).rounded(), y: top, width: Self.iconArea, height: Self.iconArea)
    nameLabel.frame = CGRect(x: 0, y: area.maxY + Self.labelGap, width: bounds.width, height: labelHeight)
    let glyphBox = CGRect(x: area.midX - Self.glyphSide / 2, y: area.midY - Self.glyphSide / 2, width: Self.glyphSide, height: Self.glyphSide)
    glyphLabel.frame = glyphBox
    // 底板是设计稿里 8.5pt 的行盒，数字两侧各留 1pt；标签按自身的自然行高在底板上居中。
    let numberFont = numberLabel.font ?? .systemFont(ofSize: Self.numberFontSize, weight: .heavy)
    let numberWidth = ceil(((numberLabel.text ?? "") as NSString).size(withAttributes: [.font: numberFont]).width) + 2
    let plate = CGRect(
      x: area.maxX - Self.numberRight - numberWidth, y: area.maxY - Self.numberBottom - Self.numberFontSize,
      width: numberWidth, height: Self.numberFontSize)
    numberPlate.frame = plate
    let numberHeight = ceil(numberFont.lineHeight)
    numberLabel.frame = CGRect(x: plate.minX, y: plate.midY - numberHeight / 2, width: plate.width, height: numberHeight)
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    // 边线画在 26pt 方框之内，与设计稿按 border-box 计算尺寸的画法一致。
    let inset = Self.glyphBorder / 2
    glyphBoxLayer.lineWidth = Self.glyphBorder
    glyphBoxLayer.path = UIBezierPath(roundedRect: glyphBox.insetBy(dx: inset, dy: inset), cornerRadius: Self.glyphRadius - inset).cgPath
    let addRect = CGRect(x: area.midX - Self.addSide / 2, y: area.midY - Self.addSide / 2, width: Self.addSide, height: Self.addSide)
    addLayer.path = KeyboardIcon.plus.path(in: addRect).cgPath
    addLayer.lineWidth = Self.addStroke * Self.addSide / KeyboardIcon.viewBox
    badgeLayer.frame = CGRect(
      x: area.maxX - Self.badgeRight - Self.badgeSide, y: area.maxY - Self.badgeBottom - Self.badgeSide,
      width: Self.badgeSide, height: Self.badgeSide)
    // 外环是一条 2pt 的描边，骑在比 13pt 圆盘大一圈 1pt 的圆上，所以强调色保留完整的 13pt，外环像设计稿的 box-shadow 一样落在它外侧。
    badgeLayer.lineWidth = Self.badgeRing
    badgeLayer.path = UIBezierPath(
      arcCenter: CGPoint(x: Self.badgeSide / 2, y: Self.badgeSide / 2), radius: Self.badgeSide / 2 + Self.badgeRing / 2,
      startAngle: 0, endAngle: .pi * 2, clockwise: true
    ).cgPath
    let checkOrigin = (Self.badgeSide - Self.badgeCheckSide) / 2
    badgeCheckLayer.frame = badgeLayer.bounds
    badgeCheckLayer.path = KeyboardIcon.check.path(
      in: CGRect(x: checkOrigin, y: checkOrigin, width: Self.badgeCheckSide, height: Self.badgeCheckSide)).cgPath
    badgeCheckLayer.lineWidth = KeyboardIcon.check.lineWidth * Self.badgeCheckSide / KeyboardIcon.viewBox
    CATransaction.commit()
  }

  /// 按当前 traits 解析皮肤颜色：layer 用的是 CGColor，不会自己跟随深浅色切换。
  private func applyColors() {
    let color = isSelected ? skin.accent : skin.keyForeground
    let resolved = color.resolvedColor(with: traitCollection).cgColor
    let background = skin.background.resolvedColor(with: traitCollection)
    for label in [glyphLabel, numberLabel, nameLabel] { label.textColor = color }
    numberPlate.backgroundColor = background
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    glyphBoxLayer.strokeColor = resolved
    addLayer.strokeColor = resolved
    badgeLayer.fillColor = skin.accent.resolvedColor(with: traitCollection).cgColor
    badgeLayer.strokeColor = background.cgColor
    badgeCheckLayer.strokeColor = background.cgColor
    CATransaction.commit()
  }
}
