import Foundation
import SwiftUI

@MainActor
struct MacSettingsAccess {
  typealias Values = [String: BackendPreferenceValue]
  var snapshot: () throws -> Values
  var validate: (Values) throws -> Void
  var apply: (Values) throws -> Void
  private static func bridge() throws -> NSObject.Type {
    guard let type = NSClassFromString("MSIMEPreferencesWindowController") as? NSObject.Type,
          type.responds(to: NSSelectorFromString("cloudSettingsSnapshot")),
          type.responds(to: NSSelectorFromString("validateCloudSettingsSnapshot:")),
          type.responds(to: NSSelectorFromString("applyCloudSettingsSnapshot:")) else { throw BackendAccountClient.Failure(status: 503) }
    return type
  }
  private static func invoke(_ selector: String, values: Values) throws {
    let object = try JSONSerialization.jsonObject(with: JSONEncoder().encode(values))
    guard let result = try bridge().perform(NSSelectorFromString(selector), with: object)?.takeUnretainedValue() as? NSNumber,
          result.boolValue else { throw BackendAccountClient.Failure(status: 400) }
  }
  static var native: Self {
    .init(snapshot: {
      guard let object = try bridge().perform(NSSelectorFromString("cloudSettingsSnapshot"))?.takeUnretainedValue() else { throw BackendAccountClient.Failure(status: 503) }
      return try JSONDecoder().decode(Values.self, from: JSONSerialization.data(withJSONObject: object))
    }, validate: { try invoke("validateCloudSettingsSnapshot:", values: $0) },
       apply: { try invoke("applyCloudSettingsSnapshot:", values: $0) })
  }
}

@MainActor
final class MacSettingsModel: ObservableObject {
  @Published var cloud: BackendAccountClient.Preferences?
  @Published var preview: MacSettingsAccess.Values?
  @Published var busy = false
  @Published var message: String?
  private var schema: BackendAccountClient.PreferenceSchema?
  private var expectedLocal: MacSettingsAccess.Values?
  private let accountID: String
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  private let local: MacSettingsAccess
  private var pending: Task<Void, Never>?
  init(accountID: String, client: BackendAccountClient = BackendAccountClient(), account: BackendAccountSession = .shared, local: MacSettingsAccess? = nil) {
    self.accountID = accountID; self.client = client; self.account = account; self.local = local ?? .native
  }
  private func authorize() async throws -> String {
    let identity = try await account.credentials(matchingUserID: accountID)
    try Task.checkCancellation()
    guard identity.userID == accountID else { throw CancellationError() }
    return identity.token
  }
  private func run(_ operation: @escaping @MainActor (String) async throws -> Void) {
    guard !busy else { return }
    busy = true; message = nil
    pending = Task {
      defer { busy = false }
      do { try await operation(try await authorize()) }
      catch is CancellationError { preview = nil; cloud = nil; schema = nil }
      catch { preview = nil; if !Task.isCancelled { message = error.localizedDescription } }
    }
  }
  func download() {
    run { token in
      self.preview = nil
      let before = try self.local.snapshot()
      let schema = try await self.client.preferenceSchema(token: token)
      let cloud = try await self.client.preferences(token: token)
      _ = try await self.authorize()
      self.cloud = cloud; self.schema = schema
      var values = cloud.settings.filter { before[$0.key] != nil }
      // Preserve device choices for fields added after the original snapshot format.
      // Missing original fields must still reject an incomplete restore.
      for key in ["platform.macos.global_theme", "platform.macos.custom_theme_base", "platform.macos.custom_candidate_skin", "platform.macos.shuangpin_preedit_uses_raw"] where values[key] == nil {
        values[key] = before[key]
      }
      guard values.count == before.count else {
        self.message = "云端还没有完整的 macOS 设置，可以先上传本机设置。"; return
      }
      try self.local.validate(values)
      self.preview = values; self.expectedLocal = before
    }
  }
  /// The global theme travels with the custom theme's base and package, so the three are uploaded together or not at all: a cloud copy with a theme but without its base or package would restore a different look elsewhere.
  static let themeKeys = ["platform.macos.global_theme", "platform.macos.custom_theme_base", "platform.macos.custom_candidate_skin"]
  func upload() {
    guard let cloud, let schema else { return }
    run { token in
      var values = try self.local.snapshot()
      try self.local.validate(values)
      // A server that has not registered the theme fields yet rejects the whole upload over them, so the other settings are uploaded without the theme and the message says so.
      let themeless = Self.themeKeys.contains { schema.fields[$0] == nil }
      if themeless { for key in Self.themeKeys { values[key] = nil } }
      let merged = try BackendAccountClient.mergedPreferences(cloud, replacing: values, schema: schema)
      let saved = try await self.client.putPreferences(merged, token: token)
      _ = try await self.authorize()
      self.cloud = saved; self.preview = nil; self.expectedLocal = nil
      self.message = themeless ? "本机设置已上传，其他平台的云端设置已保留。云端暂不支持主题设置，主题没有上传。" : "本机设置已上传，其他平台的云端设置已保留。"
    }
  }
  func apply() {
    guard let preview, let expectedLocal, let cloud else { return }
    run { token in
      let current = try await self.client.preferences(token: token)
      _ = try await self.authorize()
      guard current.revision == cloud.revision, try self.local.snapshot() == expectedLocal else { throw BackendAccountClient.Failure(status: 409) }
      try self.local.apply(preview)
      self.preview = nil; self.expectedLocal = nil
      self.message = "云端设置已应用到本机。"
    }
  }
  func close() { pending?.cancel(); preview = nil; cloud = nil; schema = nil; expectedLocal = nil }
}

