import AppKit
import SwiftUI

struct MacEmojiView: View {
  let resources: String
  let preferencesDirectory: String
  @ObservedObject var appearance = MacEmojiAppearance.shared
  @Environment(\.colorScheme) private var systemColorScheme
  private var palette: MacEmojiPalette {
    MacEmojiPalette(light: (appearance.colorScheme ?? systemColorScheme) == .light)
  }
  @State private var search = ""
  @State private var category = "home"
  @State private var pendingEmojiGroup: String?
  @State private var navigationRevision: UInt = 0
  private var scrollID: [String] { [search, category, parent, group, String(navigationRevision)] }
  private func resetNavigation() {
    navigationRevision &+= 1
    selectedIndex = 0
  }
  private var emojiPage: Bool { category == "" || category == "recent" }
  private var emojiSection: Binding<MacEmojiSectionChoice> {
    Binding(get: { category == "recent" ? .recent : .group(group) }, set: { choice in
      resetNavigation()
      switch choice {
      case .recent: pendingEmojiGroup = nil; category = "recent"
      case .group(let name):
        if category == "" { group = name }
        else { pendingEmojiGroup = name; category = "" }
      }
    })
  }
  private func navigate(_ rawValue: String) {
    guard let page = MacEmojiMainPage(rawValue: rawValue) else { return }
    resetNavigation()
    toast.dismiss()
    pendingEmojiGroup = page == .emoji ? "" : nil
    category = page.destination(hasRecents: !recent.items.isEmpty)
  }
  private var mediaPage: MacEmojiMediaPage? { MacEmojiMediaPage(rawValue: category) }
  private var columns: Int { category == "clipboard" ? 1 : MacEmojiGridMetrics.columns }
  @State private var group = ""
  @State private var parent = ""
  @State private var symbolGroups: [MacEmojiSymbolGroup] = []
  @State private var symbolTabs: [MacEmojiCategoryTab<String>] = []
  @State private var groups: [String] = []
  @State private var groupsCategory: String?
  @State private var groupsFailed = false
  @State private var selectedIndex = 0
  @State private var recent = MacEmojiRecents()
  @State private var toast = MacEmojiToastState()
  @State private var historyRevision = 0
  @State private var historyEnabled: Bool?
  @State private var enablingHistory = false
  @ObservedObject private var clipboardService = MacClipboardService.shared
  @State private var deletingHistory = false
  @State private var deletionNotice = ""
  @State private var sendingToCloud = false
  @State private var loadedQuery: [String] = []
  private var queryID: [String] { [search, category, parent, group, String(category == "recent" ? recent.revision : 0), String(category == "clipboard" ? historyRevision : 0)] }
  @State private var items: [MacEmojiCatalogItem] = []
  /// 颜文字页的分节（内置 All 加插件分组），各节 `items` 按顺序拼起来就是 `items`。
  @State private var kaomojiSections: [MacEmojiSymbolSection] = []
  @State private var status = "正在加载…"
  @State private var selection = MacEmojiSelectionState()
  var onSelect: (String) -> Bool
  var copyText: (String) -> Bool = { MacEmojiClipboard.copy($0) }

  private func copyItem(_ item: MacEmojiCatalogItem) {
    if category != "clipboard" { recent.recordSelection(item) }
    toast.copied(item.text, success: copyText(item.text))
  }

  private func enableHistory() {
    guard !enablingHistory else { return }
    enablingHistory = true
    let directory = preferencesDirectory
    let requestedID = queryID
    Task {
      defer { enablingHistory = false }
      do {
        try await Task.detached { try MacEmojiClipboardHistory.enable(directory: directory) }.value
        guard requestedID == queryID else { return }
        historyRevision += 1
        toast.show("剪贴板已开启")
      } catch {
        guard requestedID == queryID else { return }
        toast.show("无法开启剪贴板")
      }
    }
  }

