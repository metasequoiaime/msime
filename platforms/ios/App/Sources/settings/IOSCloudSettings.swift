import UIKit

/// 云同步和「设置同步」页交给 Rust 的本机键盘设置（`ios_native`，形状是 client-core 的 `IosKeyboardPreferences`，键名 camelCase），以及 Rust 要写回本机时宿主做的那一半（#8201 迁移计划 4d）。单独放一个文件，键盘测试 target 才能直接验证它。
///
/// 映射本身在 Rust（client-services 的 `account::ios_settings`）：上传哪些键、版本过滤、字段表过滤、合并和三方比对都在那里。这里只按原生 iOS 的存法读出当前值：方案、简繁、学习开关、全局主题和自定义键盘设计以共享文档为准（App Group 里的只是镜像，可能落后于文档），按键反馈只存在 App Group 里。写回时 Rust 先给出要写进 App Group 的值、宿主写好以后再由 Rust 把文档部分写进共享文档，失败时宿主把 App Group 恢复成旧值（`write` 和 `restore`）。
enum IOSCloudSettings {
  /// 当前的键盘设置。`document` 为 nil 时读共享文档。
  static func native(document: [String: Any]? = MetasequoiaInputSessionBridge.loadSharedPreferences()) throws -> [String: Any] {
    var value: [String: Any] = [
      "inputScheme": InputSchemePreference.current(in: document).scheme.rawValue,
      "traditionalChineseOutput": ChineseOutputPreference.current(in: document),
      "soundEnabled": KeyboardFeedbackPreference.soundEnabled,
      "hapticsEnabled": KeyboardFeedbackPreference.hapticsEnabled,
      "hapticStrength": KeyboardFeedbackPreference.hapticStrength.rawValue,
      "englishSuggestions": EnglishSuggestionsPreference.isEnabled,
      "candidatePaletteFollowsDesktop": CandidatePalette.followsDesktop,
      // 原生键盘没有行内预编辑。
      "inlinePreedit": false,
      "hapticsAvailable": KeyboardFeedbackPreference.hapticsAvailable,
      "dictionaryLearning": InputHabitPreference.settings(in: document).learning,
      "globalTheme": document.map(GlobalThemePreference.theme(in:)) ?? GlobalThemePreference.selected,
    ]
    // 文档里没有自定义键盘设计时（自定义主题画底主题的键盘）不上传，云端保留原来的设计。
    if let design = document == nil ? CustomKeyboardSkinStore.stored : GlobalThemePreference.design(in: document) {
      value["customKeyboardSkin"] = String(decoding: try JSONEncoder().encode(design), as: UTF8.self)
    } else {
      value["customKeyboardSkin"] = NSNull()
    }
    // 数字行与 Tab 键、横屏分离式键盘只有 iPad 全宽键盘才有，与 Tauri 插件一样只在 iPad 上报告。
    if UIDevice.current.userInterfaceIdiom == .pad {
      let defaults = KeyboardLayoutPreference.defaults
      value["tabletFullKeys"] = defaults.object(forKey: KeyboardLayoutPreference.tabletFullKeysKey) as? Bool ?? true
      value["tabletSplitKeyboard"] = defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey) as? Bool ?? false
    }
    return value
  }

  /// App Group 里只有那一份、要先写好的按键反馈三项；`restore` 用它回滚。
  struct Feedback: Equatable, Sendable {
    let sound: Bool
    let haptics: Bool
    let strength: String

    static var current: Feedback {
      Feedback(sound: KeyboardFeedbackPreference.soundEnabled, haptics: KeyboardFeedbackPreference.hapticsEnabled,
               strength: KeyboardFeedbackPreference.hapticStrength.rawValue)
    }
  }

  /// 把 Rust 要写回本机的键盘设置（`host_writes.ios_native` 或 `plan` 的 `ios_native`）写进 App Group，返回写之前的按键反馈和实际保存下来的键盘设置（交给 Rust 的 `saved_native`）。只写按键反馈三项：方案、简繁、学习、主题和自定义键盘由 Rust 写进共享文档，之后 `mirrorDocument` 再抄回 App Group。这是同步自己写本机，不记改动。
  static func write(_ requested: [String: Any]) throws -> (previous: Feedback, saved: [String: Any]) {
    guard let sound = requested["soundEnabled"] as? Bool, let haptics = requested["hapticsEnabled"] as? Bool,
          let strength = requested["hapticStrength"] as? String, KeyboardHapticStrength(rawValue: strength) != nil else {
      throw BackendRustClient.Failure.invalidResponse
    }
    let previous = Feedback.current
    write(Feedback(sound: sound, haptics: haptics, strength: strength))
    var saved = requested
    saved["soundEnabled"] = KeyboardFeedbackPreference.soundEnabled
    saved["hapticsEnabled"] = KeyboardFeedbackPreference.hapticsEnabled
    saved["hapticStrength"] = KeyboardFeedbackPreference.hapticStrength.rawValue
    return (previous, saved)
  }

  /// Rust 写共享文档失败时把按键反馈恢复成写之前的值。
  static func restore(_ previous: Feedback) { write(previous) }

  private static func write(_ feedback: Feedback) {
    CloudSyncState.suppressingMarks {
      let defaults = KeyboardFeedbackPreference.defaults
      defaults.set(feedback.sound, forKey: KeyboardFeedbackPreference.soundKey)
      defaults.set(feedback.haptics, forKey: KeyboardFeedbackPreference.hapticsKey)
      defaults.set(feedback.strength, forKey: KeyboardFeedbackPreference.strengthKey)
    }
  }

  /// Rust 写好共享文档以后，把文档里的方案、五笔版本、简繁、主题和自定义键盘设计抄回 App Group，键盘不等下一次重载就按新值画。抄镜像不是用户的改动，不记改动。
  static func mirrorDocument() {
    guard let document = MetasequoiaInputSessionBridge.loadSharedPreferences() else { return }
    CloudSyncState.suppressingMarks {
      InputSchemePreference.mirror(InputSchemePreference.current(in: document))
      WubiProfilePreference.mirror(document)
      ChineseOutputPreference.usesTraditional = ChineseOutputPreference.current(in: document)
      GlobalThemePreference.mirror(document)
    }
  }
}

