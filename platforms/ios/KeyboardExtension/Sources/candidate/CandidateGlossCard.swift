import UIKit

/// 释义卡片（#10915）里的内容：候选原文、每个目标语言一段译文和它的来源。规则与 Android、HarmonyOS 的卡片相同，按钮和来源名照 issue 写定的文案。
struct CandidateGlossCardContent: Equatable {
  /// 译文从哪里来。离线释义来自随键盘的释义词典（英文 `msime-english.db`，其他语言 `offline-glosses/zh-<语言>.db`），其余是用户在「翻译服务」里选的在线服务。
  enum Source: Equatable {
    case offline, account, tencent, niutrans, custom, ai

    /// 在线译文取自哪一个服务；没有选服务时没有在线译文。
    init?(route: TranslationRoute) {
      switch route {
      case .account: self = .account
      case .tencent: self = .tencent
      case .niutrans: self = .niutrans
      case .custom: self = .custom
      case .ai: self = .ai
      case .none: return nil
      }
    }

    var title: String {
      switch self {
      case .offline: "离线释义"
      case .account: "水杉账号"
      case .tencent: "腾讯翻译"
      case .niutrans: "小牛翻译"
      case .custom: "自定义服务"
      case .ai: "AI 辅助"
      }
    }
  }

  /// 一个目标语言的一段：来源一行和译文。离线释义有几条义项就列几条，在线译文是一整条。
  struct Section: Equatable {
    let language: String
    let source: Source
    let senses: [String]

    var sourceLine: String { "\(language) · \(source.title)" }
  }

  static let title = "释义"
  static let emptyTitle = "暂无译文"
  static let emptyHint = "可以在设置里开启翻译服务"
  static let commitTitle = "上屏译文"
  static let copyTitle = "复制译文"
  static let copyNeedsFullAccess = "开启「允许完全访问」后才能复制译文：没有完全访问时，iOS 不让键盘写入剪贴板"
  static let copied = "已复制译文"

  /// 卡片上显示的候选（已按繁简输出偏好转换）。
  let candidate: String
  let sections: [Section]
  /// 「上屏译文」和「复制译文」用的文字：第一目标语言的第一条译文。第一目标语言还没有译文时为 nil，两个按钮不可用。
  let primaryTranslation: String?
  /// 「暂无译文」后面要不要补一句去设置里开启翻译服务：只在没有可用的在线翻译服务时补，开着服务却还没答上来时这句话不对。与 HarmonyOS 的 `CandidateGlossCardPolicy.showsHint` 同一条规则。
  let showsEmptyHint: Bool

  /// `languages` 按第一、第二目标语言排好，每一项是那种语言当前用的译文（与候选条下面画的同一条，`offline` 说明它来自离线释义）；还没有译文的语言给 nil，卡片上不出那一段。
  init(candidate: String, languages: [(title: String, gloss: (text: String, offline: Bool)?)], route: TranslationRoute,
       serviceAvailable: Bool = false) {
    self.candidate = candidate
    var sections: [Section] = []
    var primary: String?
    for (position, language) in languages.enumerated() {
      guard let gloss = language.gloss else { continue }
      let source: Source
      let senses: [String]
      if gloss.offline {
        source = .offline
        senses = Self.senses(gloss.text)
      } else if let online = Source(route: route) {
        source = online
        let text = gloss.text.trimmingCharacters(in: .whitespacesAndNewlines)
        senses = text.isEmpty ? [] : [text]
      } else {
        continue
      }
      guard !senses.isEmpty else { continue }
      if position == 0 { primary = senses.first }
      sections.append(Section(language: language.title, source: source, senses: senses))
    }
    self.sections = sections
    primaryTranslation = primary
    showsEmptyHint = sections.isEmpty && !serviceAvailable
  }

