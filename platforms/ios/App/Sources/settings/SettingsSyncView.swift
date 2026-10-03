import SwiftUI

private enum IOSCloudSettings {
  static func snapshot() throws -> [String: BackendPreferenceValue] {
    let scheme = InputSchemePreference.scheme
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    var settings: [String: BackendPreferenceValue] = [
      "input.character_set": .string(ChineseOutputPreference.usesTraditional ? "traditional" : "simplified"),
      "platform.ios.sound_enabled": .boolean(KeyboardFeedbackPreference.soundEnabled),
      "platform.ios.haptics_enabled": .boolean(KeyboardFeedbackPreference.hapticsEnabled),
      "platform.ios.haptic_strength": .string(KeyboardFeedbackPreference.hapticStrength.rawValue),
      "platform.ios.dictionary_learning": .boolean(
        InputHabitPreference.settings(in: document).learning),
      "platform.ios.global_theme": .string(document.map(GlobalThemePreference.theme(in:)) ?? GlobalThemePreference.selected),
      "platform.ios.custom_theme_base": .string(GlobalThemePreference.base(in: document))
    ]
    // A custom theme without a keyboard design draws its base's keyboard; there is no design to upload then, and the cloud keeps whatever design it has (as the Tauri plugin does).
    if let design = document == nil ? CustomKeyboardSkinStore.stored : GlobalThemePreference.design(in: document) {
      settings["platform.ios.custom_keyboard_skin"] = .string(String(decoding: try JSONEncoder().encode(design), as: UTF8.self))
    }
    // A scheme the cloud cannot carry (Cantonese, Zhuyin, Vietnamese, Tibetan, Stroke) leaves the account's scheme as it is: every other device would reject the whole document over an `input.schema` it does not know.
    if let name = scheme.cloudSchema {
      settings["input.schema"] = .string(name)
      settings["platform.ios.nine_key"] = .boolean(scheme == .nineKey || scheme == .japaneseNineKey)
    }
    if let profile = scheme.shuangpinProfile { settings["input.shuangpin_schema"] = .string(profile) }
    // `input.wubi_schema` 是 `wubi_profile` 在云端的名字，与双拼版本一样只在当前方案是五笔时上传。
    if scheme == .wubi { settings["input.wubi_schema"] = .string(document.map(WubiProfilePreference.profile(in:)) ?? WubiProfilePreference.profile) }
    return settings
  }
  static func apply(_ values: [String: BackendPreferenceValue]) throws {
    let plan = try IOSPreferencePlan(values, themes: Set(GlobalThemeCatalog.ids))
    let custom = try plan.customSkinJSON.map { try JSONDecoder().decode(CustomKeyboardSkin.self, from: Data($0.utf8)).normalized }
    let design = try custom.map { skin -> [String: Any] in
      guard let value = CustomKeyboardSkin.documentValue(skin) else { throw CocoaError(.fileWriteUnknown) }
      return value
    }
    // Validate everything before writing. Unknown platforms' values stay in the
    // cloud and are never assigned to local defaults.
    // Learning, the theme, the scheme and the output form live in the shared document, which the keyboard copies over the App Group on every reload. Those are the writes that can fail, so they go first and a failure leaves every App Group setting unchanged.
    if let learning = plan.learning, InputHabitPreference.update({ $0.learning = learning }) == nil {
      throw CocoaError(.fileWriteUnknown)
    }
    let scheme = plan.scheme.flatMap(ChineseInputScheme.init(rawValue:))
    let enabled = InputSchemePreference.enabledSchemes
    let schemeFields = scheme.flatMap { MetasequoiaInputSessionBridge.schemeMapping($0, enabledSchemes: enabled) }
    let written = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      if let theme = plan.globalTheme { document["global_theme"] = theme }
      if plan.customThemeBase != nil || design != nil {
        var customTheme = GlobalThemePreference.customTheme(in: document)
        if let base = plan.customThemeBase {
          if base == GlobalThemeCatalog.systemId { customTheme.removeValue(forKey: "base") } else { customTheme["base"] = base }
        }
        if let design { customTheme["keyboard"] = design }
        document["custom_theme"] = customTheme
      }
      schemeFields?(&document)
      if let profile = plan.wubiProfile { document[WubiProfilePreference.documentKey] = profile }
      if let traditional = plan.traditional { document[ChineseOutputPreference.documentKey] = traditional }
    }
    guard written else { throw CocoaError(.fileWriteUnknown) }
    if let scheme { InputSchemePreference.scheme = scheme }
    if let profile = plan.wubiProfile { WubiProfilePreference.profile = profile }
    if let traditional = plan.traditional { ChineseOutputPreference.usesTraditional = traditional }
    let defaults = KeyboardFeedbackPreference.defaults
    if let sound = plan.sound { defaults.set(sound, forKey: KeyboardFeedbackPreference.soundKey) }
    if let haptics = plan.haptics { defaults.set(haptics, forKey: KeyboardFeedbackPreference.hapticsKey) }
    if let strength = plan.strength { defaults.set(strength, forKey: KeyboardFeedbackPreference.strengthKey) }
    if let document = MetasequoiaInputSessionBridge.loadSharedPreferences() { GlobalThemePreference.mirror(document) }
  }
}

