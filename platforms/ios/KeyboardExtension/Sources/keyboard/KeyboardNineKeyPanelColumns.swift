import UIKit

/// 全拼九键展开候选面板的左右两栏，中间是 `KeyboardCandidatePanelView` 的候选网格。
///
/// 左栏平时是拼音栏（与键区旁的拼音栏同一组选项：完整音节、下一个数字键上的大写字母、数字本身），切到笔画后换成五个笔画键，顶上显示已经按下的笔画。右栏是功能键：返回、退格、重输、拼音/笔画、全部/单字。这里只负责画和把按键交出去，状态都由键盘控制器从 Engine 的快照里取来再交给 `update`。
@MainActor
final class KeyboardNineKeyPanelColumns {
  enum Action: Equatable {
    case spelling(Int)
    case stroke(String)
    case back, delete, clear, toggleStrokes, toggleSingleCharacter
  }

  /// 筛选能用的五种笔画（不含通配，Engine 的筛选不认它）。
  static let strokeKeys = StrokeKeyLayout.rows.joined().filter { $0.ascii != StrokeKeyLayout.wildcard }

  /// 拼音栏里一项的无障碍名称。Engine 在同一列里给三种东西：完整音节（小写）、限定下一个音节首字母的字母（大写）、直接上屏的数字，读出来要分得清。
  static func spellingAccessibilityLabel(_ spelling: String) -> String {
    if spelling.count == 1, let character = spelling.first {
      if character.isNumber { return "输入数字 \(spelling)" }
      if character.isUppercase { return "选择字母 \(spelling)" }
    }
    return "选择拼音 \(spelling)"
  }

  let leading = UIView()
  let trailing = UIStackView()
  var onAction: ((Action) -> Void)?

  private let spellingScrollView = UIScrollView()
  private let spellingStack = UIStackView()
  private var spellingButtons: [UIButton] = []
  private let strokeColumn = UIStackView()
  private let strokePrefix = UILabel()
  private var strokeToggle: UIButton!
  private var singleToggle: UIButton!