struct MacCloudSettingsView: View {
  @StateObject private var model: MacSettingsModel
  @Environment(\.dismiss) private var dismiss
  @State private var action: Action?
  private enum Action { case upload, apply }
  init(accountID: String) { _model = StateObject(wrappedValue: MacSettingsModel(accountID: accountID)) }
  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      HStack { Text("桌面设置同步").font(.title2); Spacer(); Button("关闭") { model.close(); dismiss() } }
      Text("同步输入方案、辅助码、候选窗口、学习和标点等 macOS 设置。上传与替换均需主动确认，不包含账号凭据或本机文件路径。")
      Button("下载云端设置并预览") { model.download() }.disabled(model.busy)
      if model.cloud != nil { Button("上传本机设置") { action = .upload }.disabled(model.busy) }
      if let preview = model.preview {
        Text("已校验 \(preview.count) 项桌面设置。替换后将通过现有设置流程通知输入会话。")
        ScrollView {
          VStack(alignment: .leading) {
            ForEach(preview.keys.sorted(), id: \.self) { key in
              Text("\(label(key))：\(display(preview[key]!, key: key))").font(.callout)
            }
          }.frame(maxWidth: .infinity, alignment: .leading)
        }
        Button("用云端设置替换本机", role: .destructive) { action = .apply }.disabled(model.busy)
      }
      if model.busy { ProgressView() }
      if let message = model.message { Text(message).foregroundStyle(.secondary) }
      Spacer(minLength: 0)
    }.padding(20).frame(width: 540, height: 520)
    .onAppear { model.download() }.onDisappear { model.close() }
    .alert(action == .upload ? "上传本机设置？" : "替换本机设置？", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } })) {
      Button("取消", role: .cancel) { action = nil }
      Button("确认", role: action == .apply ? .destructive : nil) { if action == .upload { model.upload() } else { model.apply() }; action = nil }
    } message: { Text(action == .upload ? "更新云端 macOS 设置，并保留其他平台的设置。" : "应用这份已预览的设置。本机或云端设置发生变化时会拒绝本次替换，请重新下载确认。") }
  }
  private func label(_ key: String) -> String {
    if key == "platform.macos.shuangpin_preedit_uses_raw" { return "双拼预编辑" }
    let names = ["global_theme":"全局主题", "custom_theme_base":"自定义主题底色", "custom_candidate_skin":"自定义候选皮肤", "input_scheme":"输入方案", "quanpin_helpcode_schema":"全拼辅助码", "shuangpin_helpcode_schema":"双拼辅助码", "candidate_panel_style":"候选布局", "candidate_page_size":"每页候选数", "candidate_font_size":"候选字号", "candidate_page_shortcut":"翻页快捷键", "autocorrect":"拼音纠错", "helpcode":"辅助码", "chinese_punctuation":"中文标点", "smart_punctuation":"智能标点", "smart_punctuation_repeat":"重复标点转中文", "candidate_learning":"候选学习", "english_input_mode":"英文模式", "input_mode_shortcut":"中英切换快捷键", "full_width_input":"全角输入", "floating_toolbar":"悬浮工具栏", "traditional_chinese_output":"繁体输出", "wubi_auto_commit_unique":"五笔唯一候选自动上屏", "shuangpin_keymap":"双拼键位图", "local_input_modes":"本地扩展模式"]
    return names[String(key.dropFirst("platform.macos.".count))] ?? "桌面设置"
  }
  private func display(_ value: BackendPreferenceValue, key: String) -> String {
    if key == "platform.macos.shuangpin_preedit_uses_raw", case .boolean(let raw) = value {
      return raw ? "原始双拼显示" : "全拼显示"
    }
    if key == "platform.macos.global_theme" || key == "platform.macos.custom_theme_base", case .string(let id) = value {
      // The titles live in the host catalog; the settings window answers with the id itself when it cannot be asked.
      guard let type = NSClassFromString("MSIMEPreferencesWindowController") as? NSObject.Type,
            type.responds(to: NSSelectorFromString("themeTitleForIdentifier:")),
            let title = type.perform(NSSelectorFromString("themeTitleForIdentifier:"), with: id)?.takeUnretainedValue() as? String else { return id }
      return title
    }
    if key == "platform.macos.custom_candidate_skin", case .string(let id) = value {
      return id.isEmpty ? "不使用外部皮肤" : id
    }
    if case .integer(let n) = value {
      let names: [String]?
      switch key {
      case "platform.macos.input_scheme": names = ["全拼", "双拼", "五笔"]
      case "platform.macos.quanpin_helpcode_schema", "platform.macos.shuangpin_helpcode_schema": names = ["蓝天小雨点", "自然码", "首右2.0", "首右plus", "小鹤", "加加"]
      case "platform.macos.candidate_panel_style": names = ["横排", "竖排"]
      // The keys themselves, the way the settings window and the reference's shortcut page write
      // them. This page and that window show one setting, and a reader comparing them should not
      // have to work out that 「方括号」 and 「[ / ]」 are the same choice.
      case "platform.macos.candidate_page_shortcut": names = ["- / =", "[ / ]", "Page Up / Page Down"]
      default: names = nil
      }
      if let names, n >= 0, n < names.count { return names[Int(n)] }
    }
    switch value { case .boolean(let enabled): return enabled ? "开启" : "关闭"; case .integer(let n): return String(n); case .number(let n): return String(n); case .string(let s): return s }
  }
}
