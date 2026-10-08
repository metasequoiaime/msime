import SwiftUI

extension PersonalDictionaryStore {
  /// 设置页读写的存储。在模拟器 debug 构建中，UI 测试会传 `-personalDictionaryTestID <uuid>` 把队列放进临时目录，所有读取队列的页面都必须用同一个存储。
  static func forSettingsPages() -> PersonalDictionaryStore {
    #if DEBUG && targetEnvironment(simulator)
    let arguments = ProcessInfo.processInfo.arguments
    if let index = arguments.firstIndex(of: "-personalDictionaryTestID"), index + 1 < arguments.count,
       let id = UUID(uuidString: arguments[index + 1]) {
      return PersonalDictionaryStore(directory: FileManager.default.temporaryDirectory
        .appendingPathComponent("personal-dictionary-ui-\(id.uuidString)"))
    }
    #endif
    return PersonalDictionaryStore()
  }
}

struct PersonalDictionaryView: View {
  private struct Editing: Identifiable { let id = UUID(); var previous: PersonalWord? }
  @State private var state = PersonalDictionaryState()
  @State private var editing: Editing?
  @State private var deleting: PersonalWord?
  @State private var error: String?
  @State private var lastReadError: String?
  @State private var search = ""
  /// `nil` searches the user's own words; a kind searches that whole dictionary, bundled words included, the way the Windows dictionary manager does.
  @State private var searchKind: PersonalWordKind?
  @State private var importing = false
  @State private var showsSync = false
  /// Windows 一次导出一个词库，词在前或编码在前；导出词库旁边提供同样的两种选择。
  @State private var exportKind = PersonalWordKind.pinyin
  @State private var exportFormat = "standard"
  @State private var exportCopy: (id: UUID, url: URL)?
  @State private var exportCopyAttempt: UUID?
  private let store = PersonalDictionaryStore.forSettingsPages()

  /// An ASCII search is a code prefix, which the keyboard looks up across the whole store; anything else, such as the word itself, can only filter the page already here.
  private var codeQuery: String? {
    let query = search.trimmingCharacters(in: .whitespaces).lowercased()
    guard !query.isEmpty, query.allSatisfy(\.isASCII),
          query.utf8.count <= PersonalDictionaryStore.maximumQueryBytes else { return nil }
    return query
  }
  private var showsSearchResults: Bool { codeQuery != nil && codeQuery == state.pageQuery && searchKind == state.pageKind }
  private var awaitingKeyboard: Bool {
    state.requestedPageOffset != state.pageOffset || state.requestedQuery != state.pageQuery
      || state.requestedKind != state.pageKind
  }
  private func requestSearch() {
    guard let codeQuery, codeQuery != state.requestedQuery || searchKind != state.requestedKind else { return }
    perform { try store.requestPage(offset: 0, kind: searchKind, query: codeQuery) }
  }
  private var visibleEntries: [PersonalWord] {
    // The keyboard matched the code with the Engine's own rule, which ignores pinyin separators, so its answer is shown whole.
    if showsSearchResults { return state.entries }
    return state.entries.filter { search.isEmpty || $0.value.localizedCaseInsensitiveContains(search) || $0.key.localizedCaseInsensitiveContains(search) }
  }
  private func requestPage(_ offset: Int) {
    perform { try store.requestPage(offset: offset, kind: state.requestedKind, query: state.requestedQuery) }
  }

