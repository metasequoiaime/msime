import Foundation

/// 普通 26 键双拼自定义表的共享设置模型；校验和键位提示来自 host-api。
struct ShuangpinCustomLayout: Decodable {
  struct Failure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
  }

  typealias Table = [String: [String: String]]

  struct Key: Decodable {
    let key: String
    let hint: String
  }

  struct Problem: Decodable {
    let kind: String
    let part: String?
    let unit: String?
    let key: String?
    let code: String?
    let first: String?
    let second: String?

    var message: String {
      let place = Self.partTitle(part) + (unit.map { "「\($0)」" } ?? "")
      switch kind {
      case "unknown_unit": return place + "不是这张表认识的单位"
      case "duplicate_unit": return place + "写了两次"
      case "missing_unit": return place + "还没有放到字母键上"
      case "invalid_key": return place + "的键「\(key ?? "")」不符合规则"
      case "initial_key_taken": return place + "占用了单字母声母的键「\(key ?? "")」"
      case "ambiguous_code": return "编码「\(code ?? "")」对应了多个音节"
      default: return "这张表不能用"
      }
    }

    private static func partTitle(_ part: String?) -> String {
      switch part {
      case "initials": return "声母"
      case "finals": return "韵母"
      case "zero_initials": return "零声母音节"
      default: return ""
      }
    }
  }

  let effective: String
  let table: Table
  let problem: Problem?
  let keys: [Key]

  static let profiles = ["xiaohe", "ziranma", "shoudao", "microsoft", "smart_abc", "custom"]
  static let builtinProfiles = Array(profiles.dropLast())
  static let parts = ["initials", "finals", "zero_initials"]
  static let profileKey = "shuangpin_profile"
  static let tableKey = "shuangpin_custom_profile"

  static func profile(in document: [String: Any]?) -> String {
    let value = document?[profileKey] as? String ?? "xiaohe"
    return profiles.contains(value) ? value : "xiaohe"
  }

  static func current(in document: [String: Any]?) throws -> Self {
    try MetasequoiaInputSessionBridge.shuangpinCustomLayout(
      profile: profile(in: document), table: document?[tableKey] as? Table)
  }

  static func title(_ profile: String) -> String {
    switch profile {
    case "ziranma": return "自然码双拼"
    case "shoudao": return "首道双拼"
    case "microsoft": return "微软双拼"
    case "smart_abc": return "智能 ABC 双拼"
    case "custom": return "自定义"
    default: return "小鹤双拼"
    }
  }

  static func partTitle(_ part: String) -> String {
    switch part {
    case "initials": return "声母"
    case "finals": return "韵母"
    case "zero_initials": return "零声母音节"
    default: return part
    }
  }

  static func importTable(_ data: Data) throws -> Table {
    guard data.count <= 16_384 else { throw Failure(message: "码表文件不能超过 16 KB") }
    let table = try JSONDecoder().decode(Table.self, from: data)
    guard Set(table.keys).isSubset(of: Set(parts)) else {
      throw Failure(message: "码表只接受 initials、finals 和 zero_initials 分组")
    }
    let checked = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "custom", table: table)
    if let problem = checked.problem { throw Failure(message: problem.message) }
    return table
  }

  static func exportTable(_ table: Table) throws -> Data {
    try JSONSerialization.data(withJSONObject: table, options: [.prettyPrinted, .sortedKeys])
  }

  static func validKey(_ part: String, _ key: String) -> Bool {
    let chars = Array(key.lowercased())
    let length = part == "zero_initials" ? 2 : 1
    guard chars.count == length else { return false }
    return chars.enumerated().allSatisfy { index, value in
      value.isASCII && (value.isLetter ||
        (value == ";" && (part == "finals" || (part == "zero_initials" && index == 1))))
    }
  }

  static func save(profile: String, table: Table?) throws -> Bool {
    let checked = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: profile, table: table)
    if let problem = checked.problem { throw Failure(message: problem.message) }
    var selection: InputSchemePreference.Selection?
    guard MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      selection = write(profile: profile, table: table, into: &document)
    }, let selection else { return false }
    InputSchemePreference.mirror(selection)
    return true
  }

  /// 删除表只影响自定义方案，保留其他输入方案和个人词库。
  static func remove(from document: inout [String: Any]) {
    document.removeValue(forKey: tableKey)
    if profile(in: document) == "custom" { document[profileKey] = "xiaohe" }
  }

  static func write(profile: String, table: Table?, into document: inout [String: Any]) -> InputSchemePreference.Selection? {
    let scheme = ChineseInputScheme.allCases.first { $0.shuangpinProfile == profile } ?? .shuangpin
    guard let selected = InputSchemePreference.write({ selection in
      if !selection.enabled.contains(scheme) { selection.enabled.append(scheme) }
      selection.scheme = scheme
    }, into: &document) else { return nil }
    // 入口映射会写内置方案名；实际码表必须最后写，和触屏入口一起原子保存。
    document[profileKey] = profile
    if profile == "custom", let table { document[tableKey] = table }
    return selected
  }

  /// 键盘从其他方案回到普通双拼入口时保留用户配置；设置页仍可明确改选小鹤。
  static func runtimeMapping(_ mapping: @escaping (inout [String: Any]) -> Void,
                             scheme: ChineseInputScheme) -> (inout [String: Any]) -> Void {
    { document in
      let configured = document[profileKey] as? String
      mapping(&document)
      if scheme == .shuangpin, let configured, ["custom", "smart_abc"].contains(configured) {
        document[profileKey] = configured
      }
    }
  }
}