/// 「设置同步」页的上传和应用，与 `IOSCloudSettings` 放在一起，好在 KeyboardTests 里对着替身函数表测。
enum SettingsSyncApply {
  /// 本机设置合并进云端文档后整份上传：导出（`ios_settings_export`，按版本和字段表过滤、合并进 `cloud`）在 Rust，上传带合并后的修订号和读文档时的账号。返回上传后的云端文档。
  static func upload(cloud: [String: Any], schema: [String: Any], userID: String, account: BackendRustAccount) async throws -> [String: Any] {
    let exported = try BackendRustAccount.object(try await account.client.call(.accountIOSSettingsExport, [
      "preferences_directory": MetasequoiaInputSessionBridge.sharedStateDirectory, "ios_native": try IOSCloudSettings.native(),
      "schema": schema, "cloud": cloud]))
    guard let merged = exported["merged"] as? [String: Any], let settings = merged["settings"] as? [String: Any],
          let revision = merged["revision"] as? NSNumber else { throw BackendRustClient.Failure.invalidResponse }
    return try BackendRustAccount.object(try await account.account([
      "operation": "preferences_put", "document": settings, "revision": revision, "user_id": userID]))
  }

  /// 把云端文档应用到本机：Rust 先算出要写进 App Group 的键盘设置（plan），这里写好以后 Rust 再写共享文档（commit），commit 失败时把 App Group 恢复成写之前的值。这是同步自己写本机，不记改动。
  static func apply(_ cloud: [String: Any], client: BackendRustClient) async throws {
    let directory = MetasequoiaInputSessionBridge.sharedStateDirectory
    let plan = try BackendRustAccount.object(try await client.call(.accountIOSSettingsApply, [
      "preferences_directory": directory, "cloud": cloud, "step": "plan", "ios_native": try IOSCloudSettings.native()]))
    guard let requested = plan["ios_native"] as? [String: Any] else { throw BackendRustClient.Failure.invalidResponse }
    let written = try IOSCloudSettings.write(requested)
    do {
      _ = try await client.call(.accountIOSSettingsApply, [
        "preferences_directory": directory, "cloud": cloud, "step": "commit", "saved_native": written.saved])
    } catch {
      IOSCloudSettings.restore(written.previous)
      throw error
    }
    IOSCloudSettings.mirrorDocument()
  }
}
