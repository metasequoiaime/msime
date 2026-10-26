import Foundation

/// 设置页插件编辑器的草稿、检查和整理，逐条照搬共享设置页的 `plugin-editor-model.ts`（Windows 的 `PluginEditorModel.h` 也是它的移植）：提示文字、上限和行号的算法都相同。这里先查一遍，让用户在保存之前就知道该改哪一行；保存时 `msime_client_plugins` 的 `authoring_save` 按导入同一套规则再查。这里不碰 SwiftUI，单元测试直接测它。
enum PluginEditorModel {
  /// 「新建插件」里的一种。
  struct KindOption: Identifiable, Sendable {
    let kind: PluginKind
    let title: String
    let description: String
    var id: PluginKind { kind }
  }

  /// 「新建插件」里列出的四种，每种带一句给不读文档的人看的说明。
  static let kinds: [KindOption] = [
    KindOption(kind: .phraseTable, title: "短语表", description: "在 K 模式里输入编码，打出一段常用文字，例如输入 dz 打出家庭地址。"),
    KindOption(kind: .commandTable, title: "指令表", description: "按 / 再输入指令字母，插入签名、当天日期这类模板文字。"),
    KindOption(kind: .symbolSet, title: "符号集", description: "给符号面板加几组自己常用的符号或颜文字。"),
    KindOption(kind: .wordbook, title: "单词本", description: "做一本自己的词书，在背单词里复习。"),
  ]

  /// 「新建插件」旁边的一句说明。
  static let createDescription = "不用写代码，填好短语、指令、符号或单词就能做出自己的插件，做好后也可以发布到社区"

  static let defaultLicense = "CC0-1.0"
  static let defaultVersion = "1.0.0"

  // 以下上限与 `crates/client-services/src/plugins/` 的清单校验一致。
  static let maximumNameBytes = 80
  static let maximumVersionBytes = 32
  static let maximumLicenseBytes = 64
  static let maximumAuthorBytes = 120
  static let maximumDescriptionBytes = 500
  /// `phrase_table::MAX_PHRASES`。
  static let maximumPhrases = 2000
  /// `command_table::MAX_COMMANDS`。
  static let maximumCommands = 256
  static let maximumKeyLetters = 32
  /// 短语文字和指令模板的 UTF-16 码元上限：Windows 候选管道的文本字段。
  static let maximumTextUTF16 = 199
  static let maximumCommandTitleBytes = 48
  /// `symbol_set::MAX_GROUPS`。
  static let maximumGroups = 32
  static let maximumGroupItems = 512
  static let maximumSymbolItems = 2048
  static let maximumGroupTitleBytes = 48
  static let maximumKeywordsBytes = 256
  static let maximumItemUTF16 = 64
  /// `vocabulary::wordbook::MAX_ENTRIES`。
  static let maximumWords = 20000
  static let maximumWordCharacters = 64
  static let maximumPhoneticCharacters = 64
  static let maximumGlossCharacters = 256
  static let maximumWordbookNameCharacters = 64

  /// 长列表一次画出的行数：单词本可以有两万行，全部画出来会让页面卡住。
  static let rowPage = 100

  // MARK: - 草稿

  struct Phrase: Equatable, Sendable {
    var key = ""
    var text = ""
    var isBlank: Bool { blank(key) && blank(text) }
  }

  struct Command: Equatable, Sendable {
    var trigger = ""
    var title = ""
    var template = ""
    var isBlank: Bool { blank(trigger) && blank(title) && blank(template) }
  }

  /// 编辑中的一组符号：`items` 是文本框里的原文，一行一项。
  struct Group: Equatable, Sendable {
    var tab = "symbols"
    var title = ""
    var keywords = ""
    var items = ""
    var isBlank: Bool { blank(title) && blank(keywords) && blank(items) }
  }

  /// 编辑中的单词：音标留空时就是空串。
  struct Word: Equatable, Sendable {
    var word = ""
    var phonetic = ""
    var gloss = ""
    var isBlank: Bool { blank(word) && blank(phonetic) && blank(gloss) }
  }