  var body: some View {
    List {
      // 键盘同步、试打框和同步进度原来是页面顶上的三组，词条要往下翻才看得到。现在收成一行状态，点进去才是试打框和每一条的进度；最近一条未完成的操作直接写在这一行里，不用点进去也知道卡在哪。
      Section {
        statusRow.designRow()
      }
      Section {
        if let codeQuery, codeQuery == state.requestedQuery, searchKind == state.requestedKind, awaitingKeyboard {
          Text("已请求在\(searchedDictionary(searchKind))里搜索「\(codeQuery)」，在顶部「同步状态」里打开试打框让键盘查找。")
            .font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
            .designRow()
        }
        if visibleEntries.isEmpty {
          Text(search.isEmpty ? "还没有个人词条。"
               : showsSearchResults ? "\(searchedDictionary(state.pageKind))里没有以「\(state.pageQuery)」开头的编码。" : "本页没有匹配的词条。")
            .font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 8)
            .designRow()
        }
        ForEach(visibleEntries) { word in
          entryRow(word).designRow()
        }
        Button { editing = Editing(previous: nil) } label: {
          HStack(spacing: 8) {
            Image(systemName: "plus").font(.system(size: 16, weight: .semibold)).accessibilityHidden(true)
            Text("添加词条").font(.system(size: 17))
            Spacer(minLength: 0)
          }
          .foregroundStyle(MetasequoiaTheme.accent)
          .frame(minHeight: 34)
          .contentShape(Rectangle())
        }
        .accessibilityLabel("添加词条").accessibilityIdentifier("addPersonalWord")
        .designRow()
      } header: {
        if showsSearchResults { Text("编码以「\(state.pageQuery)」开头的词条") }
      } footer: {
        Text("包括手动添加和引擎学习生成的词条，每页最多 100 条。搜索编码时由键盘在整个个人词库里查，搜索汉字只筛当前页；搜索时选拼音、五笔、快捷短语或英文，会连同标着「内置」的词条一起查，内置词条只能调权重或删除。滑动词条可删除。")
      }
      // 翻页原本是词条组最后一行里三个并排的小按钮,和词条自己的点按、侧滑挤在同一片区域。
      if state.pageOffset > 0 || state.hasMore {
        Section {
          SettingsActionRow(title: "上一页", symbol: "chevron.left", enabled: state.pageOffset > 0) {
            requestPage(max(0, state.pageOffset - 100))
          }
          .designRow()
          SettingsActionRow(title: "下一页", symbol: "chevron.right", enabled: state.hasMore) {
            requestPage(state.pageOffset + 100)
          }
          .designRow()
        } footer: {
          Text(awaitingKeyboard
               ? "等待键盘读取第 \(state.requestedPageOffset / 100 + 1) 页…"
               : "第 \(state.pageOffset / 100 + 1) 页")
        }
      }
      fileSection
    }
    .designPage()
    .settingsStatus(busy: false, message: state.snapshotError)
    .navigationTitle(searchKind.map { "\($0.title)词库" } ?? "个人词库")
    .navigationBarTitleDisplayMode(.inline)
    .searchable(text: $search, prompt: "搜索词条")
    .searchScopes($searchKind, activation: .onSearchPresentation) {
      Text("我的词条").tag(PersonalWordKind?.none)
      ForEach(PersonalWordKind.allCases) { Text($0.title).tag(Optional($0)) }
    }
    .onSubmit(of: .search) { requestSearch() }
    .onChange(of: searchKind) { _, _ in requestSearch() }
    .onChange(of: search) { _, value in
      // Clearing the search goes back to the whole store's first page rather than leaving the last search's results in place.
      guard value.isEmpty, !state.requestedQuery.isEmpty || state.requestedKind != nil else { return }
      perform { try store.requestPage(offset: 0) }
    }
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) {
        Button { requestPage(state.pageOffset) } label: { Image(systemName: "arrow.clockwise") }
          .accessibilityLabel("刷新个人词库")
      }
    }
    .navigationDestination(isPresented: $showsSync) { PersonalDictionarySyncView(store: store) }
    .sheet(item: $editing) { item in
      PersonalWordEditor(previous: item.previous) { replacement in
        try store.enqueue(previous: item.previous, replacement: replacement)
        refresh()
      }
    }
    .sheet(isPresented: $importing) {
      PersonalDictionaryImportView { words in
        try store.enqueueImport(words)
        refresh()
        ToastCenter.shared.show("已加入导入队列")
      }
    }
    .alert(deleting?.isBundled == true ? "删除内置词条？" : "删除个人词条？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
      Button("取消", role: .cancel) { deleting = nil }
      Button("删除", role: .destructive) {
        if let word = deleting { perform { try store.enqueue(previous: word, replacement: nil) } }
        deleting = nil
      }
    } message: {
      Text(deleting?.isBundled == true
           ? "这个词随词库一起安装。删除后它不再出现在候选里，这一改动在键盘同步后生效，并保留到之后的词库升级。"
           : "删除将在键盘同步后生效，并保留到之后的词库升级。")
    }
    .alert("个人词库", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
      Button("好", role: .cancel) { error = nil }
    } message: { Text(error ?? "") }
    .task {
      while !Task.isCancelled {
        refresh()
        do { try await Task.sleep(nanoseconds: 1_000_000_000) } catch { break }
      }
    }
  }

  /// 顶部紧凑的同步行：有多少条编辑在等键盘、最新一条尚未应用的编辑，或者键盘上次确认的时间。点击它会打开试用输入框和逐条编辑的进度。其中的文本保持为独立的无障碍元素，让状态能单独朗读（也能单独被找到）；点击其中任何一个都会打开详情。
  private var statusRow: some View {
    let unapplied = state.requests.filter { $0.status != .applied }
    let failed = unapplied.filter { $0.status == .failed }.count
    let title = state.pendingCount > 0 ? "\(state.pendingCount) 项等待键盘同步" : failed > 0 ? "\(failed) 项同步失败" : "已全部同步"
    let symbol = state.pendingCount > 0 ? "clock.arrow.circlepath" : failed > 0 ? "exclamationmark.circle.fill" : "checkmark.circle.fill"
    return HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 18, weight: .semibold))
        .foregroundStyle(state.pendingCount == 0 && failed > 0 ? MetasequoiaTheme.danger : MetasequoiaTheme.accent)
        .frame(width: 30, height: 30)
        .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 2) {
        Text(title).font(.system(size: 17)).foregroundStyle(.primary)
          .accessibilityHint("轻点两下查看同步详情和试打框")
        if let latest = unapplied.last {
          HStack(spacing: 6) {
            Text(Self.requestTitle(latest)).lineLimit(1)
            Text(Self.requestStatus(latest)).lineLimit(1)
              .foregroundStyle(latest.status == .failed ? MetasequoiaTheme.danger : MetasequoiaTheme.sub)
          }
          .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        } else {
          Text(state.snapshotDate.map { "最近同步 \($0.formatted(date: .abbreviated, time: .standard))" }
               ?? "尚未收到键盘确认，保存的操作暂不会标记为已生效")
            .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      Spacer(minLength: 8)
      Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.sub.opacity(0.6))
        .accessibilityHidden(true)
    }
    .frame(minHeight: 44)
    .contentShape(Rectangle())
    .onTapGesture { showsSync = true }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("personalDictionarySyncStatus")
  }

  /// 一个词条：17pt 的词带内置标记，下方是 13pt 的编码，右侧用等宽数字显示权重。
  private func entryRow(_ word: PersonalWord) -> some View {
    let code = Self.codeLabel(word)
    return Button { editing = Editing(previous: word) } label: {
      HStack(spacing: 12) {
        VStack(alignment: .leading, spacing: 1) {
          HStack(spacing: 6) {
            Text(word.value).font(.system(size: 17)).foregroundStyle(Color.primary).lineLimit(3)
            if word.isBundled {
              Text("内置").font(.caption2.weight(.semibold)).foregroundStyle(MetasequoiaTheme.sub)
                .padding(.horizontal, 5).padding(.vertical, 1)
                .overlay(Capsule().stroke(MetasequoiaTheme.sub.opacity(0.5)))
            }
          }
          Text(code).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .lineLimit(1).truncationMode(.tail)
        }
        Spacer(minLength: 8)
        Text(String(word.weight)).font(.system(size: 14)).monospacedDigit().foregroundStyle(MetasequoiaTheme.sub)
      }
      .frame(minHeight: 40)
      .contentShape(Rectangle())
    }
    .accessibilityLabel("\(word.value)\(word.isBundled ? "，内置" : "")，\(code)，权重 \(word.weight)")
    .accessibilityHint(word.isBundled ? "内置词条，只能调整权重或删除" : "编辑词条")
    .swipeActions {
      Button("删除", role: .destructive) { deleting = word }
    }
  }

  /// 拼音音节之间用设计稿的 ’ 分隔显示；其他类型的编码按输入原样显示，并跟在所属词库之后，因为本页混合列出用户添加的所有类型。
  private static func codeLabel(_ word: PersonalWord) -> String {
    guard word.kind == .pinyin else { return "\(word.kind.title) · \(word.key)" }
    return word.key.split(whereSeparator: { $0 == " " || $0 == "'" }).joined(separator: "’")
  }

  /// `PersonalExportRequest` 接受的文件格式，命名与 Windows 词库管理器一致。
  private static let exportFormats = [
    DesignOption(title: "词在前（标准）", value: "standard"),
    DesignOption(title: "编码在前（Windows）", value: "windows"),
  ]

  /// 导入和两步导出。导出文件由键盘写出，因为只有它能打开 Engine 的词库；随后页面把文件交给分享面板。
  @ViewBuilder private var fileSection: some View {
    Section {
      Button { importing = true } label: {
        Text("导入词条").font(.system(size: 17)).foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
          .contentShape(Rectangle())
      }
      .accessibilityIdentifier("importPersonalDictionary")
      .designRow()
      DesignSelectRow(title: "导出词库类型", options: PersonalWordKind.allCases.map { DesignOption(title: $0.title, value: $0) },
                      selection: $exportKind, sheetTitle: "导出哪种词库", identifier: "personalDictionaryExportKind")
        .listRowInsets(EdgeInsets())
        .designRow()
      DesignSelectRow(title: "导出文件格式", options: Self.exportFormats, selection: $exportFormat, sheetTitle: "文件格式",
                      identifier: "personalDictionaryExportFormat")
        .listRowInsets(EdgeInsets())
        .designRow()
      Button {
        do {
          try store.requestExport(kind: exportKind, format: exportFormat)
          refresh()
          ToastCenter.shared.show("已请求导出")
        } catch { self.error = error.localizedDescription }
      } label: {
        Text("导出\(exportKind.title)词库").font(.system(size: 17))
          .foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
          .contentShape(Rectangle())
      }
      .accessibilityIdentifier("requestPersonalDictionaryExport")
      .designRow()
      if let request = state.exportRequest {
        Group {
          if let result = state.exportResult, result.request.id == request.id {
            if let failure = result.error {
              Text(failure).font(.footnote).foregroundStyle(MetasequoiaTheme.danger)
            } else {
              Text("\(request.kind.title)词库已导出 \(result.rows) 条" + (result.truncated ? "，词库过大，只导出了前面一部分。" : "。"))
                .font(.footnote).foregroundStyle(MetasequoiaTheme.sub)
              if let copy = exportCopy, copy.id == request.id {
                ShareLink(item: copy.url) { Label("分享「\(request.fileName)」", systemImage: "square.and.arrow.up") }
                  .foregroundStyle(MetasequoiaTheme.accent)
                  .accessibilityIdentifier("sharePersonalDictionaryExport")
              }
            }
          } else {
            Text("已请求导出，等待键盘生成\(request.kind.title)词库文件。在顶部「同步状态」里打开试打框。")
              .font(.footnote).foregroundStyle(MetasequoiaTheme.sub)
          }
        }
        .designRow()
      }
    } footer: {
      Text("导入支持 JSON、纯中文词表和词库文件。导出与电脑版的文件相同：每行一条，用 Tab 分隔，词在前或编码在前（Windows）；拼音导出多字词，包括调整过权重的内置词，其他类型只导出你添加的词条。文件只保存在本机，通过分享面板存到你选择的位置。")
    }
  }

  private func refresh() {
    do { state = try store.read(); lastReadError = nil; prepareExportCopy() } catch {
      let message = error.localizedDescription
      if lastReadError != message { self.error = message; lastReadError = message }
    }
  }
  /// Copies the export the keyboard just wrote, once, so the share sheet gets a file under the desktop's name.
  private func prepareExportCopy() {
    guard let result = state.exportResult, result.request.id == state.exportRequest?.id, result.error == nil,
          exportCopyAttempt != result.request.id else { return }
    // The page re-reads the queue every second; a copy that failed is reported once, not every second.
    exportCopyAttempt = result.request.id
    do { exportCopy = (result.request.id, try store.exportCopy(for: result)) } catch {
      self.error = error.localizedDescription
    }
  }

  private func searchedDictionary(_ kind: PersonalWordKind?) -> String {
    kind.map { "\($0.title)词库" } ?? "个人词库"
  }

  private func perform<T>(_ work: () throws -> T) {
    do { _ = try work(); refresh() } catch { self.error = error.localizedDescription }
  }

  /// 队列中一条编辑所涉及的词。
  fileprivate static func requestTitle(_ request: PersonalWordRequest) -> String {
    (request.replacement ?? request.previous)?.value ?? "词条"
  }

  /// 队列中一条编辑的当前状态，措辞与同步行和进度列表一致。
  fileprivate static func requestStatus(_ request: PersonalWordRequest) -> String {
    request.status == .pending ? "等待同步 · \(request.replacement == nil ? "删除" : "保存")" : (request.error ?? "同步失败")
  }
}

