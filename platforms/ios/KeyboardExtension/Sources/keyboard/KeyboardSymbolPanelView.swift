import UIKit

/// Categorized symbol surface used by the keyboard's punctuation shortcut.
///
/// Symbols the user picked before lead as 最近, the way the Windows panel opens on its recently used items. The hand-picked phone categories come next; after them come the parents of the Engine's symbol catalog in `others.db`, the same catalog the desktop, macOS and Harmony panels browse, loaded a parent at a time off the main thread.
///
/// 搜索 looks the whole catalog up by keyword, as the search box of the Windows panel does. A keyboard extension has no text field of its own to type into, so the search swaps the category column for a letter pad under the results; the catalog files every symbol under English words, full pinyin and pinyin initials, which letters are enough to type.
final class KeyboardSymbolPanelView: UIView {
  struct Category {
    let title: String
    let symbols: [String]
  }

  /// Where the Engine catalog comes from; tests substitute their own rows.
  struct Catalog {
    let parents: () throws -> [String]
    let symbols: @Sendable (String) throws -> [String]
    /// The symbols matching the typed letters, or `nil` when the letters leave nothing to search for.
    var search: (@Sendable (String) throws -> [String]?)?

    static func engine(resources: String) -> Catalog {
      Catalog(parents: { try KeyboardEmojiCatalog.symbolParents(resources: resources) },
              symbols: { try KeyboardEmojiCatalog.loadSymbols(resources: resources, parent: $0) },
              search: { try KeyboardEmojiCatalog.searchSymbols(resources: resources, query: $0) })
    }
  }

  private enum Entry {
    case fixed(Category)
    case catalog(parent: String)

    var title: String {
      switch self {
      case .fixed(let category): return category.title
      case .catalog(let parent): return KeyboardEmojiCatalog.symbolParentTitles[parent] ?? parent
      }
    }
  }

  static let categories: [Category] = [
    Category(title: "常用", symbols: [
      "，", "。", "？", "！", "、", "；", "：", "…", "—", "·",
      "“", "”", "‘", "’", "（", "）", "《", "》", "【", "】",
      "～", "￥", "＆", "＃", "＠", "％", "＋", "－", "＝", "／",
    ]),
    Category(title: "中文", symbols: [
      "〈", "〉", "「", "」", "『", "』", "〔", "〕", "〖", "〗",
      "＜", "＞", "｛", "｝", "［", "］", "︵", "︶", "﹁", "﹂",
      "￥", "〇", "※", "°", "℃", "±", "×", "÷", "≈", "≠",
      "≤", "≥", "√", "∞", "∵", "∴", "→", "←", "↑", "↓",
      "★", "☆", "●", "○", "■", "□", "◆", "◇", "▲", "△",
    ]),
    Category(title: "英文", symbols: [
      ",", ".", "?", "!", ";", ":", "'", "\"", "(", ")",
      "[", "]", "{", "}", "<", ">", "/", "\\", "|", "-",
      "_", "+", "=", "*", "&", "^", "%", "$", "#", "@",
      "~", "`", "·", "…", "–", "—", "§", "¶", "†", "‡",
    ]),
    Category(title: "数字", symbols: [
      "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
      "①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩",
      "一", "二", "三", "四", "五", "六", "七", "八", "九", "十",
      "Ⅰ", "Ⅱ", "Ⅲ", "Ⅳ", "Ⅴ", "Ⅵ", "Ⅶ", "Ⅷ", "Ⅸ", "Ⅹ",
      "½", "⅓", "¼", "‰", "′", "″", "㎡", "㎏", "㎝", "№",
    ]),
    Category(title: "网络", symbols: [
      "@", "#", "/", "\\", ":", "_", "-", "+", "=", "&",
      "?", "%", "~", "^", "*", "|", "<", ">", "$", "€",
      "http://", "https://", "www.", ".com", ".cn", ".net", ".org", ".io", "@qq.com", "@gmail.com",
    ]),
  ]

