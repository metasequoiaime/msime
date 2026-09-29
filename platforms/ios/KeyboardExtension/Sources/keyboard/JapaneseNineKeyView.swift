import UIKit

/// Physical key labels and keystrokes only; composition and conversion stay in Engine.
@MainActor
final class JapaneseNineKeyView: UIStackView {
  struct Key {
    let kana: [String]
    let strokes: [String]
  }
  static let keys: [Key] = [
    Key(kana: ["あ", "い", "う", "え", "お"], strokes: ["a", "i", "u", "e", "o"]),
    Key(kana: ["か", "き", "く", "け", "こ"], strokes: ["ka", "ki", "ku", "ke", "ko"]),
    Key(kana: ["さ", "し", "す", "せ", "そ"], strokes: ["sa", "shi", "su", "se", "so"]),
    Key(kana: ["た", "ち", "つ", "て", "と"], strokes: ["ta", "chi", "tsu", "te", "to"]),
    Key(kana: ["な", "に", "ぬ", "ね", "の"], strokes: ["na", "ni", "nu", "ne", "no"]),
    Key(kana: ["は", "ひ", "ふ", "へ", "ほ"], strokes: ["ha", "hi", "fu", "he", "ho"]),
    Key(kana: ["ま", "み", "む", "め", "も"], strokes: ["ma", "mi", "mu", "me", "mo"]),
    Key(kana: ["や", "「", "ゆ", "」", "よ"], strokes: ["ya", "", "yu", "", "yo"]),
    Key(kana: ["ら", "り", "る", "れ", "ろ"], strokes: ["ra", "ri", "ru", "re", "ro"]),
    // Long vowel mark is a real romaji stroke. Sending it as a direct symbol commits the
    // composition and produces a detached dash in words such as ラーメン.
    Key(kana: ["わ", "を", "ん", "ー", "〜"], strokes: ["wa", "wo", "n'", "-", ""]),
    Key(kana: ["、", "。", "？", "！", "…"], strokes: ["", "", "", "", ""]),
  ]
  /// The Japanese nine-key keeps its grid when switching away from kana. Empty strokes are
  /// deliberate: these symbols go straight to the host rather than through the romanization engine.
  static let digitKeys: [Key] = [
    Key(kana: ["1", "☆", "♪", "→", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["2", "¥", "$", "€", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["3", "%", "°", "#", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["4", "○", "*", "・", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["5", "+", "-", "=", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["6", "<", "^", ">", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["7", "「", "」", "：", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["8", "〒", "※", "♂", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["9", "（", "）", "／", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["0", "〜", "…", "ー", ""], strokes: ["", "", "", "", ""]),
    Key(kana: ["、", "。", "？", "！", "…"], strokes: ["", "", "", "", ""]),
  ]
  var onInput: ((String) -> Void)?
  var onSymbol: ((String) -> Void)?
  var onDelete: (() -> Void)?
  var onVariant: (() -> Void)?
  private var rows: [UIStackView] = []
  private var keyButtons: [UIButton] = []
  private var variantsKey: UIButton?
  private var scriptKeyOneRow: NSLayoutConstraint?
  private var scriptKeyTwoRows: NSLayoutConstraint?
  private var showsDigits = false
  private var isComposing = false
  private var activeKeys: [Key] { showsDigits ? Self.digitKeys : Self.keys }
  private let flickPreview = KanaFlickPreview()

  init(makeKey: (String, String, @escaping () -> Void) -> UIButton,
       makeDelete: (() -> UIButton)? = nil,
       sideKeys: [UIButton] = [],
       modeKeys: [UIButton] = []) {
    super.init(frame: .zero)
    axis = .horizontal
    spacing = 6
    accessibilityIdentifier = "japaneseNineKey"
    if !modeKeys.isEmpty {
      let modes = UIStackView()
      modes.axis = .vertical; modes.distribution = .fill; modes.spacing = 7
      modes.accessibilityIdentifier = "japaneseModeColumn"
      for key in modeKeys { modes.addArrangedSubview(key) }
      addArrangedSubview(modes)
      // The width constraint relates two views, so it can only be activated once both share an
      // ancestor. Activating it before the column joins the stack aborts the whole extension.
      modes.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.17).isActive = true

      // The mode column fills the four-row kana grid. ABC spans two rows when the system draws
      // its own globe key below the keyboard; when the globe belongs to us, all four keys use one
      // row each.
      //
      // The heights reference only this column, never a kana row: a constraint that spans the two
      // sibling stacks looks equivalent and is not, because the engine settles it by handing every
      // row the whole panel height.
      //
      // They also reference only each other, never the column. Writing each key as
      // `modes.heightAnchor × span/4` plus a constant computes the right number and is a cycle:
      // the child's height references the parent stack, and a `.fill` stack's height is decided by
      // its children. The iOS 26 solver happened to settle on that single answer and iOS 27 does
      // not - 123 came out 2.33pt taller than ^_^, and the three keys plus their gaps overflowed
      // the column by 2pt. Saying only that the one-row keys match each other, and that the
      // two-row key is two of them plus the gap between, leaves the total to the stack's own fill
      // and removes the cycle.
      //
      // Priority stops one short of required: UIStackView compresses a hidden arranged subview to
      // zero height at required priority, so two required constraints collide the moment the host
      // draws its own globe.
      let relate = { (constraint: NSLayoutConstraint) in
        constraint.priority = .required - 1
        constraint.isActive = true
      }
      let singleSpanKeys = modeKeys.enumerated().filter { $0.offset != 2 }.map(\.element)
      for key in singleSpanKeys.dropFirst() {
        relate(key.heightAnchor.constraint(equalTo: singleSpanKeys[0].heightAnchor))
      }
      let row = { (span: CGFloat) -> NSLayoutConstraint? in
        guard modeKeys.indices.contains(2), let reference = singleSpanKeys.first else { return nil }
        let constraint = modeKeys[2].heightAnchor.constraint(
          equalTo: reference.heightAnchor, multiplier: span,
          constant: span == 2 ? modes.spacing : 0)
        constraint.priority = .required - 1
        return constraint
      }
      scriptKeyOneRow = row(1)
      scriptKeyTwoRows = row(2)
      scriptKeyTwoRows?.isActive = true
    }
    let grid = UIStackView()
    grid.axis = .vertical; grid.distribution = .fillEqually; grid.spacing = 7
    addArrangedSubview(grid)
    for rowIndex in 0..<3 {
      let row = UIStackView(); row.distribution = .fillEqually; row.spacing = 6
      rows.append(row); grid.addArrangedSubview(row)
      for column in 0..<3 {
        let index = rowIndex * 3 + column
        row.addArrangedSubview(makeKanaKey(index, factory: makeKey))
      }
    }
    let variants = makeKey("小゛゜", "小假名、浊音和半浊音", { [weak self] in
      guard let self else { return }
      // Mirrors the enabled state set in setComposing: outside the digit layer the modifier only
      // means something while a composition is open, and a no-op tap must not reach the Engine.
      if showsDigits { onSymbol?("（") }
      else if isComposing { onVariant?() }
    })
    variantsKey = variants
    variants.accessibilityIdentifier = "japaneseVariants"
    variants.configuration?.contentInsets = .zero
    variants.titleLabel?.adjustsFontSizeToFitWidth = true
    variants.titleLabel?.minimumScaleFactor = 0.6
    variants.isEnabled = false

    // The fourth row keeps Japanese punctuation on the nine-key surface instead of hiding it
    // behind the general symbol page. The modifier occupies the first cell, followed by わ and
    // the sentence-ending punctuation key.
    let fourth = UIStackView()
    fourth.distribution = .fillEqually; fourth.spacing = 6
    rows.append(fourth); grid.addArrangedSubview(fourth)
    fourth.addArrangedSubview(variants)
    fourth.addArrangedSubview(makeKanaKey(9, factory: makeKey))
    fourth.addArrangedSubview(makeKanaKey(10, factory: makeKey))

    let side = UIStackView()
    side.axis = .vertical; side.distribution = .fillEqually; side.spacing = 7
    side.accessibilityIdentifier = "japaneseSideColumn"
    rows.append(side)
    addArrangedSubview(side)
    side.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.19).isActive = true
    let delete = makeDelete?() ?? makeKey("⌫", "删除", { [weak self] in self?.onDelete?() })
    delete.accessibilityIdentifier = "japaneseDelete"
    side.addArrangedSubview(delete)
    for key in sideKeys { side.addArrangedSubview(key) }
    if let firstKanaRow = grid.arrangedSubviews.first {
      delete.heightAnchor.constraint(equalTo: firstKanaRow.heightAnchor).isActive = true
      for (index, key) in sideKeys.enumerated() {
        key.heightAnchor.constraint(
          equalTo: firstKanaRow.heightAnchor,
          multiplier: index == sideKeys.count - 1 ? 2 : 1,
          constant: index == sideKeys.count - 1 ? 7 : 0).isActive = true
      }
    }
  }
  required init(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  private func makeKanaKey(_ index: Int, factory: (String, String, @escaping () -> Void) -> UIButton) -> UIButton {
    let key = Self.keys[index]
    let button = factory(key.kana[0], key.kana.joined(separator: "、"), { [weak self] in self?.select(index, direction: 0) })
    button.accessibilityIdentifier = "japaneseKana\(index)"
    button.accessibilityHint = "轻点输入\(key.kana[0])；左、上、右、下滑动选择其他假名；长按显示全部选项"
    button.configuration?.subtitle = key.kana.dropFirst().joined(separator: " ")
    button.configuration?.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer {
      var attributes = $0; attributes.font = .systemFont(ofSize: 10); return attributes
    }
    button.configuration?.contentInsets = .init(top: 2, leading: 0, bottom: 2, trailing: 0)
    // Flick preview is the selection surface for kana; a long-press menu duplicates the same
    // five choices and competes with the gesture that begins on touch down.
    button.menu = nil
    let pan = KanaFlickGesture { [weak self, weak button] direction, phase in
      guard let self else { return }
      let active = self.activeKeys[index]
      switch phase {
      case .moving:
        self.flickPreview.show(active.kana, highlighting: direction, over: button ?? self, in: self)
        button?.configuration?.title = active.kana[direction]
      case .ended:
        self.select(index, direction: direction)
        self.flickPreview.hide()
        button?.configuration?.title = active.kana[0]
      case .cancelled:
        self.flickPreview.hide()
        button?.configuration?.title = active.kana[0]
      }
    }
    button.addGestureRecognizer(pan)
    keyButtons.append(button)
    return button
  }
  func select(_ index: Int, direction: Int) {
    let keys = activeKeys
    guard keys.indices.contains(index), keys[index].kana.indices.contains(direction) else { return }
    let key = keys[index]
    guard !key.kana[direction].isEmpty else { return }
    if key.strokes[direction].isEmpty { onSymbol?(key.kana[direction]) }
    else { onInput?(key.strokes[direction]) }
  }
  /// Switches the same physical grid between kana and the Japanese numeric/symbol layer.
  func setDigits(_ enabled: Bool) {
    guard enabled != showsDigits else { return }
    showsDigits = enabled
    for (index, button) in keyButtons.enumerated() where activeKeys.indices.contains(index) {
      let key = activeKeys[index]
      button.configuration?.title = key.kana[0]
      button.configuration?.subtitle = key.kana.dropFirst().filter { !$0.isEmpty }.joined(separator: " ")
      button.accessibilityLabel = key.kana.filter { !$0.isEmpty }.joined(separator: "、")
      button.menu = enabled
        ? UIMenu(children: key.kana.enumerated().compactMap { direction, kana in
          guard !kana.isEmpty else { return nil }
          return UIAction(title: kana) { [weak self] _ in self?.select(index, direction: direction) }
        })
        : nil
    }
    guard let variantsKey else { return }
    variantsKey.configuration?.title = enabled ? "（）" : "小゛゜"
    variantsKey.accessibilityLabel = enabled ? "括弧" : "小假名、浊音和半浊音"
    variantsKey.menu = enabled
      ? UIMenu(children: ["（", "）", "「", "」", "『", "』", "【", "】"].map { symbol in
        UIAction(title: symbol) { [weak self] _ in self?.onSymbol?(symbol) }
      })
      : nil
    variantsKey.showsMenuAsPrimaryAction = enabled
    variantsKey.isEnabled = enabled || isComposing
  }
  /// The modifier only has an effect while the Engine has a completed kana at the end of the
  /// composition. Keeping the key visible but disabled makes that state discoverable without
  /// allowing a no-op tap to steal the user's input click.
  func setComposing(_ composing: Bool) {
    isComposing = composing
    variantsKey?.isEnabled = showsDigits || composing
  }

  /// The system-owned globe key is rendered below the extension. Without it, ABC spans two rows
  /// to fill the empty space; with it, the mode column has four one-row keys.
  func setModeColumnFull(_ full: Bool) {
    scriptKeyTwoRows?.isActive = !full
    scriptKeyOneRow?.isActive = full
  }

  func applyLayout() {
    let layout = KeyboardLayoutPreference.geometry
    spacing = layout.keySpacing
    for row in rows { row.spacing = row.axis == .vertical ? layout.rowSpacing : layout.keySpacing }
    (arrangedSubviews.first as? UIStackView)?.spacing = layout.rowSpacing
  }
}

@MainActor
private final class KanaFlickChip: UIView {
  let label = UILabel()

  override init(frame: CGRect) {
    super.init(frame: frame)
    layer.cornerRadius = 8
    layer.shadowColor = UIColor.black.cgColor
    layer.shadowOpacity = 0.25
    layer.shadowRadius = 6
    layer.shadowOffset = CGSize(width: 0, height: 2)
    label.textAlignment = .center
    label.font = .systemFont(ofSize: 22, weight: .regular)
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.6
    label.translatesAutoresizingMaskIntoConstraints = false
    addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
      label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
      label.topAnchor.constraint(equalTo: topAnchor),
      label.bottomAnchor.constraint(equalTo: bottomAnchor),
    ])
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
}

@MainActor
private final class KanaFlickPreview: UIView {
  private static let offsets: [CGPoint] = [
    CGPoint(x: 0, y: 0), CGPoint(x: -1, y: 0), CGPoint(x: 0, y: -1), CGPoint(x: 1, y: 0),
    CGPoint(x: 0, y: 1),
  ]
  private static let gap: CGFloat = 6
  private let chips: [KanaFlickChip] = (0..<5).map { _ in KanaFlickChip(frame: .zero) }
  private var cell = CGSize(width: 44, height: 44)

  override init(frame: CGRect) {
    super.init(frame: frame)
    isUserInteractionEnabled = false
    isHidden = true
    accessibilityIdentifier = "japaneseFlickPreview"
    backgroundColor = .clear
    chips.forEach(addSubview)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func layoutSubviews() {
    super.layoutSubviews()
    let middle = CGPoint(x: bounds.midX, y: bounds.midY)
    let step = CGSize(width: cell.width + Self.gap, height: cell.height + Self.gap)
    for (index, chip) in chips.enumerated() {
      let offset = Self.offsets[index]
      chip.frame = CGRect(
        x: middle.x + offset.x * step.width - cell.width / 2,
        y: middle.y + offset.y * step.height - cell.height / 2,
        width: cell.width,
        height: cell.height)
    }
  }

  func show(_ kana: [String], highlighting direction: Int, over key: UIView, in host: UIView) {
    let skin = KeyboardTheme.current
    for (index, chip) in chips.enumerated() {
      let text = index < kana.count ? kana[index] : ""
      chip.label.text = text
      chip.isHidden = text.isEmpty
      chip.label.textColor = index == direction ? skin.actionForeground : skin.keyForeground
      chip.backgroundColor = index == direction ? skin.accent : skin.keyBackground
    }
    var canvas = host
    while let parent = canvas.superview { canvas = parent }
    if superview !== canvas { canvas.addSubview(self) }
    canvas.bringSubviewToFront(self)
    translatesAutoresizingMaskIntoConstraints = true
    cell = CGSize(width: max(key.bounds.width, 40), height: max(key.bounds.height, 36))
    let step = CGSize(width: cell.width + Self.gap, height: cell.height + Self.gap)
    bounds = CGRect(origin: .zero, size: CGSize(width: step.width * 3, height: step.height * 3))
    center = key.convert(CGPoint(x: key.bounds.midX, y: key.bounds.midY), to: canvas)
    setNeedsLayout()
    isHidden = false
  }

  func hide() { isHidden = true }
}

@MainActor
private final class KanaFlickGesture: UIPanGestureRecognizer {
  enum Phase { case moving, ended, cancelled }
  private let feedback: (Int, Phase) -> Void
  init(feedback: @escaping (Int, Phase) -> Void) {
    self.feedback = feedback
    super.init(target: nil, action: nil)
    addTarget(self, action: #selector(update))
    maximumNumberOfTouches = 1
    cancelsTouchesInView = true
  }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
    super.touchesBegan(touches, with: event)
    feedback(0, .moving)
  }

  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
    super.touchesEnded(touches, with: event)
    if state == .possible || state == .failed { feedback(0, .cancelled) }
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
    super.touchesCancelled(touches, with: event)
    if state == .possible || state == .failed { feedback(0, .cancelled) }
  }

  @objc private func update() {
    let offset = translation(in: view)
    let direction: Int
    if max(abs(offset.x), abs(offset.y)) < 12 { direction = 0 }
    else if abs(offset.x) > abs(offset.y) { direction = offset.x < 0 ? 1 : 3 }
    else { direction = offset.y < 0 ? 2 : 4 }
    if state == .cancelled || state == .failed { feedback(0, .cancelled) }
    else { feedback(direction, state == .ended ? .ended : .moving) }
  }
}