  /// 离线释义的义项：词典把几条义项写在一行里，以半角或全角分号隔开（多行时每行都是），与 Android 拆可上屏释义（`CandidateTranslationPolicy.insertionGlosses`）的分法相同；去掉空白和重复的。
  static func senses(_ text: String) -> [String] {
    var result: [String] = []
    for line in text.split(whereSeparator: \.isNewline) {
      for part in line.split(whereSeparator: { $0 == ";" || $0 == "；" }) {
        let sense = part.trimmingCharacters(in: .whitespacesAndNewlines)
        if !sense.isEmpty, !result.contains(sense) { result.append(sense) }
      }
    }
    return result
  }
}

/// 卡片是为哪一个候选打开的：候选条上的第几个，或者展开网格里某一代候选的某一项。卡片上的按钮动作之前，键盘按它确认这个候选还在，不把过期的译文写进文档。
struct CandidateGlossCardTarget {
  enum Origin: Equatable {
    /// 候选条上的第 `index` 个；`generation` 是打开时视图的代次，候选条不是会话视图时为 nil。
    case strip(index: Int, generation: UInt64?)
    /// 展开网格里的一项：会话完整候选列表的代次和位置，以及固定排位和高亮（长按菜单的词库操作要用）。
    case panel(generation: UInt64, globalIndex: UInt64, highlighted: Bool, fixedPosition: Int)
  }

  let origin: Origin
  /// 会话里的候选原文（没有经过繁简转换）。
  let word: String
  let source: Int
  /// 会话给这个候选的英文离线释义，释义到达后由键盘更新。
  var offline: String
  /// 候选在不在会话的当前页上。在线翻译只为当前页的候选请求，与 PRIVACY.md 写的范围一致。
  let onCurrentPage: Bool
}

/// 长按候选打开的释义卡片：盖住整块按键区，上面的候选栏和读音行留着。顶部是标题「释义」和收起键，中间是候选原文和各语言的译文，底部是「上屏译文」「复制译文」，原来长按菜单里的操作收进左下角的「更多操作」。
final class CandidateGlossCardView: UIView {
  private let skin: KeyboardTheme
  private let onCommit: () -> Void
  private let onCopy: () -> Void
  private let onClose: () -> Void
  private let titleLabel = UILabel()
  private let candidateLabel = UILabel()
  private let scrollView = UIScrollView()
  private let body = UIStackView()
  private let noticeLabel = UILabel()
  let moreButton: UIButton
  let commitButton: UIButton
  let copyButton: UIButton
  let closeButton: UIButton
  private(set) var content: CandidateGlossCardContent

  init(content: CandidateGlossCardContent, skin: KeyboardTheme,
       onCommit: @escaping () -> Void, onCopy: @escaping () -> Void, onClose: @escaping () -> Void) {
    self.content = content
    self.skin = skin
    self.onCommit = onCommit
    self.onCopy = onCopy
    self.onClose = onClose
    closeButton = Self.iconButton("chevron.down", label: "收起", skin: skin)
    moreButton = Self.iconButton("ellipsis.circle", label: "更多操作", skin: skin)
    copyButton = Self.actionButton(CandidateGlossCardContent.copyTitle, filled: false, skin: skin)
    commitButton = Self.actionButton(CandidateGlossCardContent.commitTitle, filled: true, skin: skin)
    super.init(frame: .zero)
    accessibilityIdentifier = "candidateGlossCard"
    backgroundColor = skin.keyBackground.withAlphaComponent(0.98)

    titleLabel.text = CandidateGlossCardContent.title
    titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)
    titleLabel.textColor = skin.secondary
    titleLabel.accessibilityTraits = .header
    titleLabel.accessibilityIdentifier = "glossCardTitle"
    closeButton.accessibilityIdentifier = "glossCardClose"
    closeButton.addAction(UIAction { [weak self] _ in self?.onClose() }, for: .primaryActionTriggered)
    let header = UIStackView(arrangedSubviews: [titleLabel, UIView(), closeButton])
    header.alignment = .center

    candidateLabel.font = .systemFont(ofSize: 26, weight: .semibold)
    candidateLabel.textColor = skin.keyForeground
    candidateLabel.numberOfLines = 2
    candidateLabel.lineBreakMode = .byTruncatingTail
    candidateLabel.adjustsFontSizeToFitWidth = true
    candidateLabel.minimumScaleFactor = 0.6
    candidateLabel.accessibilityIdentifier = "glossCardCandidate"

