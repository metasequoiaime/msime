import UIKit

/// 键盘里的主题选择面板（皮肤）：先列目录里的全局主题（与 App 和桌面端列出的 id 相同），再列已保存的键盘设计（应用到自定义主题上），排成横向分页、带页码圆点的键盘缩略图网格。
///
/// 手机每页 4 × 2 个格子，iPad 键盘每页 6 × 2。在键盘里它以 `showsHeader: false` 在工具栏下方打开：没有标题，没有「完成」按钮，背景透明，透出键盘自己的背景，与设计稿的面板一致。带标题栏时保留全屏覆盖的形式，不透明背景上有标题和「完成」。
final class KeyboardSkinPickerView: UIView {
  /// 设计稿里的面板：分页上方留 6pt（另加分页自己的 2pt，供格子画选中环），圆点下方留 4pt。
  private static let topMargin: CGFloat = 6
  private static let bottomMargin: CGFloat = 4
  private static let headerHeight: CGFloat = 40
  /// 一行格子的高度：缩略图加名称，即设计稿约 80pt 的 MiniKb 再加标签。
  static let rowHeight: CGFloat = 92
  private static let rowGap: CGFloat = 6
  private static let columnGap: CGFloat = 10
  private static let sidePadding: CGFloat = 6

  private let grid: KeyboardPagedGridView

  /// `document` 是格子解析皮肤所用的共享文档，所以「自定义」格子显示的是按配置画出的自定义主题。选中环和名称用键盘当前皮肤的颜色，不用各格子所展示皮肤的颜色。
  init(selected: String, document: [String: Any]? = MetasequoiaInputSessionBridge.loadSharedPreferences(), showsHeader: Bool = true,
       formFactor: KeyboardFormFactor = .phone,
       onSelect: @escaping (String) -> Void, onSelectDesign: @escaping (CustomKeyboardSkin) -> Void, onClose: @escaping () -> Void) {
    grid = KeyboardPagedGridView(
      columns: formFactor == .tablet ? 6 : 4, rows: 2, rowHeight: Self.rowHeight,
      rowGap: Self.rowGap, columnGap: Self.columnGap, sidePadding: Self.sidePadding)
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardSkinPicker"
    accessibilityLabel = "皮肤"
    let panel = KeyboardTheme.current
    backgroundColor = showsHeader ? .secondarySystemBackground : .clear
    grid.apply(skin: panel)

    var tiles: [KeyboardSkinTileView] = GlobalThemeCatalog.ids.map { id in
      let skin = KeyboardTheme.resolve(id, document: document)
      let tile = KeyboardSkinTileView(preview: skin, title: skin.title, selected: skin.id == selected, panel: panel)
      tile.accessibilityIdentifier = "skinCard-\(skin.id)"
      tile.addAction(UIAction { _ in onSelect(skin.id) }, for: .primaryActionTriggered)
      return tile
    }
    let appliedDesign = GlobalThemePreference.design(in: document) ?? (document == nil ? CustomKeyboardSkinStore.stored : nil)
    // 已保存的设计排在目录之后，与 Android 一致；每一份都应用到自定义主题上。
    tiles += CustomSkinLibrary.designs.map { item in
      let active = selected == GlobalThemeCatalog.customId && item.design == appliedDesign
      let tile = KeyboardSkinTileView(preview: .designed(item.design), title: item.name, selected: active, panel: panel)
      tile.accessibilityIdentifier = "savedSkinCard-" + item.id.uuidString
      tile.addAction(UIAction { _ in onSelectDesign(item.design) }, for: .primaryActionTriggered)
      return tile
    }
    grid.setTiles(tiles, preservingPage: false)
    // 打开时停在选中皮肤所在的那一页，让它的选中环在视野里。
    if let index = tiles.firstIndex(where: \.isSelected) {
      grid.scrollToPage(index / grid.tilesPerPage, animated: false)
    }

    let header = UILabel()
    header.text = "选择皮肤"
    header.font = .systemFont(ofSize: 17, weight: .semibold)
    header.textColor = panel.keyForeground
    header.isHidden = !showsHeader
    let close = UIButton(type: .system)
    close.setTitle("完成", for: .normal)
    close.tintColor = panel.accent
    close.isHidden = !showsHeader
    close.accessibilityIdentifier = "closeSkinPicker"
    close.addAction(UIAction { _ in onClose() }, for: .primaryActionTriggered)
    for child in [header, close, grid] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    NSLayoutConstraint.activate([
      header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
      header.topAnchor.constraint(equalTo: topAnchor),
      header.heightAnchor.constraint(equalToConstant: showsHeader ? Self.headerHeight : 0),
      close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
      close.topAnchor.constraint(equalTo: topAnchor),
      close.heightAnchor.constraint(equalToConstant: Self.headerHeight),
      close.widthAnchor.constraint(equalToConstant: 56),
      grid.topAnchor.constraint(equalTo: header.bottomAnchor, constant: Self.topMargin),
      grid.leadingAnchor.constraint(equalTo: leadingAnchor),
      grid.trailingAnchor.constraint(equalTo: trailingAnchor),
      grid.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -Self.bottomMargin),
    ])
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 当前显示的页，从 0 起。
  var currentPage: Int { grid.currentPage }

