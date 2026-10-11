import Foundation

/// 五笔查码（#6789）的回答：词库请求 `{operation:"wubi_codes", text}`（host-api `wubi_codes_json`），与共享设置页 `wubi-code-lookup-section.tsx` 读的是同一个形状，排法和文字也照它。
///
/// `phrase` 是整段文字作为一个词的编码（整段不是 2 到 16 个汉字、或两张表都没有它时为空），`characters` 是每个不同的汉字，按第一次出现的顺序；编码各自短码在前。码表里没有的一列是空数组。
struct WubiCodeLookup: Equatable, Sendable {
  struct Codes: Equatable, Sendable {
    let text: String
    let wubi86: [String]
    let wubi98: [String]
  }

  enum Failure: Error {
    case invalidResponse
  }

  /// 输入框最多收的字符数；宿主拒绝超过 4096 字节的文字，一千个汉字（扩展区的字每个 4 字节）也在这之内。
  static let maximumLength = 1000
  static let description = "输入或粘贴汉字，列出每个字的 86 和 98 五笔编码，简码在前、全码在后。打字时也能查：五笔拼音混输关闭时先按 z 再打拼音，候选后面标着它的五笔编码。"
  static let failed = "查码失败，请稍后重试。"
  static let nothing = "没有可以查的汉字。"

  let phrase: Codes?
  let characters: [Codes]
  let truncated: Bool

  /// 结果表的行：整段词在前，再逐字列出。
  var rows: [Codes] { (phrase.map { [$0] } ?? []) + characters }

  static func decode(_ value: [String: Any]) throws -> WubiCodeLookup {
    guard let characters = value["characters"] as? [Any], let truncated = value["truncated"] as? Bool else {
      throw Failure.invalidResponse
    }
    let phrase: Codes?
    switch value["phrase"] {
    case nil, is NSNull: phrase = nil
    case let item?: phrase = try codes(item)
    }
    return WubiCodeLookup(phrase: phrase, characters: try characters.map(codes), truncated: truncated)
  }

  private static func codes(_ value: Any) throws -> Codes {
    guard let item = value as? [String: Any], let text = item["text"] as? String,
          let wubi86 = item["wubi86"] as? [String], let wubi98 = item["wubi98"] as? [String] else {
      throw Failure.invalidResponse
    }
    return Codes(text: text, wubi86: wubi86, wubi98: wubi98)
  }

  /// 输入框里有不是空白的字才查。
  static func searchable(_ text: String) -> Bool {
    !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  /// 列标题：当前所选的码表后面标「（当前）」，不认识的版本按 86，与键盘相同。
  static func columnTitle(_ column: String, profile: String) -> String {
    let current = profile == "wubi98" ? "wubi98" : "wubi86"
    let title = column == "wubi98" ? "98 五笔" : "86 五笔"
    return column == current ? "\(title)（当前）" : title
  }

  /// 一列的编码，空格分开；码表里没有时是「—」。
  static func codes(_ list: [String]) -> String {
    list.isEmpty ? "—" : list.joined(separator: " ")
  }

  /// 读屏念的一行：文字和两列编码。
  static func spoken(_ item: Codes, profile: String) -> String {
    "\(item.text)，\(columnTitle("wubi86", profile: profile)) \(codes(item.wubi86))，\(columnTitle("wubi98", profile: profile)) \(codes(item.wubi98))"
  }

  /// 不同的字太多、后面没有列出时的提示。
  func truncationNotice() -> String? {
    truncated ? "字太多，只列出了前 \(characters.count) 个不同的字。" : nil
  }
}
