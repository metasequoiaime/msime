import UIKit

/// Apple-style category browser backed by the shared paged Emoji catalog, with the kaomoji catalog as its last tab.
///
/// A kaomoji is a line of text, not a pictograph, so its tab lays out as many columns as fit its width: two on a phone, more on an iPad. Kaomoji stay out of 最近, whose eight-column grid is sized for Emoji; the ones used go to a 最近颜文字 tab just before 颜文字, laid out like it, which appears once there is something in it.
///
/// Search swaps the category bar for a letter pad of the picker's own. A keyboard extension has no text field it can type into, and the Engine matches Emoji and kaomoji by pinyin and English keywords, both of which are letters, so the pad types the query and the grid above it shows the matches. While searching, the category bar's place holds two scopes, 表情 and 颜文字, like the separate Emoji and kaomoji results of the Windows panel's search; the search opens on 颜文字 from the kaomoji tab and on 表情 from any other.
///
/// 带标题栏时（默认）选择器盖住整个键盘：一行带返回、搜索和删除的标题，下面是分类栏和网格。不带标题栏时（`showsHeader: false`）选择器放在按键区里，即设计稿的表情面板：网格在上，底部是一条 ABC | 分类签 | ⌫ 的栏，搜索是第一个分类签。
final class KeyboardEmojiPickerView: UIView, UICollectionViewDataSource, UICollectionViewDelegate {
  typealias PageLoader = @Sendable (
    KeyboardEmojiCatalog.Category, Int
  ) throws -> KeyboardEmojiCatalog.Page

  private enum Tab: Equatable {
    case recent
    case recentKaomoji
    case category(KeyboardEmojiCatalog.Category)

    var title: String {
      switch self {
      case .recent: return "最近"
      case .recentKaomoji: return "最近颜文字"
      case .category(let category): return category.title
      }
    }
  }

  private let onInsert: (String) -> Void
  private let onDelete: () -> Void
  private let onCatalogChange: (() -> Void)?
  private let loader: PageLoader
  private let loadQueue = DispatchQueue(label: "app.msime.ios.emoji-catalog", qos: .userInitiated)
  private let tabs = UIStackView()
  private let tabScroll = UIScrollView()
  private let status = UILabel()
  private lazy var grid = UICollectionView(frame: .zero, collectionViewLayout: makeLayout())
  private var tabButtons: [UIButton] = []
  private var availableTabs: [Tab]
  private var selectedTab = -1
  private var items: [KeyboardEmojiCatalog.Item] = []
  private var nextOffset = 0
  private var complete = false
  private var loading = false
  private var loadGeneration: UInt64 = 0
  private let skin = KeyboardTheme.current
  private let titleLabel = UILabel()
  private let letterPad = UIStackView()
  /// The letters typed so far while searching, `nil` while browsing categories.
  private(set) var searchQuery: String?
  /// Whether the open search looks through the kaomoji catalog rather than the Emoji groups.
  private(set) var searchesKaomoji = false
  private let searchScopes = UIStackView()
  private var scopeButtons: [UIButton] = []
  /// 面板是带自己的标题行盖住键盘（默认），还是放在按键区里、使用设计稿的底栏。
  private let showsHeader: Bool
  /// 底栏的搜索签，排在所有分类签之前；带标题栏时不用它，搜索是标题栏上的按钮。
  private let searchChip = UIButton(type: .system)
  /// 网格在浏览时和搜索时（字母键盘上方）的下边缘约束。
  private var browseConstraints: [NSLayoutConstraint] = []
  private var searchConstraints: [NSLayoutConstraint] = []
  private static let chipSide: CGFloat = 32
  private static let barHeight: CGFloat = 40
  private static let barGap: CGFloat = 6

  /// What the grid shows: the search while one is open, otherwise the selected tab.
  private var currentTab: Tab? {
    if let searchQuery {
      return KeyboardEmojiCatalog.search(searchQuery, kaomoji: searchesKaomoji).map(Tab.category)
    }
    return availableTabs.indices.contains(selectedTab) ? availableTabs[selectedTab] : nil
  }

