import AppKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class MacDictionaryModel: ObservableObject {
  @Published var kind = BackendAccountClient.DictionaryKind.pinyin
  @Published var search = ""
  @Published var page: BackendAccountClient.DictionaryPage?
  @Published var busy = false
  @Published var message: String?
  @Published var importText: String?
  @Published var format = BackendAccountClient.DictionaryFileFormat.standard
  private let accountID: String
  private var sessionID: UUID?
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  private var pending: Task<Void, Never>?
  private var panel: NSSavePanel?
  private var closed = false
  init(accountID: String, client: BackendAccountClient = BackendAccountClient(), account: BackendAccountSession = .shared) {
    self.accountID = accountID; self.client = client; self.account = account
  }
  func authorize() async throws -> String {
    let identity = try await account.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
    try Task.checkCancellation()
    guard !closed, identity.userID == accountID else { throw CancellationError() }
    sessionID = identity.sessionID
    return identity.token
  }
  private func run(offset: Int = 0, _ operation: @escaping @MainActor @Sendable (String, BackendAccountClient.DictionaryKind) async throws -> Void) {
    guard !busy, !closed else { return }
    let selected = kind, query = search
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do {
        let identity = try await account.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
        try Task.checkCancellation()
        guard !closed else { throw CancellationError() }
        sessionID = identity.sessionID
        _ = try await account.authenticated(matchingUserID: identity.userID,
                                            matchingSessionID: identity.sessionID) { token in
          try await operation(token, selected)
        }.value
        let result = try await account.authenticated(matchingUserID: identity.userID,
                                                     matchingSessionID: identity.sessionID) { token in
          try await client.dictionary(selected, search: query, offset: offset, token: token)
        }.value
        guard kind == selected, search == query else { throw CancellationError() }
        page = result
      } catch is CancellationError { page = nil }
      catch { page = nil; if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  func refresh(offset: Int = 0) { run(offset: offset) { _, _ in } }
  func save(_ value: BackendAccountClient.DictionaryValue, replacing entry: BackendAccountClient.DictionaryEntry? = nil) {
    run { token, kind in
      if let entry {
        guard entry.kind == kind else { throw BackendAccountClient.Failure(status: 409) }
        _ = try await self.client.updateDictionary(entry, value: value, token: token)
      } else { _ = try await self.client.addDictionary(kind, value: value, token: token) }
    }
  }
  func delete(_ entry: BackendAccountClient.DictionaryEntry) {
    run { token, kind in
      guard entry.kind == kind else { throw BackendAccountClient.Failure(status: 409) }
      _ = try await self.client.deleteDictionary(entry, token: token)
    }
  }
  func chooseImport() {
    guard !busy, panel == nil else { return }
    let selected = NSOpenPanel()
    selected.allowedContentTypes = [.plainText, .text]; selected.allowsMultipleSelection = false
    panel = selected
    selected.begin { response in
      Task { @MainActor in
        defer { self.panel = nil }
        guard !self.closed, response == .OK, let url = selected.url else { return }
        do {
          let text = try await Task.detached(priority: .userInitiated) {
            try Self.readImportText(from: url)
          }.value
          guard !self.closed else { return }
          self.importText = text; self.message = nil
        } catch { if !self.closed { self.message = "文件需为不超过 64 KiB 的 UTF-8 文本。" } }
      }
    }
  }

  private struct InvalidImportFile: LocalizedError, Sendable {}

  nonisolated private static func readImportText(from url: URL) throws -> String {
    let scoped = url.startAccessingSecurityScopedResource()
    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
    let data = try MacSecureFileReader.readData(from: url, maximumBytes: 65536)
    guard data.count <= 65536, let text = String(data: data, encoding: .utf8), !text.isEmpty, !text.contains("\0") else {
      throw InvalidImportFile()
    }
    return text
  }

  func uploadImport() {
    guard let text = importText else { return }
    let format = format
    run { token, kind in
      let result = try await self.client.importDictionary(kind, text: text, format: format, token: token)
      _ = try await self.authorize()
      self.importText = nil; self.message = "已导入 \(result.imported) 个词条。"
    }
  }
  func chooseExport() {
    guard !busy, panel == nil, format != .hans else { return }
    let selected = NSSavePanel()
    selected.allowedContentTypes = [.plainText]; selected.nameFieldStringValue = "水杉-\(kind.title).tsv"
    panel = selected
    selected.begin { response in
      Task { @MainActor in
        defer { self.panel = nil }
        guard !self.closed, response == .OK, let url = selected.url else { return }
        self.export(to: url)
      }
    }
  }
  func export(to destination: URL) {
    let format = format
    run { token, kind in
      let file = try await self.client.exportDictionary(kind, format: format, token: token)
      defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
      _ = try await self.authorize()
      try await MacCloudFileTransfer.save(file, to: destination) { _ = try await self.authorize() }
      self.message = "云词库已导出。"
    }
  }
  func close() { closed = true; pending?.cancel(); panel?.cancel(nil); panel = nil; page = nil; importText = nil; search = "" }
  var cloudAccountID: String { accountID }
  var cloudAccountSession: BackendAccountSession { account }
}

struct MacCloudDictionaryView: View {
  @StateObject private var model: MacDictionaryModel
  @Environment(\.dismiss) private var dismiss
  @State private var editing = false
  @State private var entry: BackendAccountClient.DictionaryEntry?
  @State private var code = ""
  @State private var word = ""
  @State private var weight = "100000"
  @State private var deleting: BackendAccountClient.DictionaryEntry?
  @State private var importing = false
  @State private var management: Management?
  private enum Management: String, Identifiable { case catalog, candidates; var id: String { rawValue } }
  init(accountID: String) { _model = StateObject(wrappedValue: MacDictionaryModel(accountID: accountID)) }
  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack { Text("云词库").font(.title2); Spacer(); Button("关闭") { model.close(); dismiss() } }
      Text("管理当前账号的云端词条。上传需主动确认；本页修改不会自动替换本机学习记录。")
        .font(.footnote).foregroundStyle(.secondary)
      Picker("词库", selection: $model.kind) { ForEach(BackendAccountClient.DictionaryKind.allCases) { Text($0.title).tag($0) } }
        .onChange(of: model.kind) { _ in model.page = nil; model.importText = nil; if model.kind != .pinyin && model.format == .hans { model.format = .standard }; model.refresh() }
        .disabled(model.busy)
      HStack {
        TextField("搜索词条或编码", text: $model.search).onSubmit { model.refresh() }
        Button("查询") { model.refresh() }
        Button("添加词条") { entry = nil; code = ""; word = ""; weight = "100000"; editing = true }
      }.disabled(model.busy)
      HStack {
        Button("完整词库目录…") { management = .catalog }
        Button("云端候选与排序…") { management = .candidates }
      }.disabled(model.busy)
      List(model.page?.entries ?? []) { row in
        HStack {
          VStack(alignment: .leading) { Text(row.word); Text("\(row.code) · 权重 \(row.weight)").font(.caption).foregroundStyle(.secondary) }
          Spacer()
          Button("编辑") { entry = row; code = row.code; word = row.word; weight = String(row.weight); editing = true }
          Button("删除", role: .destructive) { deleting = row }
        }.disabled(model.busy)
      }
      if let page = model.page {
        HStack {
          Button("上一页") { model.refresh(offset: max(0, page.offset - 100)) }.disabled(page.offset == 0 || model.busy)
          Text("第 \(page.offset / 100 + 1) 页")
          Button("下一页") { model.refresh(offset: page.offset + 100) }.disabled(!page.has_more || model.busy)
        }
      }
      HStack {
        Picker("文件格式", selection: $model.format) { ForEach(BackendAccountClient.DictionaryFileFormat.allCases.filter { model.kind == .pinyin || $0 != .hans }) { Text($0.title).tag($0) } }
        Button("选择导入文件") { model.chooseImport() }
        Button("导出云词库") { model.chooseExport() }.disabled(model.format == .hans)
      }.disabled(model.busy)
      if let text = model.importText {
        Text("待导入：\(text.utf8.count) 字节，\(text.split(separator: "\n").count) 行。")
        ScrollView { Text(String(text.prefix(2000))).textSelection(.enabled) }.frame(height: 65)
        HStack { Button("确认上传此文件") { importing = true }; Button("丢弃") { model.importText = nil } }.disabled(model.busy)
      }
      if model.busy { ProgressView() }
      if let message = model.message { Text(message).foregroundStyle(.secondary) }
    }.padding(20).frame(width: 660, height: 650)
    .onAppear { model.refresh() }.onDisappear { model.close() }
    .sheet(item: $management) { selected in
      VStack {
        HStack {
          Text(selected == .catalog ? "完整词库目录" : "云端候选与排序").font(.title2)
          Spacer(); Button("关闭") { management = nil }
        }.padding()
        if selected == .catalog {
          CloudDictionaryCatalogView(kind: model.kind, authorize: { try await model.authorize() },
                                     accountID: model.cloudAccountID, session: model.cloudAccountSession)
        } else {
          CloudCandidatesView(kind: model.kind, authorize: { try await model.authorize() },
                              accountID: model.cloudAccountID, session: model.cloudAccountSession)
        }
      }.frame(width: 650, height: 650)
    }
    .sheet(isPresented: $editing) {
      VStack(alignment: .leading, spacing: 12) {
        Text(entry == nil ? "添加云端词条" : "编辑云端词条").font(.headline)
        TextField("词条", text: $word); TextField("编码", text: $code); TextField("权重", text: $weight)
        Text("点击上传将保存到当前账号。版本冲突时请刷新后重新确认。")
        HStack { Button("取消") { editing = false }; Button("上传") { if let value = Int64(weight) { model.save(.init(code: code, word: word, weight: value), replacing: entry); editing = false } }.disabled(word.isEmpty || Int64(weight).map { $0 < 0 } != false) }
      }.padding(20).frame(width: 400)
    }
    .alert("删除云端词条？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
      Button("取消", role: .cancel) { deleting = nil }
      Button("删除", role: .destructive) { if let entry = deleting { model.delete(entry) }; deleting = nil }
    }
    .alert("上传并导入文件？", isPresented: $importing) {
      Button("取消", role: .cancel) { }
      Button("上传") { model.uploadImport() }
    } message: { Text("文件内容将上传到当前账号。重复或无效词条会导致整批拒绝，不会自动覆盖已有词条。") }
  }
}
