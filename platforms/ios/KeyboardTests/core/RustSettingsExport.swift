// 真的 Rust 设置导出（`msime_client_account_ios_settings_export`）：测试拿 `IOSCloudSettings.native()` 交出的本机键盘设置和共享文档，看 Rust 实际会上传哪些键。KeyboardTests 链着 Rust 宿主库，直接用链进来的函数表。
import CMSIMEClient
import Foundation
import XCTest

enum RustSettingsExport {
  /// 字段表收录这些键（都按字符串或布尔），其余的键 Rust 不导出。
  static let fields: [String: String] = [
    "input.schema": "string", "input.shuangpin_schema": "string", "input.wubi_schema": "string",
    "input.character_set": "string", "platform.ios.keyboard_layout": "string", "platform.ios.nine_key": "boolean",
    "platform.ios.sound_enabled": "boolean", "platform.ios.haptics_enabled": "boolean",
    "platform.ios.haptic_strength": "string", "platform.ios.dictionary_learning": "boolean",
    "platform.ios.global_theme": "string", "platform.ios.custom_theme_base": "string",
    "platform.ios.custom_keyboard_skin": "string",
  ]

  static var schema: [String: Any] {
    ["fields": fields.mapValues { ["type": $0] }, "maximum_bytes": 1_048_576, "update_mode": "replace", "revision_required": true]
  }

  /// Rust 按本机设置导出的账号键值。
  static func settings() throws -> [String: Any] {
    let client = try XCTUnwrap(BackendRustClient(table: msime_rust_client_linked()))
    let request = try BackendRustClient.request([
      "preferences_directory": MetasequoiaInputSessionBridge.sharedStateDirectory,
      "ios_native": try IOSCloudSettings.native(), "schema": schema])
    let value = try client.callBlocking(.accountIOSSettingsExport, request).get()
    return try XCTUnwrap(BackendRustAccount.object(value)["settings"] as? [String: Any])
  }
}
