import Foundation

@_silgen_name("msime_client_personal_wordbook_add")
private func msimeClientPersonalWordbookAdd(
  _ request: UnsafePointer<UInt8>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientPersonalWordbookStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 候选长按菜单的「加入生词本」，经由共享入口。
///
/// 释义怎么变成卡片、同一个词怎么合并、生词本满了怎么办、给用户看哪句话，都在 `client-services::vocabulary::personal`；这里只把候选原文和显示着的释义交过去，再把它答的那句提示带回来。
enum PersonalWordbookStore {
  /// 交不出去或存储出错时的提示；其余提示都由共享层给出。
  static let failureNotice = "无法加入生词本"
  /// 长按菜单里这一项的标题。
  static let menuTitle = "加入生词本"

  /// 菜单要交给共享层的那段释义：显示着的几种语言各一行，去掉空行。一行可用的释义都没有时为 `nil`，菜单里就没有「加入生词本」。
  static func menuGloss(_ glosses: [String]) -> String? {
    let gloss = glosses
      .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }
      .joined(separator: "\n")
    return gloss.isEmpty ? nil : gloss
  }

  /// 写进 `directory`（默认与背单词同一个目录）下的生词本，返回给用户看的提示。取词书目录的文件锁、读写生词本，不要在主线程调用。
  static func add(
    candidate: String, gloss: String, pronunciation: String = "",
    directory: URL = VocabularyReviewStore.defaultRoot
  ) -> String {
    let request: [String: Any] = [
      "directory": directory.path, "candidate": candidate, "gloss": gloss, "pronunciation": pronunciation,
    ]
    guard let data = try? JSONSerialization.data(withJSONObject: request), data.count <= 65536 else {
      return failureNotice
    }
    let raw = data.withUnsafeBytes { bytes in
      msimeClientPersonalWordbookAdd(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(bytes.count))
    }
    guard let raw else { return failureNotice }
    defer { msimeClientPersonalWordbookStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any],
          envelope["ok"] as? Bool == true,
          let value = envelope["value"] as? [String: Any],
          let notice = value["notice"] as? String, !notice.isEmpty else {
      return failureNotice
    }
    return notice
  }
}
