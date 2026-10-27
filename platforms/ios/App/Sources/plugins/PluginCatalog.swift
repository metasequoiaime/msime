import Foundation

/// 「插件」页的数据与规则，与共享设置页 `plugin-catalog-helpers.ts`、`plugin-preferences.ts` 同一套说法和写法。这里不碰 SwiftUI 和文件，单元测试直接测它；读写目录在 `PluginLibrary` 里，经 `msime_client_plugins`。
///
/// 手机只列在手机上起作用的几种：短语表、指令表、符号集、单词本、按键音效包（`mode = "keys"`，旋律包 iOS 不播）和辅助码表。特效包、音乐包和旋律音效包装着也不列，只在列表里用一行说明有几个。
enum PluginKind: String, CaseIterable, Sendable {
  case sound, music, commandTable = "command_table", effect, phraseTable = "phrase_table"
  case helpcode, wordbook, symbolSet = "symbol_set", script

  /// 各类插件统一的显示名，与 `kindLabels` 相同。
  var label: String {
    switch self {
    case .sound: "音效包"
    case .music: "音乐包"
    case .commandTable: "指令表"
    case .effect: "特效包"
    case .phraseTable: "短语表"
    case .helpcode: "辅助码表"
    case .wordbook: "单词本"
    case .symbolSet: "符号集"
    case .script: "脚本插件"
    }
  }

  /// 「我的插件」分组的顺序：桌面 `kindOrder` 里在手机上起作用的那几种。
  static let phoneOrder: [PluginKind] = [.sound, .commandTable, .phraseTable, .helpcode, .wordbook, .symbolSet]

  /// 设置页编辑器能写的四种，按「新建插件」里列出的顺序。
  static let authored: [PluginKind] = [.phraseTable, .commandTable, .symbolSet, .wordbook]

  var isAuthored: Bool { Self.authored.contains(self) }
}

/// 指令表的一条，详情里只列触发词和标题。
struct PluginCommandRow: Equatable, Sendable {
  let trigger: String
  let title: String
}

/// 短语表的一行。
struct PluginPhraseRow: Equatable, Sendable {
  let key: String
  let text: String
}

/// 辅助码表预览的一条。
struct PluginHelpcodeRow: Equatable, Sendable {
  let character: String
  let code: String
}

/// 符号集的一组。
struct PluginSymbolGroupRow: Equatable, Sendable {
  /// `symbols` 或 `kaomoji`。
  let tab: String
  let title: String
  let items: [String]
}

/// `msime_client_plugins` 的 `catalog` 里的一个包：清单的通用字段，加上详情页要显示的那一种内容。
struct PluginPackage: Equatable, Identifiable, Sendable {
  let kind: PluginKind
  let id: String
  let name: String
  let version: String
  let license: String
  let author: String?
  let description: String?
  /// 随包内置，不能删除，也不能编辑。
  let builtin: Bool
  /// 音效包：`keys` 或 `sequence`；没写时按 `keys`。
  var mode: String? = nil
  var commands: [PluginCommandRow] = []
  var phrases: [PluginPhraseRow] = []
  /// 辅助码表：条数和前几条。
  var entries: Int? = nil
  var preview: [PluginHelpcodeRow] = []
  /// 单词本：单词数和前几个单词。
  var wordCount: Int? = nil
  var firstWords: [String] = []
  var groups: [PluginSymbolGroupRow] = []

  /// 同一个 id 可以分属不同类型，所以列表和导航用类型加 id 认一个包。
  var key: String { "\(kind.rawValue)/\(id)" }

  var isMelody: Bool { kind == .sound && mode == "sequence" }

  /// 这台设备用得上这个包：手机上起作用的那几种，音效包只算按键音效。
  var usedOnPhone: Bool { PluginKind.phoneOrder.contains(kind) && !isMelody }

  /// 类型显示名：旋律音效包是「音效包·旋律」，与 `packKindLabel` 相同。
  var kindLabel: String { isMelody ? "音效包·旋律" : kind.label }

  /// 列表里的名字：内置包后面加「（内置）」，与 `packLabel` 相同。
  var label: String { builtin ? "\(name)（内置）" : name }

  /// 在设置页编辑器里能改：四种文本类插件里不是内置的那些。
  var editable: Bool { kind.isAuthored && !builtin }