  var pageCount: Int { grid.pageCount }
}

/// 皮肤面板里的一个格子：9pt 圆角的皮肤缩略图，下面是 12pt 的皮肤名称。
///
/// 选中的格子在缩略图外有一圈 2pt 的强调色环，名称用强调色半粗体；其余格子是 1pt 的细线环，名称用按键前景色。这些颜色取自键盘当前皮肤（`panel`），缩略图则取自格子代表的皮肤。按下时格子缩放到 0.96。
///
/// 它是不带 configuration 的 UIButton，所以键盘的皮肤处理（`applyKeyboardSkin`）会把带 configuration 的按钮重画成键帽，却不会动它。
final class KeyboardSkinTileView: UIButton {
  private static let cornerRadius: CGFloat = 9
  /// 缩略图上方给选中环留的空间；设计稿的分页顶部也留了同样的 2pt。
  private static let ringRoom: CGFloat = 2
  private static let labelGap: CGFloat = 6
  private static let labelFontSize: CGFloat = 12
  /// 设计稿 MiniKb 的比例（390 × 292）；格子比这更高时，多出的空间留在名称下方，不拉伸缩略图。
  private static let maximumHeightToWidth: CGFloat = 292 / 390
  private static let pressedScale: CGFloat = 0.96

  private let preview: KeyboardSkinMiniature
  private let ring = CAShapeLayer()
  private let nameLabel = UILabel()
  private let panel: KeyboardTheme

  init(preview skin: KeyboardTheme, title: String, selected: Bool, panel: KeyboardTheme) {
    preview = KeyboardSkinMiniature(skin: skin)
    self.panel = panel
    super.init(frame: .zero)
    // 固定了深浅色模式的主题，不论键盘当前是什么模式，都按它自己的模式显示。
    preview.overrideUserInterfaceStyle = skin.appearance ?? .unspecified
    // 缩略图画出背景的照片、渐变和图案，但不画纯色底。
    preview.backgroundColor = skin.background
    preview.layer.cornerRadius = Self.cornerRadius
    preview.layer.cornerCurve = .continuous
    preview.clipsToBounds = true
    ring.fillColor = nil
    layer.addSublayer(ring)
    nameLabel.text = title
    nameLabel.textAlignment = .center
    nameLabel.numberOfLines = 1
    nameLabel.lineBreakMode = .byTruncatingTail
    nameLabel.font = .systemFont(ofSize: Self.labelFontSize, weight: selected ? .semibold : .regular)
    for child in [preview, nameLabel] as [UIView] {
      child.isUserInteractionEnabled = false
      child.isAccessibilityElement = false
      addSubview(child)
    }
    isSelected = selected
    accessibilityLabel = title
    accessibilityValue = selected ? "已选中" : ""
    if selected { accessibilityTraits.insert(.selected) }
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (tile: KeyboardSkinTileView, _: UITraitCollection) in
      tile.applyColors()
    }
    applyColors()
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      let target: CGAffineTransform = isHighlighted ? CGAffineTransform(scaleX: Self.pressedScale, y: Self.pressedScale) : .identity
      guard window != nil, !UIAccessibility.isReduceMotionEnabled else {
        transform = target
        return
      }
      UIView.animate(withDuration: 0.1, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
        self.transform = target
      }
    }
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    let font = nameLabel.font ?? .systemFont(ofSize: Self.labelFontSize)
    let labelHeight = ceil(font.lineHeight)
    let width = bounds.width
    let available = bounds.height - Self.ringRoom - Self.labelGap - labelHeight
    let height = max(0, min(available, width * Self.maximumHeightToWidth))
    preview.frame = CGRect(x: 0, y: Self.ringRoom, width: width, height: height)
    nameLabel.frame = CGRect(x: 0, y: preview.frame.maxY + Self.labelGap, width: width, height: labelHeight)
    // 选中环完全在缩略图外侧，与设计稿用 box-shadow 画的环一样，所以永远不会盖住预览。
    let stroke: CGFloat = isSelected ? 2 : 1
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    ring.lineWidth = stroke
    ring.path = UIBezierPath(
      roundedRect: preview.frame.insetBy(dx: -stroke / 2, dy: -stroke / 2), cornerRadius: Self.cornerRadius + stroke / 2
    ).cgPath
    CATransaction.commit()
  }

  /// 按当前 traits 解析面板颜色：选中环是一个 layer，用的是 CGColor，它不会自己跟随深浅色切换。
  private func applyColors() {
    nameLabel.textColor = isSelected ? panel.accent : panel.keyForeground
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    ring.strokeColor = (isSelected ? panel.accent : panel.hairline).resolvedColor(with: traitCollection).cgColor
    CATransaction.commit()
  }
}