  /// 编辑器的草稿：全部字段都是表单里的原文，保存前由 `pack(from:)` 整理。四种内容都在，只有与 `kind` 对应的那一种会保存。
  struct Draft: Equatable, Sendable {
    var kind: PluginKind
    /// 编辑已安装的包时是它的 id；新建时为 nil，由宿主生成。
    var id: String?
    var name = ""
    var version = PluginEditorModel.defaultVersion
    var license = PluginEditorModel.defaultLicense
    var author = ""
    var description = ""
    var phrases: [Phrase] = []
    var commands: [Command] = []
    var groups: [Group] = []
    var words: [Word] = []

    /// 新建一种插件时的草稿：默认许可证和版本，内容先给一个空行。
    init(kind: PluginKind) {
      self.kind = kind
      id = nil
      phrases = kind == .phraseTable ? [Phrase()] : []
      commands = kind == .commandTable ? [Command()] : []
      groups = kind == .symbolSet ? [Group()] : []
      words = kind == .wordbook ? [Word()] : []
    }

    /// `authoring_load` 读出的已安装包，变成可以编辑的草稿；读不懂时为 nil。
    init?(authored value: Any) {
      guard let object = value as? [String: Any],
            let kind = (object["kind"] as? String).flatMap(PluginKind.init(rawValue:)), kind.isAuthored,
            let id = object["id"] as? String else { return nil }
      self.kind = kind
      self.id = id
      name = object["name"] as? String ?? ""
      version = object["version"] as? String ?? ""
      license = object["license"] as? String ?? ""
      author = object["author"] as? String ?? ""
      description = object["description"] as? String ?? ""
      phrases = (object["phrases"] as? [[String: Any]] ?? []).map {
        Phrase(key: $0["key"] as? String ?? "", text: $0["text"] as? String ?? "")
      }
      commands = (object["commands"] as? [[String: Any]] ?? []).map {
        Command(trigger: $0["trigger"] as? String ?? "", title: $0["title"] as? String ?? "",
                template: $0["template"] as? String ?? "")
      }
      groups = (object["groups"] as? [[String: Any]] ?? []).map {
        Group(tab: $0["tab"] as? String ?? "symbols", title: $0["title"] as? String ?? "",
              keywords: $0["keywords"] as? String ?? "", items: ($0["items"] as? [String] ?? []).joined(separator: "\n"))
      }
      words = (object["words"] as? [[String: Any]] ?? []).map {
        Word(word: $0["word"] as? String ?? "", phonetic: $0["phonetic"] as? String ?? "", gloss: $0["gloss"] as? String ?? "")
      }
    }
  }

  // MARK: - 整理

  /// 文本框里的符号：一行一项，去掉首尾空白，跳过空行。
  static func groupItems(_ text: String) -> [String] {
    lines(text).map(trim).filter { !$0.isEmpty }
  }

  /// 草稿整理成交给 `authoring_save` 的包：各字段去掉首尾空白，可选字段留空时为 null，整行都空着的行不保存。
  static func pack(from draft: Draft) -> [String: Any] {
    var pack: [String: Any] = [
      "kind": draft.kind.rawValue,
      "id": draft.id.map { $0 as Any } ?? NSNull(),
      "name": trim(draft.name),
      "version": trim(draft.version),
      "license": trim(draft.license),
      "author": optional(draft.author),
      "description": optional(draft.description),
    ]
    switch draft.kind {
    case .phraseTable:
      pack["phrases"] = draft.phrases.filter { !$0.isBlank }.map { ["key": trim($0.key), "text": trim($0.text)] }
    case .commandTable:
      pack["commands"] = draft.commands.filter { !$0.isBlank }.map {
        ["trigger": trim($0.trigger), "title": trim($0.title), "template": trim($0.template)]
      }
    case .symbolSet:
      pack["groups"] = draft.groups.filter { !$0.isBlank }.map {
        ["tab": $0.tab, "title": trim($0.title), "keywords": optional($0.keywords), "items": groupItems($0.items)] as [String: Any]
      }
    case .wordbook:
      pack["words"] = draft.words.filter { !$0.isBlank }.map {
        ["word": trim($0.word), "phonetic": optional($0.phonetic), "gloss": trim($0.gloss)] as [String: Any]
      }
    default:
      break
    }
    return pack
  }

  // MARK: - 检查