  /// 读 `catalog` 或 `import` 回答里的一个包；类型不认识、缺 id 时为 nil。
  init?(json: Any) {
    guard let object = json as? [String: Any],
          let kind = (object["kind"] as? String).flatMap(PluginKind.init(rawValue:)),
          let id = object["id"] as? String, !id.isEmpty else { return nil }
    let name = object["name"] as? String ?? ""
    self.init(kind: kind, id: id, name: name.isEmpty ? id : name,
              version: object["version"] as? String ?? "", license: object["license"] as? String ?? "",
              author: Self.optionalText(object["author"]), description: Self.optionalText(object["description"]),
              builtin: object["builtin"] as? Bool ?? false)
    mode = object["mode"] as? String
    commands = (object["commands"] as? [[String: Any]] ?? []).compactMap { row in
      guard let trigger = row["trigger"] as? String else { return nil }
      return PluginCommandRow(trigger: trigger, title: row["title"] as? String ?? "")
    }
    phrases = (object["phrases"] as? [[String: Any]] ?? []).compactMap { row in
      guard let key = row["key"] as? String, let text = row["text"] as? String else { return nil }
      return PluginPhraseRow(key: key, text: text)
    }
    entries = SharedNumber.nonnegativeInt(object["entries"])
    preview = (object["preview"] as? [[String: Any]] ?? []).compactMap { row in
      guard let character = row["character"] as? String, let code = row["code"] as? String else { return nil }
      return PluginHelpcodeRow(character: character, code: code)
    }
    wordCount = SharedNumber.nonnegativeInt(object["word_count"])
    firstWords = object["first_words"] as? [String] ?? []
    groups = (object["groups"] as? [[String: Any]] ?? []).compactMap { group in
      guard let tab = group["tab"] as? String, let title = group["title"] as? String else { return nil }
      return PluginSymbolGroupRow(tab: tab, title: title, items: group["items"] as? [String] ?? [])
    }
  }

  init(kind: PluginKind, id: String, name: String, version: String = "1.0.0", license: String = "CC0-1.0",
       author: String? = nil, description: String? = nil, builtin: Bool = false) {
    self.kind = kind
    self.id = id
    self.name = name
    self.version = version
    self.license = license
    self.author = author
    self.description = description
    self.builtin = builtin
  }

  private static func optionalText(_ value: Any?) -> String? {
    guard let text = value as? String, !text.isEmpty else { return nil }
    return text
  }
}

/// 插件目录下不是可载入包的一个文件夹，以及原因。
struct PluginIssue: Equatable, Sendable {
  /// 类型的原文；不认识的类型照原文显示。
  let kind: String
  let folder: String
  let reason: String

  var kindLabel: String { PluginKind(rawValue: kind)?.label ?? kind }
}

/// 一次 `catalog` 读到的全部。
struct PluginCatalog: Equatable, Sendable {
  var packages: [PluginPackage] = []
  var issues: [PluginIssue] = []

  /// 读 `catalog` 的 value；读不懂时为 nil，页面说读取失败，而不是说没有插件。
  init?(json: Any) {
    guard let object = json as? [String: Any], let packages = object["packages"] as? [Any] else { return nil }
    self.packages = packages.compactMap(PluginPackage.init(json:))
    issues = (object["issues"] as? [[String: Any]] ?? []).compactMap { issue in
      guard let kind = issue["kind"] as? String, kind != PluginKind.script.rawValue else { return nil }
      return PluginIssue(kind: kind, folder: issue["folder"] as? String ?? "", reason: issue["reason"] as? String ?? "")
    }
  }

  init(packages: [PluginPackage] = [], issues: [PluginIssue] = []) {
    self.packages = packages
    self.issues = issues
  }

  /// 「我的插件」的分组：按 `PluginKind.phoneOrder`，只留有包或有找不到的选择的类型。指令表一组总在，它带着 / 指令的说明。
  func groups(missing: [PluginMissingSelection]) -> [PluginGroup] {
    PluginKind.phoneOrder.compactMap { kind in
      let packs = packages.filter { $0.kind == kind && $0.usedOnPhone }
      let gone = missing.filter { $0.kind == kind }
      guard !packs.isEmpty || !gone.isEmpty || kind == .commandTable else { return nil }
      return PluginGroup(kind: kind, packs: packs, missing: gone)
    }
  }