  private let onInsert: (String) -> Void
  private let onDelete: () -> Void
  private let onClose: () -> Void
  private let catalog: Catalog?
  private let loadQueue = DispatchQueue(label: "app.msime.ios.symbol-catalog", qos: .userInitiated)
  private var loadGeneration: UInt64 = 0
  private var entries: [Entry]
  private let skin = KeyboardTheme.current
  private let grid = UIStackView()
  private let scroll = UIScrollView()
  private var categoryButtons: [UIButton] = []
  private(set) var selected = 0
  private var locked = false
  private var lockButton: UIButton!
  private let categoryScroll = UIScrollView()
  private let letterPad = UIStackView()
  private let status = UILabel()
  private var searchButton: UIButton?
  private var gridBesideCategories: [NSLayoutConstraint] = []
  private var gridAbovePad: [NSLayoutConstraint] = []
  /// The letters typed so far while searching, `nil` while browsing categories.
  private(set) var searchQuery: String?
  /// What the grid shows, for tests and VoiceOver: the symbols of the selected category or of the search.
  private(set) var shownSymbols: [String] = []

  init(catalog: Catalog? = nil, recents: [String] = [], onInsert: @escaping (String) -> Void, onDelete: @escaping () -> Void,
       onClose: @escaping () -> Void) {
    self.onInsert = onInsert
    self.onDelete = onDelete
    self.onClose = onClose
    self.catalog = catalog
    // Listing the parents is one small grouped query; without a readable catalog the phone categories stand alone.
    let parents = (try? catalog?.parents()) ?? []
    let recent = recents.isEmpty ? [] : [Entry.fixed(Category(title: "最近", symbols: recents))]
    entries = recent + Self.categories.map(Entry.fixed) + parents.map { Entry.catalog(parent: $0) }
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardSymbolPanel"
    backgroundColor = skin.background

    let categories = UIStackView()
    categories.axis = .vertical
    categories.spacing = 1
    for (index, entry) in entries.enumerated() {
      let button = UIButton(type: .system)
      button.setTitle(entry.title, for: .normal)
      button.titleLabel?.font = .systemFont(ofSize: 15)
      button.accessibilityIdentifier = "symbolCategory_\(index)"
      button.addAction(UIAction { [weak self] _ in self?.select(index) }, for: .primaryActionTriggered)
      button.heightAnchor.constraint(equalToConstant: Self.categoryHeight).isActive = true
      categoryButtons.append(button)
      categories.addArrangedSubview(button)
    }
    categoryScroll.showsVerticalScrollIndicator = false
    categoryScroll.disableEdgeEffects()
    categoryScroll.accessibilityIdentifier = "symbolCategories"
    categories.translatesAutoresizingMaskIntoConstraints = false
    categoryScroll.addSubview(categories)

    grid.axis = .vertical
    grid.spacing = 1
    scroll.showsVerticalScrollIndicator = true
    scroll.disableEdgeEffects()
    scroll.accessibilityIdentifier = "symbolGrid"
    grid.translatesAutoresizingMaskIntoConstraints = false
    scroll.addSubview(grid)

    // Back from a search returns to the categories; back from the categories returns to the keyboard.
    let back = bar(title: "返回", symbol: nil, identifier: "closeSymbolPanel") { [weak self] in
      guard let self else { return }
      if searchQuery != nil { endSearch() } else { onClose() }
    }
    back.accessibilityLabel = "返回键盘"
    let search = catalog?.search == nil ? nil : bar(title: nil, symbol: "magnifyingglass", identifier: "symbolSearchKey") { [weak self] in
      self?.beginSearch()
    }
    search?.accessibilityLabel = "搜索符号"
    searchButton = search
    lockButton = bar(title: nil, symbol: "lock.open", identifier: "symbolLockKey") { [weak self] in
      guard let self else { return }
      locked.toggle()
      updateLock()
    }
    let delete = bar(title: nil, symbol: "delete.left", identifier: "symbolDeleteKey") { [weak self] in
      self?.onDelete()
    }
    delete.accessibilityLabel = "删除"
    updateLock()

    let bottom = UIStackView(arrangedSubviews: [back] + (search.map { [$0] } ?? []) + [lockButton, delete])
    bottom.axis = .horizontal
    bottom.distribution = .fillEqually
    bottom.spacing = 1

    buildLetterPad()
    letterPad.isHidden = true
    status.font = .systemFont(ofSize: 14)
    status.textColor = skin.keyForeground.withAlphaComponent(0.6)
    status.textAlignment = .center
    status.numberOfLines = 2
    status.isHidden = true
    status.accessibilityIdentifier = "symbolSearchStatus"

    for item in [categoryScroll, scroll, bottom, letterPad, status] {
      item.translatesAutoresizingMaskIntoConstraints = false
      addSubview(item)
    }
    NSLayoutConstraint.activate([
      categoryScroll.leadingAnchor.constraint(equalTo: leadingAnchor),
      categoryScroll.topAnchor.constraint(equalTo: topAnchor),
      categoryScroll.bottomAnchor.constraint(equalTo: bottom.topAnchor, constant: -1),
      categoryScroll.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.2),
      categories.leadingAnchor.constraint(equalTo: categoryScroll.contentLayoutGuide.leadingAnchor),
      categories.trailingAnchor.constraint(equalTo: categoryScroll.contentLayoutGuide.trailingAnchor),
      categories.topAnchor.constraint(equalTo: categoryScroll.contentLayoutGuide.topAnchor),
      categories.bottomAnchor.constraint(equalTo: categoryScroll.contentLayoutGuide.bottomAnchor),
      categories.widthAnchor.constraint(equalTo: categoryScroll.frameLayoutGuide.widthAnchor),
      scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
      scroll.topAnchor.constraint(equalTo: topAnchor),
      letterPad.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
      letterPad.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
      letterPad.bottomAnchor.constraint(equalTo: bottom.topAnchor, constant: -4),
      letterPad.heightAnchor.constraint(equalTo: heightAnchor, multiplier: 0.42),
      status.leadingAnchor.constraint(equalTo: scroll.leadingAnchor, constant: 8),
      status.trailingAnchor.constraint(equalTo: scroll.trailingAnchor, constant: -8),
      status.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
      grid.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
      grid.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
      grid.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
      grid.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
      grid.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
      bottom.leadingAnchor.constraint(equalTo: leadingAnchor),
      bottom.trailingAnchor.constraint(equalTo: trailingAnchor),
      bottom.bottomAnchor.constraint(equalTo: bottomAnchor),
      bottom.heightAnchor.constraint(equalTo: heightAnchor, multiplier: 0.2),
    ])
    gridBesideCategories = [
      scroll.leadingAnchor.constraint(equalTo: categoryScroll.trailingAnchor, constant: 1),
      scroll.bottomAnchor.constraint(equalTo: bottom.topAnchor, constant: -1),
    ]
    gridAbovePad = [
      scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
      scroll.bottomAnchor.constraint(equalTo: letterPad.topAnchor, constant: -4),
    ]
    NSLayoutConstraint.activate(gridBesideCategories)
    select(0)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  private static let columns = 5
  private static let rowHeight: CGFloat = 46
  private static let categoryHeight: CGFloat = 40