  /// 草稿现在不能保存的第一个原因，用页面上的说法；能保存时为 nil。行号按表单里的行数（整行都空着的行不算问题）。
  static func issue(_ draft: Draft) -> String? {
    let name = trim(draft.name)
    if name.isEmpty { return "请给插件起个名字。" }
    if bytes(name) > maximumNameBytes || hasControl(name) { return "名字太长了。" }
    if draft.kind == .wordbook && characters(name) > maximumWordbookNameCharacters {
      return "单词本的名字不能超过 \(maximumWordbookNameCharacters) 个字。"
    }
    let version = trim(draft.version)
    if version.isEmpty { return "请填写版本，例如 1.0.0。" }
    if bytes(version) > maximumVersionBytes || hasControl(version) { return "版本太长了。" }
    let license = trim(draft.license)
    if license.isEmpty { return "请填写许可证，不确定时写 \(defaultLicense)。" }
    if bytes(license) > maximumLicenseBytes || !validLicense(license) {
      return "许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。"
    }
    if bytes(trim(draft.author)) > maximumAuthorBytes || hasControl(draft.author) { return "作者太长了。" }
    if bytes(trim(draft.description)) > maximumDescriptionBytes || hasControl(draft.description) { return "介绍太长了。" }
    switch draft.kind {
    case .phraseTable: return phrasesIssue(draft.phrases)
    case .commandTable: return commandsIssue(draft.commands)
    case .symbolSet: return groupsIssue(draft.groups)
    case .wordbook: return wordsIssue(draft.words)
    default: return nil
    }
  }

  private static func phrasesIssue(_ rows: [Phrase]) -> String? {
    var seen = Set<String>()
    var count = 0
    for (index, row) in rows.enumerated() where !row.isBlank {
      count += 1
      let at = "第 \(index + 1) 条短语"
      let key = trim(row.key)
      let text = trim(row.text)
      if key.isEmpty { return "\(at)还没有填写编码。" }
      if utf16(key) > maximumKeyLetters || !lowercaseLetters(key) { return "\(at)的编码只能是 1 到 \(maximumKeyLetters) 个小写字母。" }
      if text.isEmpty { return "\(at)还没有填写文字。" }
      if utf16(text) > maximumTextUTF16 { return "\(at)的文字太长了。" }
      if hasControl(text) { return "\(at)的文字里不能有换行或制表符。" }
      if !seen.insert("\(key)\n\(text)").inserted { return "\(at)和前面的一条重复了。" }
    }
    if count == 0 { return "至少要有一条短语。" }
    if count > maximumPhrases { return "一个短语表最多 \(maximumPhrases) 条。" }
    return nil
  }

  private static func commandsIssue(_ rows: [Command]) -> String? {
    var triggers = Set<String>()
    var count = 0
    for (index, row) in rows.enumerated() where !row.isBlank {
      count += 1
      let at = "第 \(index + 1) 条指令"
      let trigger = trim(row.trigger)
      let title = trim(row.title)
      let template = trim(row.template)
      if trigger.isEmpty { return "\(at)还没有填写指令字母。" }
      if utf16(trigger) > maximumKeyLetters || !lowercaseLetters(trigger) { return "\(at)的指令只能是 1 到 \(maximumKeyLetters) 个小写字母。" }
      if !triggers.insert(trigger).inserted { return "指令 /\(trigger) 重复了。" }
      if title.isEmpty { return "\(at)还没有填写标题。" }
      if bytes(title) > maximumCommandTitleBytes || hasControl(title) { return "\(at)的标题太长了。" }
      if template.isEmpty { return "\(at)还没有填写要插入的文字。" }
      if utf16(template) > maximumTextUTF16 { return "\(at)要插入的文字太长了。" }
      if hasControl(template) { return "\(at)要插入的文字里不能有换行或制表符。" }
      if !templatePlaceholdersKnown(template) { return "\(at)的花括号里只能写 {date}、{time} 或 {weekday}。" }
    }
    if count == 0 { return "至少要有一条指令。" }
    if count > maximumCommands { return "一个指令表最多 \(maximumCommands) 条。" }
    return nil
  }