  /// 装着、但这台设备用不上的包（特效包、音乐包、旋律音效包）：内置的不算，那不是用户装的。
  var unusedCount: Int { packages.filter { !$0.builtin && !$0.usedOnPhone }.count }

  func package(kind: PluginKind, id: String) -> PluginPackage? {
    packages.first { $0.kind == kind && $0.id == id }
  }
}

/// 「我的插件」里的一组：一种类型装着的包和这种类型找不到的选择。
struct PluginGroup: Identifiable, Equatable, Sendable {
  let kind: PluginKind
  let packs: [PluginPackage]
  let missing: [PluginMissingSelection]
  var id: PluginKind { kind }
}

/// 共享文档里插件页读写的那几项，以及两个快捷模式的开关，缺省值与 client-core 相同。
struct PluginPreferences: Equatable, Sendable {
  /// `PluginPreferences::MAX_COMMAND_TABLES`、`MAX_PHRASE_TABLES`。
  static let maximumTables = 16

  var keySoundPack = KeySoundPackPolicy.defaultPack
  var commandTables: [String] = []
  var phraseTables: [String] = []
  var helpcodeQuanpin = ""
  var helpcodeShuangpin = ""
  /// `local_modes.quick_phrase`，缺省为开。
  var quickPhraseMode = true
  /// `local_modes.command`，缺省为关。
  var commandMode = false

  init() {}

  init(document: [String: Any]?) {
    let plugins = document?["plugins"] as? [String: Any] ?? [:]
    keySoundPack = KeySoundPackPolicy.selectedPack(in: document)
    commandTables = plugins["command_tables"] as? [String] ?? []
    phraseTables = plugins["phrase_tables"] as? [String] ?? []
    helpcodeQuanpin = plugins["helpcode_pack_quanpin"] as? String ?? ""
    helpcodeShuangpin = plugins["helpcode_pack_shuangpin"] as? String ?? ""
    let modes = document?["local_modes"] as? [String: Any] ?? [:]
    quickPhraseMode = modes["quick_phrase"] as? Bool ?? true
    commandMode = modes["command"] as? Bool ?? false
  }
}

/// 设置里选中、但目录里已经没有的包（或者按键音选成了旋律包），每个包一条，按列表顺序。
struct PluginMissingSelection: Equatable, Identifiable, Sendable {
  let kind: PluginKind
  let id: String
  var uses: [String]
  /// 同 id 的音效包装着，但它是旋律包，不能当按键音效用。
  var mismatched = false

  var key: String { "\(kind.rawValue)/\(id)" }

  /// 列出和做标题用的名字，与 `missingTitle` 相同。
  var title: String { "\(id)\(mismatched ? "（不可用）" : "（未找到）")" }

  /// 为什么用不了，接在用途后面，与 `missingReason` 相同。
  var reason: String {
    guard mismatched else { return "已不在本机" }
    return "但它是按键旋律，不能用作按键音效"
  }
}

/// 列表标记、启用与选用、删除后的清理和失败提示。
enum PluginCatalogPolicy {
  /// 当前方案能不能打开这一种表的模式。引擎只在全拼、双拼和五笔下打开 K 和 / 模式（`opens_table_modes`）；iOS 没有 Shift+K，K 模式只能从本地输入菜单进，而这个菜单只在全拼和双拼一系（含九键、14 键、18 键和手写）里有，五笔下没有；/ 模式在五笔下仍能从符号键盘的 `/` 进。
  static func schemeOpensTableModes(_ scheme: ChineseInputScheme, kind: PluginKind) -> Bool {
    let pinyin = (scheme.engineScheme == "quanpin" || scheme.engineScheme == "shuangpin") && scheme.writesChinese
    return pinyin || (kind == .commandTable && scheme == .wubi)
  }

  /// 当前方案打不开这种表的模式时详情里的说明；能打开时为 nil。
  static func schemeTableModeNote(_ scheme: ChineseInputScheme, kind: PluginKind) -> String? {
    guard !schemeOpensTableModes(scheme, kind: kind) else { return nil }
    return kind == .commandTable
      ? "当前输入方案「\(scheme.title)」打不开指令（/ 模式），切换到全拼、双拼或五笔后才能用。"
      : "当前输入方案「\(scheme.title)」打不开快捷短语（K 模式），切换到全拼或双拼后才能用。"
  }

