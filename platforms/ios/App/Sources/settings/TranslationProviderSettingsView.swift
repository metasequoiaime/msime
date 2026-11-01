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
  /// Whether 「AI 辅助」 can translate: the keyboard has an AI configuration published to it and the shared document names its provider, endpoint and model. Its summary is 「服务商 · 模型」.
  @State private var aiReady = false
  @State private var aiSummary = ""
  @State private var status: String?
  @State private var testing = false
  @State private var operation: Task<Void, Never>?
  @State private var testGeneration = OperationGeneration()
  /// What the shared document holds, so reading it in does not write it back; nil until it has been read.
  @State private var savedProvider: TranslationProvider?
  @State private var savedFields: [String] = []
  @StateObject private var autosave = SettingsAutosave()

  /// 翻译服务分区「?」里的说明：先讲当前选择的去向，再讲自选服务的通用规则。
  private var providerInfo: String {
    let general = "选择自己的翻译服务后，键盘把这一页的中文候选直接发给该服务，不经过水杉账号；凭据只保存在本设备的共享设置里。所选服务的凭据不完整时，键盘不会联网翻译，也不会改用其他服务。"
    switch provider {
    case .off: return "键盘不会把候选词发给任何在线服务；英文释义仍来自离线词库。\n\n" + general
    case .account: return "候选词会发送到水杉服务器（api.msime.app）翻译，首次使用会自动创建匿名账号。\n\n" + general
    default: return general
    }
  }

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
      } header: {
        DesignSectionHeader(info: providerInfo)
      }
      .designRow()
      switch provider {
      case .off, .account:
        EmptyView()
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
          DesignSectionHeader(info: "地域留空时使用 \(TranslationProviderPreference.defaultTencentRegion)。建议为输入法单独创建只授权机器翻译的子账号密钥。") { SettingsGroupHeader(title: "腾讯云机器翻译") }
        }
        .designRow()
      case .custom:
        Section {
          TextField("https://example.com/translate", text: $customEndpoint).credentialField()
            .keyboardType(.URL)
          SecureField("API Key（DeepL 必填）", text: $customKey).credentialField()
        } header: {
          DesignSectionHeader(info: "兼容 DeepLX 的 POST 接口，或 DeepL 官方 API 地址（https://api-free.deepl.com/v2/translate 或 https://api.deepl.com/v2/translate）配 DeepL 的 Auth Key；使用 DeepL 时候选词会发送到 DeepL，按 DeepL 的数据政策处理。iOS 只允许 HTTPS 地址，HTTP 地址保存后不会被调用。") { SettingsGroupHeader(title: "自定义接口") }
        }
        .designRow()
      case .ai:
        Section {
          LabeledContent("当前服务商与模型", value: aiSummary.isEmpty ? "未配置" : aiSummary)
            .accessibilityIdentifier("translationAISummary")
        } header: {
          DesignSectionHeader(info: "复用「AI 润色与回复」页当前服务商的接口地址、Token 与模型，无需在此重复填写；AI 模型逐条生成释义，成本与延迟高于专用翻译服务。不需要打开「候选栏 AI 候选」。") { SettingsGroupHeader(title: "复用 AI 辅助配置") }
        }
        .designRow()
      }
      Section {
        if credentialsIncomplete {
          Text(provider == .ai
            ? "AI 配置不完整，键盘暂不联网翻译。请在「AI 润色与回复」页打开「在键盘中启用 AI」，填好接口地址、模型和 API Key 后保存配置。"
            : "凭据不完整，键盘暂不联网翻译。").font(.footnote).foregroundStyle(.secondary)
        }
        // AI 辅助的连接在「AI 润色与回复」页测试，本页没有它的凭据。
        if provider != .account && provider != .off && provider != .ai {
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
    .onDisappear {
      ViewRequestTask.cancel(generation: $testGeneration, task: $operation)
      testing = false
    }
    .flushesAutosave(autosave)
  }

  /// Every credential and address on the page, compared to decide whether an edit needs saving.
  private var fields: [String] {
    [niutransAppID, niutransKey, tencentID, tencentKey, tencentRegion, customEndpoint, customKey]
  }

  private var credentialsIncomplete: Bool {
    guard provider != .off, provider != .account else { return false }
    if provider == .ai { return !aiReady }
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
    let ai = preferences?["ai_assistant"] as? [String: Any] ?? [:]
    let probe: [String: Any] = [TranslationProviderPreference.aiKey: true,
                                TranslationProviderPreference.tencentKey: ["enabled": false], "ai_assistant": ai]
    if case let .ai(_, endpoint, _, _) = TranslationProviderPreference.route(in: probe) {
      aiReady = KeyboardAIService.translationToken(endpoint: endpoint) != nil
    } else {
      aiReady = false
    }
    let aiProvider = ai["provider"] as? String ?? ""
    let aiModel = ai["model"] as? String ?? ""
    let aiTitle = AIProviderPreset(rawValue: aiProvider)?.title ?? aiProvider
    aiSummary = aiProvider.isEmpty ? "" : "\(aiTitle.isEmpty ? aiProvider : aiTitle) · \(aiModel.isEmpty ? "未配置" : aiModel)"
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
    let id = testGeneration.begin()
    operation = Task {
      let result = await TranslationProviderClient().translate(words: ["你好"], target: "EN", route: route)
      guard testGeneration.isCurrent(id) else { return }
      testing = false
      operation = nil
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
