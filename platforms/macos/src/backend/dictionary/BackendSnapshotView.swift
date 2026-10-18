import AppKit
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class MacSnapshotModel: ObservableObject {
  /// Rust 给出的快照元数据（`snapshot`）里页面显示和恢复要用的几项。
  struct Preview: Equatable {
    let entries: Int64
    let overlays: Int64
    let positions: Int64
    let selections: Int64
    let sha256: String

    init(_ value: Any?) throws {
      guard let object = value as? [String: Any], let sha256 = object["sha256"] as? String, !sha256.isEmpty else {
        throw BackendRustClient.Failure.invalidResponse
      }
      func count(_ key: String) throws -> Int64 {
        guard let number = object[key] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.int64Value >= 0 else {
          throw BackendRustClient.Failure.invalidResponse
        }
        return number.int64Value
      }
      entries = try count("entries"); overlays = try count("overlays"); positions = try count("positions")
      selections = try count("selections"); self.sha256 = sha256
    }
  }
  @Published var preview: Preview?
  @Published var localPreview = false
  @Published var busy = false
  @Published var message: String?
  private var expectedRevision: Int64?
  private var restoreFile: URL?
  private var previewToken: String?
  private let accountID: String
  /// 整份快照经 Rust（#8201 迁移计划 4c）：下载、备份、恢复走文件（`snapshot_export_file`、`snapshot_restore_preview_file`、`snapshot_restore_file`），「应用到本机」排进 Rust 的快照队列（`snapshot_preview`、`snapshot_enqueue`），由 Rust 请输入会话放开词库后在本进程里激活。nil 是宿主没有注册会话存储。
  private let rust: BackendRustAccount?
  /// 本机的 HostOptions 文档（`MacLocalDictionary.hostOptions`），只有输入法进程里有活动的输入会话时才有。
  private let hostOptions: @MainActor () throws -> [String: Any]
  private var pending: Task<Void, Never>?
  private var watching: Task<Void, Never>?
  private var panel: NSSavePanel?
  private var closed = false
  init(accountID: String, rust: BackendRustAccount? = .shared,
       hostOptions: @escaping @MainActor () throws -> [String: Any] = MacLocalDictionary.hostOptions) {
    self.accountID = accountID; self.rust = rust; self.hostOptions = hostOptions
  }
  private func request(_ action: [String: Any], options: [String: Any]? = nil) async throws -> [String: Any] {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    return try BackendRustAccount.object(try await rust.cloudDictionary(action, snapshotDirectory: try MacCloudTransferDirectory.url(), options: options))
  }
  /// 打开窗口时的账号还登录着、窗口也没关才继续。C 上拿不到会话代次，只按 user.id 绑定。
  private func requireAccount() async throws {
    guard let rust else { throw BackendRustClient.Failure.unavailable }
    let current = try await rust.status()?.id
    try Task.checkCancellation()
    guard !closed, current == accountID else { throw CancellationError() }
  }
  private func run(_ action: @escaping @MainActor @Sendable () async throws -> Void) {
    guard !busy, !closed else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do {
        try await requireAccount()
        try await action()
      }
      catch is CancellationError { discard() }
      catch { if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  /// Rust 下载、校验完才经 `destination` 旁边的临时文件换进去；失败时 `destination` 上原有的文件不动。
  func backup(to destination: URL) {
    run {
      let scoped = destination.startAccessingSecurityScopedResource()
      defer { if scoped { destination.stopAccessingSecurityScopedResource() } }
      _ = try await self.request(["operation": "snapshot_export_file", "path": destination.path])
      try await self.requireAccount()
      self.message = "已保存完整云词库备份。"
    }
  }
  func prepare(_ url: URL) {
    run {
      self.discard()
      let scoped = url.startAccessingSecurityScopedResource()
      defer { if scoped { url.stopAccessingSecurityScopedResource() } }
      let result = try await self.request(["operation": "snapshot_restore_preview_file", "path": url.path])
      guard let revision = result["expectedRevision"] as? NSNumber else { throw BackendRustClient.Failure.invalidResponse }
      let preview = try Preview(result["snapshot"])
      try await self.requireAccount()
      self.preview = preview; self.expectedRevision = revision.int64Value; self.restoreFile = url
    }
  }
  func downloadForLocal() {
    run {
      self.discard()
      let result = try await self.request(["operation": "snapshot_preview"], options: try self.hostOptions())
      guard let token = result["previewToken"] as? String, !token.isEmpty else { throw BackendRustClient.Failure.invalidResponse }
      let preview = try Preview(result["snapshot"])
      try await self.requireAccount()
      self.preview = preview; self.previewToken = token; self.localPreview = true
    }
  }
  /// 入列后 Rust 马上请输入会话放开词库、准备并激活；放不开时请求留在磁盘上的队列里，窗口开着时每两秒问一次状态（Rust 最多每 60 秒重试一次），关掉窗口也不丢，下一次有面板问状态时接着处理。
  func applyLocal() {
    guard localPreview, let token = previewToken else { return }
    run {
      let options = try self.hostOptions()
      let result = try await MacLocalDictionary.wakingInputSessions { try await self.request(["operation": "snapshot_enqueue", "token": token], options: options) }
      self.discard()
      self.show(result["request"])
      if Self.waiting(result["request"]) { self.watch(options) }
    }
  }
  func restore() {
    guard let preview, let revision = expectedRevision, let file = restoreFile else { return }
    run {
      let scoped = file.startAccessingSecurityScopedResource()
      defer { if scoped { file.stopAccessingSecurityScopedResource() } }
      _ = try await self.request(["operation": "snapshot_restore_file", "path": file.path, "expected_sha256": preview.sha256, "revision": revision])
      try await self.requireAccount()
      self.discard(); self.message = "完整备份已恢复到云端。本机词库尚未替换。"
    }
  }
  private static func waiting(_ request: Any?) -> Bool {
    let status = (request as? [String: Any])?["status"] as? String
    return status == "queued" || status == "preparing"
  }
  private func show(_ request: Any?) {
    switch (request as? [String: Any])?["status"] as? String {
    case "applied": message = "云词库已应用到本机，后续输入将使用新词库。"
    case "queued", "preparing": message = "已排进本机的词库队列，输入法放开词库后应用。关掉窗口也不会丢。"
    case "conflict": message = "本机或云端词库在预览之后有变化，没有应用。请重新下载预览。"
    case "cancelled": message = "已取消应用到本机。"
    case "failed": message = "没有应用到本机，请重试。"
    default: break
    }
  }
  private func watch(_ options: [String: Any]) {
    watching?.cancel()
    watching = Task {
      while !Task.isCancelled, !closed {
        try? await Task.sleep(nanoseconds: 2_000_000_000)
        guard !Task.isCancelled, !closed,
              let result = try? await MacLocalDictionary.wakingInputSessions({ try await self.request(["operation": "snapshot_status"], options: options) }) else { continue }
        show(result["request"])
        if !Self.waiting(result["request"]) { return }
      }
    }
  }
  func discard() { preview = nil; expectedRevision = nil; restoreFile = nil; previewToken = nil; localPreview = false }
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
  func close() { closed = true; pending?.cancel(); watching?.cancel(); panel?.cancel(nil); panel = nil; discard() }
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
        Text("\(preview.entries) 个个人词条，\(preview.overlays) 条修改记录，\(preview.positions) 个固定位置，\(preview.selections) 条调频计数。")
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