  /// 列表行尾的标记：正在用的按键音效包是「使用中」，按键音关着时是「已选 · 按键音未开」；启用的短语表、指令表标出排位，方案打不开或模式关着时先说这个；辅助码表标出用在哪个方案上。与 `packMarker` 相同，iOS 没有上屏音和成就音效，所以按键音效包只看按键音开关。
  static func marker(_ pack: PluginPackage, preferences: PluginPreferences, soundEnabled: Bool,
                     scheme: ChineseInputScheme) -> String? {
    let schemeOpens = schemeOpensTableModes(scheme, kind: pack.kind)
    switch pack.kind {
    case .sound:
      guard !pack.isMelody, preferences.keySoundPack == pack.id else { return nil }
      return soundEnabled ? "使用中" : "已选 · 按键音未开"
    case .commandTable:
      guard let position = preferences.commandTables.firstIndex(of: pack.id) else { return nil }
      if !schemeOpens { return "已启用 · 当前方案打不开 / 指令" }
      return preferences.commandMode ? "已启用 · 第 \(position + 1) 位" : "已启用 · / 指令未开"
    case .phraseTable:
      guard let position = preferences.phraseTables.firstIndex(of: pack.id) else { return nil }
      if !schemeOpens { return "已启用 · 当前方案打不开 K 模式" }
      return preferences.quickPhraseMode ? "已启用 · 第 \(position + 1) 位" : "已启用 · K 模式未开"
    case .helpcode:
      return helpcodeUses(preferences, id: pack.id)
    default:
      return nil
    }
  }

  /// 辅助码表用在哪个方案上：「用于全拼」「用于双拼」「用于全拼和双拼」，都没用时为 nil。
  static func helpcodeUses(_ preferences: PluginPreferences, id: String) -> String? {
    let schemes = [preferences.helpcodeQuanpin == id ? "全拼" : nil, preferences.helpcodeShuangpin == id ? "双拼" : nil]
      .compactMap { $0 }
    return schemes.isEmpty ? nil : "用于\(schemes.joined(separator: "和"))"
  }

  /// 选中但已不在目录里的包。只在读到了目录之后才有意义；内置音效包随包带着，找不到时（设置 App 没找到键盘扩展里的 `sound-packs`）也照样列出来。
  static func missingSelections(_ preferences: PluginPreferences, packages: [PluginPackage]) -> [PluginMissingSelection] {
    var missing: [PluginMissingSelection] = []
    func add(_ kind: PluginKind, _ id: String, _ use: String, keys: Bool = false) {
      guard !id.isEmpty else { return }
      let found = packages.contains { $0.kind == kind && $0.id == id && (!keys || !$0.isMelody) }
      guard !found else { return }
      let mismatched = packages.contains { $0.kind == kind && $0.id == id }
      if let index = missing.firstIndex(where: { $0.kind == kind && $0.id == id }) {
        missing[index].uses.append(use)
      } else {
        missing.append(PluginMissingSelection(kind: kind, id: id, uses: [use], mismatched: mismatched))
      }
    }
    add(.sound, preferences.keySoundPack, "当前音效包", keys: true)
    for id in preferences.commandTables { add(.commandTable, id, "已启用的指令表") }
    for id in preferences.phraseTables { add(.phraseTable, id, "已启用的短语表") }
    add(.helpcode, preferences.helpcodeQuanpin, "全拼辅助码")
    add(.helpcode, preferences.helpcodeShuangpin, "双拼辅助码")
    return missing
  }

  // MARK: - 写共享文档

  /// 改 `plugins` 对象：其余的键（按键音音量、特效、桌面才用的选择等）原样保留。
  static func updatePlugins(_ document: inout [String: Any], _ change: (inout [String: Any]) -> Void) {
    var plugins = document["plugins"] as? [String: Any] ?? [:]
    change(&plugins)
    document["plugins"] = plugins
  }

  /// 启用或停用一个指令表或短语表：启用时排到已启用的之后，与 `setCommandTable`、`setPhraseTable` 相同。
  static func setTable(_ kind: PluginKind, id: String, enabled: Bool, document: inout [String: Any]) {
    let key = kind == .commandTable ? "command_tables" : "phrase_tables"
    updatePlugins(&document) { plugins in
      var tables = (plugins[key] as? [String] ?? []).filter { $0 != id }
      if enabled { tables.append(id) }
      plugins[key] = tables
    }
  }

