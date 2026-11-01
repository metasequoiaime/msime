import SwiftUI

/// 应用到本机：整份云词库由 Rust 下载并校验（`snapshot_export_file`，#8201 迁移计划 4c），再排进与键盘共用的 Swift 快照队列（`DictionarySnapshotQueue`），由键盘在空闲时应用。iOS 的「应用到本机」仍由键盘扩展激活，不走 Rust 的快照队列。
struct CloudDictionaryApplyView: View {
  let accountID: String
  @State private var state = DictionarySnapshotQueueState()
  @State private var preview: BackendPreparedSnapshot?
  @State private var expectedVersion: String?
  @State private var confirming = false
  @State private var busy = false
  @State private var message: String?
  @State private var probe = ""
  @State private var pending: Task<Void, Never>?
  @State private var request = ViewRequestGeneration()
  private let queue = DictionarySnapshotQueue()
  private var dictionary: IOSCloudDictionary { IOSCloudDictionary(accountID: accountID) }

  var body: some View {
    List {
      // 说明、试打框、状态、动作原来是四个平铺的列表行,说明排在最前面,而它讲的是整组要做的事。
      Section {
        SettingsFactRow(title: state.localVersion == nil ? "尚未获取键盘词库版本" : "已获取本地词库版本",
                        detail: state.localVersion == nil ? "在下面的输入框里打开水杉键盘" : nil,
                        symbol: state.localVersion == nil ? "questionmark.circle.fill" : "checkmark.circle.fill")
        TextField("点此打开水杉键盘", text: $probe)
          .textInputAutocapitalization(.never).autocorrectionDisabled()
        SettingsActionRow(title: "下载云词库并预览", detail: "先看清楚要替换成什么，再决定",
                          symbol: "icloud.and.arrow.down.fill",
                          enabled: !busy && state.localVersion != nil && state.request?.status.active != true) { download() }
      } header: {
        DesignSectionHeader(subject: "准备本地词库", info: "请启用水杉键盘的完全访问权限，并在上方打开水杉键盘，让键盘提供当前词库版本。测试区内容不会上传。") { SettingsGroupHeader(title: "准备本地词库") }
      }
      .designRow()
      if let preview {
        Section {
          SettingsFactRow(title: "云端版本 \(preview.envelope.revision)",
                          detail: "\(preview.envelope.entries) 个词条 · \(preview.envelope.overlays) 条覆盖 · \(preview.envelope.positions) 个固定位置",
                          symbol: "icloud.fill")
          SettingsActionRow(title: "替换本机词库", detail: "覆盖本机个人词库、学习、删除和排序记录",
                            symbol: "arrow.left.arrow.right", destructive: true, enabled: !busy) { confirming = true }
          SettingsActionRow(title: "丢弃预览", symbol: "xmark.circle.fill", enabled: !busy) {
            self.preview = nil; expectedVersion = nil
          }
        } header: {
          DesignSectionHeader(subject: "确认应用", info: "下载期间本机状态发生变化时，键盘会拒绝本次应用。") { SettingsGroupHeader(title: "确认应用") }
        }
        .designRow()
      }
      if let request = state.request, request.accountID == accountID {
        Section {
          SettingsFactRow(title: status(request.status),
                          symbol: request.status.active ? "clock.arrow.circlepath" : "checkmark.circle.fill")
          if request.status.active {
            SettingsActionRow(title: "取消待应用快照", symbol: "xmark.circle.fill", destructive: true) {
              do { try queue.cancel(accountID: accountID); refresh() }
              catch { message = error.localizedDescription }
            }
          }
        } header: {
          SettingsGroupHeader(title: "处理结果")
        } footer: {
          if request.status.active {
            Text("保持水杉键盘开启，等待准备完成；结束当前输入后会尝试应用。")
          }
        }
        .designRow()
      }
    }
    .designPage()
    .settingsStatus(busy: busy, busyTitle: "正在准备…", message: message)
    .navigationTitle("应用云词库")
    .task {
      await ViewRequestTask.poll(every: .seconds(2)) { refresh() }
    }
    .trackViewRequest(request: $request, pending: $pending, onDisappear: {
      busy = false
      preview = nil; expectedVersion = nil; probe = ""
    })
    .alert("替换本机词库？", isPresented: $confirming) {
      Button("取消", role: .cancel) { }
      Button("确认替换", role: .destructive) { enqueue() }
    } message: { Text("将以这份云端快照替换本机个人词库及学习记录。确认后由水杉键盘在空闲时处理，请先确认云端数据完整。") }
  }
  private func status(_ value: DictionarySnapshotRequest.Status) -> String {
    switch value {
    case .queued: return "等待水杉键盘接收。"
    case .preparing: return "键盘正在准备词库或等待空闲会话。"
    case .applied: return "已应用到本机词库。"
    case .conflict: return "本地词库已变化，未应用。请重新下载并确认。"
    case .failed: return "准备失败，原有词库未更改。请重新下载后重试。"
    case .cancelled: return "待应用请求已取消。"
    }
  }
  @MainActor private func refresh() {
    do { state = try queue.read() }
    catch { message = error.localizedDescription }
  }
  @MainActor private func download() {
    guard request.isVisible, !busy, let version = state.localVersion else { return }
    preview = nil
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending) { id in
      guard try PersonalDictionaryStore().read().pendingCount == 0 else { throw DictionarySnapshotQueue.Failure.busy }
      let file = try IOSCloudDictionary.privateFile("msime-dictionary-snapshot.ndjson")
      defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
      _ = try await dictionary.request(["operation": "snapshot_export_file", "path": file.path])
      try request.checkCurrent(id)
      let prepared = try await BackendPreparedSnapshot.prepareDocument(file)
      try request.checkCurrent(id)
      preview = prepared; expectedVersion = version
    }
  }
  @MainActor private func enqueue() {
    guard request.isVisible, !busy, let preview, let version = expectedVersion else { return }
    ViewRequestTask.run(request: $request, busy: $busy, message: $message, pending: $pending) { id in
      let changes = try await dictionary.object(["operation": "changes", "after": preview.envelope.revision, "limit": 1])
      try request.checkCurrent(id)
      guard let list = changes["changes"] as? [Any] else { throw BackendRustClient.Failure.invalidResponse }
      guard list.isEmpty else {
        self.preview = nil; expectedVersion = nil
        message = "预览后云词库已变化，请重新下载并确认。"
        return
      }
      let copy = Task.detached(priority: .utility) {
        try queue.enqueue(file: preview.url, accountID: accountID, cloudRevision: preview.envelope.revision,
          expectedLocalVersion: version, fileSHA256: preview.fileSHA256)
      }
      _ = try await withTaskCancellationHandler(operation: { try await copy.value }, onCancel: { copy.cancel() })
      try request.checkCurrent(id)
      self.preview = nil; expectedVersion = nil; refresh()
    }
  }
}
