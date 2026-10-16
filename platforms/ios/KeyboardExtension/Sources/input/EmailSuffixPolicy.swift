import Foundation

/// 邮箱后缀：光标前打到 `xxx@`（或 `xxx@` 后面接着打了域名的开头）时，候选栏列出能接上的常用后缀，点一下补全。规则与 Android 的 `EmailSuffixPolicy` 逐条相同。
///
/// 只看光标前的文字：末尾要是 `<本地部分>@<已打的域名>`，紧挨 `@` 的那一个字符必须是 ASCII 字母、数字或 `._%+-`；已打的域名可以为空，只含 ASCII 字母、数字、`.` 和 `-`。以最后一个 `@` 为准。给出以已打域名开头（不分大小写）的后缀；已打的域名已经是一个完整后缀时不再给。全角的 `＠` 不算。与 Android 只在邮箱输入框里给不同，iOS 在任何输入框里都给，由宿主决定何时读上文。
enum EmailSuffixPolicy {
  /// 最多看光标前多少个字符，与 Android 的 `MAX_CONTEXT` 相同。
  static let maxContext = 64

  /// 常用邮箱后缀：国内常用的在前，然后是海外的。与 Android 的 `EmailSuffixPolicy.SUFFIXES` 和引擎网址模式的 `EMAIL_SUFFIXES`（`crates/engine/src/local/url.rs`）是同一份、同一个顺序，`scripts/test-email-suffixes.py` 核对各处一致，所以保持为一行字面量。
  static let suffixes = ["@qq.com", "@163.com", "@126.com", "@foxmail.com", "@139.com", "@189.cn", "@aliyun.com", "@gmail.com", "@outlook.com", "@hotmail.com", "@live.com", "@icloud.com", "@yahoo.com", "@proton.me", "@protonmail.com", "@aol.com", "@mail.com", "@gmx.com", "@naver.com", "@yahoo.co.jp"]

  /// 光标前已经打到 `@` 之后的一段：`typedDomain` 是 `@` 之后已经打的域名（原样，可能为空），`suffixes` 是以它开头的后缀，按 `suffixes` 的顺序，至少一个。
  struct Match: Equatable {
    let typedDomain: String
    let suffixes: [String]
  }

  /// 选中一个后缀时对文档做的事：先删掉光标前 `deleteCount` 个字符（已经打的域名），再上屏 `insert`（后缀去掉 `@`，统一小写，始终是 ASCII）。
  struct Replacement: Equatable {
    let deleteCount: Int
    let insert: String
  }

  /// 光标前的文字末尾是不是一个打到 `@` 的邮箱；不是或没有可给的后缀时返回 nil。只看最后 `maxContext` 个字符。
  static func match(textBeforeCursor: String?) -> Match? {
    guard let textBeforeCursor else { return nil }
    let scalars = Array(textBeforeCursor.unicodeScalars.suffix(maxContext))
    var at = scalars.count
    while at > 0 && isDomainScalar(scalars[at - 1]) { at -= 1 }
    guard at > 0, scalars[at - 1] == "@" else { return nil }
    let atIndex = at - 1
    guard atIndex > 0, isLocalScalar(scalars[atIndex - 1]) else { return nil }
    var typed = String.UnicodeScalarView()
    typed.append(contentsOf: scalars[at...])
    let typedDomain = String(typed)
    let needle = "@" + typedDomain.lowercased()
    var found: [String] = []
    for suffix in suffixes {
      if suffix == needle { return nil }
      if suffix.hasPrefix(needle) { found.append(suffix) }
    }
    return found.isEmpty ? nil : Match(typedDomain: typedDomain, suffixes: found)
  }

  /// 在 `match` 处补全成 `suffix`；后缀不在这次的匹配里时返回 nil。
  static func replacement(_ match: Match?, suffix: String) -> Replacement? {
    guard let match, match.suffixes.contains(suffix) else { return nil }
    return Replacement(deleteCount: match.typedDomain.unicodeScalars.count, insert: String(suffix.dropFirst()).lowercased())
  }

  private static func isAsciiLetterOrDigit(_ scalar: Unicode.Scalar) -> Bool {
    switch scalar.value {
    case 0x30...0x39, 0x41...0x5A, 0x61...0x7A: return true
    default: return false
    }
  }

  private static func isDomainScalar(_ scalar: Unicode.Scalar) -> Bool {
    isAsciiLetterOrDigit(scalar) || scalar == "." || scalar == "-"
  }

  private static func isLocalScalar(_ scalar: Unicode.Scalar) -> Bool {
    isAsciiLetterOrDigit(scalar) || "._%+-".unicodeScalars.contains(scalar)
  }
}
