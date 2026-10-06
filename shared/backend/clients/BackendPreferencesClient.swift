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
  func preferences(token: String) async throws -> Preferences {
    let value: Preferences = try await json("GET", "/v1/users/me/preferences", token: token)
    guard Self.validPreferences(value) else { throw Failure(status: 0) }
    return value
  }
  func preferenceSchema(token: String) async throws -> PreferenceSchema {
    let value: PreferenceSchema = try await json("GET", "/v1/users/me/preferences/schema", token: token)
    guard Self.validPreferenceSchema(value) else { throw Failure(status: 0) }
    return value
  }
  static func mergedPreferences(_ base: Preferences, replacing values: [String: BackendPreferenceValue], schema: PreferenceSchema) throws -> Preferences {
    guard validPreferences(base), validPreferenceSchema(schema) else { throw Failure(status: 0) }
    for (key, value) in values {
      guard let field = schema.fields[key], field.type == value.kind || (field.type == "number" && value.kind == "integer") else { throw Failure(status: 503) }
    }
    // Preserve every other platform's fields. A revision conflict is returned to
    // the user, never resolved by an automatic last-writer-wins retry.
    let merged = Preferences(revision: base.revision, settings: base.settings.merging(values) { _, new in new })
    guard validPreferences(merged), try JSONEncoder().encode(merged).count <= min(schema.maximum_bytes, 1024 * 1024) else { throw Failure(status: 400) }
    return merged
  }
  func putPreferences(_ preferences: Preferences, token: String) async throws -> Preferences {
    guard Self.validPreferences(preferences) else { throw Failure(status: 400) }
    let body = try JSONEncoder().encode(preferences)
    guard body.count <= 1024 * 1024 else { throw Failure(status: 400) }
    let result: Preferences = try await json("PUT", "/v1/users/me/preferences", token: token, body: body)
    guard Self.validPreferences(result) else { throw Failure(status: 0) }
    return result
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