struct SettingsSyncView: View {
  let session: BackendAccountSession
  let client: BackendAccountClient
  @State private var loadedUserID: String?
  @State private var cloud: BackendAccountClient.Preferences?
  @State private var schema: BackendAccountClient.PreferenceSchema?
  @State private var busy = false
  @State private var message: String?
  @State private var applying = false
  @State private var uploading = false
  @State private var pending: Task<Void, Never>?
  private let device = UIDevice.current.userInterfaceIdiom == .pad ? " iPad " : " iPhone "

  var body: some View {
    Form {
      Section("云端") {
        SettingsFactRow(title: "云端版本",
                        detail: cloud.map { "第 \($0.revision) 版" } ?? "尚未读取",
                        symbol: "icloud")
        SettingsActionRow(title: "刷新", detail: "重新读取云端当前版本", symbol: "arrow.clockwise") {
          pending = Task { await load() }
        }
      }
      Section {
        SettingsActionRow(title: "上传本机设置", detail: "用这台\(device)的设置覆盖云端",
                          symbol: "icloud.and.arrow.up",
                          enabled: cloud != nil && schema != nil) { uploading = true }
        SettingsActionRow(title: "下载并应用", detail: "用云端设置覆盖这台\(device)",
                          symbol: "icloud.and.arrow.down",
                          enabled: cloud?.settings.isEmpty == false) { applying = true }
      } header: {
        Text("同步")
      } footer: {
        Text("同步输入方案、简繁体、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。")
      }
    }
    .settingsStatus(busy: busy, message: message)
    .disabled(busy)
    .navigationTitle("设置同步")
    .task { await load() }
    .onDisappear { pending?.cancel(); cloud = nil }
    .alert("上传本机设置？", isPresented: $uploading) {
      Button("取消", role: .cancel) { }
      Button("上传") { pending = Task { await upload() } }
    } message: { Text("更新云端对应设置，包括自定义皮肤的背景图片；保留其他平台专属设置。版本冲突时需刷新后重新确认。") }
    .alert("应用云端设置？", isPresented: $applying) {
      Button("取消", role: .cancel) { }
      Button("应用") {
        pending = Task { await apply() }
      }
    } message: { Text("将替换本机对应设置，包括词库学习开关；不会下载词库或开启数据上传。") }
  }
  @MainActor private func load() async {
    guard !busy else { return }
    busy = true; message = nil
    defer { busy = false }
    do {
      let identity = try await session.credentials()
      let fields = try await client.preferenceSchema(token: identity.token)
      let values = try await client.preferences(token: identity.token)
      guard try await session.user()?.id == identity.userID else { throw CancellationError() }
      try Task.checkCancellation()
      schema = fields; cloud = values; loadedUserID = identity.userID
    } catch is CancellationError { }
    catch { message = "无法读取云端设置，请稍后重试。" }
  }
  @MainActor private func apply() async {
    do {
      guard let cloud, let loadedUserID, try await session.user()?.id == loadedUserID else { throw BackendAccountClient.Failure(status: 401) }
      try Task.checkCancellation()
      try IOSCloudSettings.apply(cloud.settings)
      message = "已应用云端设置。"
    } catch is CancellationError { }
    catch { message = "账号已变化或云端设置不兼容，本机设置未更改，请重新读取。" }
  }
  @MainActor private func upload() async {
    guard !busy, let cloud, let schema else { return }
    busy = true; message = nil
    defer { busy = false }
    do {
      let values = try BackendAccountClient.mergedPreferences(cloud, replacing: IOSCloudSettings.snapshot(), schema: schema)
      let identity = try await session.credentials()
      guard identity.userID == loadedUserID else { throw BackendAccountClient.Failure(status: 401) }
      try Task.checkCancellation()
      self.cloud = try await client.putPreferences(values, token: identity.token)
      message = "本机设置已上传。"
    } catch is CancellationError { }
    catch let error as BackendAccountClient.Failure {
      message = error.status == 409 ? "云端设置已被其他设备更新，请刷新后重新确认。" : error.localizedDescription
    } catch { message = "上传未完成，请稍后重试。" }
  }
}
