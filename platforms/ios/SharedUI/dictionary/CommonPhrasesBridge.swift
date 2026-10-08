import Foundation

@_silgen_name("msime_client_common_phrases")
private func msimeClientCommonPhrases(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeClientCommonPhrasesStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 键盘常用语面板列出的无编码常用语：与 Android 的 `CommonPhrasesStore` 用同一个 `CommonPhrases.json` 存储和同一个 `msime_client_common_phrases` ABI。
///
/// 存储放在共享偏好文档所在的 App Group 目录里，app 和键盘读的是同一个文件。每次调用都会拿存储的文件锁并读文件，所以要在后台队列上调，绝不能在主线程上调；键盘每次打开常用语面板都会重新读一遍。
///
/// 键盘只读不写。app 的常用语页（`CommonPhrasesSettingsView`）通过 `add` 和 `remove` 增删常用语；有完全访问权限时，键盘下次打开面板就能读到它最后写入的内容。没有完全访问权限时键盘写不了 App Group（见 `DiagnosticLog`），而存储即使只是读，也要以写方式打开文档旁边的 `CommonPhrases.json.lock`，所以这时读取可能以 `common_phrases_io` 失败；面板就会说常用语读不出来，而不是显示一份它其实没有的列表。
enum CommonPhrasesBridge {
  /// 一条常用语。用户自己添加的 `pack` 为 nil，否则是它所属社区常用语包的 id。
  struct Phrase: Decodable, Identifiable, Equatable {
    let id: String
    let text: String
    let pack: String?
  }

  enum Failure: Error, Equatable {
    /// 无法解析出 App Group 目录，或者原生侧没有给出能解析的回复。
    case unavailable
    /// 存储返回了它的某个稳定错误码（`common_phrases_io`、`common_phrases_corrupt`……）。
    case rejected(String)
  }

  /// ABI 外层结构：`{"ok": true, "value": {phrases, packs, skipped?}}` 或 `{"ok": false, "error": code}`。这里只读 phrases；packs 和跳过计数是给负责安装常用语包的设置端用的。
  private struct Reply: Decodable {
    struct Document: Decodable {
      let phrases: [Phrase]
    }

    let ok: Bool
    let value: Document?
    let error: String?
  }

  /// 按面板顺序排列的常用语。`directory` 是共享状态目录；测试传入自己的目录。
  static func load(directory: String = MetasequoiaInputSessionBridge.sharedStateDirectory) throws -> [Phrase] {
    try perform(["operation": "load"], directory: directory)
  }

  /// 追加一条用户输入的常用语，返回追加后存储里的列表。存储会拒绝空白文本、超过 1000 个 UTF-16 单元的文本、重复项，以及用户自建的第 201 条，各自带稳定错误码。
  static func add(_ text: String, directory: String = MetasequoiaInputSessionBridge.sharedStateDirectory) throws -> [Phrase] {
    try perform(["operation": "add", "text": text], directory: directory)
  }

  /// 按 id 删除一条常用语，返回删除后存储里的列表。
  static func remove(id: String, directory: String = MetasequoiaInputSessionBridge.sharedStateDirectory) throws -> [Phrase] {
    try perform(["operation": "remove", "id": id], directory: directory)
  }

  /// 操作失败时设置页显示的文案，措辞与 Android 的 `CommonPhrasesStore.failureMessage` 一致。
  static func message(for error: Error) -> String {
    switch error as? Failure {
    case .rejected("common_phrases_invalid"): "常用语不能为空，最多 1000 字。"
    case .rejected("common_phrases_duplicate"): "已经有这条常用语了。"
    case .rejected("common_phrases_limit"): "常用语已达上限（自己添加的最多 200 条），请先删掉一些。"
    case .rejected("common_phrases_too_large"): "常用语太多，存不下了，请先删掉一些。"
    case .rejected("common_phrases_not_found"): "这条常用语已经不在了，列表已刷新。"
    case .rejected("common_phrases_corrupt"): "常用语文件已损坏，为避免丢失没有改动它。"
    case .unavailable: "词库还没准备好，请先完成首次设置。"
    default: "常用语保存失败，请稍后重试。"
    }
  }

  private static func perform(_ action: [String: Any], directory: String) throws -> [Phrase] {
    guard directory.hasPrefix("/") else { throw Failure.unavailable }
    let request = try JSONSerialization.data(withJSONObject: [
      "directory": directory,
      "action": action,
    ])
    let pointer = request.withUnsafeBytes { bytes in
      msimeClientCommonPhrases(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(request.count))
    }
    guard let pointer else { throw Failure.unavailable }
    let response = Data(String(cString: pointer).utf8)
    msimeClientCommonPhrasesStringFree(pointer)
    guard let reply = try? JSONDecoder().decode(Reply.self, from: response) else { throw Failure.unavailable }
    guard reply.ok else { throw Failure.rejected(reply.error ?? "") }
    guard let document = reply.value else { throw Failure.unavailable }
    return document.phrases
  }
}
