import UIKit

/// iOS shows a paste notice whenever a keyboard reads what is on the pasteboard, so history is never recorded in the background the way the desktop records it. The change count and `hasStrings` can be read without that notice: they tell the panel that text has been copied since the last save, and the user's tap does the actual read.
enum ClipboardCapturePrompt {
  static let key = "clipboard.lastCapturedChangeCount"

  static func hasNewCopy(changeCount: Int, hasStrings: Bool, defaults: UserDefaults = .standard) -> Bool {
    hasStrings && defaults.object(forKey: key) as? Int != changeCount
  }

  static func markCaptured(changeCount: Int, defaults: UserDefaults = .standard) {
    defaults.set(changeCount, forKey: key)
  }
}

/// 本机历史多选的规则，与 Android 的 `ClipboardSelectionPolicy` 逐条相同（#10411）：最多选 20 项，选中 1 到 4 项时可以按 ①②③④ 的顺序一次插入。iOS 的多选目前只有插入，上限仍按 20 项，以后补删除时不用再改规则。
enum ClipboardSelectionPolicy {
  static let maxSelections = 20
  static let maxInsertions = 4
  /// 选中项超过插入上限、「插入」置灰时给的提示。
  static let insertionLimitHint = "一次最多插入 \(maxInsertions) 项"

  static func canSelect(_ selectedCount: Int) -> Bool { selectedCount >= 0 && selectedCount < maxSelections }

  static func canInsert(_ selectedCount: Int) -> Bool { selectedCount >= 1 && selectedCount <= maxInsertions }

  /// 第 `index` 个选中项（从 0 起）的角标：①、②……⑳；超出范围时为 nil。
  static func ordinalLabel(_ index: Int) -> String? {
    guard index >= 0, index < maxSelections, let scalar = Unicode.Scalar(0x2460 + index) else { return nil }
    return String(Character(scalar))
  }

  /// `text` 在按点选先后排列的选中项里的序号（从 0 起），没选中时为 nil。取消一项后，排在它后面的各项序号自然依次前移。
  static func ordinal(of text: String, in selected: [String]) -> Int? { selected.firstIndex(of: text) }

  /// 点按一条后的选中项：已选中的取消，未选中且没到上限的追加到末尾，到了上限的不变。
  static func toggled(_ text: String, in selected: [String]) -> [String] {
    if let index = selected.firstIndex(of: text) {
      var next = selected
      next.remove(at: index)
      return next
    }
    return canSelect(selected.count) ? selected + [text] : selected
  }

  /// 按选中顺序把各条首尾相接成一次插入的文字，中间不加分隔符。
  static func insertionText(_ texts: [String]) -> String { texts.joined() }

  /// 选中卡片的读屏前缀；没选中时为空。
  static func selectedDescription(_ index: Int?) -> String {
    guard let index else { return "" }
    return "已选择第 \(index + 1) 项；"
  }
}