    body.axis = .vertical
    body.spacing = 4
    body.translatesAutoresizingMaskIntoConstraints = false
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    scrollView.showsVerticalScrollIndicator = false
    scrollView.alwaysBounceVertical = false
    scrollView.disableEdgeEffects()
    scrollView.contentInsetAdjustmentBehavior = .never
    scrollView.addSubview(body)

    noticeLabel.font = .systemFont(ofSize: 13)
    noticeLabel.textColor = skin.accent
    noticeLabel.numberOfLines = 2
    noticeLabel.isHidden = true
    noticeLabel.accessibilityIdentifier = "glossCardNotice"

    moreButton.accessibilityIdentifier = "glossCardMore"
    moreButton.showsMenuAsPrimaryAction = true
    moreButton.isHidden = true
    copyButton.accessibilityIdentifier = "glossCardCopy"
    copyButton.addAction(UIAction { [weak self] _ in self?.onCopy() }, for: .primaryActionTriggered)
    commitButton.accessibilityIdentifier = "glossCardCommit"
    commitButton.addAction(UIAction { [weak self] _ in self?.onCommit() }, for: .primaryActionTriggered)
    let actions = UIStackView(arrangedSubviews: [moreButton, copyButton, commitButton])
    actions.spacing = 8
    actions.alignment = .fill
    NSLayoutConstraint.activate([
      copyButton.widthAnchor.constraint(equalTo: commitButton.widthAnchor),
      copyButton.heightAnchor.constraint(equalToConstant: 40),
      commitButton.heightAnchor.constraint(equalToConstant: 40),
      moreButton.widthAnchor.constraint(equalToConstant: 40),
    ])

