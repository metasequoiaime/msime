import SwiftUI

struct SettingsSyncView: View {
  let session: BackendAccountSession
  let client: BackendAccountClient
  @State private var loadedUserID: String?
  @State private var loadedSessionID: UUID?
  @State private var cloud: BackendAccountClient.Preferences?
  @State private var schema: BackendAccountClient.PreferenceSchema?
  @State private var busy = false
  @State private var message: String?
  @State private var applying = false
  @State private var uploading = false
  @State private var pending: Task<Void, Never>?
  /// 让离开页面后仍在取消中的旧请求不能释放新页面的忙碌状态或覆盖新数据。
  @State private var request = ViewRequestGeneration()
  private let device = UIDevice.current.userInterfaceIdiom == .pad ? " iPad " : " iPhone "

  var body: some View {
    Form {
      Section {
        SettingsFactRow(title: "云端版本",
                        detail: cloud.map { "第 \($0.revision) 版" } ?? "尚未读取",
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
                          enabled: cloud?.settings.isEmpty == false) { applying = true }
      } header: {
        SettingsGroupHeader(title: "同步")
      } footer: {
        Text("同步输入方案、简繁体、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。")
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
      cloud = nil; schema = nil; loadedUserID = nil; loadedSessionID = nil
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
      let identity = try await session.credentials()
      try request.checkCurrent(id)
      let fields = try await session.authenticated(matchingUserID: identity.userID,
                                                   matchingSessionID: identity.sessionID) { token in
        try await client.preferenceSchema(token: token)
      }.value
      try request.checkCurrent(id)
      let values = try await session.authenticated(matchingUserID: identity.userID,
                                                   matchingSessionID: identity.sessionID) { token in
        try await client.preferences(token: token)
      }.value
      try request.checkCurrent(id)
      try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
      try request.checkCurrent(id)
      try Task.checkCancellation()
      schema = fields; cloud = values; loadedUserID = identity.userID; loadedSessionID = identity.sessionID
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
    }
    catch {
      guard request.isCurrent(id) else { return }
      message = "无法读取云端设置，请稍后重试。"
    }
  }
  @MainActor private func startApply() {
    guard request.isVisible, !busy, let cloud, let loadedUserID, let loadedSessionID else { return }
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in
      await apply(cloud: cloud, userID: loadedUserID, sessionID: loadedSessionID, requestID: id)
    }
  }
  @MainActor private func apply(cloud: BackendAccountClient.Preferences,
                                userID: String,
                                sessionID: UUID,
                                requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      try await session.requireSession(matchingUserID: userID, matchingSessionID: sessionID)
      try request.checkCurrent(id)
      try Task.checkCancellation()
      try IOSCloudSettings.apply(cloud.settings)
      try request.checkCurrent(id)
      message = "已应用云端设置。"
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
    }
    catch {
      guard request.isCurrent(id) else { return }
      message = "账号已变化或云端设置不兼容，本机设置未更改，请重新读取。"
    }
  }
  @MainActor private func startUpload() {
    guard request.isVisible, !busy, let cloud, let schema, let loadedUserID, let loadedSessionID else { return }
    ViewRequestTask.run(request: $request, busy: $busy, message: $message,
                        pending: $pending) { id in
      await upload(cloud: cloud, schema: schema, userID: loadedUserID, sessionID: loadedSessionID, requestID: id)
    }
  }
  @MainActor private func upload(cloud: BackendAccountClient.Preferences,
                                 schema: BackendAccountClient.PreferenceSchema,
                                 userID: String,
                                 sessionID: UUID,
                                 requestID id: UUID) async {
    guard request.isCurrent(id) else { return }
    do {
      let values = try BackendAccountClient.mergedPreferences(cloud, replacing: IOSCloudSettings.snapshot(), schema: schema)
      let identity = try await session.credentials(matchingUserID: userID,
                                                   matchingSessionID: sessionID)
      try request.checkCurrent(id)
      try Task.checkCancellation()
      let updated = try await session.authenticated(matchingUserID: identity.userID,
                                                   matchingSessionID: identity.sessionID) { token in
        try await client.putPreferences(values, token: token)
      }.value
      try request.checkCurrent(id)
      try await session.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
      try request.checkCurrent(id)
      try Task.checkCancellation()
      self.cloud = updated
      message = "本机设置已上传。"
    } catch is CancellationError {
      guard request.isCurrent(id) else { return }
    }
    catch let error as BackendAccountClient.Failure {
      guard request.isCurrent(id) else { return }
      message = error.status == 409 ? "云端设置已被其他设备更新，请刷新后重新确认。" : error.localizedDescription
    } catch {
      guard request.isCurrent(id) else { return }
      message = "上传未完成，请稍后重试。"
    }
  }
}
