import Foundation

/// 只承载共享引擎返回的数据，Swift 不另存声韵母映射或实现表校验。
struct NineKeyShuangpinLayout: Decodable {
  struct Failure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
  }

  typealias Table = [String: [String: String]]

  struct Key: Decodable {
    let key: String
    let initials: [String]
    let finals: [String]
    let zero_initial: Bool

    var compactHint: String {
      let units = (zero_initial ? ["零"] : []) + initials
      return units.isEmpty ? (finals.isEmpty ? "" : "韵母") : units.joined(separator: " ")
    }

    var accessibilityLabel: String {
      var parts = [key]
      if !initials.isEmpty { parts.append("声母 " + initials.joined(separator: " ")) }
      if zero_initial { parts.append("零声母") }
      if !finals.isEmpty { parts.append("韵母 " + finals.joined(separator: " ")) }
      return parts.joined(separator: "，")
    }
  }

  struct Problem: Decodable {
    let kind: String
    let part: String
    let unit: String
    let key: String?

    var message: String {
      let place = NineKeyShuangpinLayout.partTitle(part) + "「\(unit)」"
      switch kind {
      case "unknown_unit": return place + "不是这张表认识的单位"
      case "duplicate_unit": return place + "写了两次"
      case "missing_unit": return place + "还没有放到数字键上"
      case "invalid_key": return place + "的键「\(key ?? "")」不对：" + (part == "zero_initials" ? "零声母要两位数字" : "声母和韵母各是一位数字")
      default: return "这张表不能用"
      }
    }
  }

  let effective: String
  let table: Table
  let problem: Problem?
  let keys: [Key]

  static let profiles = ["migala3", "migala2", "gushi2_low", "custom"]
  static let parts = ["initials", "finals", "zero_initials"]
  static let profileKey = "nine_key_shuangpin_profile"
  static let tableKey = "nine_key_shuangpin_custom_profile"

  static func profile(in document: [String: Any]?) -> String {
    let value = document?[profileKey] as? String ?? "migala3"
    return profiles.contains(value) ? value : "migala3"
  }

  static func current(in document: [String: Any]?) throws -> Self {
    try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(
      profile: profile(in: document), table: document?[tableKey] as? Table)
  }

  static func title(_ profile: String) -> String {
    switch profile {
    case "migala2": "米旮旯 2"
    case "gushi2_low": "辜氏 2 低重码"
    case "custom": "自定义"
    default: "米旮旯 3"
    }
  }

  static func partTitle(_ part: String) -> String {
    switch part {
    case "initials": "声母"
    case "finals": "韵母"
    case "zero_initials": "零声母音节"
    default: part
    }
  }

  /// 文件边界限制体积，结构交给共享接口检查；有可编辑的单位错误时保留原表。
  static func importTable(_ data: Data) throws -> Table {
    guard data.count <= 16_384 else { throw Failure(message: "码表文件不能超过 16 KB") }
    let table = try JSONDecoder().decode(Table.self, from: data)
    guard Set(table.keys).isSubset(of: Set(parts)) else {
      throw Failure(message: "码表只接受 initials、finals 和 zero_initials 分组")
    }
    _ = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: "custom", table: table)
    return table
  }

  static func exportTable(_ table: Table) throws -> Data {
    try JSONSerialization.data(withJSONObject: table, options: [.prettyPrinted, .sortedKeys])
  }

  /// 同一次比较并交换里保存表和选择，文档成功后才更新 App Group 镜像。
  static func save(profile: String, table: Table?, stateRoot: URL? = nil) throws -> Bool {
    guard ChineseInputScheme.nineKeyShuangpin.isOfferedByEdition else { return false }
    let layout = try MetasequoiaInputSessionBridge.nineKeyShuangpinLayout(profile: profile, table: table)
    if let problem = layout.problem { throw Failure(message: problem.message) }
    var selection: InputSchemePreference.Selection?
    guard MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: stateRoot, { document in
      document[profileKey] = profile
      if profile == "custom", let table { document[tableKey] = table }
      selection = InputSchemePreference.write({ selected in
        if !selected.enabled.contains(.nineKeyShuangpin) { selected.enabled.append(.nineKeyShuangpin) }
        selected.scheme = .nineKeyShuangpin
      }, into: &document)
    }), let selection else { return false }
    InputSchemePreference.mirror(selection)
    return true
  }
}