  init(
    resources: String,
    loader: PageLoader? = nil,
    showsHeader: Bool = true,
    onInsert: @escaping (String) -> Void,
    onDelete: @escaping () -> Void,
    onClose: @escaping () -> Void,
    onABC: (() -> Void)? = nil,
    onCatalogChange: (() -> Void)? = nil
  ) {
    self.onInsert = onInsert
    self.onDelete = onDelete
    self.onCatalogChange = onCatalogChange
    self.showsHeader = showsHeader
    self.loader = loader ?? { category, offset in
      try KeyboardEmojiCatalog.loadPage(
        resources: resources, category: category, offset: offset)
    }
    availableTabs = (KeyboardEmojiRecents.stored.isEmpty ? [] : [.recent])
      + KeyboardEmojiCatalog.categories.map(Tab.category)
      + (KeyboardKaomojiRecents.stored.isEmpty ? [] : [.recentKaomoji])
      + [.category(KeyboardEmojiCatalog.kaomoji)]
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardEmojiPicker"
    // 在按键区里透出键盘自己的背景，和被面板替换掉的按键底下一样。
    backgroundColor = showsHeader ? skin.background : .clear

    let title = titleLabel
    title.text = "表情"
    title.font = showsHeader ? .systemFont(ofSize: 17, weight: .semibold) : .systemFont(ofSize: 15, weight: .medium)
    title.textColor = skin.keyForeground
    title.lineBreakMode = .byTruncatingHead
    title.accessibilityIdentifier = "emojiTitle"

    tabs.axis = .horizontal
    tabs.spacing = 4
    tabScroll.showsHorizontalScrollIndicator = false
    tabScroll.alwaysBounceHorizontal = false
    tabScroll.disableEdgeEffects()
    tabScroll.contentInsetAdjustmentBehavior = .never
    for (index, tab) in availableTabs.enumerated() {
      let button = UIButton(type: .system)
      button.configuration = showsHeader ? Self.tabConfiguration(title: tab.title) : chipConfiguration(Self.chipGlyph(tab))
      button.accessibilityIdentifier = "emojiCategory-\(index)"
      button.accessibilityLabel = tab.title
      button.addAction(UIAction { [weak self] _ in self?.selectTab(index) },
                       for: .primaryActionTriggered)
      if !showsHeader {
        button.widthAnchor.constraint(equalToConstant: Self.chipSide).isActive = true
        button.heightAnchor.constraint(equalToConstant: Self.chipSide).isActive = true
      }
      tabButtons.append(button)
      tabs.addArrangedSubview(button)
    }

    status.font = .systemFont(ofSize: 11)
    status.textColor = showsHeader ? skin.keyForeground.withAlphaComponent(0.65) : skin.secondary
    status.textAlignment = .center
    status.numberOfLines = showsHeader ? 1 : 2
    status.accessibilityIdentifier = "emojiCatalogStatus"

    grid.dataSource = self
    grid.delegate = self
    grid.backgroundColor = .clear
    grid.accessibilityIdentifier = "emojiGrid"
    grid.register(KeyboardEmojiCell.self, forCellWithReuseIdentifier: KeyboardEmojiCell.reuseIdentifier)
    grid.disableEdgeEffects()
    grid.contentInsetAdjustmentBehavior = .never

    buildLetterPad()
    letterPad.isHidden = true
    searchScopes.axis = .horizontal
    searchScopes.spacing = 4
    searchScopes.isHidden = true
    for (index, title) in ["表情", "颜文字"].enumerated() {
      let button = UIButton(type: .system)
      button.configuration = Self.tabConfiguration(title: title)
      button.accessibilityIdentifier = index == 0 ? "emojiSearchScopeEmoji" : "emojiSearchScopeKaomoji"
      button.accessibilityLabel = "搜索\(title)"
      button.addAction(UIAction { [weak self] _ in self?.setSearchScope(kaomoji: index == 1) },
                       for: .primaryActionTriggered)
      button.setContentCompressionResistancePriority(.required, for: .horizontal)
      scopeButtons.append(button)
      searchScopes.addArrangedSubview(button)
    }

    tabScroll.addSubview(tabs)
    tabs.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      tabs.topAnchor.constraint(equalTo: tabScroll.contentLayoutGuide.topAnchor),
      tabs.bottomAnchor.constraint(equalTo: tabScroll.contentLayoutGuide.bottomAnchor),
      tabs.leadingAnchor.constraint(equalTo: tabScroll.contentLayoutGuide.leadingAnchor),
      tabs.trailingAnchor.constraint(equalTo: tabScroll.contentLayoutGuide.trailingAnchor),
      tabs.heightAnchor.constraint(equalTo: tabScroll.frameLayoutGuide.heightAnchor),
    ])
    if showsHeader {
      layOutWithHeader(onClose: onClose)
    } else {
      layOutInKeyArea(onLeave: onABC ?? onClose)
    }
    NSLayoutConstraint.activate(browseConstraints)
    selectTab(0)
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 盖住键盘的面板：一行带返回、搜索和删除的标题，下面是分类栏，搜索时字母键盘占据底部。
  private func layOutWithHeader(onClose: @escaping () -> Void) {
    let title = titleLabel
    let close = headerButton(symbol: "chevron.left", label: "返回键盘", id: "closeEmojiPicker")
    // 在搜索中按返回回到分类，在分类中按返回回到键盘。
    close.addAction(UIAction { [weak self] _ in
      if self?.searchQuery != nil { self?.endSearch() } else { onClose() }
    }, for: .primaryActionTriggered)
    let delete = headerButton(symbol: "delete.left", label: "删除", id: "emojiDeleteKey")
    delete.addAction(UIAction { [weak self] _ in self?.onDelete() }, for: .primaryActionTriggered)
    let search = headerButton(symbol: "magnifyingglass", label: "搜索表情", id: "emojiSearchButton")
    search.addAction(UIAction { [weak self] _ in self?.beginSearch() }, for: .primaryActionTriggered)

    for child in [title, close, search, delete, tabScroll, searchScopes, status, grid, letterPad] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    NSLayoutConstraint.activate([
      close.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      close.topAnchor.constraint(equalTo: topAnchor),
      close.widthAnchor.constraint(equalToConstant: 44),
      close.heightAnchor.constraint(equalToConstant: 40),
      title.centerXAnchor.constraint(equalTo: centerXAnchor),
      title.centerYAnchor.constraint(equalTo: close.centerYAnchor),
      title.leadingAnchor.constraint(greaterThanOrEqualTo: close.trailingAnchor, constant: 4),
      title.trailingAnchor.constraint(lessThanOrEqualTo: search.leadingAnchor, constant: -4),
      search.trailingAnchor.constraint(equalTo: delete.leadingAnchor),
      search.centerYAnchor.constraint(equalTo: close.centerYAnchor),
      search.widthAnchor.constraint(equalToConstant: 44),
      search.heightAnchor.constraint(equalToConstant: 40),
      delete.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      delete.centerYAnchor.constraint(equalTo: close.centerYAnchor),
      delete.widthAnchor.constraint(equalToConstant: 44),
      delete.heightAnchor.constraint(equalToConstant: 40),

      tabScroll.topAnchor.constraint(equalTo: close.bottomAnchor),
      tabScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      tabScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      tabScroll.heightAnchor.constraint(equalToConstant: 30),
      searchScopes.centerXAnchor.constraint(equalTo: centerXAnchor),
      searchScopes.topAnchor.constraint(equalTo: tabScroll.topAnchor),
      searchScopes.heightAnchor.constraint(equalTo: tabScroll.heightAnchor),

      status.topAnchor.constraint(equalTo: tabScroll.bottomAnchor),
      status.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
      status.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
      status.heightAnchor.constraint(equalToConstant: 20),
      grid.topAnchor.constraint(equalTo: status.bottomAnchor),
      grid.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
      grid.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
      letterPad.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
      letterPad.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
      letterPad.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
      letterPad.heightAnchor.constraint(equalToConstant: 3 * 38 + 2 * 6),
    ])
    browseConstraints = [grid.bottomAnchor.constraint(equalTo: bottomAnchor)]
    searchConstraints = [grid.bottomAnchor.constraint(equalTo: letterPad.topAnchor, constant: -4)]
  }

  /// 放在按键区里的面板（设计稿的表情面板）：网格占满上方，底部是一条 ABC | 分类 | ⌫ 的栏，ABC 和 ⌫ 各占 1.3 份，分类占 5 份。搜索是第一个分类签：打开后字母键盘夹在一行结果条和底栏之间，原来放分类的位置换成查询词和「表情」/「颜文字」两个范围。
  ///
  /// 竖屏手机上按键区约 201pt：底栏占 40，间距 6，字母键盘 106，留给结果条约 45pt，正好一整行 Emoji 或颜文字。横屏或调低键盘高度后按键区更矮，这时字母键盘先让出高度，结果条始终保留一整行。
  private func layOutInKeyArea(onLeave: @escaping () -> Void) {
    let abc = barKey(id: "closeEmojiPicker", label: "返回键盘") { onLeave() }
    var abcConfiguration = abc.configuration
    abcConfiguration?.attributedTitle = AttributedString("ABC", attributes: AttributeContainer([
      .font: UIFont.systemFont(ofSize: 15, weight: .medium),
    ]))
    abc.configuration = abcConfiguration
    let delete = barKey(id: "emojiDeleteKey", label: "删除") { [weak self] in self?.onDelete() }
    var deleteConfiguration = delete.configuration
    deleteConfiguration?.image = KeyboardIcon.backspace.image(pointSize: 22)
    delete.configuration = deleteConfiguration

    let search = searchChip
    search.configuration = chipConfiguration((nil, "magnifyingglass"))
    search.accessibilityIdentifier = "emojiSearchButton"
    search.accessibilityLabel = "搜索表情"
    // 这个分类签打开搜索；搜索开着时再点则关闭。
    search.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if searchQuery == nil { beginSearch() } else { endSearch() }
    }, for: .primaryActionTriggered)
    titleLabel.isHidden = true
    titleLabel.setContentHuggingPriority(.init(1), for: .horizontal)
    titleLabel.setContentCompressionResistancePriority(.init(1), for: .horizontal)
    let strip = UIStackView(arrangedSubviews: [search, tabScroll, titleLabel, searchScopes])
    strip.axis = .horizontal
    strip.alignment = .center
    strip.spacing = 4
    let bar = UIStackView(arrangedSubviews: [abc, strip, delete])
    bar.axis = .horizontal
    bar.alignment = .center
    bar.spacing = Self.barGap

    for child in [grid, status, letterPad, bar] {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    // 字母键盘的设计高度不是必需的：按键区放不下时由它缩小，而不是把结果条压没。
    let padHeight = letterPad.heightAnchor.constraint(equalToConstant: 3 * 32 + 2 * 5)
    padHeight.priority = .defaultHigh
    NSLayoutConstraint.activate([
      search.widthAnchor.constraint(equalToConstant: Self.chipSide),
      search.heightAnchor.constraint(equalToConstant: Self.chipSide),
      tabScroll.heightAnchor.constraint(equalToConstant: Self.chipSide),
      strip.heightAnchor.constraint(equalToConstant: Self.barHeight),
      abc.heightAnchor.constraint(equalToConstant: Self.barHeight),
      delete.heightAnchor.constraint(equalToConstant: Self.barHeight),
      delete.widthAnchor.constraint(equalTo: abc.widthAnchor),
      strip.widthAnchor.constraint(equalTo: abc.widthAnchor, multiplier: 5 / 1.3),
      bar.leadingAnchor.constraint(equalTo: leadingAnchor),
      bar.trailingAnchor.constraint(equalTo: trailingAnchor),
      bar.bottomAnchor.constraint(equalTo: bottomAnchor),
      bar.heightAnchor.constraint(equalToConstant: Self.barHeight),

      grid.topAnchor.constraint(equalTo: topAnchor),
      grid.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
      grid.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
      // 状态行只在网格没有内容可显示时才出现，所以它叠在网格上，不单独占按键区的一行。
      status.centerYAnchor.constraint(equalTo: grid.centerYAnchor),
      status.leadingAnchor.constraint(equalTo: grid.leadingAnchor, constant: 8),
      status.trailingAnchor.constraint(equalTo: grid.trailingAnchor, constant: -8),
      letterPad.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
      letterPad.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
      letterPad.bottomAnchor.constraint(equalTo: bar.topAnchor, constant: -Self.barGap),
      padHeight,
    ])
    browseConstraints = [grid.bottomAnchor.constraint(equalTo: bar.topAnchor, constant: -Self.barGap)]
    // 结果条至少一整行：44pt 是颜文字的行高，比 Emoji 行高。优先级略低于必需，按键区还没布局（高度为 0）时不和其他约束冲突。
    let gridMinimum = grid.heightAnchor.constraint(greaterThanOrEqualToConstant: 44)
    gridMinimum.priority = .init(999)
    searchConstraints = [grid.bottomAnchor.constraint(equalTo: letterPad.topAnchor, constant: -4), gridMinimum]
  }

  /// ABC 和 ⌫：功能键底色的 40pt 按键，按下效果与旁边的按键相同。
  private func barKey(id: String, label: String, action: @escaping () -> Void) -> KeyboardKeyButton {
    var configuration = UIButton.Configuration.plain()
    configuration.baseForegroundColor = skin.keyForeground
    configuration.background.backgroundColor = skin.functionKeyBackground
    configuration.background.cornerRadius = skin.cornerRadius
    configuration.contentInsets = .zero
    let key = KeyboardKeyButton(configuration: configuration, primaryAction: UIAction { _ in action() })
    key.isFunctionKey = true
    key.accessibilityIdentifier = id
    key.accessibilityLabel = label
    return key
  }

  private func headerButton(symbol: String, label: String, id: String) -> UIButton {
    let button = UIButton(type: .system)
    button.setImage(UIImage(systemName: symbol), for: .normal)
    button.tintColor = skin.accent
    button.accessibilityIdentifier = id
    button.accessibilityLabel = label
    return button
  }

  /// 分类签显示的内容：和设计稿的底栏一样用该组的一个 Emoji，没有 Emoji 的标签页用一个符号。
  private static func chipGlyph(_ tab: Tab) -> (text: String?, symbol: String?) {
    switch tab {
    case .recent: return (nil, "clock")
    case .recentKaomoji: return (nil, "clock.arrow.circlepath")
    case .category(let category) where category.isKaomoji: return ("^_^", nil)
    case .category(let category):
      return (categoryGlyphs[category.group] ?? String(category.title.prefix(1)), nil)
    }
  }

  private static let categoryGlyphs = [
    "Smileys and emotion": "😀", "People and body": "👍", "Animals and nature": "🌲", "Food and drink": "🍜",
    "Travel and places": "🚗", "Activities": "⚽", "Objects": "💡", "Symbols": "❤️", "Flags": "🏳️",
  ]

  /// 底栏上 32pt、圆角 9 的分类签；`updateTabAppearance` 给选中的那个铺上按键底色。
  private func chipConfiguration(_ glyph: (text: String?, symbol: String?)) -> UIButton.Configuration {
    var configuration = UIButton.Configuration.plain()
    if let text = glyph.text {
      // 数字和 ASCII 标点在 Unicode 里也算 Emoji，只有图形符号才用 18pt 字号。
      let emoji = text.unicodeScalars.contains { $0.properties.isEmoji && $0.value > 0xFF }
      configuration.attributedTitle = AttributedString(text, attributes: AttributeContainer([
        .font: UIFont.systemFont(ofSize: emoji ? 18 : 11, weight: .medium),
      ]))
    }
    configuration.image = glyph.symbol.flatMap {
      UIImage(systemName: $0, withConfiguration: UIImage.SymbolConfiguration(pointSize: 15, weight: .medium))
    }
    configuration.baseForegroundColor = skin.secondary
    configuration.contentInsets = .zero
    configuration.background.cornerRadius = 9
    configuration.background.backgroundColor = .clear
    return configuration
  }

  private var showsKaomoji: Bool {
    switch currentTab {
    case .recentKaomoji: return true
    case .category(let category): return category.isKaomoji
    default: return false
    }
  }

  /// Columns for the kaomoji tab: as many 170-point columns as the width holds, and never fewer than two.
  static func kaomojiColumns(width: CGFloat) -> Int { max(2, Int(width / 170)) }
  /// Columns for emoji: the catalog's eight on a phone, and as many 56-point cells as a wider keyboard holds, so an iPad shows several rows instead of two rows of oversized cells.
  static func emojiColumns(width: CGFloat) -> Int { max(KeyboardEmojiCatalog.columns, Int(width / 56)) }

  /// 网格：带标题栏时是正方形的 Emoji 格子；在按键区里是设计稿的 40pt 格子、2pt 间距。颜文字在两种情况下都是 44pt 一行。
  private func makeLayout() -> UICollectionViewCompositionalLayout {
    UICollectionViewCompositionalLayout { [weak self] _, environment in
      let kaomoji = self?.showsKaomoji ?? false
      let columns = kaomoji
        ? Self.kaomojiColumns(width: environment.container.effectiveContentSize.width)
        : Self.emojiColumns(width: environment.container.effectiveContentSize.width)
      if self?.showsHeader == false {
        let gap: CGFloat = 2
        let width = environment.container.effectiveContentSize.width
        let item = NSCollectionLayoutItem(layoutSize: NSCollectionLayoutSize(
          widthDimension: .absolute(max(1, (width - gap * CGFloat(columns - 1)) / CGFloat(columns))),
          heightDimension: .fractionalHeight(1)))
        let group = NSCollectionLayoutGroup.horizontal(
          layoutSize: NSCollectionLayoutSize(widthDimension: .fractionalWidth(1),
                                             heightDimension: .absolute(kaomoji ? 44 : 40)),
          subitems: [item])
        group.interItemSpacing = .fixed(gap)
        let section = NSCollectionLayoutSection(group: group)
        section.interGroupSpacing = gap
        return section
      }
      let item = NSCollectionLayoutItem(layoutSize: NSCollectionLayoutSize(
        widthDimension: .fractionalWidth(1.0 / CGFloat(columns)),
        heightDimension: .fractionalHeight(1)))
      let group = NSCollectionLayoutGroup.horizontal(layoutSize: NSCollectionLayoutSize(
        widthDimension: .fractionalWidth(1),
        heightDimension: kaomoji ? .absolute(44) : .fractionalWidth(1.0 / CGFloat(columns))),
        subitems: [item])
      return NSCollectionLayoutSection(group: group)
    }
  }

  private static func tabConfiguration(title: String) -> UIButton.Configuration {
    var configuration = UIButton.Configuration.plain()
    configuration.title = title
    configuration.contentInsets = NSDirectionalEdgeInsets(top: 2, leading: 8, bottom: 2, trailing: 8)
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = .systemFont(ofSize: 13, weight: .medium)
      return attributes
    }
    return configuration
  }

  private func selectTab(_ index: Int) {
    guard availableTabs.indices.contains(index) else { return }
    loadGeneration &+= 1
    selectedTab = index
    items = []
    nextOffset = 0
    complete = false
    loading = false
    updateTabAppearance()
    switch availableTabs[index] {
    case .recent:
      items = KeyboardEmojiRecents.stored.map {
        KeyboardEmojiCatalog.Item(text: $0, annotation: "", group: "最近")
      }
      complete = true
      reloadCatalog()
    case .recentKaomoji:
      items = KeyboardKaomojiRecents.stored.map {
        KeyboardEmojiCatalog.Item(text: $0, annotation: "", group: "最近")
      }
      complete = true
      reloadCatalog()
    case .category:
      reloadCatalog()
      loadNextPage()
    }
  }

  /// 带标题栏时选中的标签页用强调色；在底栏里选中的分类签铺上按键底色，与设计稿一致。
  private func updateTabAppearance() {
    for (index, button) in tabButtons.enumerated() {
      let selected = index == selectedTab
      if showsHeader {
        button.configuration?.baseForegroundColor = selected
          ? skin.accent : skin.keyForeground.withAlphaComponent(0.6)
      } else {
        button.configuration?.background.backgroundColor = selected ? skin.keyBackground : .clear
        button.configuration?.baseForegroundColor = selected ? skin.keyForeground : skin.secondary
      }
      button.accessibilityTraits = selected ? [.button, .selected] : .button
    }
    guard availableTabs.indices.contains(selectedTab) else { return }
    tabScroll.scrollRectToVisible(tabButtons[selectedTab].frame.insetBy(dx: -8, dy: 0), animated: false)
  }

  private func loadNextPage() {
    guard !loading, !complete, case .category(let category) = currentTab,
          items.count < KeyboardEmojiCatalog.maximumItems else { return }
    loading = true
    updateStatus()
    let offset = nextOffset
    let generation = loadGeneration
    let targetLoader = loader
    loadQueue.async { [weak self] in
      let page = try? targetLoader(category, offset)
      DispatchQueue.main.async { [weak self] in
        self?.accept(page: page, category: category, offset: offset, generation: generation)
      }
    }
  }

  private func accept(
    page: KeyboardEmojiCatalog.Page?, category: KeyboardEmojiCatalog.Category,
    offset: Int, generation: UInt64
  ) {
    guard generation == loadGeneration, currentTab == .category(category),
          nextOffset == offset else { return }
    loading = false
    guard let page, items.count + page.items.count <= KeyboardEmojiCatalog.maximumItems else {
      complete = true
      status.text = searchQuery == nil ? "表情目录暂时不可用；点分类重试" : "表情目录暂时不可用；改一下搜索词重试"
      updateStatusVisibility()
      onCatalogChange?()
      return
    }
    items.append(contentsOf: page.items)
    nextOffset = page.nextOffset
    complete = page.complete
    reloadCatalog()
    // More pages load on scroll, so a page that does not fill the grid (a wide iPad keyboard) would otherwise never be followed.
    grid.layoutIfNeeded()
    if !complete && (page.items.isEmpty || grid.contentSize.height <= grid.bounds.height) { loadNextPage() }
  }

  private func reloadCatalog() {
    grid.collectionViewLayout.invalidateLayout()
    grid.reloadData()
    updateStatus()
    onCatalogChange?()
  }

  private func updateStatus() {
    let noun = showsKaomoji || (searchQuery != nil && searchesKaomoji) ? "颜文字" : "表情"
    if searchQuery?.isEmpty == true { status.text = "输入拼音或英文搜索\(noun)" }
    else if loading && items.isEmpty { status.text = searchQuery == nil ? "正在加载\(noun)…" : "正在搜索…" }
    else if items.isEmpty { status.text = searchQuery != nil ? "没有找到相关\(noun)"
      : currentTab == .recent ? "暂无最近使用" : "暂无\(noun)" }
    else if complete { status.text = "\(items.count) 个\(noun)" }
    else { status.text = "\(items.count) 个\(noun) · 继续滚动加载" }
    updateStatusVisibility()
  }

  /// 在按键区里状态叠在网格上，所以只在网格为空时显示；带标题栏时它单独占一行。
  private func updateStatusVisibility() {
    guard !showsHeader else { return }
    status.isHidden = !items.isEmpty
  }

  private func buildLetterPad() {
    letterPad.axis = .vertical
    letterPad.spacing = showsHeader ? 6 : 5
    letterPad.distribution = .fillEqually
    letterPad.accessibilityIdentifier = "emojiSearchPad"
    for (index, row) in ["qwertyuiop", "asdfghjkl", "zxcvbnm"].enumerated() {
      let line = UIStackView()
      line.axis = .horizontal
      line.spacing = 5
      line.distribution = .fillEqually
      for letter in row {
        line.addArrangedSubview(padKey(title: String(letter), id: "emojiSearchKey-\(letter)") { [weak self] in
          self?.typeSearch(String(letter))
        })
      }
      if index == 2 {
        line.addArrangedSubview(padKey(symbol: "delete.left", label: "删除搜索字母", id: "emojiSearchDelete") { [weak self] in
          self?.deleteSearchLetter()
        })
      }
      letterPad.addArrangedSubview(line)
    }
  }

  private func padKey(
    title: String? = nil, symbol: String? = nil, label: String? = nil, id: String, action: @escaping () -> Void
  ) -> UIButton {
    var configuration = UIButton.Configuration.filled()
    configuration.title = title
    configuration.image = symbol.flatMap { UIImage(systemName: $0) }
    configuration.baseBackgroundColor = skin.keyBackground
    configuration.baseForegroundColor = skin.keyForeground
    configuration.cornerStyle = .medium
    configuration.contentInsets = .zero
    configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
      var attributes = attributes
      attributes.font = .systemFont(ofSize: 18)
      return attributes
    }
    let button = UIButton(configuration: configuration)
    button.accessibilityIdentifier = id
    button.accessibilityLabel = label ?? title
    button.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    return button
  }

  /// Open the search with an empty query: the category bar gives way to the letter pad.
  func beginSearch() {
    guard searchQuery == nil else { return }
    searchQuery = ""
    if availableTabs.indices.contains(selectedTab), case .category(let category) = availableTabs[selectedTab] {
      searchesKaomoji = category.isKaomoji
    } else {
      searchesKaomoji = false
    }
    tabScroll.isHidden = true
    searchScopes.isHidden = false
    letterPad.isHidden = false
    if !showsHeader {
      titleLabel.isHidden = false
      searchChip.configuration?.background.backgroundColor = skin.keyBackground
      searchChip.configuration?.baseForegroundColor = skin.keyForeground
      searchChip.accessibilityTraits = [.button, .selected]
    }
    NSLayoutConstraint.deactivate(browseConstraints)
    NSLayoutConstraint.activate(searchConstraints)
    showSearch()
  }

  /// Close the search and return to the category that was showing before it.
  func endSearch() {
    guard searchQuery != nil else { return }
    searchQuery = nil
    titleLabel.text = "表情"
    titleLabel.textColor = skin.keyForeground
    letterPad.isHidden = true
    searchScopes.isHidden = true
    tabScroll.isHidden = false
    if !showsHeader {
      titleLabel.isHidden = true
      searchChip.configuration?.background.backgroundColor = .clear
      searchChip.configuration?.baseForegroundColor = skin.secondary
      searchChip.accessibilityTraits = .button
    }
    NSLayoutConstraint.deactivate(searchConstraints)
    NSLayoutConstraint.activate(browseConstraints)
    selectTab(selectedTab)
  }

  /// Search the other catalog for the letters already typed.
  func setSearchScope(kaomoji: Bool) {
    guard searchQuery != nil, searchesKaomoji != kaomoji else { return }
    searchesKaomoji = kaomoji
    showSearch()
  }

  private func typeSearch(_ letter: String) {
    guard let searchQuery, searchQuery.count < KeyboardEmojiCatalog.maximumSearchLength else { return }
    self.searchQuery = searchQuery + letter
    showSearch()
  }

  private func deleteSearchLetter() {
    guard let searchQuery, !searchQuery.isEmpty else { return }
    self.searchQuery = String(searchQuery.dropLast())
    showSearch()
  }

  /// Show the query in the title and restart the results from the first page; a page still loading for the previous query is dropped by the generation check.
  private func showSearch() {
    let query = searchQuery ?? ""
    for (index, button) in scopeButtons.enumerated() {
      let selected = (index == 1) == searchesKaomoji
      button.configuration?.baseForegroundColor = selected
        ? skin.accent : skin.keyForeground.withAlphaComponent(0.6)
      button.accessibilityTraits = selected ? [.button, .selected] : .button
    }
    titleLabel.text = query.isEmpty ? (searchesKaomoji ? "搜索颜文字" : "搜索表情") : query
    titleLabel.textColor = query.isEmpty ? skin.keyForeground.withAlphaComponent(0.5) : skin.keyForeground
    loadGeneration &+= 1
    items = []
    nextOffset = 0
    loading = false
    complete = query.isEmpty
    reloadCatalog()
    loadNextPage()
  }

  func numberOfSections(in collectionView: UICollectionView) -> Int { 1 }

  func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
    items.count
  }

  func collectionView(
    _ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath
  ) -> UICollectionViewCell {
    let cell = collectionView.dequeueReusableCell(
      withReuseIdentifier: KeyboardEmojiCell.reuseIdentifier, for: indexPath)
    (cell as? KeyboardEmojiCell)?.show(items[indexPath.item], kaomoji: showsKaomoji, compact: !showsHeader)
    return cell
  }

  func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
    guard items.indices.contains(indexPath.item) else { return }
    collectionView.deselectItem(at: indexPath, animated: false)
    let emoji = items[indexPath.item].text
    if showsKaomoji { KeyboardKaomojiRecents.record(emoji) } else { KeyboardEmojiRecents.record(emoji) }
    onInsert(emoji)
  }

  func scrollViewDidScroll(_ scrollView: UIScrollView) {
    guard scrollView === grid, !complete,
          let last = grid.indexPathsForVisibleItems.map(\.item).max(),
          last >= max(0, items.count - 16) else { return }
    loadNextPage()
  }
}