/// 剪贴板面板：本机历史，登录账号后还有「云端」列表，按设计稿画成跟随主题的卡片。
///
/// 带标题栏时（默认）面板独自盖住键盘：卡片上方依次是一行 44pt 的控件（含「返回」）、保存按钮和状态行。不带标题栏时（`showsHeader: false`）面板放在工具栏下方的按键区里，背景透明、透出键盘的背景：一行紧凑的 32pt 控件放「本机」/「云端」分段、保存、搜索、刷新和清空，下面只有一行状态。这时搜索由它自己的按钮开关，因为工具栏会关闭面板。
final class KeyboardClipboardView: UIView, UITableViewDataSource, UITableViewDelegate {
  private let store: ClipboardHistoryStore
  private var items: [ClipboardHistoryItem] = []
  private var historyLimit = 50
  private let table = UITableView(frame: .zero, style: .plain)
  private let status = UILabel()
  private let onInsert: (String) -> Void
  private var confirmingClear = false
  /// The account's cloud clipboard; `nil` in password and one-time-code fields, where the panel stays local only.
  private let cloud: KeyboardCloudClipboard?
  private let source = UISegmentedControl(items: ["本机", "云端"])
  private let searchButton = UIButton(type: .system)
  private let clearButton = UIButton(type: .system)
  private let refresh = UIButton(type: .system)
  /// 多选（#10411）：点「多选」进入，点卡片按先后选中并在右上角标出 ①②③④，「插入」把选中的几条按序号一次插入，「取消」退出。只作用于本机历史，与 Android 的多选入口一致。
  private let selectButton = UIButton(type: .system)
  private let insertSelectedButton = UIButton(type: .system)
  private let cancelSelectionButton = UIButton(type: .system)
  private(set) var selecting = false
  /// 按点选先后排列的选中文字（本机历史按文字去重，文字就是条目的标识）。
  private(set) var selectedTexts: [String] = []
  private let hasFullAccess: Bool
  /// 当前能不能把剪贴板存进历史（`KeyboardPrivacyGate` 的 `clipboardHistory`）：隐私模式和凭据输入框里不能，点保存时只说明原因，与 Android 的 `captureClipboard` 一致。每次点按时现问，面板开着时输入框也可能换掉。
  private let capturesHistory: () -> Bool
  private let showsHeader: Bool
  private let skin = KeyboardTheme.current
  /// Whether the 云端 tab is showing.
  var showsCloud: Bool { cloud != nil && source.selectedSegmentIndex == 1 }
  /// Windows' 搜索剪贴板 box. A keyboard cannot type into a text field of its own, so the panel brings a letter and digit pad; `nil` while not searching.
  private(set) var searchQuery: String?
  /// What the table shows: the whole history, or the matches while searching.
  private var shown: [ClipboardHistoryItem] { ClipboardHistoryItem.matching(items, query: searchQuery ?? "") }
  private let title = UILabel()
  private let capture = UIButton(type: .system)
  private let letterPad = UIStackView()
  private let empty = UILabel()
  private lazy var captureHeight = capture.heightAnchor.constraint(equalToConstant: 44)
  private lazy var statusHeight = status.heightAnchor.constraint(equalToConstant: statusShownHeight)
  private lazy var tableAboveBottom = table.bottomAnchor.constraint(equalTo: bottomAnchor)
  private lazy var tableAbovePad = table.bottomAnchor.constraint(equalTo: letterPad.topAnchor, constant: -4)
  /// 字母键盘的设计高度：优先级低于结果表的最小高度，按键区太矮时（手机横屏约 168pt）由它让出空间，而不是把结果表压成一条缝。
  private lazy var letterPadHeight: NSLayoutConstraint = {
    // 四行键、三道行距；行高和行距拆成单独的常量：写成一个长表达式时，CI 上的编译器会在类型推断上超时。
    let rowHeight: CGFloat = showsHeader ? 30 : 26
    let rowGap: CGFloat = showsHeader ? 4 : 3
    let padHeight: CGFloat = 4 * rowHeight + 3 * rowGap
    let height = letterPad.heightAnchor.constraint(equalToConstant: padHeight)
    height.priority = .defaultHigh
    return height
  }()
  /// 搜索时结果表至少放得下一张单行卡片（上下 9pt 内边距、一行 14.5pt 正文和 6pt 卡片间距）。
  private lazy var tableMinimumHeight: NSLayoutConstraint = {
    let height = table.heightAnchor.constraint(greaterThanOrEqualToConstant: 44)
    height.priority = .init(999)
    return height
  }()
  static let searchLimit = 32
  static let fullAccessMessage = "请在系统键盘设置中开启「允许完全访问」，再使用剪贴板历史。"
  /// 与 Android 隐私闸门挡下保存时的提示相同。
  static let privacyMessage = "隐私模式或当前输入框下不保存剪贴板"

  /// 状态行：完整标题栏下是两行 12pt，紧凑标题栏下是一行设计稿的 11.5pt 附注字号。
  private var statusShownHeight: CGFloat { showsHeader ? 34 : 16 }
  /// 卡片与面板两侧的距离：与标题栏的控件对齐，在按键区里则是设计稿的 2pt。
  private var cardInset: CGFloat { showsHeader ? 12 : 2 }

