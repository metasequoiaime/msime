import SwiftUI

/// 「翻译服务」: which service fills candidate glosses that the offline dictionary cannot. Saved into the shared preference document, which the keyboard reloads the next time it appears.
struct TranslationProviderSettingsView: View {
  @State private var provider = TranslationProvider.off
  @State private var niutransAppID = ""
  @State private var niutransKey = ""
  @State private var tencentID = ""
  @State private var tencentKey = ""
  @State private var tencentRegion = TranslationProviderPreference.defaultTencentRegion
  @State private var customEndpoint = ""
  @State private var customKey = ""
  @State private var status: String?
  @State private var testing = false
  /// What the shared document holds, so reading it in does not write it back; nil until it has been read.
  @State private var savedProvider: TranslationProvider?
  @State private var savedFields: [String] = []
  @StateObject private var autosave = SettingsAutosave()

  var body: some View {
    Form {
      Section {
        Picker("翻译服务", selection: $provider) {
          ForEach(TranslationProvider.allCases, id: \.self) { Text($0.title).tag($0) }
        }
        .accessibilityIdentifier("translationProviderPicker")
        .onChange(of: provider) { _, next in
          guard savedProvider != nil, next != savedProvider else { return }
          autosave.saveNow { try save() }
        }
      } footer: {
        Text("选择自己的翻译服务后，键盘把这一页的中文候选直接发给该服务，不经过水杉账号；凭据只保存在本设备的共享设置里。所选服务的凭据不完整时，键盘不会联网翻译，也不会改用其他服务。")
      }
      .designRow()
      switch provider {
      case .off:
        Section {
          Text("键盘不会把候选词发给任何在线服务；英文释义仍来自离线词库。").foregroundStyle(.secondary)
        }
        .designRow()
      case .account:
        Section {
          Text("候选词会发送到水杉服务器（api.msime.app）翻译，首次使用会自动创建匿名账号。").foregroundStyle(.secondary)
        }
        .designRow()
      case .niutrans:
        Section {
          TextField("APPID", text: $niutransAppID).credentialField()
          SecureField("API Key", text: $niutransKey).credentialField()
        } header: {
          SettingsGroupHeader(title: "小牛翻译")
        }
        .designRow()
      case .tencent:
        Section {
          TextField("SecretId", text: $tencentID).credentialField()
          SecureField("SecretKey", text: $tencentKey).credentialField()
          TextField("地域", text: $tencentRegion).credentialField()
        } header: {
          SettingsGroupHeader(title: "腾讯云机器翻译")
        } footer: {
          Text("地域留空时使用 \(TranslationProviderPreference.defaultTencentRegion)。建议为输入法单独创建只授权机器翻译的子账号密钥。")
        }
        .designRow()
      case .custom:
        Section {
          TextField("https://example.com/translate", text: $customEndpoint).credentialField()
            .keyboardType(.URL)
          SecureField("API Key（可选）", text: $customKey).credentialField()
        } header: {
          SettingsGroupHeader(title: "自定义接口")
        } footer: {
          Text("兼容 DeepLX 的 POST 接口。iOS 只允许 HTTPS 地址，HTTP 地址保存后不会被调用。")
        }
        .designRow()
      }
      Section {
        if credentialsIncomplete {
          Text("凭据不完整，键盘暂不联网翻译。").font(.footnote).foregroundStyle(.secondary)
        }
        if provider != .account && provider != .off {
          Button(testing ? "正在测试…" : "测试翻译「你好」") { test() }
            .disabled(testing)
            .accessibilityIdentifier("translationProviderTest")
        }
        if let status {
          Text(status).font(.footnote).foregroundStyle(.secondary)
        }
      } footer: {
        SettingsAutosaveStatus(autosave: autosave)
      }
      .designRow()
    }
    .designPage()
    .navigationTitle("翻译服务")
    .navigationBarTitleDisplayMode(.inline)
    .onAppear(perform: load)
    .onChange(of: fields) { _, next in
      guard savedProvider != nil, next != savedFields || autosave.hasPending else { return }
      autosave.schedule { try save() }
    }
    .flushesAutosave(autosave)
  }

  /// Every credential and address on the page, compared to decide whether an edit needs saving.
  private var fields: [String] {
    [niutransAppID, niutransKey, tencentID, tencentKey, tencentRegion, customEndpoint, customKey]
  }

  private var credentialsIncomplete: Bool {
    guard provider != .off, provider != .account else { return false }
    var document: [String: Any] = [:]
    write(into: &document)
    return TranslationProviderPreference.route(in: document) == .none
  }

  private func load() {
    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    provider = TranslationProviderPreference.selected(in: preferences)
    let niutrans = preferences?[TranslationProviderPreference.niutransKey] as? [String: Any] ?? [:]
    let tencent = preferences?[TranslationProviderPreference.tencentKey] as? [String: Any] ?? [:]
    let custom = preferences?[TranslationProviderPreference.customKey] as? [String: Any] ?? [:]
    niutransAppID = niutrans["app_id"] as? String ?? ""
    niutransKey = niutrans["apikey"] as? String ?? ""
    tencentID = tencent["secret_id"] as? String ?? ""
    tencentKey = tencent["secret_key"] as? String ?? ""
    let region = tencent["region"] as? String ?? ""
    tencentRegion = region.isEmpty ? TranslationProviderPreference.defaultTencentRegion : region
    customEndpoint = custom["endpoint"] as? String ?? ""
    customKey = custom["api_key"] as? String ?? ""
    savedProvider = provider
    savedFields = fields
    autosave.reset()
  }

  private func write(into document: inout [String: Any]) {
    TranslationProviderPreference.select(provider, niutrans: (niutransAppID, niutransKey),
                                         tencent: (tencentID, tencentKey, tencentRegion),
                                         custom: (customEndpoint, customKey), in: &document)
  }

  /// Writes the whole page, so a provider switch also keeps the credentials typed so far.
  private func save() throws {
    let saved = MetasequoiaInputSessionBridge.updateSharedPreferences { write(into: &$0) }
    guard saved else { throw ServiceFailure(message: "保存失败，请稍后再试。") }
    savedProvider = provider
    savedFields = fields
  }

  private func test() {
    var document: [String: Any] = [:]
    write(into: &document)
    let route = TranslationProviderPreference.route(in: document)
    guard route != .none, route != .account else { status = "凭据不完整。"; return }
    testing = true
    status = nil
    Task {
      let result = await TranslationProviderClient().translate(words: ["你好"], target: "EN", route: route)
      testing = false
      if let gloss = result.first ?? nil {
        status = "测试成功：你好 → \(gloss)"
      } else {
        status = "测试失败：请检查凭据、地址与网络。"
      }
    }
  }
}

private extension View {
  func credentialField() -> some View {
    textInputAutocapitalization(.never).autocorrectionDisabled()
  }
}
