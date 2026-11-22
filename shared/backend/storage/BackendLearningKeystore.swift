import Foundation
import Security

/// 学习同步的独立存储名。Swift 旧会话和 Rust 注入存储共用位置，不与剪贴板混用。
enum BackendLearningKeystore {
  static let name = "learning-keystore"

  /// 旧 Swift 退出路径匹配当前进程持有的所有访问组，与会话清理的口径一致。
  static func clearKeychain(service: String,
                            delete: ([String: Any]) -> OSStatus = { SecItemDelete($0 as CFDictionary) }) throws {
    let status = delete([kSecClass as String: kSecClassGenericPassword,
                         kSecAttrService as String: service, kSecAttrAccount as String: name])
    guard status == errSecSuccess || status == errSecItemNotFound else { throw BackendAccountClient.Failure(status: 0) }
  }
}