  /// `makeKey` 造一个与键区同样外观的功能键：标题、无障碍名称、按下动作。
  init(makeKey: (String, String, @escaping () -> Void) -> UIButton) {
    leading.accessibilityIdentifier = "candidatePanelSpellingColumn"
    leading.backgroundColor = KeyboardTheme.current.keyBackground.withAlphaComponent(0.5)
    leading.layer.cornerRadius = 8
    leading.clipsToBounds = true

    spellingScrollView.showsVerticalScrollIndicator = false
    spellingScrollView.disableEdgeEffects()
    spellingStack.axis = .vertical
    spellingStack.spacing = 6
    spellingStack.translatesAutoresizingMaskIntoConstraints = false
    spellingScrollView.addSubview(spellingStack)

    strokePrefix.font = .systemFont(ofSize: 15, weight: .medium)
    strokePrefix.textAlignment = .center
    strokePrefix.adjustsFontSizeToFitWidth = true
    strokePrefix.minimumScaleFactor = 0.5
    strokePrefix.lineBreakMode = .byTruncatingHead
    strokePrefix.textColor = KeyboardTheme.current.accent
    strokePrefix.accessibilityIdentifier = "candidatePanelStrokePrefix"
    strokeColumn.axis = .vertical
    strokeColumn.spacing = 4
    strokeColumn.distribution = .fillEqually
    strokeColumn.addArrangedSubview(strokePrefix)
    for key in Self.strokeKeys {
      let button = makeKey(key.face, "笔画 \(key.name)") { [weak self] in self?.onAction?(.stroke(key.ascii)) }
      button.configuration?.contentInsets = .zero
      button.accessibilityIdentifier = "candidatePanelStroke_\(key.ascii)"
      strokeColumn.addArrangedSubview(button)
    }

    for content in [spellingScrollView, strokeColumn] as [UIView] {
      content.translatesAutoresizingMaskIntoConstraints = false
      leading.addSubview(content)
      NSLayoutConstraint.activate([
        content.leadingAnchor.constraint(equalTo: leading.leadingAnchor, constant: 2),
        content.trailingAnchor.constraint(equalTo: leading.trailingAnchor, constant: -2),
        content.topAnchor.constraint(equalTo: leading.topAnchor, constant: 2),
        content.bottomAnchor.constraint(equalTo: leading.bottomAnchor, constant: -2),
      ])
    }
    NSLayoutConstraint.activate([
      spellingStack.leadingAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.leadingAnchor),
      spellingStack.trailingAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.trailingAnchor),
      spellingStack.topAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.topAnchor),
      spellingStack.bottomAnchor.constraint(equalTo: spellingScrollView.contentLayoutGuide.bottomAnchor),
      spellingStack.widthAnchor.constraint(equalTo: spellingScrollView.frameLayoutGuide.widthAnchor),
    ])

    trailing.axis = .vertical
    trailing.spacing = 6
    trailing.distribution = .fillEqually
    trailing.accessibilityIdentifier = "candidatePanelFunctionColumn"
    let functions: [(String, String, String, Action)] = [
      ("返回", "收起候选", "candidatePanelBack", .back),
      ("⌫", "删除", "candidatePanelDelete", .delete),
      ("重输", "清空重新输入", "candidatePanelClear", .clear),
      ("笔画", "按笔画筛选", "candidatePanelStrokeToggle", .toggleStrokes),
      ("单字", "只显示单字", "candidatePanelSingleToggle", .toggleSingleCharacter),
    ]
    for (title, label, identifier, action) in functions {
      let button = makeKey(title, label) { [weak self] in self?.onAction?(action) }
      button.configuration?.contentInsets = .zero
      button.titleLabel?.adjustsFontSizeToFitWidth = true
      button.accessibilityIdentifier = identifier
      trailing.addArrangedSubview(button)
      if action == .toggleStrokes { strokeToggle = button }
      if action == .toggleSingleCharacter { singleToggle = button }
    }
  }

  /// 按 Engine 当前的状态重画两栏。`strokesAvailable` 为假时（这个版本没带笔画词库）不提供拼音/笔画切换。
  func update(spellings: [String], strokeMode: Bool, strokes: String, singleCharacter: Bool,
              strokesAvailable: Bool) {
    let skin = KeyboardTheme.current
    spellingScrollView.isHidden = strokeMode
    strokeColumn.isHidden = !strokeMode
    if strokeMode {
      let glyphs = strokes.compactMap { StrokeKeyLayout.keycap(for: String($0)) }.joined()
      strokePrefix.text = glyphs.isEmpty ? "笔画" : glyphs
      strokePrefix.textColor = glyphs.isEmpty ? skin.keyForeground.withAlphaComponent(0.5) : skin.accent
      strokePrefix.accessibilityLabel = glyphs.isEmpty
        ? "还没有笔画"
        : "已选笔画 " + strokes.compactMap { key in Self.strokeKeys.first { $0.ascii == String(key) }?.name }.joined(separator: " ")
    } else {
      updateSpellings(spellings)
    }

    strokeToggle.isHidden = !strokesAvailable
    strokeToggle.configuration?.title = strokeMode ? "拼音" : "笔画"
    strokeToggle.accessibilityLabel = strokeMode ? "回到拼音" : "按笔画筛选"
    strokeToggle.configuration?.baseForegroundColor = strokeMode ? skin.accent : skin.keyForeground
    strokeToggle.isSelected = strokeMode
    singleToggle.configuration?.title = singleCharacter ? "全部" : "单字"
    singleToggle.accessibilityLabel = singleCharacter ? "显示全部候选" : "只显示单字"
    singleToggle.configuration?.baseForegroundColor = singleCharacter ? skin.accent : skin.keyForeground
    singleToggle.isSelected = singleCharacter
  }

  private func updateSpellings(_ spellings: [String]) {
    while spellingButtons.count < spellings.count {
      let button = UIButton(type: .system)
      button.addAction(UIAction { [weak self, weak button] _ in
        guard let self, let button else { return }
        onAction?(.spelling(button.tag))
      }, for: .primaryActionTriggered)
      spellingButtons.append(button)
      spellingStack.addArrangedSubview(button)
    }
    for (index, button) in spellingButtons.enumerated() {
      guard spellings.indices.contains(index) else {
        button.isHidden = true
        continue
      }
      let spelling = spellings[index]
      // 与键区旁的拼音栏一样：透明底、键面文字色，强调色留给读音和选中的候选。
      var configuration = UIButton.Configuration.plain()
      configuration.title = spelling
      configuration.titleLineBreakMode = .byClipping
      configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 2, bottom: 6, trailing: 2)
      configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
        var attributes = attributes
        attributes.font = .systemFont(ofSize: 15)
        return attributes
      }
      configuration.baseForegroundColor = KeyboardTheme.current.keyForeground
      button.configuration = configuration
      button.tag = index
      button.isHidden = false
      button.accessibilityLabel = Self.spellingAccessibilityLabel(spelling)
      button.accessibilityIdentifier = "candidatePanelSpelling_\(spelling)"
    }
    spellingScrollView.setContentOffset(.zero, animated: false)
  }
}