  /// The number of categories on the left: 最近 when there is one, the phone ones, and any catalog parents.
  var categoryCount: Int { entries.count }

  private func select(_ index: Int) {
    guard entries.indices.contains(index) else { return }
    selected = index
    loadGeneration &+= 1
    for (position, button) in categoryButtons.enumerated() {
      let active = position == index
      button.backgroundColor = active ? skin.background : skin.keyBackground
      button.setTitleColor(active ? skin.accent : skin.keyForeground, for: .normal)
      button.accessibilityTraits = active ? [.button, .selected] : [.button]
    }
    switch entries[index] {
    case .fixed(let category):
      show(category.symbols)
    case .catalog(let parent):
      guard let catalog else { return }
      show([])
      let generation = loadGeneration
      loadQueue.async { [weak self] in
        let symbols = (try? catalog.symbols(parent)) ?? []
        DispatchQueue.main.async {
          guard let self, self.loadGeneration == generation else { return }
          self.show(symbols)
        }
      }
    }
  }

  private func show(_ symbols: [String]) {
    shownSymbols = symbols
    for row in grid.arrangedSubviews {
      grid.removeArrangedSubview(row)
      row.removeFromSuperview()
    }
    scroll.setContentOffset(.zero, animated: false)
    for start in stride(from: 0, to: symbols.count, by: Self.columns) {
      let row = UIStackView()
      row.axis = .horizontal
      row.distribution = .fillEqually
      row.spacing = 1
      let end = min(start + Self.columns, symbols.count)
      for symbol in symbols[start..<end] { row.addArrangedSubview(key(symbol)) }
      for _ in (end - start)..<Self.columns { row.addArrangedSubview(UIView()) }
      row.heightAnchor.constraint(equalToConstant: Self.rowHeight).isActive = true
      grid.addArrangedSubview(row)
    }
  }