  private func removeHistory(_ text: String) {
    guard !deletingHistory else { return }
    deletingHistory = true
    deletionNotice = ""
    let directory = preferencesDirectory
    let requestedID = queryID
    Task {
      defer { deletingHistory = false }
      do {
        let removed = try await Task.detached {
          try MacEmojiClipboardHistory.remove(directory: directory, text: text)
        }.value
        guard queryID == requestedID else { return }
        historyRevision += 1
        deletionNotice = removed ? "已删除历史记录（不会清空系统剪贴板）" : "记录已不存在"
      } catch {
        guard queryID == requestedID else { return }
        deletionNotice = "无法删除历史记录，请重试"
      }
    }
  }
  // Only the entry the user picked is uploaded; the provider checks the account and the server's enabled flag first.
  private func sendToCloud(_ text: String) {
    guard !sendingToCloud else { return }
    sendingToCloud = true
    Task {
      defer { sendingToCloud = false }
      toast.show(await BackendCloudClipboardProvider.send(text).message)
    }
  }
  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack(spacing: 0) {
        Text("Emoji and more").font(.system(size: 12, weight: .semibold))
        Spacer()
        MacEmojiCloseButton().frame(width: 28, height: 40 * 2 / 3)
      }.frame(height: 27)
      if category == "home" {
        MacEmojiMainTabs(selected: .home, palette: palette, navigate: navigate)
      } else {
        HStack {
          MacEmojiBackButton(palette: palette) { navigate("home") }
          if emojiPage {
            MacEmojiCategoryTabs(tabs: MacEmojiCategoryIcons.emojiTabs(groupsCategory == category ? groups : []),
              selected: emojiSection.wrappedValue, palette: palette, select: { emojiSection.wrappedValue = $0 })
          } else if category == "symbols" && !symbolTabs.isEmpty {
            MacEmojiCategoryTabs(tabs: symbolTabs, selected: parent, palette: palette, select: { resetNavigation(); parent = $0 })
          } else { Text(MacEmojiMainPage.title(category: category)).font(.system(size: 16, weight: .semibold)) }
        }
      }
      if groupsFailed { Text("分类加载失败，可返回首页重试").font(.caption).foregroundStyle(MacEmojiPalette.color(palette.muted)) }
      MacEmojiSearchField(text: $search,
        presentation: MacEmojiSearchPresentation(category: category),
        palette: palette)
      if category == "clipboard" {
        if historyEnabled == false && loadedQuery == queryID {
          MacEmojiClipboardDisabledView(palette: palette, enabling: enablingHistory, enable: enableHistory)
        }
        HStack {
          Text(clipboardService.status?.message ?? "正在检查剪贴板采集设置…").font(.caption)
          Spacer()
          Button("刷新") { historyRevision += 1 }
        }
        if !deletionNotice.isEmpty { Text(deletionNotice).font(.caption) }
      }
      if selection.rejected {
        Text(MacEmojiSelectionState.failureMessage).font(.caption)
          .foregroundStyle(MacEmojiPalette.color(palette.text))
      }
      if category == "home" {
        MacEmojiHomeView(resources: resources, search: search, recent: recent.matching(search),
          navigationRevision: navigationRevision, palette: palette, copy: copyItem, more: navigate)
      } else if let page = mediaPage {
        MacEmojiMediaPlaceholder(page: page, palette: palette)
      } else {
      GeometryReader { geometry in
      let flowWidth = max(0, geometry.size.width - 16)
      let flowSections = loadedQuery == queryID && category == "kaomoji" ? kaomojiSections : []
      let sectionCells = flowSections.map { MacEmojiFlow.cells(texts: $0.items.map(\.text), width: flowWidth) }
      let flowCells = MacEmojiFlow.stacked(sectionCells)
      ScrollViewReader { proxy in
        VStack(spacing: 4) {
          MacEmojiKeyboardEntry(enabled: loadedQuery == queryID && !items.isEmpty) { command in
            if command == .activate && !items.indices.contains(selectedIndex) { return }
            guard loadedQuery == queryID,
                  let index = category == "kaomoji" && (command == .up || command == .down)
                    ? MacEmojiFlow.vertical(from: selectedIndex, direction: command == .up ? -1 : 1, cells: flowCells)
                    : command.destination(from: selectedIndex, count: items.count, columns: columns) else { return }
            selectedIndex = index
            proxy.scrollTo(index)
            if case .activate = command { copyItem(items[index]) }
          }.frame(height: 24)
          MacEmojiScroll(resetID: scrollID) {
            if loadedQuery == queryID && items.isEmpty && category != "clipboard" {
              MacEmojiEmptyStateView(category: category, search: search, group: group, hasRecents: !recent.items.isEmpty, palette: palette)
            } else if category == "kaomoji" {
              VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(flowSections.enumerated()), id: \.element.id) { index, section in
                  MacEmojiDetailSection(title: section.title, palette: palette) {
                    MacEmojiFlowGrid(items: section.items, cells: sectionCells[index], width: flowWidth, palette: palette,
                      selected: { selectedIndex == section.start + $0 }, identity: { section.start + $0 },
                      copy: { selectedIndex = section.start + $0; copyItem(section.items[$0]) })
                  }.frame(width: flowWidth, alignment: .leading)
                }
              }
            } else if category == "symbols" {
              MacEmojiSymbolSectionsView(items: loadedQuery == queryID ? items : [], palette: palette,
                selectedIndex: selectedIndex, copy: { selectedIndex = $0; copyItem(items[$0]) })
            } else if category == "clipboard" {
            LazyVGrid(columns: [GridItem(.flexible())], spacing: MacClipboardPreview.gap) {
              ForEach(Array((loadedQuery == queryID ? items : []).enumerated()), id: \.offset) { index, item in
                  MacEmojiClipboardRow(text: item.text, palette: palette, selected: selectedIndex == index,
                    deleting: deletingHistory, copy: { selectedIndex = index; copyItem(item) },
                    remove: { removeHistory(item.text) }, sendToCloud: { sendToCloud(item.text) })
                    .id(index)
              }
            }
            } else {
              if loadedQuery == queryID {
              MacEmojiDetailSection(title: category == "recent" ? "Recent" : group, palette: palette) {
              MacEmojiGrid(items: loadedQuery == queryID ? items : [], palette: palette,
                selected: { selectedIndex == $0 }, identity: { $0 },
                copy: { selectedIndex = $0; copyItem(items[$0]) })
              }.frame(width: MacEmojiGridMetrics.width, alignment: .leading)
              }
            }
          }
        }
      }
      }
      .overlayPreferenceValue(MacClipboardTooltipPreference.self) { anchors in
        MacClipboardTooltipOverlay(anchors: anchors, light: palette.background == 0xF7F7FA)
      }
      Button("插入所选项") {
        guard loadedQuery == queryID, items.indices.contains(selectedIndex) else { return }
        selection.submit(items[selectedIndex].text, send: onSelect)
      }.disabled(loadedQuery != queryID || !items.indices.contains(selectedIndex))
      }
    }.padding(20).frame(minWidth: MacEmojiGridMetrics.minimumPanelWidth, minHeight: 500)
      .background(MacEmojiPalette.color(palette.background))
      .foregroundStyle(MacEmojiPalette.color(palette.text))
      .tint(MacEmojiPalette.color(palette.accent))
      .preferredColorScheme(appearance.colorScheme)
      .overlay { MacEmojiToastOverlay(message: toast.message, light: palette.background == 0xF7F7FA) }
      .task(id: toast.generation) {
        guard toast.message != nil else { return }
        let generation = toast.generation
        await MacEmojiToastState.expire(generation: generation) { toast.dismiss(ifGeneration: $0) }
      }
      .onChange(of: search) { _ in toast.dismiss() }
      .onChange(of: category) { _ in group = ""; parent = ""; toast.dismiss() }
      .onChange(of: parent) { _ in group = "" }
      .task(id: category) {
        groupsCategory = nil
        groups = []
        symbolGroups = []
        symbolTabs = []
        groupsFailed = false
        let selectedCategory = category
        let directory = resources
        if selectedCategory == "home" || selectedCategory == "clipboard" || MacEmojiMediaPage(rawValue: selectedCategory) != nil { groupsCategory = selectedCategory; return }
        do {
          if selectedCategory == "symbols" {
            let preferences = preferencesDirectory
            let worker = Task.detached {
              let groups = try MacEmojiCatalog.loadSymbolGroups(resources: directory)
              let tabs = try MacEmojiCategoryIcons.symbolTabs(groups) { parent in
                try Task.checkCancellation()
                return try MacEmojiCatalog.load(resources: directory, search: "", category: "symbols", parent: parent, limit: 1).first?.text
              }
              return (groups, tabs)
            }
            // 插件分类与内置目录分开读：插件读不出来时只剩内置分类，内置目录读不出来时插件分类照常显示。
            let plugins = Task.detached {
              MacEmojiCategoryIcons.pluginSymbolTabs(MacEmojiCatalog.installedPluginSymbolGroups(resources: directory, preferencesDirectory: preferences))
            }
            let result = try? await withTaskCancellationHandler(operation: { try await worker.value }, onCancel: { worker.cancel() })
            let pluginTabs = await withTaskCancellationHandler(operation: { await plugins.value }, onCancel: { plugins.cancel() })
            try Task.checkCancellation()
            symbolGroups = result?.0 ?? []
            symbolTabs = (result?.1 ?? []) + pluginTabs
            parent = symbolTabs.first?.id ?? ""
            groupsFailed = result == nil
            if result != nil { groupsCategory = selectedCategory }
            return
          }
          let result = try await Task.detached {
            try MacEmojiCatalog.loadGroups(resources: directory, category: selectedCategory == "recent" ? "" : selectedCategory)
          }.value
          try Task.checkCancellation()
          groups = result
          groupsCategory = selectedCategory
          if selectedCategory == "" {
            group = MacEmojiSectionChoice.resolvedGroup(requested: pendingEmojiGroup, current: group, groups: result)
            pendingEmojiGroup = nil
          }
        } catch {
          guard !Task.isCancelled else { return }
          groupsFailed = true
        }
      }
      .task(id: queryID) {
        historyEnabled = nil
        items = []
        kaomojiSections = []
        selectedIndex = 0
        loadedQuery = []
        status = "正在加载…"
        if MacEmojiMediaPage(rawValue: category) != nil { status = ""; loadedQuery = queryID; return }
        if category == "home" { status = "首页预览 · 更多可进入完整目录"; return }
        if category == "recent" {
          items = recent.matching(search)
          loadedQuery = queryID
          status = items.isEmpty ? "最近使用为空或没有匹配项" : "共 \(items.count) 项"
          return
        }
        if category == "clipboard" {
          let directory = preferencesDirectory
          let requestedID = queryID
          let query = search
          await MacEmojiClipboardHistory.observe(read: {
            try await Task.detached {
              try MacEmojiClipboardHistory.load(directory: directory)
            }.value
          }, publish: { history in
            historyEnabled = history?.enabled
            // Preserve the selected record across insertions/reordering, never
            // silently move the explicit-insert action to a different record.
            let selectedText = items.indices.contains(selectedIndex) ? items[selectedIndex].text : nil
            items = history?.matching(query) ?? []
            selectedIndex = selectedText.flatMap { text in items.firstIndex { $0.text == text } } ?? -1
            loadedQuery = requestedID
            guard let history else {
              status = "剪贴板历史不可用，请检查共享存储配置"
              return
            }
            status = !history.enabled ? "剪贴板历史已关闭" : items.isEmpty ? "没有已保存的匹配记录" : "共 \(items.count) 项"
          })
          return
        }
        do {
          try await Task.sleep(nanoseconds: 200_000_000)
          let query = search
          let directory = resources
          let selectedCategory = category
          let filters = selectedCategory == "symbols"
            ? MacEmojiSymbolGroup.queryFilters(search: query, parent: parent, group: group) : (parent: parent, group: group)
          let selectedGroup = filters.group
          let selectedParent = filters.parent
          let requestedID = queryID
          let preferences = preferencesDirectory
          let worker = Task.detached { () -> ([MacEmojiCatalogItem], [MacEmojiSymbolSection]) in
            guard selectedCategory == "symbols" || selectedCategory == "kaomoji" else {
              return (try MacEmojiCatalog.loadAll(resources: directory, search: query, category: selectedCategory, group: selectedGroup, parent: selectedParent), [])
            }
            // 插件组追加在内置目录之后；读取失败时得到空列表，只显示内置目录。
            let plugins = MacEmojiCatalog.installedPluginSymbolGroups(resources: directory, preferencesDirectory: preferences)
            if selectedCategory == "kaomoji" {
              let builtIn = try MacEmojiCatalog.loadAll(resources: directory, search: query, category: selectedCategory, group: selectedGroup, parent: selectedParent)
              let sections = MacEmojiSymbolSections.kaomoji(builtIn: builtIn, plugins: plugins, search: query)
              return (sections.flatMap(\.items), sections)
            }
            // 选中插件包时内置目录没有对应分类，不去查询。
            let builtIn = MacEmojiPluginSymbolGroup.isParentID(selectedParent) ? []
              : try MacEmojiCatalog.loadAll(resources: directory, search: query, category: selectedCategory, group: selectedGroup, parent: selectedParent)
            return (builtIn + MacEmojiPluginSymbolGroup.symbolItems(plugins, search: query, parent: selectedParent), [])
          }
          let loaded = try await withTaskCancellationHandler(operation: { try await worker.value }, onCancel: { worker.cancel() })
          try Task.checkCancellation()
          guard requestedID == queryID else { return }
          let result = loaded.0
          items = result
          kaomojiSections = loaded.1
          loadedQuery = requestedID
          status = result.isEmpty ? "没有匹配的表情" : "共 \(result.count) 项"
        } catch {
          guard !Task.isCancelled else { return }
          items = []
          status = category == "clipboard" ? "剪贴板历史不可用，请检查共享存储配置" : "表情目录不可用，请检查本地资源配置"
        }
      }
  }
}