/// 设计稿里的键盘缩略图（MiniKb），用格子所代表的皮肤绘制：空闲状态的工具栏（品牌圆盘、表情、常用语、剪贴板、皮肤、输入方式和收起箭头），小写字母行（中间一行两侧各缩进 5%），用功能键颜色画的 shift 和删除键（`kb.spec`），123 / ， / 带麦克风和「全拼」的空格 / 。 / 中 / 回车这一行，以及 Home 指示条区域。它与 App 里 `KeyboardSkinPreview` 的带工具栏样式一致，所以同一套皮肤在键盘面板里和 App 的「皮肤」页上看起来一样。
///
/// 缩略图在设计稿 390 × 292 的画布上排版，再按格子宽度缩放；格子比这个比例矮时裁掉底部，与设计稿里 `overflow: hidden` 的盒子一样。
final class KeyboardSkinMiniature: UIView {
  /// 设计稿 MiniKb 的画布尺寸，含工具栏为 390 × 292。
  private static let canvasWidth: CGFloat = 390
  private static let canvasHeight: CGFloat = 292
  /// 列两侧的内边距、工具栏高度及其下方的间距。
  private static let sidePadding: CGFloat = 3
  private static let toolbarHeight: CGFloat = 50
  private static let toolbarGap: CGFloat = 8
  private static let rowHeight: CGFloat = 43
  private static let rowGap: CGFloat = 11
  private static let keyGap: CGFloat = 6
  private static let keyRadius: CGFloat = 5

  let skin: KeyboardTheme
  let nineKey: Bool
  init(skin: KeyboardTheme, nineKey: Bool = false) {
    self.nineKey = nineKey
    self.skin = skin
    super.init(frame: .zero)
    isOpaque = false
    contentMode = .redraw
    // 绘制时只解析一次动态颜色，所以深浅色模式变化后要重画。
    registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (miniature: KeyboardSkinMiniature, _: UITraitCollection) in
      miniature.setNeedsDisplay()
    }
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 缩略图里的一个键：它在行内占的权重、显示的内容和用哪种填充。
  private struct Key {
    enum Fill { case letter, function, action }
    enum Face {
      case text(String, size: CGFloat, weight: UIFont.Weight)
      case icon(KeyboardIcon, side: CGFloat)
      /// 空格键：麦克风和方案名，用次要颜色。
      case space(String)
    }

    var weight: CGFloat
    var face: Face
    var fill: Fill = .letter
    var hint: String?

    static func letter(_ title: String, hint: String? = nil, size: CGFloat = 22) -> Key {
      Key(weight: 1, face: .text(title, size: size, weight: .regular), hint: hint)
    }
  }