  private static func groupsIssue(_ groups: [Group]) -> String? {
    var titles = Set<String>()
    var count = 0
    var total = 0
    for (index, group) in groups.enumerated() where !group.isBlank {
      count += 1
      let at = "第 \(index + 1) 组"
      let title = trim(group.title)
      if title.isEmpty { return "\(at)还没有填写标题。" }
      if bytes(title) > maximumGroupTitleBytes || hasControl(title) { return "\(at)的标题太长了。" }
      if !titles.insert("\(group.tab)\n\(title)").inserted { return "\(at)的标题和同一页的另一组重复了。" }
      if bytes(trim(group.keywords)) > maximumKeywordsBytes || hasControl(group.keywords) { return "\(at)的搜索关键词太长了。" }
      let items = groupItems(group.items)
      if items.isEmpty { return "\(at)还没有符号。" }
      if items.count > maximumGroupItems { return "\(at)最多 \(maximumGroupItems) 项。" }
      total += items.count
      var seen = Set<String>()
      for item in items {
        if utf16(item) > maximumItemUTF16 || hasControl(item) { return "\(at)的「\(item)」太长了。" }
        if !seen.insert(item).inserted { return "\(at)里「\(item)」重复了。" }
      }
    }
    if count == 0 { return "至少要有一组符号。" }
    if count > maximumGroups { return "一个符号集最多 \(maximumGroups) 组。" }
    if total > maximumSymbolItems { return "一个符号集合计最多 \(maximumSymbolItems) 项。" }
    return nil
  }

  private static func wordsIssue(_ rows: [Word]) -> String? {
    var seen = Set<String>()
    var count = 0
    for (index, row) in rows.enumerated() where !row.isBlank {
      count += 1
      let at = "第 \(index + 1) 个单词"
      let word = trim(row.word)
      let phonetic = trim(row.phonetic)
      let gloss = trim(row.gloss)
      if word.isEmpty { return "\(at)还没有填写单词。" }
      if characters(word) > maximumWordCharacters || hasControl(word) { return "\(at)太长了。" }
      if word.hasPrefix("#") { return "\(at)不能以 # 开头。" }
      if characters(phonetic) > maximumPhoneticCharacters || hasControl(phonetic) { return "\(at)的音标太长了。" }
      if gloss.isEmpty { return "\(at)还没有填写释义。" }
      if characters(gloss) > maximumGlossCharacters || hasControl(gloss) { return "\(at)的释义太长了。" }
      if !seen.insert(word).inserted { return "「\(word)」出现了不止一次。" }
    }
    if count == 0 { return "至少要有一个单词。" }
    if count > maximumWords { return "一本单词本最多 \(maximumWords) 个单词。" }
    return nil
  }

  /// 指令模板里的占位符是否都认得：`{date}`、`{time}`（都可以带 `:格式`）和 `{weekday}`，花括号成对且不嵌套。格式本身对不对、展开后会不会太长由宿主检查。
  static func templatePlaceholdersKnown(_ template: String) -> Bool {
    var rest = Substring(template)
    while let open = rest.firstIndex(where: { $0 == "{" || $0 == "}" }) {
      if rest[open] == "}" { return false }
      let after = rest[rest.index(after: open)...]
      guard let close = after.firstIndex(where: { $0 == "{" || $0 == "}" }), after[close] == "}" else { return false }
      let placeholder = after[..<close]
      let known = placeholder == "weekday" || ["date", "time"].contains { name in
        placeholder == name || placeholder.hasPrefix(name + ":")
      }
      if !known { return false }
      rest = after[after.index(after: close)...]
    }
    return true
  }

  // MARK: - 批量添加

  /// 批量粘贴的结果：能读出的行，以及第一行读不出的原因。
  struct Bulk<Row> {
    var rows: [Row]
    var issue: String?
  }

  /// 批量添加短语：每行「编码 文字」，编码和文字之间用空格或 Tab 隔开，文字里可以再有空格；空行跳过。
  static func parsePhraseLines(_ text: String) -> Bulk<Phrase> {
    var rows: [Phrase] = []
    for (index, line) in lines(text).enumerated() where !blank(line) {
      let body = trim(line)
      guard let gap = body.firstIndex(where: \.isWhitespace) else {
        return Bulk(rows: rows, issue: "第 \(index + 1) 行要写成「编码 文字」，中间用空格隔开。")
      }
      rows.append(Phrase(key: String(body[..<gap]), text: trim(String(body[gap...]))))
    }
    return Bulk(rows: rows, issue: nil)
  }