  /// Open the search with an empty query: the category column gives way to the letter pad.
  func beginSearch() {
    guard catalog?.search != nil, searchQuery == nil else { return }
    searchQuery = ""
    categoryScroll.isHidden = true
    letterPad.isHidden = false
    NSLayoutConstraint.deactivate(gridBesideCategories)
    NSLayoutConstraint.activate(gridAbovePad)
    showSearch()
  }

  /// Close the search and return to the category that was showing before it.
  func endSearch() {
    guard searchQuery != nil else { return }
    searchQuery = nil
    status.isHidden = true
    letterPad.isHidden = true
    categoryScroll.isHidden = false
    NSLayoutConstraint.deactivate(gridAbovePad)
    NSLayoutConstraint.activate(gridBesideCategories)
    searchButton?.setTitle(nil, for: .normal)
    select(selected)
  }

  func typeSearch(_ letter: String) {
    guard let searchQuery, searchQuery.count < KeyboardEmojiCatalog.maximumSearchLength else { return }
    self.searchQuery = searchQuery + letter
    showSearch()
  }

  func deleteSearchLetter() {
    guard let searchQuery, !searchQuery.isEmpty else { return }
    self.searchQuery = String(searchQuery.dropLast())
    showSearch()
  }

  /// Show the query on the search key and look it up off the main thread; an answer for an older query is dropped by the generation check.
  private func showSearch() {
    guard let query = searchQuery, let search = catalog?.search else { return }
    searchButton?.setTitle(query.isEmpty ? nil : " \(query)", for: .normal)
    loadGeneration &+= 1
    show([])
    guard !query.isEmpty else {
      setStatus("输入英文或拼音搜索符号，如 arrow、jiantou")
      return
    }
    setStatus(nil)
    let generation = loadGeneration
    loadQueue.async { [weak self] in
      let result = Result { try search(query) ?? [] }
      DispatchQueue.main.async {
        guard let self, self.loadGeneration == generation, self.searchQuery == query else { return }
        switch result {
        case .success(let symbols):
          self.show(symbols)
          self.setStatus(symbols.isEmpty ? "没有找到相关符号" : nil)
        case .failure:
          self.setStatus("符号目录暂时不可用；改一下搜索词重试")
        }
      }
    }
  }

  private func setStatus(_ text: String?) {
    status.text = text
    status.isHidden = text == nil
  }