/// 同步状态：键盘同步状态、唤醒键盘的试用输入框，以及所有尚未应用的编辑，失败的可以重试。它自己读取队列，因为本页盖住列表页时列表页的刷新会暂停。
private struct PersonalDictionarySyncView: View {
  let store: PersonalDictionaryStore
  @State private var state = PersonalDictionaryState()
  @State private var trial = ""
  @State private var error: String?
  @State private var lastReadError: String?

  var body: some View {
    List {
      Section {
        SettingsFactRow(title: state.pendingCount > 0 ? "\(state.pendingCount) 项等待键盘同步" : "已全部同步",
                        detail: state.snapshotDate.map { "最近同步 \($0.formatted(date: .abbreviated, time: .standard))" }
                          ?? "尚未收到键盘确认，保存的操作暂不会标记为已生效",
                        symbol: state.pendingCount > 0 ? "clock.arrow.circlepath" : "checkmark.circle.fill")
          .designRow()
        TextField("点此打开键盘并试打", text: $trial)
          .accessibilityIdentifier("personalDictionaryTrial")
          .designRow()
      } header: {
        Text("键盘同步")
      } footer: {
        Text("开启水杉键盘的「允许完全访问」，再打开键盘完成本机同步。保存、删除、导入、搜索编码和导出都在键盘打开时处理。已保存的学习记录和词条不会上传。")
      }
      let requests = state.requests.filter { $0.status != .applied }
      if !requests.isEmpty {
        Section("同步进度") {
          ForEach(requests) { request in
            VStack(alignment: .leading, spacing: 5) {
              Text(PersonalDictionaryView.requestTitle(request))
              Text(PersonalDictionaryView.requestStatus(request))
                .font(.caption).foregroundStyle(request.status == .pending ? MetasequoiaTheme.sub : MetasequoiaTheme.danger)
              if request.status == .failed {
                HStack {
                  Button("重试") { perform { try store.retry(request.id) } }.buttonStyle(.borderless)
                  Button("移除失败记录", role: .destructive) { perform { try store.dismissFailure(request.id) } }
                    .buttonStyle(.borderless)
                }
              }
            }
            .designRow()
          }
        }
      }
    }
    .designPage()
    .settingsStatus(busy: false, message: state.snapshotError)
    .navigationTitle("同步状态")
    .navigationBarTitleDisplayMode(.inline)
    .alert("个人词库", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
      Button("好", role: .cancel) { error = nil }
    } message: { Text(error ?? "") }
    .task {
      while !Task.isCancelled {
        refresh()
        do { try await Task.sleep(nanoseconds: 1_000_000_000) } catch { break }
      }
    }
  }

