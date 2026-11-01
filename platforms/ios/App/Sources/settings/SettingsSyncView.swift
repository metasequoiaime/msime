import SwiftUI

/// 「设置同步」：手动上传本机设置或下载云端设置并应用（#8201 迁移计划 4d）。字段表和云端文档经 Rust 的 `preferences_schema`、`preferences_load`；本机设置整理成账号文档、合并进云端文档在 Rust 的 `msime_client_account_ios_settings_export`，上传用 `preferences_put` 并带读文档时的修订号和账号（P4）；应用分两步（`msime_client_account_ios_settings_apply` 的 plan 和 commit），中间由这里写 App Group，失败时恢复。上传和应用之前都核对读文档时的账号，换了号就不写。
struct SettingsSyncView: View {
  /// 默认是 App 启动时注册的那一份，测试和预览传别的。
  var rust: @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }
  @State private var loadedUserID: String?
  /// 云端文档 `{revision, settings}`，原样交给 Rust。
  @State private var cloud: [String: Any]?
  /// 服务端的字段表，原样交给 Rust 的导出。
  @State private var schema: [String: Any]?
  @State private var busy = false
  @State private var message: String?
  @State private var applying = false
  @State private var uploading = false
  @State private var pending: Task<Void, Never>?
  /// 让离开页面后仍在取消中的旧请求不能释放新页面的忙碌状态或覆盖新数据。
  @State private var request = ViewRequestGeneration()
  private let device = UIDevice.current.userInterfaceIdiom == .pad ? " iPad " : " iPhone "

  private var revision: Int? { (cloud?["revision"] as? NSNumber)?.intValue }
  private var cloudIsEmpty: Bool { (cloud?["settings"] as? [String: Any])?.isEmpty ?? true }

  var body: some View {
    Form {
      Section {
        SettingsFactRow(title: "云端版本",
                        detail: revision.map { "第 \($0) 版" } ?? "尚未读取",
                        symbol: "icloud")
        SettingsActionRow(title: "刷新", detail: "重新读取云端当前版本", symbol: "arrow.clockwise") {
          startLoad()
        }
      } header: {
        SettingsGroupHeader(title: "云端")
      }
      .designRow()
      Section {
        SettingsActionRow(title: "上传本机设置", detail: "用这台\(device)的设置覆盖云端",
                          symbol: "icloud.and.arrow.up",
                          enabled: cloud != nil && schema != nil) { uploading = true }
        SettingsActionRow(title: "下载并应用", detail: "用云端设置覆盖这台\(device)",
                          symbol: "icloud.and.arrow.down",
                          enabled: cloud != nil && !cloudIsEmpty) { applying = true }
      } header: {
        DesignSectionHeader(subject: "同步", info: "同步输入方案、简繁体、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。") { SettingsGroupHeader(title: "同步") }
      }
      .designRow()
    }
    .designPage()
    .settingsStatus(busy: busy, message: message)
    .disabled(busy)
    .navigationTitle("设置同步")
    .task { startLoad() }
    .trackViewRequest(request: $request, pending: $pending, onDisappear: {
      busy = false
      cloud = nil; schema = nil; loadedUserID = nil
    })
    .alert("上传本机设置？", isPresented: $uploading) {
      Button("取消", role: .cancel) { }
      Button("上传") { startUpload() }
    } message: { Text("更新云端对应设置，包括自定义皮肤的背景图片；保留其他平台专属设置。版本冲突时需刷新后重新确认。") }
    .alert("应用云端设置？", isPresented: $applying) {
      Button("取消", role: .cancel) { }
      Button("应用") {
        startApply()
      }
    } message: { Text("将替换本机对应设置，包括词库学习开关；不会下载词库或开启数据上传。") }
  }
  @MainActor private func startLoad() {
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in await load(requestID: id) }
  }
  @MainActor private func load(requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let account = try rust()
      guard let userID = try await account.status()?.id else { throw BackendRustClient.Failure(code: "account_unauthorized") }
      try request.checkCurrent(id)
      let fields = try BackendRustAccount.object(try await account.account(["operation": "preferences_schema"]))
      try request.checkCurrent(id)
      let values = try BackendRustAccount.object(try await account.account(["operation": "preferences_load"]))
      try request.checkCurrent(id)
      guard try await account.status()?.id == userID else { throw CancellationError() }
      try request.checkCurrent(id)
      schema = fields; cloud = values; loadedUserID = userID
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
    }
    catch {
      guard request.isCurrent(id) else { return }
      message = "无法读取云端设置，请稍后重试。"
    }
  }
  @MainActor private func startApply() {
    guard request.isVisible, !busy, let cloud, let loadedUserID else { return }
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in
      await apply(cloud: cloud, userID: loadedUserID, requestID: id)
    }
  }
  @MainActor private func apply(cloud: [String: Any], userID: String, requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let account = try rust()
      guard try await account.status()?.id == userID else { throw CancellationError() }
      try request.checkCurrent(id)
      try await SettingsSyncApply.apply(cloud, client: account.client)
      try request.checkCurrent(id)
      message = "已应用云端设置。"
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
      message = "账号已变化，本机设置未更改，请重新读取。"
    }
    catch {
      guard request.isCurrent(id) else { return }
      message = "账号已变化或云端设置不兼容，本机设置未更改，请重新读取。"
    }
  }
  @MainActor private func startUpload() {
    guard request.isVisible, !busy, let cloud, let schema, let loadedUserID else { return }
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in
      await upload(cloud: cloud, schema: schema, userID: loadedUserID, requestID: id)
    }
  }
  @MainActor private func upload(cloud: [String: Any], schema: [String: Any], userID: String, requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let account = try rust()
      let updated = try await SettingsSyncApply.upload(cloud: cloud, schema: schema, userID: userID, account: account)
      try request.checkCurrent(id)
      guard try await account.status()?.id == userID else { throw CancellationError() }
      try request.checkCurrent(id)
      self.cloud = updated
      message = "本机设置已上传。"
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
    }
    catch let error as BackendRustClient.Failure {
      guard request.isCurrent(id) else { return }
      message = error.code == "account_conflict" ? "云端设置已被其他设备更新，请刷新后重新确认。" : error.localizedDescription
    } catch {
      guard request.isCurrent(id) else { return }
      message = "上传未完成，请稍后重试。"
    }
  }
}