  private func buildLetterPad() {
    letterPad.axis = .vertical
    letterPad.spacing = 5
    letterPad.distribution = .fillEqually
    letterPad.accessibilityIdentifier = "symbolSearchPad"
    for (index, row) in ["qwertyuiop", "asdfghjkl", "zxcvbnm"].enumerated() {
      let line = UIStackView()
      line.axis = .horizontal
      line.spacing = 4
      line.distribution = .fillEqually
      for letter in row {
        let key = bar(title: String(letter), symbol: nil, identifier: "symbolSearchKey-\(letter)") { [weak self] in
          self?.typeSearch(String(letter))
        }
        key.layer.cornerRadius = 5
        line.addArrangedSubview(key)
      }
      if index == 2 {
        let delete = bar(title: nil, symbol: "delete.left", identifier: "symbolSearchDelete") { [weak self] in
          self?.deleteSearchLetter()
        }
        delete.accessibilityLabel = "删除搜索字母"
        delete.layer.cornerRadius = 5
        line.addArrangedSubview(delete)
      }
      letterPad.addArrangedSubview(line)
    }
  }

  private func key(_ symbol: String) -> UIButton {
    let button = KeyboardKeyButton(type: .system)
    button.setTitle(symbol, for: .normal)
    button.titleLabel?.font = .systemFont(ofSize: 18)
    button.titleLabel?.adjustsFontSizeToFitWidth = true
    button.titleLabel?.minimumScaleFactor = 0.6
    button.setTitleColor(skin.keyForeground, for: .normal)
    button.backgroundColor = skin.keyBackground
    button.accessibilityIdentifier = "symbolKey_\(symbol)"
    button.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      onInsert(symbol)
      if !locked { onClose() }
    }, for: .primaryActionTriggered)
    return button
  }

  private func bar(title: String?, symbol: String?, identifier: String,
                   action: @escaping () -> Void) -> UIButton {
    let button = KeyboardKeyButton(type: .system)
    if let title {
      button.setTitle(title, for: .normal)
      button.titleLabel?.font = .systemFont(ofSize: 15)
    }
    if let symbol { button.setImage(UIImage(systemName: symbol), for: .normal) }
    button.tintColor = skin.keyForeground
    button.setTitleColor(skin.keyForeground, for: .normal)
    button.backgroundColor = skin.keyBackground
    button.accessibilityIdentifier = identifier
    button.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    return button
  }

  private func updateLock() {
    lockButton.setImage(UIImage(systemName: locked ? "lock" : "lock.open"), for: .normal)
    lockButton.tintColor = locked ? skin.accent : skin.keyForeground
    lockButton.accessibilityLabel = locked ? "已锁定，连续输入符号" : "锁定，连续输入符号"
    lockButton.accessibilityTraits = locked ? [.button, .selected] : [.button]
  }
}

/// Symbols picked in the symbol panel, newest first, kept apart from the emoji recents.
///
/// Windows keeps one recent list for its whole panel. The iOS emoji recents fill an eight-column pictograph grid, while a symbol here can be a run of text such as `@gmail.com` in a five-column grid, so each panel keeps its own list.
enum KeyboardSymbolRecents {
  static let key = "symbolRecents"
  /// Six full rows of the five-column grid.
  static let limit = 30
  private static var defaults: UserDefaults { KeyboardFeedbackPreference.defaults }

  static var stored: [String] {
    normalize(defaults.stringArray(forKey: key) ?? [])
  }

  static func record(_ symbol: String) {
    guard KeyboardEmojiCatalog.validRecent(symbol) else { return }
    var recents = stored.filter { $0 != symbol }
    recents.insert(symbol, at: 0)
    defaults.set(Array(recents.prefix(limit)), forKey: key)
  }

  static func normalize(_ values: [String]) -> [String] {
    var seen = Set<String>()
    var output: [String] = []
    for value in values where KeyboardEmojiCatalog.validRecent(value) && seen.insert(value).inserted {
      output.append(value)
      if output.count == limit { break }
    }
    return output
  }
}
