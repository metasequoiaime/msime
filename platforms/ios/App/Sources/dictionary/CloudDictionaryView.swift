import SwiftUI

struct CloudDictionaryView: View {
  private struct Edit: Identifiable { let id = UUID(); let entry: BackendAccountClient.DictionaryEntry?; let kind: BackendAccountClient.DictionaryKind; let userID: String }
  @State private var kind = BackendAccountClient.DictionaryKind.pinyin
  @State private var search = ""
  @State private var page: BackendAccountClient.DictionaryPage?
  @State private var userID: String?
  @State private var sessionID: UUID?
  @State private var busy = false
  @State private var message: String?
  @State private var editing: Edit?
  @State private var deleting: BackendAccountClient.DictionaryEntry?
  @State private var downloading: BackendAccountClient.DictionaryEntry?
  @State private var pending: Task<Void, Never>?
  private let client = BackendAccountClient()
  private let session = BackendAccountSession.shared

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
        SettingsGroupHeader(title: "云端个人词条")
      } footer: {
        if let page, !page.entries.isEmpty {
          Text("点一条编辑，左滑下载到本机或删除。第 \(page.offset / 100 + 1) 页。")
        } else {
          Text("仅管理当前账号的云端个人词条。上传需主动保存；下载需确认后交给本机键盘处理，不会自动上传本机学习记录。")
        }
      }
      .designRow()

      if let page, page.offset > 0 || page.has_more {
        Section {
          Button { run { try await load(offset: max(0, page.offset - 100)) } } label: {
            Label("上一页", systemImage: "chevron.left")
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          .disabled(page.offset == 0)
          Button { run { try await load(offset: page.offset + 100) } } label: {
            Label("下一页", systemImage: "chevron.right")
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          .disabled(!page.has_more)
        }
        .designRow()
      }

      if let userID {
        Section {
          NavigationLink(destination: CloudCandidatesView(kind: kind,
            authorize: { try await authorizedToken(matching: userID) })) {
            CloudDictionaryRowLabel(
              title: "云端候选与排序", detail: "调整这个词库里候选的先后",
              symbol: "list.number")
          }
          NavigationLink(destination: CloudDictionaryCatalogView(kind: kind,
            authorize: { try await authorizedToken(matching: userID) })) {
            CloudDictionaryRowLabel(
              title: "完整目录", detail: "查询与批量管理全部词条",
              symbol: "square.stack.3d.up.fill")
          }
          NavigationLink(destination: CloudDictionaryApplyView(accountID: userID,
            authorize: { try await authorizedToken(matching: userID) })) {
            CloudDictionaryRowLabel(
              title: "应用到本机", detail: "把整份云词库交给本机键盘",
              symbol: "iphone.and.arrow.forward")
          }
          NavigationLink(destination: CloudDictionaryFilesView(kind: kind,
            authorize: { try await authorizedToken(matching: userID) }, imported: { try await load(offset: 0) })) {
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
    .onSubmit(of: .search) { run { try await load(offset: 0) } }
    .navigationTitle("云词库")
    .disabled(busy)
    .toolbar {
      Button("添加") { if let userID { editing = Edit(entry: nil, kind: kind, userID: userID) } }
        .disabled(busy || userID == nil)
    }
    .task { run { try await load(offset: 0) } }
    .onChange(of: kind) { _ in page = nil; run { try await load(offset: 0) } }
    .onDisappear { pending?.cancel(); page = nil }
    .sheet(item: $editing) { edit in
      CloudDictionaryEditor(kind: edit.kind, value: edit.entry.map { .init(code: $0.code, word: $0.word, weight: $0.weight) }) { value in
        let token = try await authorizedToken(matching: edit.userID)
        if let entry = edit.entry { _ = try await client.updateDictionary(entry, value: value, token: token) }
        else { _ = try await client.addDictionary(edit.kind, value: value, token: token) }
        try await load(offset: 0)
      }
    }
    .alert("删除云端词条？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
      Button("取消", role: .cancel) { deleting = nil }
      Button("删除", role: .destructive) {
        guard let entry = deleting, let userID else { return }
        run {
          let token = try await authorizedToken(matching: userID)
          _ = try await client.deleteDictionary(entry, token: token)
          try await load(offset: 0)
        }
        deleting = nil
      }
    } message: { Text("只删除云端这一版本的词条。已下载的本机词条需在本机词库中删除。") }
    .alert("下载词条到本机？", isPresented: Binding(get: { downloading != nil }, set: { if !$0 { downloading = nil } })) {
      Button("取消", role: .cancel) { downloading = nil }
      Button("下载") {
        guard let entry = downloading, let userID else { return }
        run {
          _ = try await authorizedToken(matching: userID)
          let word = try entry.localWord()
          try PersonalDictionaryStore().enqueue(previous: nil, replacement: word)
          message = "已加入本机队列。请打开允许完全访问的水杉键盘，确认同步后再试打。"
        }
        downloading = nil
      }
    } message: { Text("下载内容仅在 Engine 完成处理后生效，可在本机词库查看成功或失败状态。") }
  }
  @MainActor private func authorizedToken(matching expected: String) async throws -> String {
    let identity = try await session.credentials(matchingUserID: expected, matchingSessionID: sessionID)
    guard identity.userID == expected else { throw BackendAccountClient.Failure(status: 401) }
    try Task.checkCancellation()
    sessionID = identity.sessionID
    return identity.token
  }
  @MainActor private func load(offset: Int) async throws {
    guard let identity = try await session.user()?.id else { throw BackendAccountClient.Failure(status: 401) }
    let token = try await authorizedToken(matching: identity)
    let result = try await client.dictionary(kind, search: search, offset: offset, token: token)
    _ = try await authorizedToken(matching: identity)
    page = result; userID = identity
  }
  @MainActor private func run(_ work: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await work() }
      catch is CancellationError { }
      catch { message = error.localizedDescription }
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
      VStack(alignment: .leading, spacing: 2) {
        Text(title).foregroundStyle(.primary)
        Text(detail).font(.caption).foregroundStyle(.secondary)
      }
    }
  }
}