  private func refresh() {
    do { state = try store.read(); lastReadError = nil } catch {
      let message = error.localizedDescription
      if lastReadError != message { self.error = message; lastReadError = message }
    }
  }

  private func perform(_ work: () throws -> Void) {
    do { try work(); refresh() } catch { self.error = error.localizedDescription }
  }
}

private struct PersonalWordEditor: View {
  let previous: PersonalWord?
  let save: (PersonalWord) throws -> Void
  @Environment(\.dismiss) private var dismiss
  @State private var word: PersonalWord
  @State private var error: String?
  init(previous: PersonalWord?, save: @escaping (PersonalWord) throws -> Void) {
    self.previous = previous
    self.save = save
    _word = State(initialValue: previous ?? PersonalWord(key: "", value: ""))
  }
  var body: some View {
    NavigationView {
      Form {
        if word.isBundled {
          Section {
            SettingsFactRow(title: word.value, detail: "\(word.kind.title) · \(word.key)", symbol: "lock.fill")
          } footer: {
            Text("这是随词库安装的词条，编码和词不能修改。可以调整它的权重，或在列表里滑动删除。")
          }
        } else {
        Section {
          Picker("类型", selection: $word.kind) { ForEach(PersonalWordKind.allCases) { Text($0.title).tag($0) } }
          if word.kind == .quickPhrase {
            VStack(alignment: .leading) {
              Text("短语内容").font(.caption).foregroundStyle(.secondary)
              TextEditor(text: $word.value).frame(minHeight: 90).accessibilityIdentifier("personalWordValue")
            }
          } else {
            TextField("词条内容", text: $word.value).accessibilityIdentifier("personalWordValue")
          }
          TextField(word.kind == .pinyin ? "完整拼音，例如 ni hao" : "输入编码", text: $word.key)
            .keyboardType(.asciiCapable).textInputAutocapitalization(.never).autocorrectionDisabled()
            .accessibilityIdentifier("personalWordCode")
            .onChange(of: word.key) { _, key in sanitizeCode(key) }
            .onChange(of: word.kind) { _, _ in sanitizeCode(word.key) }
        } footer: {
          Text(word.kind == .pinyin ? "每个字填写一个完整拼音音节，用空格或英文单引号分隔；ü 用 v。全拼、双拼和九键共用这个词条。" : "五笔使用 1–4 个字母；快捷短语使用英文字母，在键盘“本地输入 → 快捷短语”输入；英文编码使用字母、连字符或撇号，可以和词条不同，例如用 dont 打出 don't。")
        }
        }
        Section {
          TextField("权重", value: $word.weight, format: .number.grouping(.never))
            .keyboardType(.numberPad)
            .accessibilityIdentifier("personalWordWeight")
        } header: {
          Text("权重")
        } footer: {
          Text("同一编码下权重越大，候选越靠前。新词默认 \(PersonalWord.defaultWeight)，可填 1–\(PersonalWord.weightRange.upperBound)。")
        }
        if let error { Section { Text(error).foregroundStyle(.red) } }
        Section { Text("保存后等待水杉键盘确认同步。这里只保存本机词条，不会发送到 AI 或语音服务。").font(.footnote).foregroundStyle(.secondary) }
      }
      .navigationTitle(previous == nil ? "添加词条" : word.isBundled ? "调整权重" : "编辑词条")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("取消") { dismiss() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("保存") {
            guard PersonalWord.weightRange.contains(word.weight) else {
              error = "权重需要在 1–\(PersonalWord.weightRange.upperBound) 之间。"
              return
            }
            // A bundled row keeps the code and word it was listed with; the keyboard's Engine checks that only the weight changed.
            do { try save(word.isBundled ? word : word.validated()); dismiss() } catch { self.error = error.localizedDescription }
          }.disabled(word.key.isEmpty || word.value.isEmpty).accessibilityIdentifier("savePersonalWord")
        }
      }
    }
  }

  /// 拼音和五笔编码都是小写字母，拼音音节用 ' 或空格分隔，所以其他字符在输入或粘贴时就被丢弃，与设计稿的添加词条对话框一致。智能标点会把输入的 ' 变成 ’，读回时再当作 '。快捷短语和英文有各自的规则（英文编码可以含连字符），由 Engine 在保存时检查。
  private func sanitizeCode(_ key: String) {
    guard [.pinyin, .wubi, .wubi98].contains(word.kind) else { return }
    let clean = Self.sanitizedCode(key)
    if clean != key { word.key = clean }
  }

  static func sanitizedCode(_ key: String) -> String {
    String(key.lowercased().compactMap { character -> Character? in
      switch character {
      case "a"..."z", " ", "'": return character
      case "\u{2019}", "\u{2018}": return "'"
      default: return nil
      }
    })
  }
}