/// 一个 Emoji 或颜文字。按下时在 0.08 秒内放大到 1.25 倍，与设计稿的格子一致。
private final class KeyboardEmojiCell: UICollectionViewCell {
  static let reuseIdentifier = "KeyboardEmojiCell"
  private let label = UILabel()

  override var isHighlighted: Bool {
    didSet {
      guard isHighlighted != oldValue else { return }
      let scale: CGFloat = isHighlighted ? 1.25 : 1
      guard !UIAccessibility.isReduceMotionEnabled, window != nil else {
        label.transform = .identity
        return
      }
      UIView.animate(withDuration: 0.08, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
        self.label.transform = CGAffineTransform(scaleX: scale, y: scale)
      }
    }
  }

  override func prepareForReuse() {
    super.prepareForReuse()
    label.transform = .identity
  }

  override init(frame: CGRect) {
    super.init(frame: frame)
    label.textAlignment = .center
    label.adjustsFontSizeToFitWidth = true
    label.minimumScaleFactor = 0.5
    label.font = .systemFont(ofSize: 28)
    label.translatesAutoresizingMaskIntoConstraints = false
    contentView.addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
      label.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
      label.topAnchor.constraint(equalTo: contentView.topAnchor),
      label.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
    ])
    isAccessibilityElement = true
    accessibilityTraits = .button
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// `compact` 在按键区的 40pt 格子里画 26pt 的 Emoji；带标题栏时的正方形格子仍用 28pt。
  func show(_ item: KeyboardEmojiCatalog.Item, kaomoji: Bool = false, compact: Bool = false) {
    label.font = .systemFont(ofSize: kaomoji ? 17 : compact ? 26 : 28)
    label.textColor = KeyboardTheme.current.keyForeground
    label.text = item.text
    accessibilityLabel = item.annotation.isEmpty ? item.text : "\(item.text)，\(item.annotation)"
  }
}
