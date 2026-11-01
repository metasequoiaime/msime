import SwiftUI

/// 「完整词库目录」：查询和编辑、删除目录词条都经 Rust 的 `msime_client_cloud_dictionary`（`catalog`、`edit_catalog`，#8201 迁移计划 4c）。macOS 与 iOS 共用；页面只按打开时的 user.id 绑定，每个请求前后都核对，退出或换了号就丢掉结果。
struct CloudDictionaryCatalogView: View {
  let kind: BackendAccountClient.DictionaryKind
  let accountID: String
  /// nil 是宿主没有注册会话存储，每个操作都报 `rust_client_unavailable`。
  let rust: BackendRustAccount?
  @State private var code = ""
  @State private var scheme = "pinyin"
  @State private var profile = "xiaohe"
  @State private var page: BackendAccountClient.DictionaryCatalog?
  @State private var confirmedQuery: Query?
  @State private var editing: Edit?
  @State private var deleting: Edit?
  @State private var busy = false
  @State private var message: String?
  @State private var pending: Task<Void, Never>?
  private struct Query: Sendable { let code: String; let scheme: String; let profile: String }
  private struct Edit: Identifiable, Sendable { let id = UUID(); let entry: BackendAccountClient.CatalogEntry; let revision: Int64 }

  init(kind: BackendAccountClient.DictionaryKind, accountID: String, rust: BackendRustAccount? = .shared) {
    self.kind = kind; self.accountID = accountID; self.rust = rust
  }

  var body: some View {
    List {
      Section {
        TextField(kind == .quick ? "快捷编码，可留空" : "输入编码，例如 shi、a 或 hello", text: $code)
          .backendCodeInput()
        if kind == .pinyin {
          Picker("编码方案", selection: $scheme) { Text("全拼").tag("pinyin"); Text("双拼").tag("shuangpin") }
          if scheme == "shuangpin" {
            Picker("双拼方案", selection: $profile) {
              Text("小鹤").tag("xiaohe"); Text("自然码").tag("ziranma")
              Text("微软").tag("microsoft"); Text("首道").tag("shoudao")
            }
          }
        }
        SettingsActionRow(title: "查询完整目录", symbol: "magnifyingglass",
                          enabled: kind == .quick || !code.isEmpty) {
          run { try await load(query: .init(code: code, scheme: scheme, profile: profile), offset: 0) }
        }
      } header: {
        SettingsSectionInfoHeader(subject: kind.title, info: "包含基础词库与当前账号的修改。编辑和删除仅影响当前账号的云端词库；不会立即修改本机词库。按编码查询，拼音使用全拼或所选双拼方案。") { Text(kind.title) }
      }
      if let page {
        Section {
          if page.entries.isEmpty { Text("没有匹配的词条。").foregroundStyle(.secondary) }
          // 每条原本挂着「编辑」「删除」两个行内按钮。点一条即编辑,删除交给侧滑。
          ForEach(page.entries) { entry in
            Button { editing = Edit(entry: entry, revision: page.revision) } label: {
              VStack(alignment: .leading, spacing: 3) {
                Text(entry.word).foregroundStyle(.primary)
                Text("\(entry.code) · 权重 \(entry.weight)").font(.caption).foregroundStyle(.secondary)
              }
              .frame(maxWidth: .infinity, alignment: .leading)
              .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .swipeActions(edge: .trailing) {
              Button("删除", role: .destructive) { deleting = Edit(entry: entry, revision: page.revision) }
            }
          }
        } header: {
          SettingsSectionInfoHeader(subject: "目录结果 · 云端版本 \(page.revision)", info: "点一条编辑，左滑删除。") { Text("目录结果 · 云端版本 \(page.revision)") }
        } footer: {
          Text("查询编码：\(page.normalized)")
        }
        if page.offset > 0 || page.has_more {
          Section {
            SettingsActionRow(title: "上一页", symbol: "chevron.left", enabled: page.offset > 0) {
              run { try await reload(offset: max(0, page.offset - 100)) }
            }
            SettingsActionRow(title: "下一页", symbol: "chevron.right", enabled: page.has_more) {
              run { try await reload(offset: page.offset + 100) }
            }
          } footer: {
            Text("第 \(page.offset / 100 + 1) 页")
          }
        }
      }
    }
    .settingsStatus(busy: busy, busyTitle: "正在查询或保存…", message: message)
    .navigationTitle("完整词库目录")
    .disabled(busy)
    .onDisappear { pending?.cancel(); page = nil }
    .sheet(item: $editing) { edit in
      CloudDictionaryEditor(kind: kind, value: edit.entry.value) { value in
        try await editCatalog(edit, replacement: value)
        try await reload(offset: 0)
      }
    }
    .alert("删除云端目录词条？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })) {
      Button("取消", role: .cancel) { deleting = nil }
      Button("删除", role: .destructive) {
        guard let edit = deleting else { return }
        run {
          try await editCatalog(edit, replacement: nil)
          try await reload(offset: 0)
        }
        deleting = nil
      }
    } message: { Text("基础词条也可从当前账号的云端目录删除。本机词库保持原样；云端版本变化时须重新查询并确认。") }
  }
  @MainActor private func load(query: Query, offset: Int) async throws {
    let value = try BackendRustAccount.object(try await request(["operation": "catalog", "kind": kind.rawValue, "code": query.code,
                                                                "offset": offset, "scheme": query.scheme, "profile": query.profile]))
    page = try Self.catalog(value); confirmedQuery = query
  }
  /// Rust 的目录页把词条放在 `catalog_entries` 里，其余与服务端相同。
  static func catalog(_ value: [String: Any]) throws -> BackendAccountClient.DictionaryCatalog {
    var page = value
    page["entries"] = page.removeValue(forKey: "catalog_entries")
    return try BackendRustAccount.decode(BackendAccountClient.DictionaryCatalog.self,
                                         try BackendRustClient.request(page))
  }
  /// 编辑（`replacement` 是新值）或删除（nil）一条目录词条；云端版本变了是 `account_conflict`。
  @MainActor private func editCatalog(_ edit: Edit, replacement: BackendAccountClient.DictionaryValue?) async throws {
    let value: Any = replacement.map { ["code": $0.code, "word": $0.word, "weight": $0.weight] as [String: Any] } ?? NSNull()
    _ = try await request(["operation": "edit_catalog", "kind": kind.rawValue, "code": edit.entry.code, "word": edit.entry.word,
                           "revision": edit.revision, "replacement": value])
  }
  /// 发一个云词库 action；请求前后都核对打开页面时的账号。
  @MainActor private func request(_ action: [String: Any]) async throws -> Data {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    guard try await rust.status()?.id == accountID else { throw CancellationError() }
    let data = try await rust.cloudDictionary(action)
    guard try await rust.status()?.id == accountID else { throw CancellationError() }
    try Task.checkCancellation()
    return data
  }
  @MainActor private func reload(offset: Int) async throws {
    guard let query = confirmedQuery else { return }
    try await load(query: query, offset: offset)
  }
  @MainActor private func run(_ action: @escaping @MainActor () async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await action() }
      catch is CancellationError { page = nil; confirmedQuery = nil }
      catch { message = error.localizedDescription }
    }
  }
}