  /// 打开或关掉一个快捷模式：`local_modes` 里的其他开关原样保留。
  static func setLocalMode(_ key: String, on: Bool, document: inout [String: Any]) {
    var modes = document["local_modes"] as? [String: Any] ?? [:]
    modes[key] = on
    document["local_modes"] = modes
  }

  /// 辅助码表用于全拼或双拼；关掉时清成空字符串，回到原来的方案。
  static func setHelpcode(_ id: String, quanpin: Bool, on: Bool, document: inout [String: Any]) {
    updatePlugins(&document) { plugins in
      plugins[quanpin ? "helpcode_pack_quanpin" : "helpcode_pack_shuangpin"] = on ? id : ""
    }
  }

  /// 包删掉以后的设置：启用它的表移除，选它的辅助码清掉，选它的按键音效包回到内置的 `default`，与 `withoutRemovedPack` 相同。单词本和符号集没有偏好。
  static func withoutRemovedPack(_ kind: PluginKind, id: String, document: inout [String: Any]) {
    switch kind {
    case .commandTable, .phraseTable:
      let key = kind == .commandTable ? "command_tables" : "phrase_tables"
      guard ((document["plugins"] as? [String: Any])?[key] as? [String])?.contains(id) == true else { return }
      setTable(kind, id: id, enabled: false, document: &document)
    case .helpcode:
      HelpcodeChoicePolicy.clearSelections(of: id, document: &document)
    case .sound:
      guard KeySoundPackPolicy.selectedPack(in: document) == id else { return }
      KeySoundPackPolicy.choose(KeySoundPackPolicy.defaultPack, document: &document)
    default:
      return
    }
  }

  // MARK: - 提示

  /// `msime_client_plugins` 失败时的一句话，与 `pluginErrorMessage` 相同；`detail` 是 client-services 报告的规则。
  static func errorMessage(_ failure: MetasequoiaInputSessionBridge.PluginFailure, fallback: String) -> String {
    let reason = failure.detail.map { $0.isEmpty ? "" : "（\($0)）" } ?? ""
    switch failure.code {
    case "plugin_invalid": return "插件不符合要求\(reason)。"
    case "plugin_archive": return "压缩包无法读取\(reason)。"
    case "plugin_unsupported_source": return "只能导入插件文件夹或 .zip 文件。"
    case "plugin_reserved": return "这个 id 属于内置插件，不能覆盖或删除。"
    case "plugin_storage": return "插件目录不可用，请检查数据目录的权限。"
    case "plugin_exists": return "本机已经有一个同样 id 的插件，没有覆盖它。"
    case "plugin_not_editable": return "这个插件不能在设置页编辑\(reason)。"
    case "storage": return "插件的数据读写失败，请检查数据目录的权限。"
    default: return fallback
    }
  }

  /// 导入成功的提示，与桌面相同；这台设备用不上的类型多说一句它不会列出来。
  static func importedNotice(_ pack: PluginPackage) -> String {
    let notice = "已导入\(pack.kindLabel)「\(pack.name)」\(pack.version)。"
    return pack.usedOnPhone ? notice : notice + "这台设备用不上\(pack.kindLabel)，它不会出现在「我的插件」里。"
  }

  /// 删除前确认的标题和说明，与桌面相同。
  static func removeConfirmation(_ pack: PluginPackage) -> (title: String, message: String) {
    ("删除\(pack.kindLabel)", "删除「\(pack.name)」？包里的文件会从本机移除。")
  }

  /// 「本地输入」菜单和候选栏怎么打开一个模式，与 `iosLocalModeEntry` 相同。
  static func localModeEntry(_ title: String) -> String {
    "在候选栏点「水杉输入法」，或在「更多 → 本地输入」里选「\(title)」，"
  }

  /// 单词本插件在背单词里的词书 id：`wordbook_pack::book_id`。
  static func wordbookBookID(_ id: String) -> String { "pack-\(id)" }

  /// 详情页各类内容的预览条数，与桌面相同。
  static let phrasePreviewRows = 20
  static let wordbookPreviewWords = 5
  static let symbolPreviewItems = 16
}
