import Foundation

/// 「辅助码」里内置方案和已导入码表的选择规则，与共享设置页 `helpcode-page.tsx`、Android 的 `HelpcodeChoicePolicy` 相同（#7176）。
///
/// 导入的码表是 `kind = "helpcode"` 的插件包（`msime_client_plugins` 的 `import_helpcode_table`）。选中时只写当前方案那一份的 `plugins.helpcode_pack_quanpin` 或 `helpcode_pack_shuangpin`，`*_helpcode.schema` 不动；选内置方案时写 `schema`，原来选着包时把包的选择清成空字符串。包载入不了时 host-api 退回 `schema`，所以清掉选择总是安全的。`plugins` 里的其他键（按键音包等）一律原样保留。这里不碰 SwiftUI 和文件，单元测试直接测它。
enum HelpcodeChoicePolicy {
  /// 方案列表里已导入码表的值前缀：`pack:<包 id>`，与共享设置页相同。
  static let packPrefix = "pack:"
  /// 码表文件的字节上限，即 client-services 的 `helpcode_pack::MAX_TABLE_BYTES`。超过它就不读进内存、不调用 host-api。
  static let maximumTableBytes = 1_048_576
  /// 两份方案各自的包选择键。
  static let packKeys = ["helpcode_pack_quanpin", "helpcode_pack_shuangpin"]

  /// 一个已导入的码表：包 id 和显示名。
  struct Pack: Equatable, Identifiable {
    let id: String
    let name: String
  }

  /// 方案列表里的一项：写回的值和显示的文字。
  struct Option: Equatable {
    let value: String
    let label: String
  }

  /// 这一份方案的包选择键：双拼用 `helpcode_pack_shuangpin`，其余用 `helpcode_pack_quanpin`。
  static func packKey(_ scheme: HelpcodeSettingsView.Scheme) -> String {
    scheme.key == HelpcodeSettingsView.shuangpin.key ? "helpcode_pack_shuangpin" : "helpcode_pack_quanpin"
  }

  static func packValue(_ id: String) -> String { packPrefix + id }

  /// `catalog` 的 value 里 `kind = "helpcode"` 的包，按目录给出的顺序。没有名字的包用 id 显示；读不懂的 value 得到空列表，方案列表照样列出内置方案。
  static func packs(inCatalog value: Any) -> [Pack] {
    guard let packages = (value as? [String: Any])?["packages"] as? [[String: Any]] else { return [] }
    return packages.compactMap { package in
      guard package["kind"] as? String == "helpcode", let id = package["id"] as? String, !id.isEmpty else { return nil }
      let name = package["name"] as? String ?? ""
      return Pack(id: id, name: name.isEmpty ? id : name)
    }
  }

  /// 文档里这一份方案选中的包，没选时是空字符串。
  static func selectedPack(of scheme: HelpcodeSettingsView.Scheme, in document: [String: Any]?) -> String {
    (document?["plugins"] as? [String: Any])?[packKey(scheme)] as? String ?? ""
  }

  /// 列表里的已导入码表：每个包一项「名称（插件）」；选中的包已经不在了时补一项「id（插件，未找到）」，让用户看得见当前选择并能改掉它。
  static func packOptions(_ packs: [Pack], selectedPack: String) -> [Option] {
    var options = packs.map { Option(value: packValue($0.id), label: "\($0.name)（插件）") }
    if !selectedPack.isEmpty, !packs.contains(where: { $0.id == selectedPack }) {
      options.append(Option(value: packValue(selectedPack), label: "\(selectedPack)（插件，未找到）"))
    }
    return options
  }

  /// 当前选中的那一项的值：选了包时是 `pack:<id>`，否则是方案。
  static func selectedValue(schema: String, selectedPack: String) -> String {
    selectedPack.isEmpty ? schema : packValue(selectedPack)
  }

  /// 在列表里选了 `value`（内置方案 id 或 `pack:<id>`）：选包只改这一份的包选择；选内置方案写 `schema`，原来选着包时一并清掉。
  static func choose(_ value: String, for scheme: HelpcodeSettingsView.Scheme, document: inout [String: Any]) {
    let selected = selectedPack(of: scheme, in: document)
    if value.hasPrefix(packPrefix) {
      setPack(String(value.dropFirst(packPrefix.count)), for: scheme, document: &document)
      return
    }
    HelpcodeSettingsView.merge("schema", value, into: scheme, document: &document)
    if !selected.isEmpty { setPack("", for: scheme, document: &document) }
  }

  /// 把这一份方案的包选择写成 `id`（空字符串表示清掉），`plugins` 里的其他键保持原样。
  static func setPack(_ id: String, for scheme: HelpcodeSettingsView.Scheme, document: inout [String: Any]) {
    var plugins = document["plugins"] as? [String: Any] ?? [:]
    plugins[packKey(scheme)] = id
    document["plugins"] = plugins
  }

  /// 删除包 `id` 之后，把全拼、双拼两份里指向它的选择清掉，退回内置方案；返回是否清掉了什么。
  @discardableResult
  static func clearSelections(of id: String, document: inout [String: Any]) -> Bool {
    guard var plugins = document["plugins"] as? [String: Any] else { return false }
    var cleared = false
    for key in packKeys where plugins[key] as? String == id {
      plugins[key] = ""
      cleared = true
    }
    if cleared { document["plugins"] = plugins }
    return cleared
  }

  /// 读到的码表字节严格按 UTF-8 解码；`data` 为 nil 表示超过了 `maximumTableBytes`。不宽松解码：GBK 码表被替换成 U+FFFD 后，报出来的会是「等号左边必须恰好是一个字」这种看不出原因的错误。
  static func decode(_ data: Data?) -> Result<String, DecodeFailure> {
    guard let data else { return .failure(.tooLarge) }
    guard let text = String(data: data, encoding: .utf8) else { return .failure(.notUTF8) }
    return .success(text)
  }

  enum DecodeFailure: Error, Equatable {
    case tooLarge
    case notUTF8

    var message: String {
      switch self {
      case .tooLarge: return "码表超过 1 MiB，无法导入"
      case .notUTF8: return "码表不是 UTF-8 编码，请先另存为 UTF-8 再导入"
      }
    }
  }

  /// 导入成功的提示；`selected` 为 false 时导入成功但没能切换过去。
  static func importedMessage(name: String, entries: Int, selected: Bool) -> String {
    let imported = "已导入「\(name)」，共 \(entries) 条"
    return selected ? imported + "，已切换到这套辅助码" : imported + "；切换失败，请在列表里选择它"
  }

  /// `import_helpcode_table` 失败时的提示：码表不合格时把 client-services 给的原因（带文件名和行号）原样告诉用户。
  static func importFailure(_ failure: MetasequoiaInputSessionBridge.PluginFailure) -> String {
    if failure.code == "plugin_invalid", let detail = failure.detail, !detail.isEmpty { return "无法导入：\(detail)" }
    if failure.code == "plugin_storage" || failure.code == "storage" { return "保存码表失败，请重试" }
    if failure.code == "invalid" { return "无法导入这个码表，请确认它是 1 MiB 以内的 UTF-8 文本" }
    return "导入失败，请重试"
  }

  /// 删除成功的提示；`wasSelected` 时说明已经回到内置方案。
  static func removedMessage(name: String, wasSelected: Bool) -> String {
    wasSelected ? "已删除「\(name)」，已改回内置方案" : "已删除「\(name)」"
  }
}
