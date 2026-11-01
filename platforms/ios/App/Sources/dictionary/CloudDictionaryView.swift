import SwiftUI

/// 云词库：当前账号的云端个人词条，列表和增删改经 Rust 的 `msime_client_cloud_dictionary`（#8201 迁移计划 4c）。
struct CloudDictionaryView: View {
  private struct Edit: Identifiable { let id = UUID(); let entry: BackendAccountClient.DictionaryEntry?; let kind: BackendAccountClient.DictionaryKind; let userID: String }
  @State private var kind = BackendAccountClient.DictionaryKind.pinyin
  @State private var search = ""
  @State private var page: BackendAccountClient.DictionaryPage?
  @State private var userID: String?
  @State private var busy = false
  @State private var message: String?
  @State private var editing: Edit?
  @State private var deleting: BackendAccountClient.DictionaryEntry?
  @State private var downloading: BackendAccountClient.DictionaryEntry?
  @State private var request = ViewRequestGeneration()
  @State private var pending: Task<Void, Never>?

  var body: some View {
    List {
      Section {
        Picker("词库", selection: $kind) {
          ForEach(BackendAccountClient.DictionaryKind.allCases) { Text($0.title).tag($0) }
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .listRowInsets(EdgeInsets(top: 10, leading: 16, bottom: 10, trailing: 16))
      }
      .designRow()

      Section {
        if let page {
          if page.entries.isEmpty {
            Text(search.isEmpty ? "这个词库还没有云端词条" : "没有匹配「\(search)」的词条")
              .foregroundStyle(.secondary)
          }
          ForEach(page.entries) { entry in
            Button {
              if let userID { editing = Edit(entry: entry, kind: kind, userID: userID) }
            } label: {
              VStack(alignment: .leading, spacing: 3) {
                Text(entry.word).foregroundStyle(.primary)
                Text("\(entry.code) · 权重 \(entry.weight)").font(.caption).foregroundStyle(.secondary)
              }
              .frame(maxWidth: .infinity, alignment: .leading)
              .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
              Button("删除", role: .destructive) { deleting = entry }
              Button("下载") { downloading = entry }.tint(MetasequoiaTheme.accent)
            }
          }
        } else if !busy {
          Text("还没有读取云端词条").foregroundStyle(.secondary)
        }
      } header: {
        DesignSectionHeader(info: "仅管理当前账号的云端个人词条。点一条编辑，左滑下载到本机或删除。上传需主动保存；下载需确认后交给本机键盘处理，不会自动上传本机学习记录。") {
          SettingsGroupHeader(title: "云端个人词条")
        }
      } footer: {
        if let page, !page.entries.isEmpty {
          Text("第 \(page.offset / 100 + 1) 页")
        }
      }
      .designRow()

      if let page, page.offset > 0 || page.has_more {
        Section {
          Button { run { _ in try await load(offset: max(0, page.offset - 100)) } } label: {
            Label("上一页", systemImage: "chevron.left")
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          .disabled(page.offset == 0)
          Button { run { _ in try await load(offset: page.offset + 100) } } label: {
            Label("下一页", systemImage: "chevron.right")
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          .disabled(!page.has_more)
        }
        .designRow()
      }

      if let userID {
        Section {
          NavigationLink(destination: CloudCandidatesView(kind: kind, accountID: userID)) {
            CloudDictionaryRowLabel(
              title: "云端候选与排序", detail: "调整这个词库里候选的先后",
              symbol: "list.number")
          }
          NavigationLink(destination: CloudDictionaryCatalogView(kind: kind, accountID: userID)) {
            CloudDictionaryRowLabel(
              title: "完整目录", detail: "查询与批量管理全部词条",
              symbol: "square.stack.3d.up.fill")
          }
          NavigationLink(destination: CloudDictionaryApplyView(accountID: userID)) {
            CloudDictionaryRowLabel(
              title: "应用到本机", detail: "把整份云词库交给本机键盘",
              symbol: "iphone.and.arrow.forward")
          }
          NavigationLink(destination: CloudDictionaryFilesView(accountID: userID,
            kind: kind, imported: { try await refreshAfterImport() })) {
            CloudDictionaryRowLabel(
              title: "导入与导出", detail: "用文件搬运词条",
              symbol: "doc.badge.arrow.up.fill")
          }
        } header: {
          SettingsGroupHeader(title: "管理")
        }
        .designRow()
      }

      Section {
        NavigationLink(destination: PersonalDictionaryView()) {
          CloudDictionaryRowLabel(
            title: "本机词库", detail: "查看本机词条与同步进度",
            symbol: "iphone")
        }
      }
      .designRow()

      if busy { ProgressView("正在处理…").designRow() }
      if let message { Section { Text(message).foregroundStyle(.secondary) }.designRow() }
    }
    .designPage()
    .searchable(text: $search, prompt: "搜索云端词条或编码")
    .onSubmit(of: .search) { run { _ in try await load(offset: 0) } }
    .navigationTitle("云词库")
    .disabled(busy)
    .toolbar {
      Button("添加") { if let userID { editing = Edit(entry: nil, kind: kind, userID: userID) } }
        .disabled(busy || userID == nil)
    }
    .task { run { _ in try await load(offset: 0) } }
    .onChange(of: kind) {
      ViewRequestTask.cancel(task: $pending)
      request.invalidate()
      busy = false
      page = nil
      run { _ in try await load(offset: 0) }
    }
    .trackViewRequest(request: $request, pending: $pending, onDisappear: {
      busy = false
      page = nil
    })
    .sheet(item: $editing) { edit in
      CloudDictionaryEditor(kind: edit.kind, value: edit.entry.map { .init(code: $0.code, word: $0.word, weight: $0.weight) }) { value in
        try await save(value, for: edit)
      }
    }
    .alert("删除云端词条？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
      Button("取消", role: .cancel) { deleting = nil }
      Button("删除", role: .destructive) {
        guard let entry = deleting, let userID else { return }
        run { id in
          _ = try await IOSCloudDictionary(accountID: userID).request([
            "operation": "delete", "kind": entry.kind.rawValue, "id": entry.id, "revision": entry.revision])
          try request.checkCurrent(id)
          try await load(offset: 0)
        }
        deleting = nil
      }
    } message: { Text("只删除云端这一版本的词条。已下载的本机词条需在本机词库中删除。") }
    .alert("下载词条到本机？", isPresented: Binding(get: { downloading != nil }, set: { if !$0 { downloading = nil } })) {
      Button("取消", role: .cancel) { downloading = nil }
      Button("下载") {
        guard let entry = downloading, let userID else { return }
        run { id in
          guard try await BackendRustAccount.current().status()?.id == userID else { throw CancellationError() }
          try request.checkCurrent(id)
          let word = try entry.localWord()
          try PersonalDictionaryStore().enqueue(previous: nil, replacement: word)
          message = "已加入本机队列。请打开允许完全访问的水杉键盘，确认同步后再试打。"
        }
        downloading = nil
      }
    } message: { Text("下载内容仅在 Engine 完成处理后生效，可在本机词库查看成功或失败状态。") }
  }
  @MainActor private func save(_ value: BackendAccountClient.DictionaryValue, for edit: Edit) async throws {
    guard let id = request.current() else { throw CancellationError() }
    let dictionary = IOSCloudDictionary(accountID: edit.userID)
    if let entry = edit.entry {
      _ = try await dictionary.request(["operation": "update", "kind": entry.kind.rawValue, "id": entry.id, "code": value.code,
                                        "word": value.word, "weight": value.weight, "revision": entry.revision])
    } else {
      _ = try await dictionary.request(["operation": "add", "kind": edit.kind.rawValue, "code": value.code,
                                        "word": value.word, "weight": value.weight])
    }
    try request.checkCurrent(id)
    try await load(offset: 0)
  }
  @MainActor private func refreshAfterImport() async throws {
    guard request.isVisible else { return }
    try await load(offset: 0)
  }
  @MainActor private func load(offset: Int) async throws {
    guard let id = request.current() else { throw CancellationError() }
    let requestedKind = kind
    // 没登录时整页报「登录已失效」；登录了就按当前的账号读，换了号时页面跟着换。
    guard let identity = try await BackendRustAccount.current().status()?.id else {
      throw BackendRustClient.Failure(code: "account_unauthorized")
    }
    let result = try BackendRustAccount.decode(BackendAccountClient.DictionaryPage.self,
      try await IOSCloudDictionary(accountID: identity).request([
        "operation": "list", "kind": requestedKind.rawValue, "offset": offset, "search": search]))
    try request.checkCurrent(id)
    guard kind == requestedKind else { return }
    page = result; userID = identity
  }
  @MainActor private func run(_ work: @escaping @MainActor (UUID) async throws -> Void) {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending) { id in
      try request.checkCurrent(id)
      try await work(id)
      try request.checkCurrent(id)
    }
  }
}

private struct CloudDictionaryRowLabel: View {
  let title: String
  let detail: String
  let symbol: String

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: symbol)
        .foregroundStyle(MetasequoiaTheme.onAccent)
        .frame(width: 30, height: 30)
        .background(MetasequoiaTheme.accent, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
      SettingsOptionLabel(title: title, detail: detail, detailFont: .caption)
    }
  }
}