    // 译文少时按钮贴着下沿，中间的空白给 `filler`；译文多时滚动区缩到放得下，`filler` 缩成零。
    let filler = UIView()
    let column = UIStackView(arrangedSubviews: [header, candidateLabel, scrollView, filler, noticeLabel, actions])
    column.axis = .vertical
    column.spacing = 6
    column.setCustomSpacing(2, after: header)
    column.translatesAutoresizingMaskIntoConstraints = false
    addSubview(column)
    NSLayoutConstraint.activate([
      column.topAnchor.constraint(equalTo: topAnchor, constant: 6),
      column.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
      column.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
      column.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
      body.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor),
      body.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor),
      body.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor),
      body.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor),
      body.widthAnchor.constraint(equalTo: scrollView.frameLayoutGuide.widthAnchor),
    ])
    // 译文少时滚动区按内容收起，按钮跟在下面；多时滚动区占满剩下的高度。
    let fit = scrollView.heightAnchor.constraint(equalTo: body.heightAnchor)
    fit.priority = .defaultHigh
    let collapsed = filler.heightAnchor.constraint(equalToConstant: 0)
    collapsed.priority = UILayoutPriority(1)
    NSLayoutConstraint.activate([fit, collapsed, header.heightAnchor.constraint(equalToConstant: 32)])
    for fixed in [candidateLabel, noticeLabel] as [UIView] {
      fixed.setContentHuggingPriority(.required, for: .vertical)
      fixed.setContentCompressionResistancePriority(.required, for: .vertical)
    }
    apply(content)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  /// 换上新的内容（译文到了），或者原来长按菜单里的操作变了；提示行留着。
  func update(content: CandidateGlossCardContent, moreMenu: UIMenu?) {
    if content != self.content {
      self.content = content
      apply(content)
    }
    setMoreMenu(moreMenu)
  }

  /// 原来长按菜单里的操作。没有可做的操作（例如上屏后联想之外的符号候选）时不出这个按钮。
  func setMoreMenu(_ menu: UIMenu?) {
    moreButton.menu = menu
    moreButton.isHidden = menu?.children.isEmpty ?? true
  }

  /// 按钮下方的一行提示，例如复制成功，或者没有完全访问时为什么复制不了。
  func showNotice(_ text: String) {
    noticeLabel.text = text
    noticeLabel.isHidden = false
  }

  var notice: String? { noticeLabel.isHidden ? nil : noticeLabel.text }

  /// 读屏焦点在卡片打开时落到标题上。
  var initialAccessibilityElement: UIView { titleLabel }

  private func apply(_ content: CandidateGlossCardContent) {
    candidateLabel.text = content.candidate
    candidateLabel.accessibilityLabel = "候选 \(content.candidate)"
    for view in body.arrangedSubviews { view.removeFromSuperview() }
    if content.sections.isEmpty {
      let empty = label(CandidateGlossCardContent.emptyTitle, size: 16, color: skin.keyForeground)
      empty.accessibilityIdentifier = "glossCardEmpty"
      let hint = label(CandidateGlossCardContent.emptyHint, size: 13, color: skin.secondary)
      hint.accessibilityIdentifier = "glossCardEmptyHint"
      body.addArrangedSubview(empty)
      if content.showsEmptyHint { body.addArrangedSubview(hint) }
    }
    for (position, section) in content.sections.enumerated() {
      if position > 0 { body.setCustomSpacing(10, after: body.arrangedSubviews[body.arrangedSubviews.count - 1]) }
      let source = label(section.sourceLine, size: 12, color: skin.secondary)
      source.accessibilityIdentifier = "glossCardSource-\(position)"
      source.accessibilityLabel = "\(section.language)，来源：\(section.source.title)"
      body.addArrangedSubview(source)
      for (number, sense) in section.senses.enumerated() {
        let text = section.senses.count > 1 ? "\(number + 1). \(sense)" : sense
        let line = label(text, size: 17, color: skin.keyForeground)
        line.accessibilityIdentifier = "glossCardSense-\(position)-\(number)"
        line.accessibilityLabel = section.senses.count > 1 ? "第 \(number + 1) 条：\(sense)" : sense
        body.addArrangedSubview(line)
      }
    }
    let available = content.primaryTranslation != nil
    commitButton.isEnabled = available
    copyButton.isEnabled = available
    let hint = available ? "第一目标语言的译文：\(content.primaryTranslation ?? "")" : CandidateGlossCardContent.emptyTitle
    commitButton.accessibilityHint = hint
    copyButton.accessibilityHint = hint
  }

  private func label(_ text: String, size: CGFloat, color: UIColor) -> UILabel {
    let label = UILabel()
    label.text = text
    label.font = .systemFont(ofSize: size)
    label.textColor = color
    label.numberOfLines = 0
    return label
  }

  private static func iconButton(_ symbol: String, label: String, skin: KeyboardTheme) -> UIButton {
    var configuration = UIButton.Configuration.plain()
    configuration.image = UIImage(systemName: symbol, withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .medium))
    configuration.baseForegroundColor = skin.keyForeground
    configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 6, bottom: 6, trailing: 6)
    let button = UIButton(configuration: configuration)
    button.accessibilityLabel = label
    NSLayoutConstraint.activate([
      button.widthAnchor.constraint(greaterThanOrEqualToConstant: 36),
      button.heightAnchor.constraint(greaterThanOrEqualToConstant: 32),
    ])
    return button
  }

  private static func actionButton(_ title: String, filled: Bool, skin: KeyboardTheme) -> UIButton {
    var configuration = filled ? UIButton.Configuration.filled() : UIButton.Configuration.plain()
    configuration.attributedTitle = AttributedString(title, attributes: AttributeContainer([
      .font: UIFont.systemFont(ofSize: 15, weight: .semibold),
    ]))
    configuration.baseBackgroundColor = filled ? skin.accent : skin.functionKeyBackground
    configuration.baseForegroundColor = filled ? skin.onAccent : skin.keyForeground
    configuration.background.backgroundColor = filled ? skin.accent : skin.functionKeyBackground
    configuration.background.cornerRadius = 10
    configuration.cornerStyle = .fixed
    let button = UIButton(configuration: configuration)
    button.configurationUpdateHandler = { button in
      button.alpha = button.isEnabled ? 1 : 0.45
    }
    return button
  }
}
