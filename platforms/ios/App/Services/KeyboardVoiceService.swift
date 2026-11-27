import Foundation
import Security

/// App 发布给键盘的语音识别服务，键盘自己录音时（`KeyboardDirectDictation`）把整段录音发到这里，不用打开 App。只有说完整段上传的联网服务会发布，哪些算由 `KeyboardVoicePublisher` 决定；字段都是 App 换算好的，键盘照着发请求，不再读 App 的设置。
struct KeyboardVoiceConfiguration: Codable, Equatable, Sendable {
  var endpoint: String
  var model: String
  /// 百炼的 chat_audio 请求（录音作为 Base64 放进 Chat Completions），否则是 multipart 转写接口。
  var chatAudio: Bool
  /// 转写请求的 `language` 字段，按服务规则换算过（`VoicePolishSettings.transcriptionLanguage`）；nil 时不带。
  var language: String?
  /// 开始和结束录音的提示音，已经并上「录音提示音」总开关。
  var startSound: Bool
  var endSound: Bool
}

enum KeyboardVoiceService {
  static let group = MSIMEAppEdition.appGroupIdentifier
  private static let preference = "keyboard.voice.configuration"
  private static var defaults: UserDefaults? { UserDefaults(suiteName: group) }

  /// 同一时间只发布一个语音服务，钥匙串里只有一条，不按来源区分。
  private static func query() -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccessGroup as String: group,
      kSecAttrService as String: "app.msime.ios.keyboard-voice"
    ]
  }

  static func configuration() -> KeyboardVoiceConfiguration? {
    guard let data = defaults?.data(forKey: preference) else { return nil }
    return try? JSONDecoder().decode(KeyboardVoiceConfiguration.self, from: data)
  }

  /// 密钥为空时删掉共享的那条：不需要密钥的服务照样发布，请求不带 `Authorization`，与 App 一样。
  static func publish(_ configuration: KeyboardVoiceConfiguration, token: String) throws {
    let data = try JSONEncoder().encode(configuration)
    var status: OSStatus
    if token.isEmpty {
      status = SecItemDelete(query() as CFDictionary)
      if status == errSecItemNotFound { status = errSecSuccess }
    } else {
      let attributes = [kSecValueData as String: Data(token.utf8)]
      status = SecItemUpdate(query() as CFDictionary, attributes as CFDictionary)
      if status == errSecItemNotFound {
        var item = query().merging(attributes) { _, new in new }
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        status = SecItemAdd(item as CFDictionary, nil)
      }
    }
    guard status == errSecSuccess else { throw ServiceFailure(message: "无法向键盘共享语音密钥，请解锁设备后重试。") }
    defaults?.set(data, forKey: preference)
  }

  /// 没有共享密钥时是空串。
  static func token() throws -> String {
    var lookup = query()
    lookup[kSecReturnData as String] = true
    var result: CFTypeRef?
    let status = SecItemCopyMatching(lookup as CFDictionary, &result)
    if status == errSecItemNotFound { return "" }
    guard status == errSecSuccess, let data = result as? Data, let token = String(data: data, encoding: .utf8)
    else { throw ServiceFailure(message: "无法读取键盘的语音密钥，请解锁设备后重试。") }
    return token
  }

  /// 撤下已发布的服务，键盘回到经后台会话交给 App 识别。
  static func withdraw() {
    defaults?.removeObject(forKey: preference)
    SecItemDelete(query() as CFDictionary)
  }
}