  init(hasFullAccess: Bool, store: ClipboardHistoryStore = ClipboardHistoryStore(), cloud: KeyboardCloudClipboard? = nil,
       capturesHistory: @escaping () -> Bool = { true },
       showsHeader: Bool = true, onInsert: @escaping (String) -> Void, onClose: @escaping () -> Void) {
    self.store = store
    self.onInsert = onInsert
    self.cloud = cloud
    self.hasFullAccess = hasFullAccess
    self.capturesHistory = capturesHistory
    self.showsHeader = showsHeader
    super.init(frame: .zero)
    accessibilityIdentifier = "keyboardClipboardHistory"
    backgroundColor = showsHeader ? skin.background : .clear
    title.text = "剪贴板历史"
    title.font = showsHeader ? .boldSystemFont(ofSize: 16) : .systemFont(ofSize: 14, weight: .semibold)
    title.textColor = skin.keyForeground
    title.lineBreakMode = .byTruncatingHead
    title.accessibilityIdentifier = "clipboardTitle"
    let search = searchButton
    search.setImage(UIImage(systemName: "magnifyingglass"), for: .normal)
    search.accessibilityLabel = "搜索剪贴板"
    search.accessibilityIdentifier = "clipboardSearch"
    search.isEnabled = hasFullAccess
    search.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      if searchQuery == nil { beginSearch() } else { endSearch() }
    }, for: .primaryActionTriggered)
    let clear = clearButton
    clear.setTitle("清空", for: .normal)
    if !showsHeader { clear.titleLabel?.font = .systemFont(ofSize: 14, weight: .medium) }
    clear.accessibilityIdentifier = "clearClipboardHistory"
    clear.addAction(UIAction { [weak self, weak clear] _ in
      guard let self else { return }
      if !confirmingClear {
        confirmingClear = true
        clear?.setTitle("确认清空", for: .normal)
        status.text = "再次点按清空将删除全部历史，包括固定项。"
      } else {
        perform { try store.clear() }
        confirmingClear = false
        clear?.setTitle("清空", for: .normal)
      }
    }, for: .primaryActionTriggered)
    source.selectedSegmentIndex = 0
    source.accessibilityIdentifier = "clipboardSource"
    source.setContentHuggingPriority(.defaultLow, for: .horizontal)
    source.selectedSegmentTintColor = skin.keyBackground
    for state in [UIControl.State.normal, .selected] {
      source.setTitleTextAttributes([
        .foregroundColor: skin.keyForeground,
        .font: UIFont.systemFont(ofSize: showsHeader ? 13 : 12, weight: .medium),
      ], for: state)
    }
    source.isHidden = cloud == nil
    title.isHidden = cloud != nil
    source.addAction(UIAction { [weak self] _ in self?.showSource() }, for: .valueChanged)
    refresh.setImage(UIImage(systemName: "arrow.clockwise"), for: .normal)
    refresh.accessibilityLabel = "刷新云端内容"
    refresh.accessibilityIdentifier = "refreshCloudClipboard"
    refresh.isHidden = true
    refresh.addAction(UIAction { [weak self] _ in self?.cloud?.refresh() }, for: .primaryActionTriggered)
    let selectionControls = [
      (selectButton, "多选", "clipboardSelect"), (insertSelectedButton, "插入", "clipboardInsertSelected"),
      (cancelSelectionButton, "取消", "clipboardCancelSelection"),
    ]
    for (button, label, identifier) in selectionControls {
      button.setTitle(label, for: .normal)
      if !showsHeader { button.titleLabel?.font = .systemFont(ofSize: 14, weight: .medium) }
      button.accessibilityIdentifier = identifier
    }
    selectButton.accessibilityLabel = "多选剪贴板记录"
    selectButton.isEnabled = false
    selectButton.addAction(UIAction { [weak self] _ in self?.beginSelection() }, for: .primaryActionTriggered)
    insertSelectedButton.addAction(UIAction { [weak self] _ in self?.insertSelection() }, for: .primaryActionTriggered)
    cancelSelectionButton.addAction(UIAction { [weak self] _ in self?.endSelection() }, for: .primaryActionTriggered)
    insertSelectedButton.isHidden = true
    cancelSelectionButton.isHidden = true
    title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    let pasteboard = UIPasteboard.general
    let newCopy = hasFullAccess && capturesHistory() && ClipboardCapturePrompt.hasNewCopy(
      changeCount: pasteboard.changeCount, hasStrings: pasteboard.hasStrings)
    capture.configuration = captureConfiguration(newCopy: newCopy)
    capture.accessibilityIdentifier = "captureClipboard"
    capture.isEnabled = hasFullAccess
    clear.isEnabled = hasFullAccess
    capture.addAction(UIAction { [weak self] _ in
      guard let self else { return }
      guard self.capturesHistory() else {
        status.text = Self.privacyMessage
        return
      }
      let changeCount = pasteboard.changeCount
      var captured = false
      perform {
        try store.add(pasteboard.string ?? "")
        ClipboardCapturePrompt.markCaptured(changeCount: changeCount)
        captured = true
      }
      guard captured else { return }
      capture.configuration = captureConfiguration(newCopy: false)
    }, for: .primaryActionTriggered)
    status.font = .systemFont(ofSize: showsHeader ? 12 : 11.5)
    status.textColor = skin.secondary
    status.numberOfLines = showsHeader ? 2 : 1
    status.adjustsFontSizeToFitWidth = !showsHeader
    status.minimumScaleFactor = 0.85
    table.dataSource = self
    table.delegate = self
    table.backgroundColor = .clear
    table.separatorStyle = .none
    table.rowHeight = UITableView.automaticDimension
    table.estimatedRowHeight = 64
    table.register(ClipboardCardCell.self, forCellReuseIdentifier: ClipboardCardCell.reuseIdentifier)
    table.accessibilityIdentifier = "clipboardHistoryList"
    table.keyboardDismissMode = .none
    table.disableEdgeEffects()
    table.contentInsetAdjustmentBehavior = .never
    empty.text = "没有匹配的记录"
    empty.textAlignment = .center
    empty.textColor = skin.secondary
    empty.font = .systemFont(ofSize: 14)
    buildLetterPad()
    letterPad.isHidden = true

    let header: UIStackView
    if showsHeader {
      let close = UIButton(type: .system)
      close.setTitle("返回", for: .normal)
      // 返回先退出多选或搜索，再退出面板，与符号面板一致。
      close.addAction(UIAction { [weak self] _ in
        guard let self else { return }
        if selecting { endSelection() } else if searchQuery != nil { endSearch() } else { onClose() }
      }, for: .primaryActionTriggered)
      header = UIStackView(arrangedSubviews: [
        source, title, search, refresh, selectButton, cancelSelectionButton, insertSelectedButton, clear, close,
      ])
      header.spacing = 8
      NSLayoutConstraint.activate([
        close.widthAnchor.constraint(equalToConstant: 44), clear.widthAnchor.constraint(equalToConstant: 76),
        search.widthAnchor.constraint(equalToConstant: 36), refresh.widthAnchor.constraint(equalToConstant: 36),
      ])
    } else {
      let spacer = UIView()
      spacer.setContentHuggingPriority(.init(1), for: .horizontal)
      let symbol = UIImage.SymbolConfiguration(pointSize: 15, weight: .medium)
      search.setPreferredSymbolConfiguration(symbol, forImageIn: .normal)
      refresh.setPreferredSymbolConfiguration(symbol, forImageIn: .normal)
      header = UIStackView(arrangedSubviews: [
        source, title, spacer, capture, search, refresh, selectButton, cancelSelectionButton, insertSelectedButton, clear,
      ])
      header.spacing = 4
      header.alignment = .center
      source.setContentHuggingPriority(.defaultHigh, for: .horizontal)
      NSLayoutConstraint.activate([
        source.widthAnchor.constraint(lessThanOrEqualToConstant: 132), source.heightAnchor.constraint(equalToConstant: 28),
        capture.widthAnchor.constraint(equalToConstant: 32), capture.heightAnchor.constraint(equalToConstant: 32),
        search.widthAnchor.constraint(equalToConstant: 32), search.heightAnchor.constraint(equalToConstant: 32),
        refresh.widthAnchor.constraint(equalToConstant: 32), refresh.heightAnchor.constraint(equalToConstant: 32),
        clear.widthAnchor.constraint(greaterThanOrEqualToConstant: 44), clear.heightAnchor.constraint(equalToConstant: 32),
      ])
      for button in [selectButton, insertSelectedButton, cancelSelectionButton] {
        NSLayoutConstraint.activate([
          button.widthAnchor.constraint(greaterThanOrEqualToConstant: 44), button.heightAnchor.constraint(equalToConstant: 32),
        ])
      }
    }
    header.tintColor = skin.accent
    let children: [UIView] = showsHeader ? [header, capture, status, table, letterPad] : [header, status, table, letterPad]
    for child in children {
      child.translatesAutoresizingMaskIntoConstraints = false
      addSubview(child)
    }
    let sideInset: CGFloat = showsHeader ? 12 : 4
    let headerHeight: CGFloat = showsHeader ? 44 : 32
    // 约束先放进有类型的数组再激活：整个写在 activate 的字面量里时，CI 上的编译器会在类型推断上超时。
    let constraints: [NSLayoutConstraint] = [
      header.topAnchor.constraint(equalTo: topAnchor), header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: sideInset),
      header.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -sideInset),
      header.heightAnchor.constraint(equalToConstant: headerHeight),
      status.leadingAnchor.constraint(equalTo: header.leadingAnchor),
      status.trailingAnchor.constraint(equalTo: header.trailingAnchor), statusHeight,
      table.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 4), table.leadingAnchor.constraint(equalTo: leadingAnchor),
      table.trailingAnchor.constraint(equalTo: trailingAnchor), tableAboveBottom,
      letterPad.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
      letterPad.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
      letterPad.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
      // 竖屏手机上按键区约 200pt 高，紧凑字母键盘每行 26pt，上方正好放下紧凑标题栏和一条搜索结果；横屏更矮，字母键盘按 `tableMinimumHeight` 让出高度。
      letterPadHeight,
    ]
    NSLayoutConstraint.activate(constraints)
    if showsHeader {
      NSLayoutConstraint.activate([
        capture.topAnchor.constraint(equalTo: header.bottomAnchor), capture.leadingAnchor.constraint(equalTo: header.leadingAnchor),
        capture.trailingAnchor.constraint(equalTo: header.trailingAnchor), captureHeight,
        status.topAnchor.constraint(equalTo: capture.bottomAnchor, constant: 4),
      ])
    } else {
      status.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 2).isActive = true
    }
    if hasFullAccess { reload() }
    else { status.text = Self.fullAccessMessage }
    if let cloud {
      cloud.onChange = { [weak self] in self?.cloudChanged() }
      // The panel opening is the one moment the list is fetched without the user pressing refresh.
      cloud.refresh()
    }
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  override func willMove(toSuperview newSuperview: UIView?) {
    super.willMove(toSuperview: newSuperview)
    // Closing the panel drops whatever the cloud fetch still brings back.
    if newSuperview == nil { cloud?.close() }
  }

  /// 保存按钮：完整标题栏下是一个带文字的通栏按钮，紧凑控件行里是一个图标，其无障碍标签仍说明它的作用。有尚未保存的复制内容时显示为填充样式。
  private func captureConfiguration(newCopy: Bool) -> UIButton.Configuration {
    let label = newCopy ? "保存新复制的内容" : "保存当前剪贴板"
    capture.accessibilityLabel = label
    guard showsHeader else {
      var config = UIButton.Configuration.plain()
      config.image = UIImage(systemName: newCopy ? "doc.on.clipboard.fill" : "doc.on.clipboard")
      config.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(pointSize: 15, weight: .medium)
      config.baseForegroundColor = skin.accent
      config.contentInsets = .zero
      return config
    }
    var config = newCopy ? UIButton.Configuration.filled() : UIButton.Configuration.tinted()
    config.title = label
    config.image = UIImage(systemName: "doc.on.clipboard")
    config.imagePadding = 8
    config.baseBackgroundColor = skin.accent
    config.baseForegroundColor = newCopy ? skin.actionForeground : skin.accent
    return config
  }

  /// 显示或收起保存按钮；完整标题栏下它所在的那一行也随之收起。
  private func setCaptureShown(_ shown: Bool) {
    capture.isHidden = !shown
    if showsHeader { captureHeight.constant = shown ? 44 : 0 }
  }

  private func setStatusShown(_ shown: Bool) {
    status.isHidden = !shown
    statusHeight.constant = shown ? statusShownHeight : 0
  }

  private func reload() {
    do {
      let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
      let stored = preferences?["clipboard_history_limit"] as? Int ?? 50
      historyLimit = [50, 100, 200, 500, 1000].contains(stored) ? stored : 50
      items = try store.load()
      selectButton.isEnabled = hasFullAccess && !items.isEmpty
      if selecting {
        // 选中的记录在别处被删掉时不再记着它；一条都不剩就退出多选。
        let present = Set(items.map(\.text))
        selectedTexts = selectedTexts.filter { present.contains($0) }
        if items.isEmpty { endSelection() } else { updateSelectionChrome() }
      }
      localStatus()
      if searchQuery != nil { showSearch() } else if !showsCloud { table.reloadData() }
    } catch { status.text = error.localizedDescription }
  }
  private func localStatus() {
    guard !showsCloud else { return }
    if selecting {
      status.text = selectedTexts.count > ClipboardSelectionPolicy.maxInsertions
        ? "\(ClipboardSelectionPolicy.insertionLimitHint)，取消几项后再插入" : "点按选择，按 ①②③④ 的顺序插入"
      return
    }
    if let notice = cloud?.notice { status.text = notice; return }
    status.text = items.isEmpty ? "暂无历史。保存后点按插入；记录仅保存在本机。" : "\(items.count)/\(historyLimit) 条 · 点按插入 · 右侧菜单可固定或删除"
  }

  /// Switch between the local history and the 云端 list. The capture button, search and clear belong to the local history and fold away on the cloud side, where refresh takes their place.
  private func showSource() {
    confirmingClear = false
    clearButton.setTitle("清空", for: .normal)
    let cloudSide = showsCloud
    setCaptureShown(!cloudSide)
    searchButton.isHidden = cloudSide
    clearButton.isHidden = cloudSide
    selectButton.isHidden = cloudSide
    refresh.isHidden = !cloudSide
    setNeedsLayout()
    if cloudSide { cloudChanged() }
    else if hasFullAccess { reload() }
    else { status.text = Self.fullAccessMessage; table.reloadData() }
  }

  private func cloudChanged() {
    guard let cloud else { return }
    if showsCloud {
      let allowed = cloud.fieldAllowsCloud()
      status.text = allowed ? cloud.message : "当前输入框不显示云端内容。"
      refresh.isEnabled = allowed
      table.backgroundView = nil
      table.reloadData()
    } else {
      // The upload notice and whether 发到云剪贴板 is offered live in the local rows.
      localStatus()
      if searchQuery != nil { showSearch() } else { table.reloadData() }
    }
  }
  private var cloudItems: [BackendAccountClient.ClipboardItem] {
    guard let cloud, cloud.fieldAllowsCloud() else { return [] }
    return cloud.items
  }
  /// Open the search with an empty query. The capture button and the status line fold away, since a phone-height panel with a pad under it has room for the rows and little else, and the title carries the query.
  func beginSearch() {
    guard searchQuery == nil, !selecting else { return }
    confirmingClear = false
    clearButton.setTitle("清空", for: .normal)
    searchQuery = ""
    source.isHidden = true
    title.isHidden = false
    selectButton.isHidden = true
    setCaptureShown(false)
    setStatusShown(false)
    letterPad.isHidden = false
    tableAboveBottom.isActive = false
    tableAbovePad.isActive = true
    tableMinimumHeight.isActive = true
    setNeedsLayout()
    showSearch()
  }

  func endSearch() {
    guard searchQuery != nil else { return }
    searchQuery = nil
    setCaptureShown(true)
    setStatusShown(true)
    letterPad.isHidden = true
    tableAbovePad.isActive = false
    tableMinimumHeight.isActive = false
    tableAboveBottom.isActive = true
    setNeedsLayout()
    title.text = "剪贴板历史"
    source.isHidden = cloud == nil
    title.isHidden = cloud != nil
    selectButton.isHidden = showsCloud
    table.backgroundView = nil
    localStatus()
    table.reloadData()
  }

  /// 进入多选：清空的确认先收起，顶行只留计数、「取消」和「插入」（和 Android 多选顶行的先后一致），保存按钮和行菜单让开。只在本机历史、不在搜索中、且有记录时可进。
  func beginSelection() {
    guard hasFullAccess, !selecting, !showsCloud, searchQuery == nil, !items.isEmpty else { return }
    confirmingClear = false
    clearButton.setTitle("清空", for: .normal)
    selecting = true
    selectedTexts = []
    updateSelectionChrome()
    localStatus()
    table.reloadData()
  }

  func endSelection() {
    guard selecting else { return }
    selecting = false
    selectedTexts = []
    updateSelectionChrome()
    localStatus()
    table.reloadData()
  }

  /// 按序号把选中的几条首尾相接，经和单条插入同一个 `onInsert` 一次插入，面板随之关闭。
  private func insertSelection() {
    guard selecting, ClipboardSelectionPolicy.canInsert(selectedTexts.count) else { return }
    let text = ClipboardSelectionPolicy.insertionText(selectedTexts)
    endSelection()
    onInsert(text)
  }

  private func toggleSelection(_ text: String) {
    let next = ClipboardSelectionPolicy.toggled(text, in: selectedTexts)
    let capped = next == selectedTexts
    selectedTexts = next
    updateSelectionChrome()
    localStatus()
    if capped { status.text = "最多只能选择 \(ClipboardSelectionPolicy.maxSelections) 项" }
    table.reloadData()
  }

  /// 多选进出时顶行换一套控件；计数写在标题里，「插入」只在选中 1 到 4 项时可点。
  private func updateSelectionChrome() {
    let count = selectedTexts.count
    source.isHidden = selecting || cloud == nil
    title.isHidden = !selecting && cloud != nil
    title.text = selecting ? "\(count) 项已选择" : "剪贴板历史"
    setCaptureShown(!selecting)
    searchButton.isHidden = selecting
    clearButton.isHidden = selecting
    selectButton.isHidden = selecting
    insertSelectedButton.isHidden = !selecting
    cancelSelectionButton.isHidden = !selecting
    insertSelectedButton.isEnabled = ClipboardSelectionPolicy.canInsert(count)
    insertSelectedButton.accessibilityLabel = count > ClipboardSelectionPolicy.maxInsertions
      ? "插入不可用，\(ClipboardSelectionPolicy.insertionLimitHint)" : "按序号插入选中的剪贴板记录"
    setNeedsLayout()
  }

  func typeSearch(_ character: String) {
    guard let searchQuery, searchQuery.count < Self.searchLimit else { return }
    self.searchQuery = searchQuery + character
    showSearch()
  }

  func deleteSearchCharacter() {
    guard let searchQuery, !searchQuery.isEmpty else { return }
    self.searchQuery = String(searchQuery.dropLast())
    showSearch()
  }

  private func showSearch() {
    guard let query = searchQuery else { return }
    title.text = query.isEmpty ? "搜索剪贴板" : "搜索：\(query)"
    table.backgroundView = !query.isEmpty && shown.isEmpty ? empty : nil
    table.reloadData()
  }

  /// Letters and digits: what is searchable in a clipboard without a Chinese composition — links, codes, numbers and English. The digit row sits on top as on the desktop keyboard.
  private func buildLetterPad() {
    let spacing: CGFloat = showsHeader ? 4 : 3
    letterPad.axis = .vertical
    letterPad.spacing = spacing
    letterPad.distribution = .fillEqually
    letterPad.accessibilityIdentifier = "clipboardSearchPad"
    for (index, row) in ["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm"].enumerated() {
      let line = UIStackView()
      line.axis = .horizontal
      line.spacing = spacing
      line.distribution = .fillEqually
      for character in row {
        line.addArrangedSubview(padKey(title: String(character), symbol: nil, identifier: "clipboardSearchKey-\(character)") { [weak self] in
          self?.typeSearch(String(character))
        })
      }
      if index == 3 {
        let delete = padKey(title: nil, symbol: "delete.left", identifier: "clipboardSearchDelete") { [weak self] in
          self?.deleteSearchCharacter()
        }
        delete.accessibilityLabel = "删除搜索字符"
        line.addArrangedSubview(delete)
      }
      letterPad.addArrangedSubview(line)
    }
  }

  private func padKey(title: String?, symbol: String?, identifier: String, action: @escaping () -> Void) -> UIButton {
    let key = UIButton(type: .system)
    key.setTitle(title, for: .normal)
    if let symbol { key.setImage(UIImage(systemName: symbol), for: .normal) }
    key.setTitleColor(skin.keyForeground, for: .normal)
    key.tintColor = skin.keyForeground
    key.titleLabel?.font = .systemFont(ofSize: 16)
    key.backgroundColor = symbol == nil ? skin.keyBackground : skin.functionKeyBackground
    key.layer.cornerRadius = skin.cornerRadius
    key.accessibilityIdentifier = identifier
    key.addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
    return key
  }

  private func perform(_ action: () throws -> Void) {
    do { try action(); reload() }
    catch { status.text = error.localizedDescription }
  }
  func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
    showsCloud ? cloudItems.count : shown.count
  }
  func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
    let cell = tableView.dequeueReusableCell(withIdentifier: ClipboardCardCell.reuseIdentifier) as? ClipboardCardCell
      ?? ClipboardCardCell(style: .default, reuseIdentifier: ClipboardCardCell.reuseIdentifier)
    // 在按键区里搜索时字母键盘上方大约只放得下一张卡片，所以这时卡片只显示一行。
    let singleLine = !showsHeader && searchQuery != nil
    if showsCloud {
      let item = cloudItems[indexPath.row]
      cell.show(text: item.text, meta: Self.cloudMeta(item), skin: skin, inset: cardInset, singleLine: singleLine)
      cell.accessoryView = nil
      cell.accessibilityIdentifier = "cloudClipboardItem"
      cell.accessibilityLabel = nil
      cell.accessibilityHint = nil
      return cell
    }
    let item = shown[indexPath.row]
    if selecting {
      // 多选时不放行菜单，右上角留给序号角标；序号按点选先后算，取消一项后后面的各项在这次重画时自然前移。
      let ordinal = ClipboardSelectionPolicy.ordinal(of: item.text, in: selectedTexts)
      cell.accessoryView = nil
      cell.show(text: item.text, meta: Self.localMeta(item), skin: skin, inset: cardInset, singleLine: singleLine,
                ordinal: ordinal.flatMap(ClipboardSelectionPolicy.ordinalLabel))
      cell.accessibilityIdentifier = "clipboardSelectableItem"
      cell.accessibilityLabel = ClipboardSelectionPolicy.selectedDescription(ordinal) + item.text
      cell.accessibilityHint = "点按选择剪贴板记录"
      return cell
    }
    cell.show(text: item.text, meta: Self.localMeta(item), skin: skin, inset: cardInset, singleLine: singleLine)
    cell.accessibilityIdentifier = nil
    cell.accessibilityLabel = nil
    cell.accessibilityHint = nil
    let menu = UIButton(type: .system)
    menu.frame = CGRect(x: 0, y: 0, width: 44, height: 44)
    menu.setImage(UIImage(systemName: item.pinned ? "pin.fill" : "ellipsis"), for: .normal)
    menu.tintColor = item.pinned ? skin.accent : skin.secondary
    menu.accessibilityLabel = "管理记录"
    menu.showsMenuAsPrimaryAction = true
    menu.menu = UIMenu(children: [
      UIAction(title: item.pinned ? "取消固定" : "固定", image: UIImage(systemName: "pin")) { [weak self] _ in
        self?.change(item, delete: false)
      },
      UIAction(title: "删除", image: UIImage(systemName: "trash"), attributes: .destructive) { [weak self] _ in
        self?.change(item, delete: true)
      }
    ] + uploadAction(item))
    cell.accessoryView = menu
    return cell
  }
  private func change(_ item: ClipboardHistoryItem, delete: Bool) {
    perform {
      if delete { try store.remove(text: item.text) }
      else { try store.setPinned(!item.pinned, text: item.text) }
    }
  }
  /// 发到云剪贴板, offered wherever the 云端 tab is, and enabled once the panel has found a signed-in account whose cloud clipboard is on.
  private func uploadAction(_ item: ClipboardHistoryItem) -> [UIMenuElement] {
    guard let cloud else { return [] }
    return [UIAction(title: "发到云剪贴板", image: UIImage(systemName: "icloud.and.arrow.up"),
                     attributes: cloud.canUpload ? [] : .disabled) { [weak cloud] _ in cloud?.upload(item.text) }]
  }

  private static let relativeFormatter: RelativeDateTimeFormatter = {
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "zh-Hans")
    formatter.unitsStyle = .full
    formatter.dateTimeStyle = .named
    return formatter
  }()

  /// `date` 距今多久，写成「刚刚」「2分钟前」或「昨天」这样。晚于 `now` 的时间（来自另一台设备的时钟）也是「刚刚」。
  static func relativeTime(_ date: Date, now: Date = Date()) -> String {
    CloudClipboardTimePolicy.relative(date, now: now, formatter: relativeFormatter)
  }

  /// 本机条目的附注行：固定时先写「已固定」，再写保存于多久之前。本机存储不记录设备名。
  static func localMeta(_ item: ClipboardHistoryItem, now: Date = Date()) -> String {
    ((item.pinned ? ["已固定"] : []) + [relativeTime(item.date, now: now)]).joined(separator: " · ")
  }

  /// 云端条目的附注行：服务器标记为固定时写「已固定」，然后是写入它的设备（服务器没给时写「云端」），最后是多久之前。解析不了的时间戳原样显示。
  static func cloudMeta(_ item: BackendAccountClient.ClipboardItem, now: Date = Date()) -> String {
    let device = item.device.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.flatMap { $0.isEmpty ? nil : $0 } ?? "云端"
    let time = cloudDate(item.updated_at).map { relativeTime($0, now: now) } ?? item.updated_at
    return ((item.pinned == true ? ["已固定"] : []) + [device, time]).joined(separator: " · ")
  }

  /// 服务器返回的 ISO 8601 时间戳，带不带小数秒都能解析。
  static func cloudDate(_ value: String) -> Date? {
    CloudClipboardTimePolicy.date(value)
  }

  func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
    tableView.deselectRow(at: indexPath, animated: true)
    if showsCloud {
      let items = cloudItems
      guard indexPath.row < items.count else { return }
      cloud?.insert(items[indexPath.row], onInsert: onInsert)
      return
    }
    guard indexPath.row < shown.count else { return }
    if selecting {
      toggleSelection(shown[indexPath.row].text)
      return
    }
    onInsert(shown[indexPath.row].text)
  }
}

