import Foundation
import CoreFoundation

// A versioned envelope for user-entered records. Linguistic validation and normalization
// remain in the public Engine API; this is only the file transport used by the host UI.
struct PersonalDictionaryImport: Codable, Sendable {
  var format = "msime-personal-dictionary"
  var version = 1
  var entries: [PersonalWord]
  static let maximumBytes = 1_048_576

  struct ImportError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
  }

  static func decode(_ data: Data) throws -> Self {
    guard data.count <= maximumBytes else { throw ImportError(message: "文件不能超过 1 MB。") }
    guard let file = try? JSONDecoder().decode(Self.self, from: data),
          file.format == "msime-personal-dictionary", file.version == 1 else {
      throw ImportError(message: "文件格式不支持，请按示例 JSON 文件填写。")
    }
    guard !file.entries.isEmpty, file.entries.count <= 128 else {
      throw ImportError(message: "每次导入需要 1–128 条词条，请将较大的词库拆分后导入。")
    }
    var entries: [PersonalWord] = []
    var identities = Set<String>()
    for (index, entry) in file.entries.enumerated() {
      let word: PersonalWord
      do { word = try entry.validated() } catch {
        throw ImportError(message: "第 \(index + 1) 条：\(error.localizedDescription)")
      }
      guard identities.insert(word.id).inserted else {
        throw ImportError(message: "第 \(index + 1) 条与前面的词条重复，请删除重复项后重试。")
      }
      entries.append(word)
    }
    return Self(entries: entries)
  }

  /// Plain Chinese words, one per line, annotated with pinyin by the shared Engine. Repeated words collapse to their first line; the queue takes at most 128 at a time, the same as a JSON import.
  static func hans(_ text: String,
                   annotate: (String) throws -> [PersonalWord] = { try PersonalDictionaryBridge.hansEntries($0) }) throws -> Self {
    guard text.utf8.count <= maximumBytes else { throw ImportError(message: "词表不能超过 1 MB。") }
    var identities = Set<String>()
    let entries = try annotate(text).filter { identities.insert($0.id).inserted }
    guard !entries.isEmpty, entries.count <= 128 else {
      throw ImportError(message: "每次导入需要 1–128 个词语，请将较大的词表拆分后导入。")
    }
    return Self(entries: entries)
  }

  /// The file layouts the desktop settings page imports, by the name the shared Engine takes.
  static let fileFormats: [(format: String, title: String)] = [
    ("standard", "词在前（标准）"),
    ("windows", "编码在前（Windows）"),
    ("rime", "Rime userdb / dict.yaml"),
  ]

  /// A dictionary file in one of `fileFormats`, parsed by the shared Engine the way the desktop settings page parses it. Rows the Engine refuses are skipped and a file longer than the queue is queued in part, so a real file still imports; `notice` says what was left out, in the desktop's words.
  static func file(
    _ text: String, kind: PersonalWordKind, format: String,
    parse: (String, String, String) throws -> (entries: [PersonalWord], report: [String: Any]) = {
      try PersonalDictionaryBridge.importEntries(kind: $0, format: $1, text: $2)
    }
  ) throws -> (file: Self, notice: String) {
    guard text.utf8.count <= maximumBytes else { throw ImportError(message: "文件不能超过 1 MB。") }
    let parsed: (entries: [PersonalWord], report: [String: Any])
    do { parsed = try parse(kind.bridgeName, format, text) } catch DictionaryFileImportFailure.rejected {
      throw ImportError(message: "文件里没有能导入的\(kind.title)词条，请确认词库类型和文件格式。")
    } catch DictionaryFileImportFailure.unavailable {
      throw ImportError(message: "键盘词典暂时无法读取，请稍后再试。")
    }
    guard !parsed.entries.isEmpty, parsed.entries.count <= 128 else {
      throw ImportError(message: "每次导入需要 1–128 条词条，请将较大的词库拆分后导入。")
    }
    return (Self(entries: parsed.entries), notice(parsed.report))
  }

  /// Mirrors `describeImportResult` in the shared settings page, minus the count the preview already shows.
  static func notice(_ report: [String: Any]) -> String {
    var parts: [String] = []
    let failed = integer(report["failed"]) ?? 0
    if failed > 0 {
      let failures = report["first_failures"] as? [[String: Any]] ?? []
      let lines = failures.compactMap { integer($0["line"]) }.map(String.init).joined(separator: "、")
      parts.append(lines.isEmpty ? "跳过 \(failed) 行。" : "跳过 \(failed) 行，首先出现在第 \(lines) 行。")
      if failures.contains(where: { $0["issue"] as? String == "rejected" }) {
        parts.append("其中部分行的编码与词不匹配，例如简拼、或音节数与汉字数不一致。")
      }
    }
    if report["truncated"] as? Bool == true { parts.append("文件过长，仅导入前 128 条，其余请拆分后再导入。") }
    if report["swapped"] as? Bool == true { parts.append("该文件的两列与所选格式相反，已按文件本身的顺序读取。") }
    return parts.joined()
  }

  private static func integer(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = Int(number.stringValue),
          integer >= 0,
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }

  static func read(from url: URL) throws -> Self {
    try decode(readData(from: url))
  }

  /// A UTF-8 text file, for `hans` and `file`.
  static func readText(from url: URL) throws -> String {
    guard let text = String(data: try readData(from: url), encoding: .utf8) else {
      throw ImportError(message: "词表需要是 UTF-8 编码的文本文件。")
    }
    return text
  }

  private static func readData(from url: URL) throws -> Data {
    let granted = url.startAccessingSecurityScopedResource()
    defer { if granted { url.stopAccessingSecurityScopedResource() } }
    var coordinationError: NSError?
    var result: Result<Data, Error>?
    // File providers may need to materialize a cloud document before it can be read.
    // Create and use this coordinator on the worker thread, never on the UI thread.
    NSFileCoordinator().coordinate(readingItemAt: url, options: [], error: &coordinationError) { readableURL in
      result = Result {
        let handle = try FileHandle(forReadingFrom: readableURL)
        defer { try? handle.close() }
        var data = Data()
        while data.count <= maximumBytes {
          let chunk = try handle.read(upToCount: min(65_536, maximumBytes + 1 - data.count)) ?? Data()
          if chunk.isEmpty { break }
          data.append(chunk)
        }
        guard data.count <= maximumBytes else { throw ImportError(message: "文件不能超过 1 MB。") }
        return data
      }
    }
    if let coordinationError { throw coordinationError }
    guard let result else { throw ImportError(message: "无法读取所选文件，请重新选择。") }
    return try result.get()
  }

  static let example = Self(entries: [
    PersonalWord(key: "ni hao", value: "你好"),
    PersonalWord(kind: .wubi, key: "wq", value: "你"),
    PersonalWord(kind: .english, key: "hello", value: "Hello"),
    PersonalWord(kind: .quickPhrase, key: "greeting", value: "你好！\n很高兴认识你。")
  ])

  func encoded() throws -> Data {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    return try encoder.encode(self)
  }
}
