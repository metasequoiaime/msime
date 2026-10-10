import AppKit
import SwiftUI

@MainActor
final class MacClipboardModel: ObservableObject {
  @Published var enabled = false
  @Published var loaded = false
  @Published var items: [BackendAccountClient.ClipboardItem] = []
  @Published var text = ""
  @Published var search = ""
  @Published var busy = false
  @Published var message: String?
  private let accountID: String
  private var sessionID: UUID?
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  private var pending: Task<Void, Never>?
  init(accountID: String, client: BackendAccountClient = BackendAccountClient(), account: BackendAccountSession = .shared) {
    self.accountID = accountID; self.client = client; self.account = account
  }
  private func credentials() async throws -> String {
    let identity = try await account.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
    try Task.checkCancellation()
    guard identity.userID == accountID else { throw CancellationError() }
    sessionID = identity.sessionID
    return identity.token
  }
  func run(_ action: @escaping @MainActor (String) async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do {
        let identity = try await account.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
        try Task.checkCancellation()
        sessionID = identity.sessionID
        _ = try await account.authenticated(matchingUserID: identity.userID,
                                            matchingSessionID: identity.sessionID) { token in
          try await action(token)
        }.value
        let page = try await client.clipboard(search: search, session: account,
                                              matchingUserID: identity.userID,
                                              matchingSessionID: identity.sessionID)
        try await account.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
        enabled = page.enabled; items = page.items; loaded = true
      } catch is CancellationError { items = []; text = ""; loaded = false }
      catch { items = []; loaded = false; if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  func refresh() { run { _ in } }
  func setEnabled(_ value: Bool) { run { try await self.client.setClipboardEnabled(value, token: $0) } }
  func upload() {
    let value = text
    run { token in
      _ = try await self.client.addClipboard(value, token: token)
      _ = try await self.credentials()
      self.text = ""
    }
  }
  func delete(id: String? = nil) { run { try await self.client.deleteClipboard(id: id, token: $0) } }
  func copy(_ item: BackendAccountClient.ClipboardItem) {
    run { _ in
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(item.text, forType: .string)
    }
  }
  func close() { pending?.cancel(); items = []; text = ""; search = ""; loaded = false }
}

struct MacCloudClipboardView: View {
  @StateObject private var model: MacClipboardModel
  @Environment(\.dismiss) private var dismiss
  @State private var confirmation: Confirmation?
  private enum Confirmation { case enable, disable, clear }
  init(accountID: String) { _model = StateObject(wrappedValue: MacClipboardModel(accountID: accountID)) }
  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack {
        Text("云剪贴板").font(.title2)
        Spacer()
        Button("关闭") { model.close(); dismiss() }
      }
      Text("只上传你明确添加的内容，不自动读取系统剪贴板。最多保存 50 条；关闭云剪贴板会删除云端历史。")
        .font(.footnote).foregroundStyle(.secondary)
      if model.loaded {
        Button(model.enabled ? "关闭云剪贴板" : "开启云剪贴板") { confirmation = model.enabled ? .disable : .enable }
          .disabled(model.busy)
      }
      if model.enabled && model.loaded {
        TextEditor(text: $model.text).frame(height: 80).border(Color.secondary.opacity(0.3))
        HStack {
          Text("\(model.text.utf16.count) / 4000").font(.caption)
          Spacer()
          Button("上传这段文字") { model.upload() }
            .disabled(model.busy || model.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.text.utf16.count > 4000)
        }
        HStack {
          TextField("搜索云端历史", text: $model.search).onSubmit { model.refresh() }
          Button("搜索 / 刷新") { model.refresh() }.disabled(model.busy)
        }
        List(model.items) { item in
          VStack(alignment: .leading, spacing: 6) {
            Text(item.text).textSelection(.enabled)
            HStack {
              Button("复制到本机") { model.copy(item) }
              Button("删除", role: .destructive) { model.delete(id: item.id) }
            }.disabled(model.busy)
          }.padding(.vertical, 4)
        }
        Button("清空云端历史", role: .destructive) { confirmation = .clear }.disabled(model.busy || model.items.isEmpty)
      }
      if !model.loaded && !model.busy { Button("加载云剪贴板") { model.refresh() } }
      if model.busy { ProgressView() }
      if let message = model.message { Text(message).foregroundStyle(.secondary) }
      Spacer(minLength: 0)
    }
    .padding(20).frame(width: 560, height: 560)
    .onAppear { model.refresh() }
    .onDisappear { model.close() }
    .alert(confirmation == .enable ? "开启云剪贴板？" : "删除云端历史？", isPresented: Binding(
      get: { confirmation != nil }, set: { if !$0 { confirmation = nil } })) {
      Button("取消", role: .cancel) { confirmation = nil }
      Button("确认", role: confirmation == .enable ? nil : .destructive) {
        switch confirmation {
        case .enable: model.setEnabled(true)
        case .disable: model.setEnabled(false)
        case .clear: model.delete()
        case nil: break
        }
        confirmation = nil
      }
    } message: { Text(confirmation == .enable ? "开启后可以手动上传文字，并在已登录的设备上获取。" : "云端历史删除后不可恢复，已复制到其他应用的内容不会被删除。") }
  }
}