/// 按设计稿画成卡片的一条剪贴板记录：按键底色，圆角 10，内边距 9/12，无阴影，最多两行 14.5pt、行高 1.45 的正文，下面是一行 11.5pt 的附注，卡片下方留 6pt 间距，按下时亮度降到 0.9。卡片就是单元格的背景，所以附件位里的行菜单落在卡片内部。
private final class ClipboardCardCell: UITableViewCell {
  static let reuseIdentifier = "ClipboardCardCell"
  private static let gap: CGFloat = 6
  private let card = UIView()
  private let body = UILabel()
  private let meta = UILabel()
  /// 多选时选中项右上角的序号 ①②③④（#10411）；没选中或不在多选时隐藏。
  private let badge = UILabel()
  private var fill: UIColor = .clear
  private var cardLeading: NSLayoutConstraint!
  private var cardTrailing: NSLayoutConstraint!
  private var textLeading: NSLayoutConstraint!
  private var textTrailing: NSLayoutConstraint!

  override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
    super.init(style: style, reuseIdentifier: reuseIdentifier)
    backgroundColor = .clear
    contentView.backgroundColor = .clear
    selectionStyle = .none
    let background = UIView()
    background.backgroundColor = .clear
    card.layer.cornerRadius = 10
    card.layer.cornerCurve = .continuous
    card.translatesAutoresizingMaskIntoConstraints = false
    background.addSubview(card)
    backgroundView = background
    cardLeading = card.leadingAnchor.constraint(equalTo: background.leadingAnchor)
    cardTrailing = card.trailingAnchor.constraint(equalTo: background.trailingAnchor)