  private var rows: [(inset: CGFloat, keys: [Key])] {
    let letters: [(CGFloat, [Key])]
    if nineKey {
      letters = [["分词", "ABC", "DEF"], ["GHI", "JKL", "MNO"], ["PQRS", "TUV", "WXYZ"]].map { row in
        (0, row.map { Key.letter($0, size: 18) })
      }
    } else {
      let hints = Array("1234567890").map(String.init)
      letters = [
        (0, Array("qwertyuiop").enumerated().map { Key.letter(String($1), hint: hints[$0]) }),
        (0.05, Array("asdfghjkl").map { Key.letter(String($0)) }),
        (0, [Key(weight: 1.4, face: .icon(.shift, side: 22), fill: .function)]
          + Array("zxcvbnm").map { Key.letter(String($0)) }
          + [Key(weight: 1.4, face: .icon(.backspace, side: 22), fill: .function)]),
      ]
    }
    let bottom: [Key] = [
      Key(weight: 1.25, face: .text("123", size: 15, weight: .medium), fill: .function),
      Key.letter("，"),
      Key(weight: 4, face: .space("全拼")),
      Key.letter("。"),
      Key(weight: 1.05, face: .text("中", size: 15, weight: .medium), fill: .function),
      Key(weight: 1.9, face: .icon(.returnKey, side: 22), fill: .action),
    ]
    return letters.map { (inset: $0.0, keys: $0.1) } + [(inset: 0, keys: bottom)]
  }

  override func draw(_ rect: CGRect) {
    guard let context = UIGraphicsGetCurrentContext(), bounds.width > 0 else { return }
    let scale = bounds.width / Self.canvasWidth
    context.saveGState()
    defer { context.restoreGState() }
    context.scaleBy(x: scale, y: scale)
    let canvas = CGRect(x: 0, y: 0, width: Self.canvasWidth, height: max(Self.canvasHeight, bounds.height / scale))
    let backdrop = KeyboardSkinBackgroundView(frame: canvas)
    backdrop.skin = skin
    backdrop.overrideUserInterfaceStyle = traitCollection.userInterfaceStyle
    backdrop.draw(canvas)

    let width = Self.canvasWidth - 2 * Self.sidePadding
    drawToolbar(in: CGRect(x: Self.sidePadding, y: 0, width: width, height: Self.toolbarHeight))
    var y = Self.toolbarHeight + Self.toolbarGap
    for row in rows {
      let inset = width * row.inset
      let rowWidth = width - 2 * inset
      let available = rowWidth - Self.keyGap * CGFloat(row.keys.count - 1)
      let unit = available / row.keys.reduce(0) { $0 + $1.weight }
      var x = Self.sidePadding + inset
      for key in row.keys {
        let frame = CGRect(x: x, y: y, width: unit * key.weight, height: Self.rowHeight)
        drawKey(key, in: frame)
        x = frame.maxX + Self.keyGap
      }
      y += Self.rowHeight + Self.rowGap
    }
    // Home 指示条在按键下方剩余的空间里居中，用按键前景色、透明度 .85。
    let keysBottom = y - Self.rowGap + Self.toolbarGap
    let indicator = CGRect(x: (Self.canvasWidth - 134) / 2, y: keysBottom + (Self.canvasHeight - keysBottom - 5) / 2, width: 134, height: 5)
    skin.keyForeground.withAlphaComponent(0.85).setFill()
    UIBezierPath(roundedRect: indicator, cornerRadius: 2.5).fill()
  }

  private func drawToolbar(in frame: CGRect) {
    let glyphs: [KeyboardIcon] = [.toolbarEmoji, .toolbarPhrase, .toolbarClipboard, .toolbarSkin, .toolbarScheme]
    let columns = CGFloat(glyphs.count + 2)
    let column = frame.width / columns
    func center(_ index: Int) -> CGPoint { CGPoint(x: frame.minX + column * (CGFloat(index) + 0.5), y: frame.midY) }

    // 品牌圆盘按工具栏的画法：`logoCircle` 底色，白色描边下是 `logoMark` 颜色的标志外框。
    let discSide: CGFloat = 26
    let disc = CGRect(x: center(0).x - discSide / 2, y: center(0).y - discSide / 2, width: discSide, height: discSide)
    skin.logoCircle.setFill()
    UIBezierPath(ovalIn: disc).fill()
    let markSide = discSide * 18 / 30
    let mark = CGRect(x: disc.midX - markSide / 2, y: disc.midY - markSide / 2, width: markSide, height: markSide)
    skin.logoMark.setFill()
    UIBezierPath(cgPath: MSIMELogo.framePath(in: mark)).fill()
    let stroke = UIBezierPath(cgPath: MSIMELogo.strokePath(in: mark))
    stroke.lineWidth = MSIMELogo.strokeWidth * MSIMELogo.scale(for: mark)
    stroke.lineCapStyle = .round
    stroke.lineJoinStyle = .round
    UIColor.white.setStroke()
    stroke.stroke()

    for (index, glyph) in glyphs.enumerated() {
      strokeIcon(glyph, side: 22, center: center(index + 1), color: skin.keyForeground)
    }
    strokeIcon(.collapse, side: 21, center: center(glyphs.count + 1), color: skin.keyForeground)
  }

