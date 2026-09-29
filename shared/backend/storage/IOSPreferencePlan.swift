import Foundation

/// Validated values only; the platform applies this plan through its existing setters.
struct IOSPreferencePlan {
  let scheme: String?
  let traditional: Bool?
  let sound: Bool?
  let haptics: Bool?
  let learning: Bool?
  let strength: String?
  /// `global_theme`, one of `themes`.
  let globalTheme: String?
  /// `custom_theme.base`: `system` or a built-in theme, never `custom`.
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
    if let value = try string("input.schema") {
      switch value {
      case "quanpin": scheme = nineKey == true ? "nineKey" : "quanpin"
      case "shuangpin":
        let profile = try string("input.shuangpin_schema") ?? "xiaohe"
        guard ["xiaohe", "ziranma", "microsoft", "shoudao"].contains(profile) else { throw BackendAccountClient.Failure(status: 400) }
        scheme = profile == "xiaohe" ? "shuangpin" : profile
      case "wubi":
        guard try string("input.wubi_schema") ?? "wubi86" == "wubi86" else { throw BackendAccountClient.Failure(status: 400) }
        scheme = "wubi"
      case "japanese":
        guard try string("input.japanese_schema") ?? "romaji" == "romaji" else { throw BackendAccountClient.Failure(status: 400) }
        scheme = nineKey == true ? "japaneseNineKey" : "japanese"
      default: throw BackendAccountClient.Failure(status: 400)
      }
    } else { scheme = nil }
    if let charset = try string("input.character_set") {
      guard ["simplified", "traditional"].contains(charset) else { throw BackendAccountClient.Failure(status: 400) }
      traditional = charset == "traditional"
    } else { traditional = nil }
    sound = try bool("platform.ios.sound_enabled")
    haptics = try bool("platform.ios.haptics_enabled")
    learning = try bool("platform.ios.dictionary_learning")
    strength = try string("platform.ios.haptic_strength")
    guard strength == nil || ["light", "medium", "strong"].contains(strength!) else { throw BackendAccountClient.Failure(status: 400) }
    globalTheme = try string("platform.ios.global_theme")
    guard globalTheme == nil || themes.contains(globalTheme!) else { throw BackendAccountClient.Failure(status: 400) }
    customThemeBase = try string("platform.ios.custom_theme_base")
    guard customThemeBase == nil || (themes.contains(customThemeBase!) && customThemeBase != "custom") else { throw BackendAccountClient.Failure(status: 400) }
    customSkinJSON = try string("platform.ios.custom_keyboard_skin")
  }
}
