import Foundation

/// 「设置同步」页上传和应用的那组原生设置。单独放一个文件，键盘测试 target 才能直接验证上传的内容。
enum IOSCloudSettings {
  static func snapshot() throws -> [String: BackendPreferenceValue] {
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences()
    // 上传的是文档里的方案，不是 App Group 镜像：镜像可能落后于文档，上传旧方案会让其他设备、以及之后在这台设备上「下载并应用」，都换回用户已经不用的方案。
    let scheme = InputSchemePreference.current(in: document).scheme
    var settings: [String: BackendPreferenceValue] = [
      // 简繁与方案同理取文档里的值，不取可能落后的 App Group 镜像。
      "input.character_set": .string(ChineseOutputPreference.current(in: document) ? "traditional" : "simplified"),
      "platform.ios.sound_enabled": .boolean(KeyboardFeedbackPreference.soundEnabled),
      "platform.ios.haptics_enabled": .boolean(KeyboardFeedbackPreference.hapticsEnabled),
      "platform.ios.dictionary_learning": .boolean(
        InputHabitPreference.settings(in: document).learning),
      "platform.ios.custom_theme_base": .string(GlobalThemePreference.base(in: document))
    ]
    // 「原生」先不上传，云端保留原来的主题：加入它之前的版本只认七个主题 id，同一账号下还没升级的设备收到它会拒绝整份文档。
    let theme = document.map(GlobalThemePreference.theme(in:)) ?? GlobalThemePreference.selected
    if theme != GlobalThemeCatalog.nativeId { settings["platform.ios.global_theme"] = .string(theme) }
    // 「跟随系统」先不上传，云端保留原来的强度：这一版的 `IOSPreferencePlan` 已经接受 `system`，但更早的版本只认 light、medium、strong，同一账号下还没升级的设备收到它会拒绝整份文档。
    let strength = KeyboardFeedbackPreference.hapticStrength
    if strength != .system { settings["platform.ios.haptic_strength"] = .string(strength.rawValue) }
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
    // 单方案版本不上传方案，多方案版本只上传本版本提供的方案；full 什么也不去掉。
    IOSPreferencePlan.filterUploaded(&settings, offered: MSIMEAppEdition.inputSchemes)
    return settings
  }
  static func apply(_ cloud: [String: BackendPreferenceValue]) throws {
    // 单方案版本不应用账号里的方案，多方案版本把本版本没有的方案当作缺失；full 什么也不去掉。
    var values = cloud
    IOSPreferencePlan.filterDownloaded(&values, offered: MSIMEAppEdition.inputSchemes)
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
    // 方案在写文档的闭包里按文档当时的启用列表落下，不拿 App Group 镜像里可能过时的列表覆盖文档。
    var writtenScheme: InputSchemePreference.Selection?
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
      if let scheme { writtenScheme = InputSchemePreference.write({ $0.scheme = scheme }, into: &document) }
      if let profile = plan.wubiProfile { document[WubiProfilePreference.documentKey] = profile }
      if let traditional = plan.traditional { document[ChineseOutputPreference.documentKey] = traditional }
    }
    guard written else { throw CocoaError(.fileWriteUnknown) }
    if let writtenScheme { InputSchemePreference.mirror(writtenScheme) }
    if let profile = plan.wubiProfile { WubiProfilePreference.profile = profile }
    if let traditional = plan.traditional { ChineseOutputPreference.usesTraditional = traditional }
    let defaults = KeyboardFeedbackPreference.defaults
    if let sound = plan.sound { defaults.set(sound, forKey: KeyboardFeedbackPreference.soundKey) }
    if let haptics = plan.haptics { defaults.set(haptics, forKey: KeyboardFeedbackPreference.hapticsKey) }
    if let strength = plan.strength { defaults.set(strength, forKey: KeyboardFeedbackPreference.strengthKey) }
    if let document = MetasequoiaInputSessionBridge.loadSharedPreferences() { GlobalThemePreference.mirror(document) }
  }
}