private struct MacEmojiCloseButton: NSViewRepresentable {
  func makeNSView(context: Context) -> NSButton {
    let button = CloseButton(title: "", target: nil, action: nil)
    button.target = button; button.action = #selector(CloseButton.closePanel)
    button.isBordered = false
    button.addSubview(CloseIcon(frame: .zero))
    button.setAccessibilityLabel("关闭表情面板"); button.toolTip = "关闭"
    return button
  }
  func updateNSView(_ button: NSButton, context: Context) {}
  final class CloseButton: NSButton {
    override func layout() { super.layout(); subviews.first?.frame = bounds.insetBy(dx: 2, dy: 2) }
    @objc func closePanel() { window?.close() }
  }
  final class CloseIcon: NSView {
    override func draw(_ dirtyRect: NSRect) {
      guard let context = NSGraphicsContext.current?.cgContext else { return }
      let half = min(bounds.width, bounds.height) * 0.19
      let width = max(min(bounds.width, bounds.height) * 0.055, 1)
      context.setStrokeColor(NSColor.labelColor.cgColor); context.setLineWidth(width)
      context.move(to: CGPoint(x: bounds.midX - half, y: bounds.midY - half)); context.addLine(to: CGPoint(x: bounds.midX + half, y: bounds.midY + half))
      context.move(to: CGPoint(x: bounds.midX + half, y: bounds.midY - half)); context.addLine(to: CGPoint(x: bounds.midX - half, y: bounds.midY + half)); context.strokePath()
    }
  }
}