    body.font = .systemFont(ofSize: 14.5)
    meta.font = .systemFont(ofSize: 11.5)
    meta.numberOfLines = 1
    meta.lineBreakMode = .byTruncatingTail
    let column = UIStackView(arrangedSubviews: [body, meta])
    column.axis = .vertical
    column.spacing = 2
    column.translatesAutoresizingMaskIntoConstraints = false
    contentView.addSubview(column)
    textLeading = column.leadingAnchor.constraint(equalTo: contentView.leadingAnchor)
    textTrailing = column.trailingAnchor.constraint(equalTo: contentView.trailingAnchor)
    badge.font = .systemFont(ofSize: 18, weight: .semibold)
    badge.textAlignment = .center
    badge.isAccessibilityElement = false
    badge.isHidden = true
    badge.translatesAutoresizingMaskIntoConstraints = false
    background.addSubview(badge)
    NSLayoutConstraint.activate([
      card.topAnchor.constraint(equalTo: background.topAnchor),
      card.bottomAnchor.constraint(equalTo: background.bottomAnchor, constant: -Self.gap),
      cardLeading, cardTrailing,
      column.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 9),
      column.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -(9 + Self.gap)),
      textLeading, textTrailing,
      badge.topAnchor.constraint(equalTo: card.topAnchor, constant: 4),
      badge.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -6),
      badge.widthAnchor.constraint(equalToConstant: 22), badge.heightAnchor.constraint(equalToConstant: 22),
    ])
  }

  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// `ordinal` 不为 nil 时是多选中选中的一条：卡片铺强调色，右上角画序号，正文给角标让出位置。
  func show(text: String, meta metaText: String, skin: KeyboardTheme, inset: CGFloat, singleLine: Bool,
            ordinal: String? = nil) {
    let selected = ordinal != nil
    fill = selected ? skin.accent : skin.keyBackground
    card.backgroundColor = isHighlighted ? fill.keyboardPressedBrightness(0.9) : fill
    cardLeading.constant = inset
    cardTrailing.constant = -inset
    textLeading.constant = inset + 12
    badge.text = ordinal
    badge.textColor = skin.onAccent
    badge.isHidden = !selected
    // 行菜单放在附件位里，附件位本身已经和卡片边缘留有距离。
    textTrailing.constant = textTrailingConstant
    let font = body.font ?? .systemFont(ofSize: 14.5)
    let paragraph = NSMutableParagraphStyle()
    paragraph.lineSpacing = max(0, 14.5 * 1.45 - font.lineHeight)
    body.attributedText = NSAttributedString(string: text, attributes: [
      .font: font, .foregroundColor: selected ? skin.onAccent : skin.keyForeground, .paragraphStyle: paragraph,
    ])
    body.numberOfLines = singleLine ? 1 : 2
    body.lineBreakMode = .byTruncatingTail
    meta.text = metaText
    meta.textColor = selected ? skin.onAccent.withAlphaComponent(0.8) : skin.secondary
    meta.isHidden = singleLine
  }

  /// 正文右边距：有行菜单时贴着附件位，有序号角标时让出角标的宽度，否则与左边对称。
  private var textTrailingConstant: CGFloat {
    guard accessoryView == nil else { return -4 }
    return -(cardLeading.constant + (badge.isHidden ? 12 : 32))
  }

  override var accessoryView: UIView? {
    didSet { if textTrailing != nil { textTrailing.constant = textTrailingConstant } }
  }

  override func setHighlighted(_ highlighted: Bool, animated: Bool) {
    super.setHighlighted(highlighted, animated: animated)
    card.backgroundColor = highlighted ? fill.keyboardPressedBrightness(0.9) : fill
  }
}
