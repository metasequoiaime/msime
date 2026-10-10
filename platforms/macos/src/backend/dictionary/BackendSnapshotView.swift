import AppKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class MacSnapshotModel: ObservableObject {
  @Published var preview: BackendPreparedSnapshot?
  @Published var localPreview = false
  private var localContext: NSDictionary?
  @Published var busy = false
  @Published var message: String?
  private var expectedRevision: Int64?
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
  private func authorize() async throws -> String {
    let identity = try await account.credentials(matchingUserID: accountID, matchingSessionID: sessionID)
    try Task.checkCancellation()
    guard !closed, identity.userID == accountID else { throw CancellationError() }
    sessionID = identity.sessionID
    return identity.token
  }
  private func run(_ action: @escaping @MainActor (String) async throws -> Void) {
    guard !busy, !closed else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await action(try await authorize()) }
      catch is CancellationError { discard() }
      catch { if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  func backup(to destination: URL) {
    run { token in
      let snapshot = try await self.client.dictionarySnapshot(token: token)
      defer { try? FileManager.default.removeItem(at: snapshot.url.deletingLastPathComponent()) }
      try await MacCloudFileTransfer.save(snapshot.url, to: destination) { _ = try await self.authorize() }
      self.message = "已保存完整云词库备份。"
    }
  }
  func prepare(_ url: URL) {
    run { token in
      self.discard()
      let snapshot = try await BackendPreparedSnapshot.prepareDocument(url)
      let cloud = try await self.client.dictionaryCatalog(.quick, code: "", token: token)
      _ = try await self.authorize()
      self.preview = snapshot; self.expectedRevision = cloud.revision
    }
  }
  func downloadForLocal() {
    run { token in
      self.discard()
      let context = try MacPreparedLocalSnapshot.invoke("activeHostOptions")
      let file = try await self.client.dictionarySnapshot(token: token)
      defer { try? FileManager.default.removeItem(at: file.url.deletingLastPathComponent()) }
      let source = file.url
      let copy = Task.detached(priority: .utility) { try BackendPreparedSnapshot(copying: source) }
      let snapshot = try await withTaskCancellationHandler(operation: { try await copy.value }, onCancel: { copy.cancel() })
      _ = try await self.authorize()
      self.preview = snapshot; self.localContext = context; self.localPreview = true
    }
  }
  func applyLocal() {
    guard localPreview, let preview, let context = localContext else { return }
    run { _ in
      let prepared = MacPreparedLocalSnapshot(context: context, snapshot: preview)
      let staging = Task.detached(priority: .utility) { try prepared.stage() }
      try await withTaskCancellationHandler(operation: { try await staging.value }, onCancel: { staging.cancel() })
      let currentToken = try await self.authorize()
      let cloud = try await self.client.dictionaryCatalog(.quick, code: "", token: currentToken)
      _ = try await self.authorize()
      guard cloud.revision == preview.envelope.revision else { throw BackendAccountClient.Failure(status: 409) }
      try prepared.activate()
      self.discard(); self.message = "云词库已应用到本机，后续输入将使用新词库。"
    }
  }
  func restore() {
    guard let preview, let revision = expectedRevision else { return }
    run { token in
      _ = try await self.client.restoreDictionarySnapshot(file: preview.url, expectedSHA256: preview.envelope.sha256, revision: revision, token: token)
      _ = try await self.authorize()
      self.discard(); self.message = "完整备份已恢复到云端。本机词库尚未替换。"
    }
  }
  func discard() { preview = nil; expectedRevision = nil; localPreview = false; localContext = nil }
  func chooseBackup() {
    guard !busy, panel == nil else { return }
    let selected = NSSavePanel(); selected.nameFieldStringValue = "水杉完整云词库.ndjson"; selected.allowedContentTypes = [.data]
    panel = selected
    selected.begin { response in Task { @MainActor in
      defer { self.panel = nil }
      guard !self.closed, response == .OK, let url = selected.url else { return }
      self.backup(to: url)
    } }
  }
  func chooseRestore() {
    guard !busy, panel == nil else { return }
    let selected = NSOpenPanel(); selected.allowsMultipleSelection = false; selected.allowedContentTypes = [.data]
    panel = selected
    selected.begin { response in Task { @MainActor in
      defer { self.panel = nil }
      guard !self.closed, response == .OK, let url = selected.url else { return }
      self.prepare(url)
    } }
  }
  func close() { closed = true; pending?.cancel(); panel?.cancel(nil); panel = nil; discard() }
}

struct MacCloudSnapshotView: View {
  @StateObject private var model: MacSnapshotModel
  @Environment(\.dismiss) private var dismiss
  @State private var restoring = false
  @State private var applyingLocal = false
  init(accountID: String) { _model = StateObject(wrappedValue: MacSnapshotModel(accountID: accountID)) }
  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      HStack { Text("云词库同步与备份").font(.title2); Spacer(); Button("关闭") { model.close(); dismiss() } }
      Text("完整备份包含个人词条、基础词库修改、删除记录、固定位置和调频计数。恢复会替换当前账号的云端词库，其他设备需另行同步。")
      Button("下载云词库并预览本机替换…") { model.downloadForLocal() }.disabled(model.busy)
      Button("保存完整云端备份…") { model.chooseBackup() }.disabled(model.busy)
      Button("选择备份并预览…") { model.chooseRestore() }.disabled(model.busy)
      if let preview = model.preview {
        Text("\(preview.envelope.entries) 个个人词条，\(preview.envelope.overlays) 条修改记录，\(preview.envelope.positions) 个固定位置，\(preview.envelope.selections) 条调频计数。")
        Text(model.localPreview ? "文件完整性已校验。确认后将替换本机个人词库。" : "文件完整性已校验。请确认这是要恢复到当前账号的备份。")
        if model.localPreview {
          Button("用云词库替换本机词库", role: .destructive) { applyingLocal = true }.disabled(model.busy)
        } else {
          Button("用此备份替换云端词库", role: .destructive) { restoring = true }.disabled(model.busy)
        }
        Button("丢弃预览") { model.discard() }.disabled(model.busy)
      }
      if model.busy { ProgressView("正在处理…") }
      if let message = model.message { Text(message).foregroundStyle(.secondary) }
      Spacer()
    }.padding(20).frame(width: 560, height: 460)
    .onDisappear { model.close() }
    .alert("替换本机词库？", isPresented: $applyingLocal) {
      Button("取消", role: .cancel) { }
      Button("确认应用", role: .destructive) { model.applyLocal() }
    } message: { Text("替换本机个人词条、修改、删除和学习记录。请先结束正在输入的内容。本地或云端数据变化时会拒绝应用，请重新下载预览。") }
    .alert("替换完整云词库？", isPresented: $restoring) {
      Button("取消", role: .cancel) { }
      Button("确认恢复", role: .destructive) { model.restore() }
    } message: { Text("此操作将替换全部云端个人词库及学习、删除和排序记录。预览后云端有新变化时会拒绝恢复，请重新选择并确认。") }
  }
}
