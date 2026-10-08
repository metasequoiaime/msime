import Foundation
import Security

enum KeyboardAIService {
  static let group = MSIMEAppEdition.appGroupIdentifier
  private static let preference = "keyboard.ai.configuration"
  private static var defaults: UserDefaults? { UserDefaults(suiteName: group) }

  static func query(url: URL? = nil) -> [String: Any] {
    var result: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrAccessGroup as String: group,
      kSecAttrService as String: "app.msime.ios.keyboard-ai"
    ]
    if let url {
      // 来源键与设置页、Tauri 插件一致：`scheme://host:port`，局域网的 http 接口按 `http://host:port` 存。
      result[kSecAttrAccount as String] = AIEndpointPolicy.origin(of: url) ?? ""
    }
    return result
  }

  static func configuration() -> CustomServiceConfiguration? {
    guard let data = defaults?.data(forKey: preference) else { return nil }
    return try? JSONDecoder().decode(CustomServiceConfiguration.self, from: data)
  }

  static func publish(_ configuration: CustomServiceConfiguration, token: String) throws {
    let url = try configuration.validatedURL(allowsLocalHTTP: true)
    let data = try JSONEncoder().encode(configuration)
    let lookup = query(url: url)
    let attributes = [kSecValueData as String: Data(token.utf8)]
    var status = SecItemUpdate(lookup as CFDictionary, attributes as CFDictionary)
    if status == errSecItemNotFound {
      var item = lookup.merging(attributes) { _, new in new }
      item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
      status = SecItemAdd(item as CFDictionary, nil)
    }
    guard status == errSecSuccess else { throw ServiceFailure(message: "无法向键盘共享密钥，请解锁设备后重试。") }
    defaults?.set(data, forKey: preference)
  }

  static func token(for configuration: CustomServiceConfiguration) throws -> String {
    var lookup = query(url: try configuration.validatedURL(allowsLocalHTTP: true))
    lookup[kSecReturnData as String] = true
    var result: CFTypeRef?
    let status = SecItemCopyMatching(lookup as CFDictionary, &result)
    guard status == errSecSuccess, let data = result as? Data, let token = String(data: data, encoding: .utf8)
    else { throw ServiceFailure(message: "无法读取键盘密钥，请在水杉 App 中重新保存 AI 配置。") }
    return token
  }

  static func disable() throws {
    defaults?.removeObject(forKey: preference)
    let status = SecItemDelete(query() as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw ServiceFailure(message: "AI 已停用，但共享密钥未能删除，请解锁设备后重试。")
    }
  }
}
