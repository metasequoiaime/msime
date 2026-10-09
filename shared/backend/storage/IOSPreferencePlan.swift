import Foundation

/// Validated values only; the platform applies this plan through its existing setters.
struct IOSPreferencePlan {
  let scheme: String?
  /// `input.wubi_schema`：`wubi86` 或 `wubi98`，只在 `input.schema` 是 `wubi` 时读取。云端没有这一项时为 nil：那份设置来自还不认识 98 五笔的设备，本机的五笔版本保持不变。
  let wubiProfile: String?
  let traditional: Bool?
  let sound: Bool?
  let haptics: Bool?
  let learning: Bool?
  let strength: String?
  /// `global_theme`, one of `themes`.
  let globalTheme: String?
  /// `custom_theme.base`：`system` 或内置主题，不能是 `custom`，也不能是只在 iOS 提供的 `native`（client-core `GlobalTheme::is_base`）。
  let customThemeBase: String?
  /// `custom_theme.keyboard`, the keyboard design as JSON; absent when the uploading device had none, which leaves the local design alone.
  let customSkinJSON: String?

  /// `themes` is the global theme catalog of the client-core the app links (`msime_client_theme_catalog`); the plan does not keep its own copy of the ids.
  init(_ values: [String: BackendPreferenceValue], themes: Set<String>) throws {
    func string(_ key: String) throws -> String? {
      guard let value = values[key] else { return nil }
      guard case .string(let text) = value else { throw BackendAccountClient.Failure(status: 400) }
      return text
    }
    func bool(_ key: String) throws -> Bool? {
      guard let value = values[key] else { return nil }
      guard case .boolean(let value) = value else { throw BackendAccountClient.Failure(status: 400) }
      return value
    }
    let nineKey = try bool("platform.ios.nine_key")
    var wubiProfile: String?
    if let value = try string("input.schema") {
      switch value {
      case "quanpin": scheme = nineKey == true ? "nineKey" : "quanpin"
      case "shuangpin":
        let profile = try string("input.shuangpin_schema") ?? "xiaohe"
        guard ["xiaohe", "ziranma", "microsoft", "shoudao"].contains(profile) else { throw BackendAccountClient.Failure(status: 400) }
        scheme = profile == "xiaohe" ? "shuangpin" : profile
      case "wubi":
        wubiProfile = try string("input.wubi_schema")
        guard wubiProfile == nil || ["wubi86", "wubi98"].contains(wubiProfile!) else { throw BackendAccountClient.Failure(status: 400) }
        scheme = "wubi"
      case "japanese":
        guard try string("input.japanese_schema") ?? "romaji" == "romaji" else { throw BackendAccountClient.Failure(status: 400) }
        scheme = nineKey == true ? "japaneseNineKey" : "japanese"
      case "korean": scheme = "korean"
      default: throw BackendAccountClient.Failure(status: 400)
      }
    } else { scheme = nil }
    self.wubiProfile = wubiProfile
    if let charset = try string("input.character_set") {
      guard ["simplified", "traditional"].contains(charset) else { throw BackendAccountClient.Failure(status: 400) }
      traditional = charset == "traditional"
    } else { traditional = nil }
    sound = try bool("platform.ios.sound_enabled")
    haptics = try bool("platform.ios.haptics_enabled")
    learning = try bool("platform.ios.dictionary_learning")
    strength = try string("platform.ios.haptic_strength")
    // `system` 是「跟随系统」。这一版只接受、不上传它（`IOSCloudSettings`）：更早的版本不认识它，会因此拒绝整份文档。
    guard strength == nil || ["light", "medium", "strong", "system"].contains(strength!) else { throw BackendAccountClient.Failure(status: 400) }
    globalTheme = try string("platform.ios.global_theme")
    guard globalTheme == nil || themes.contains(globalTheme!) else { throw BackendAccountClient.Failure(status: 400) }
    customThemeBase = try string("platform.ios.custom_theme_base")
    // `themes` 是整份目录，含不能当底的 `custom` 和 `native`；这里先拒掉，免得 apply 先写了别的键，再在共享文档校验 `custom_theme.base` 时失败，留下只应用了一半的设置。
    guard customThemeBase == nil || (themes.contains(customThemeBase!) && !["custom", "native"].contains(customThemeBase!)) else {
      throw BackendAccountClient.Failure(status: 400)
    }
    customSkinJSON = try string("platform.ios.custom_keyboard_skin")
  }
}

/// 按产品版本过滤账号设置，规则与 client-core 的 `filter_uploaded_account_settings` 和 `filter_downloaded_account_settings`（crates/client-core/src/edition.rs）相同，Tauri 公共组件的 iOS 工程走的就是那两个函数。`offered` 是本版本提供的方案（版本表里的方案名，见 `MSIMEAppEdition.inputSchemes`），nil 表示 full：提供全部方案，什么也不去掉。
extension IOSPreferencePlan {
  /// 账号设置里 `input.schema` 认得的取值，即 client-core `InputScheme` 的全部方案。不在这里的取值留给 `init` 按原来的规则处理。
  static let accountSchemes: Set<String> = ["quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"]

  /// 上传前过滤本机整理出的账号设置：
  /// - 只有一个方案的版本不上传 `input.schema` 和随它的 iOS 九键开关、Android 触屏布局，否则会把 full 等其他版本记在账号里的方案盖掉；
  /// - 有多个方案的版本只上传本版本提供的方案；
  /// - 不提供双拼、五笔的版本不上传双拼方案、五笔版本这两项。
  static func filterUploaded(_ settings: inout [String: BackendPreferenceValue], offered: [String]?) {
    dropUnsyncedScheme(&settings, offered: offered)
    guard let offered else { return }
    if !offered.contains("shuangpin") { settings.removeValue(forKey: "input.shuangpin_schema") }
    if !offered.contains("wubi") { settings.removeValue(forKey: "input.wubi_schema") }
  }

  /// 应用前过滤从账号下载的设置：只有一个方案的版本忽略 `input.schema` 和随它的九键开关、触屏布局；有多个方案的版本把本版本不提供的方案当作账号里没有这一项，本机方案保持不变，其余设置照常应用。
  static func filterDownloaded(_ settings: inout [String: BackendPreferenceValue], offered: [String]?) {
    dropUnsyncedScheme(&settings, offered: offered)
  }

  private static func dropUnsyncedScheme(_ settings: inout [String: BackendPreferenceValue], offered: [String]?) {
    guard let offered else { return }
    var keeps = offered.count > 1
    if keeps, case .string(let scheme)? = settings["input.schema"], accountSchemes.contains(scheme) {
      keeps = offered.contains(scheme)
    }
    guard !keeps else { return }
    for key in ["input.schema", "platform.ios.nine_key", "platform.android.keyboard_layout"] {
      settings.removeValue(forKey: key)
    }
  }
}
