import UIKit

/// 工具栏的常用语面板：在按键区用普通列表列出不带编码的常用语（`CommonPhrasesBridge`），点一下发送一条。
///
/// 列表由键盘在主线程之外读取后传进来，这个视图只负责绘制。行样式按设计稿：内边距 12/8，15pt 文字单行截断，每行下方一条细线，按下时不透明度 .55，外层是内边距 6/2/2 的纵向滚动视图。没有常用语时显示提示并提供「添加常用语」，键盘会把它转到 App 的常用语页面，因为键盘自己只列出已存的内容。
final class KeyboardPhrasesView: UIView {
  /// 面板为空时的提示，以及存储读不出来时的提示。
  static let emptyMessage = "还没有常用语"
  static let failureMessage = "常用语读取失败"

  private let phrases: [CommonPhrasesBridge.Phrase]
  private let skin: KeyboardTheme
  private let onInsert: (String) -> Void
  private let onAddPhrase: () -> Void
  private let scrollView = UIScrollView()
  private let list = UIStackView()

  /// 存储读不出来时，`loadFailed` 用 `failureMessage` 代替空状态，免得把读不出来的列表误当成空列表。
  init(phrases: [CommonPhrasesBridge.Phrase], skin: KeyboardTheme, loadFailed: Bool = false,
       onInsert: @escaping (String) -> Void, onAddPhrase: @escaping () -> Void) {
    self.phrases = phrases.filter { !$0.text.isEmpty }
    self.skin = skin
    self.onInsert = onInsert
    self.onAddPhrase = onAddPhrase
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardPhrasesPanel"
    backgroundColor = .clear

    scrollView.translatesAutoresizingMaskIntoConstraints = false
    scrollView.showsVerticalScrollIndicator = false
    scrollView.alwaysBounceVertical = false
    scrollView.disableEdgeEffects()
    scrollView.contentInsetAdjustmentBehavior = .never
    addSubview(scrollView)
    list.axis = .vertical
    list.translatesAutoresizingMaskIntoConstraints = false
    scrollView.addSubview(list)
    NSLayoutConstraint.activate([
      scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: topAnchor),
      scrollView.bottomAnchor.constraint(equalTo: bottomAnchor),
      list.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor, constant: 6),
      list.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor, constant: -2),
      list.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor, constant: 2),
      list.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor, constant: -2),
      list.widthAnchor.constraint(equalTo: scrollView.frameLayoutGuide.widthAnchor, constant: -4),
    ])

    if self.phrases.isEmpty {
      showEmptyState(failed: loadFailed)
    } else {
      for (index, phrase) in self.phrases.enumerated() { list.addArrangedSubview(row(phrase, index: index)) }
    }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  private func row(_ phrase: CommonPhrasesBridge.Phrase, index: Int) -> UIView {
    let row = PhraseRowButton(frame: .zero)
    // 常用语可能有多行；行里只显示成一行，发送的是完整文本。
    row.text = phrase.text.replacingOccurrences(of: "\r\n", with: " ").replacingOccurrences(of: "\n", with: " ")
    row.textColor = skin.keyForeground
    row.hairlineColor = skin.hairline
    row.accessibilityIdentifier = "phraseRow-\(index)"
    row.accessibilityLabel = "常用语 \(phrase.text.prefix(20))"
    row.addAction(UIAction { [weak self] _ in self?.onInsert(phrase.text) }, for: .primaryActionTriggered)
    return row
  }

  /// 面板中央的提示文字；空状态的提示下面还有一个强调色的「添加常用语」文字按钮。
  private func showEmptyState(failed: Bool) {
    let note = UILabel()
    note.text = failed ? Self.failureMessage : Self.emptyMessage
    note.font = .systemFont(ofSize: 14)
    note.textColor = skin.secondary
    note.textAlignment = .center
    note.numberOfLines = 0
    note.accessibilityIdentifier = "phrasesEmptyNote"

    let empty = UIStackView(arrangedSubviews: [note])
    empty.axis = .vertical
    empty.alignment = .center
    empty.spacing = 8
    if !failed {
      var configuration = UIButton.Configuration.plain()
      configuration.attributedTitle = AttributedString("添加常用语", attributes: AttributeContainer([
        .font: UIFont.systemFont(ofSize: 15, weight: .medium),
      ]))
      configuration.baseForegroundColor = skin.accent
      configuration.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 12)
      let add = UIButton(configuration: configuration, primaryAction: UIAction { [weak self] _ in self?.onAddPhrase() })
      add.accessibilityIdentifier = "addCommonPhrase"
      empty.addArrangedSubview(add)
    }
    empty.translatesAutoresizingMaskIntoConstraints = false
    addSubview(empty)
    NSLayoutConstraint.activate([
      empty.centerXAnchor.constraint(equalTo: centerXAnchor),
      empty.centerYAnchor.constraint(equalTo: centerYAnchor),
      empty.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 12),
      empty.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -12),
    ])
  }
}

/// 一条常用语：15pt 单行截断，上下各 12pt、左右各 8pt，底部一条 1pt 细线（设计稿的 `inset 0 -1px 0 kbHair`），按下时不透明度 .55。
private final class PhraseRowButton: UIButton {
  private let label = UILabel()
  private let hairline = UIView()

  var text: String? {
    get { label.text }
    set { label.text = newValue }
  }
  var textColor: UIColor? {
    get { label.textColor }
    set { label.textColor = newValue }
  }
  var hairlineColor: UIColor? {
    get { hairline.backgroundColor }
    set { hairline.backgroundColor = newValue }
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    label.font = .systemFont(ofSize: 15)
    label.numberOfLines = 1
    label.lineBreakMode = .byTruncatingTail
    label.isUserInteractionEnabled = false
    hairline.isUserInteractionEnabled = false
    for child in [label, hairline] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    NSLayoutConstraint.activate([
      label.topAnchor.constraint(equalTo: topAnchor, constant: 12),
      label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
      label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      hairline.leadingAnchor.constraint(equalTo: leadingAnchor),
      hairline.trailingAnchor.constraint(equalTo: trailingAnchor),
      hairline.bottomAnchor.constraint(equalTo: bottomAnchor),
      hairline.heightAnchor.constraint(equalToConstant: 1),
    ])
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  override var isHighlighted: Bool {
    didSet { alpha = isHighlighted ? 0.55 : 1 }
  }
}