  private func drawKey(_ key: Key, in frame: CGRect) {
    if let design = skin.design, let context = UIGraphicsGetCurrentContext() {
      // 键盘设计的功能键与字母键用同样的填充，与 `functionKeyBackground` 的规定一致。
      let surface = SkinKeySurfaceView(frame: CGRect(origin: .zero, size: frame.size))
      surface.design = design
      surface.scale = 1
      surface.fillColor = key.fill == .action
        ? CustomKeyboardSkin.color(design.actionBackground)
        : CustomKeyboardSkin.color(design.keyBackground).withAlphaComponent(design.keyOpacity ?? 1)
      context.saveGState()
      context.translateBy(x: frame.minX, y: frame.minY)
      surface.draw(surface.bounds)
      context.restoreGState()
    } else {
      let path = UIBezierPath(roundedRect: frame, cornerRadius: Self.keyRadius)
      if skin.hasShadow {
        skin.shadowColor.setFill()
        UIBezierPath(roundedRect: frame.offsetBy(dx: 0, dy: 1), cornerRadius: Self.keyRadius).fill()
      }
      switch key.fill {
      case .letter: skin.keyBackground.setFill()
      case .function: skin.functionKeyBackground.setFill()
      case .action: skin.actionBackground.setFill()
      }
      path.fill()
      if skin.borderWidth > 0 {
        skin.borderColor.setStroke()
        path.lineWidth = 1
        path.stroke()
      }
    }

    let foreground = key.fill == .action ? skin.actionForeground : skin.keyForeground
    switch key.face {
    case .text(let title, let size, let weight):
      drawText(title, font: font(size: size, weight: weight), color: foreground, center: CGPoint(x: frame.midX, y: frame.midY))
    case .icon(let icon, let side):
      strokeIcon(icon, side: side, center: CGPoint(x: frame.midX, y: frame.midY), color: foreground)
    case .space(let label):
      let text = font(size: 13, weight: .regular)
      let labelWidth = (label as NSString).size(withAttributes: [.font: text]).width
      let iconSide: CGFloat = 20
      let gap: CGFloat = 4
      let start = frame.midX - (iconSide + gap + labelWidth) / 2
      strokeIcon(.mic, side: iconSide, center: CGPoint(x: start + iconSide / 2, y: frame.midY), color: skin.secondary)
      drawText(label, font: text, color: skin.secondary, center: CGPoint(x: start + iconSide + gap + labelWidth / 2, y: frame.midY))
    }
    if let hint = key.hint {
      let attributes: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 10), .foregroundColor: skin.secondary]
      let size = (hint as NSString).size(withAttributes: attributes)
      (hint as NSString).draw(at: CGPoint(x: frame.maxX - 5 - size.width, y: frame.minY + 3), withAttributes: attributes)
    }
  }

  private func font(size: CGFloat, weight: UIFont.Weight) -> UIFont {
    (skin.design?.monospaced ?? skin.usesMonospacedFont)
      ? .monospacedSystemFont(ofSize: size, weight: weight)
      : .systemFont(ofSize: size, weight: weight)
  }

  private func drawText(_ text: String, font: UIFont, color: UIColor, center: CGPoint) {
    let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
    let size = (text as NSString).size(withAttributes: attributes)
    (text as NSString).draw(at: CGPoint(x: center.x - size.width / 2, y: center.y - size.height / 2), withAttributes: attributes)
  }

  /// 键盘自带的一个图标，按工具栏和按键画它时的线宽描边。
  private func strokeIcon(_ icon: KeyboardIcon, side: CGFloat, center: CGPoint, color: UIColor) {
    let path = icon.path(in: CGRect(x: center.x - side / 2, y: center.y - side / 2, width: side, height: side))
    path.lineWidth = icon.lineWidth * side / KeyboardIcon.viewBox
    path.lineCapStyle = .round
    path.lineJoinStyle = .round
    color.setStroke()
    path.stroke()
  }
}