  /// 批量添加单词：每行「单词 Tab 释义」或「单词 Tab 音标 Tab 释义」，从表格复制过来就是这样；空行和 # 开头的行跳过，与单词本的词表文件同一种写法。
  static func parseWordLines(_ text: String) -> Bulk<Word> {
    var rows: [Word] = []
    for (index, line) in lines(text).enumerated() where !blank(line) && !line.hasPrefix("#") {
      let columns = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
      switch columns.count {
      case 2: rows.append(Word(word: columns[0], phonetic: "", gloss: columns[1]))
      case 3: rows.append(Word(word: columns[0], phonetic: columns[1], gloss: columns[2]))
      default:
        return Bulk(rows: rows, issue: "第 \(index + 1) 行要写成「单词、释义」两列或「单词、音标、释义」三列，列之间用 Tab 隔开。")
      }
    }
    return Bulk(rows: rows, issue: nil)
  }

  /// 把批量读出的行接到已有的行后面；已有的只剩一个空行时直接替换它。
  static func appendRows<Row>(_ current: [Row], _ added: [Row], isBlank: (Row) -> Bool) -> [Row] {
    let kept = current.count == 1 && isBlank(current[0]) ? [] : current
    return kept + added
  }

  // MARK: - 保存之后

  /// 刚保存的包详情页顶上的一句话：短语表和指令表还要打开「启用」，K 模式或 / 指令关着时也要打开，别的类型装上就能用。打开模式的说法换成 iOS 的入口。
  static func saveNote(kind: PluginKind, enabled: Bool, modeOn: Bool) -> String {
    switch kind {
    case .phraseTable:
      if enabled {
        return modeOn ? "已保存，修改已经生效。" : "已保存。快捷短语（K 模式）现在关着，在「输入 → 快捷模式」打开后才会出候选。"
      }
      return modeOn
        ? "已保存。打开下面的「启用」后，\(PluginCatalogPolicy.localModeEntry("快捷短语"))再输入编码就能用到这些短语。"
        : "已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开快捷短语（K 模式），这些短语才会出候选。"
    case .commandTable:
      if enabled {
        return modeOn ? "已保存，修改已经生效。" : "已保存。/ 指令现在关着，在「输入 → 快捷模式」打开后才能用。"
      }
      return modeOn
        ? "已保存。打开下面的「启用」后，按 / 再输入指令字母就能用。"
        : "已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开 / 指令，这些指令才能用。"
    case .symbolSet: return "已保存，符号面板里会显示这些符号。"
    case .wordbook: return "已保存，这本书出现在背单词的词书里。"
    default: return "已保存。"
    }
  }

  // MARK: - 文本工具：与 JavaScript 的 `trim`、`length`（UTF-16）、`[...s].length`（码点）、`\p{Cc}` 相同

  static func trim(_ value: String) -> String { value.trimmingCharacters(in: .whitespacesAndNewlines) }
  static func blank(_ value: String) -> Bool { trim(value).isEmpty }
  private static func optional(_ value: String) -> Any { blank(value) ? NSNull() : trim(value) }
  private static func bytes(_ value: String) -> Int { value.utf8.count }
  private static func utf16(_ value: String) -> Int { value.utf16.count }
  private static func characters(_ value: String) -> Int { value.unicodeScalars.count }
  private static func hasControl(_ value: String) -> Bool {
    value.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
  private static func lowercaseLetters(_ value: String) -> Bool {
    !value.isEmpty && value.unicodeScalars.allSatisfy { ("a"..."z").contains($0) }
  }
  private static func validLicense(_ value: String) -> Bool {
    value.unicodeScalars.allSatisfy { scalar in
      scalar.isASCII && (CharacterSet.alphanumerics.contains(scalar) || ".-+() ".unicodeScalars.contains(scalar))
    }
  }
  /// 按 `\r?\n` 分行，与 `split(/\r?\n/)` 相同。
  private static func lines(_ text: String) -> [String] {
    text.components(separatedBy: "\n").map { $0.hasSuffix("\r") ? String($0.dropLast()) : $0 }
  }
}
