import Foundation

enum BackendPreferenceValue: Codable, Equatable, Sendable {
  case boolean(Bool), integer(Int64), number(Double), string(String)
  init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer()
    if let bool = try? value.decode(Bool.self) { self = .boolean(bool) }
    else if let integer = try? value.decode(Int64.self) { self = .integer(integer) }
    else if let number = try? value.decode(Double.self) { self = .number(number) }
    else { self = .string(try value.decode(String.self)) }
  }
  func encode(to encoder: Encoder) throws {
    var value = encoder.singleValueContainer()
    switch self {
    case .boolean(let v): try value.encode(v)
    case .integer(let v): try value.encode(v)
    case .number(let v): try value.encode(v)
    case .string(let v): try value.encode(v)
    }
  }
  var kind: String {
    switch self { case .boolean: return "boolean"; case .integer: return "integer"; case .number: return "number"; case .string: return "string" }
  }
}
/// macOS 桌面设置同步在 Swift 里做的合并（`mergedPreferences`）用的几种形状：字段表和云端文档由 Rust 的 `preferences_schema`、`preferences_load` 给，上传用 `preferences_put`（#8201 迁移计划 4e）。
extension BackendAccountClient {
  struct Preferences: Codable, Sendable {
    let revision: Int64
    let settings: [String: BackendPreferenceValue]
  }
  struct PreferenceSchema: Decodable, Sendable {
    struct Field: Decodable, Sendable { let type: String }
    let fields: [String: Field]
    let maximum_bytes: Int
    let update_mode: String
    let revision_required: Bool
  }
  /// 把本机要上传的值并进云端文档。只上传服务端字段表（`/preferences/schema`）里有的键，与 Android（host-api `android_data.rs`）和 Tauri 的 iOS 工程（`ios_account.rs`）一样：客户端比服务端新、多出一个服务端还不认识的键时，那一项留在本机，其余照常上传，不让整次上传失败。字段表里有、类型却对不上的值仍然拒绝；一项都不在字段表里时也拒绝，免得把一份什么也没改的文档当成上传成功。
  static func mergedPreferences(_ base: Preferences, replacing values: [String: BackendPreferenceValue], schema: PreferenceSchema) throws -> Preferences {
    guard validPreferences(base), validPreferenceSchema(schema) else { throw Failure(status: 0) }
    var known: [String: BackendPreferenceValue] = [:]
    for (key, value) in values {
      guard let field = schema.fields[key] else { continue }
      guard field.type == value.kind || (field.type == "number" && value.kind == "integer") else { throw Failure(status: 503) }
      known[key] = value
    }
    guard !known.isEmpty else { throw Failure(status: 503) }
    // Preserve every other platform's fields. 修订号冲突时不拿旧文档重放：设置同步页把冲突交给用户，重新读取云端以后再合并。
    let merged = Preferences(revision: base.revision, settings: base.settings.merging(known) { _, new in new })
    guard validPreferences(merged), try JSONEncoder().encode(merged).count <= min(schema.maximum_bytes, 1024 * 1024) else { throw Failure(status: 400) }
    return merged
  }
  private static func validPreferences(_ value: Preferences) -> Bool {
    value.revision >= 0 && value.settings.count <= 512
      && value.settings.allSatisfy { key, setting in
        validPreferenceKey(key) && {
          if case .string(let string) = setting { return string.utf8.count <= 1024 * 1024 }
          if case .number(let number) = setting { return number.isFinite }
          return true
        }()
      }
  }

  private static func validPreferenceSchema(_ value: PreferenceSchema) -> Bool {
    value.fields.count <= 512 && (1...1024 * 1024).contains(value.maximum_bytes)
      && value.update_mode == "replace" && value.revision_required
      && value.fields.allSatisfy { key, field in
        validPreferenceKey(key) && ["boolean", "integer", "number", "string"].contains(field.type)
      }
  }

  private static func validPreferenceKey(_ value: String) -> Bool {
    !value.isEmpty && value.utf8.count <= 128
      && value.utf8.allSatisfy { byte in
        (48...57).contains(byte) || (65...90).contains(byte) || (97...122).contains(byte)
          || byte == 45 || byte == 46 || byte == 95
      }
  }
}
