import Foundation

/// The user's own candidate glosses, `custom_translations.txt` in the Engine's user directory.
///
/// Windows documents dropping the file into the profile directory: one entry per line, source and gloss separated by a Tab, `#` for a comment, and the last spelling of a source wins. The Engine reads it on every host, from the user directory before the resources, but an iPhone has no directory a person can reach, so the app edits the file in the App Group where the keyboard's Engine looks. The parsing rules mirror `packages/ui/src/dictionary/custom-translations.ts`, which mirrors `EnglishDictionary::load_custom_translations`: a line accepted here that the Engine drops would be one the page promised to apply and did not. The file rules mirror the desktop shell's `write_custom_translations_at`.
enum CustomTranslations {
  struct Report: Equatable {
    var entries: Int
    var chineseSources: Int
    /// Lines that carried something but could not be read as an entry.
    var skipped: Int
  }

  static let fileName = "custom_translations.txt"
  static let maximumBytes = 1024 * 1024
  static let maximumEntries = 20_000
  static let example = "# 每行一条，用 Tab 分隔源词和译文\n你好\thello\n刚才\ta moment ago; just now\nserendipity\t意外发现珍奇事物的本领\n"

  static func parse(_ text: String) -> Report {
    let body = text.hasPrefix("\u{FEFF}") ? String(text.dropFirst()) : text
    var sources: [String: Bool] = [:]
    var skipped = 0
    for raw in body.split(omittingEmptySubsequences: false, whereSeparator: { $0 == "\n" || $0 == "\r" || $0 == "\r\n" }) {
      let line = raw.trimmingCharacters(in: .whitespaces)
      if line.isEmpty || line.hasPrefix("#") { continue }
      // A leading tab leaves no source and a trailing one no gloss; the Engine drops both.
      guard let tab = line.firstIndex(of: "\t"), tab != line.startIndex, line.index(after: tab) != line.endIndex else {
        skipped += 1
        continue
      }
      let source = String(line[..<tab]).trimmingCharacters(in: .whitespaces)
      let gloss = line[line.index(after: tab)...].trimmingCharacters(in: .whitespaces)
      guard !source.isEmpty, !gloss.isEmpty else {
        skipped += 1
        continue
      }
      // Last wins, as the Engine's map assignment does; a source with any non-ASCII character is Chinese, which is how the Engine picks the direction.
      sources[source] = source.unicodeScalars.contains { $0.value > 0x7F }
      if sources.count > maximumEntries { break }
    }
    return Report(entries: sources.count, chineseSources: sources.values.filter { $0 }.count, skipped: skipped)
  }

  /// The file in the `user` directory under a state root, which is where `prepare_host_configuration` points the Engine.
  static func url(stateRoot: URL) -> URL {
    stateRoot.appendingPathComponent("user", isDirectory: true).appendingPathComponent(fileName)
  }

  /// The file under the keyboard's state root, `MSIME` in the App Group.
  static var defaultURL: URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.msime.ios")
      .map { url(stateRoot: $0.appendingPathComponent("MSIME", isDirectory: true)) }
  }

  enum Failure: LocalizedError {
    case tooLarge, unreadable, storage
    var errorDescription: String? {
      switch self {
      case .tooLarge: "释义文件超过 1 MB，或含有无法保存的字符。"
      case .unreadable: "无法读取释义文件，请确认是 UTF-8 文本。"
      case .storage: "无法保存自定义释义，请重试。"
      }
    }
  }

  /// The file as text without its BOM, or empty when there is none yet.
  static func read(at url: URL) throws -> String {
    let data: Data
    do {
      data = try BoundedFileReader.read(from: url, maximumBytes: maximumBytes)
    } catch CocoaError.fileReadNoSuchFile {
      return ""
    } catch BoundedFileReader.Failure.tooLarge {
      throw Failure.unreadable
    } catch {
      throw Failure.storage
    }
    guard data.count <= maximumBytes, let text = String(data: data, encoding: .utf8) else { throw Failure.unreadable }
    return text.hasPrefix("\u{FEFF}") ? String(text.dropFirst()) : text
  }

  /// Writes beside the target and moves it into place, so a failure leaves the previous file whole. An empty document removes the file, which is what "no overlay" means to the Engine.
  static func write(_ text: String, to url: URL) throws {
    guard text.utf8.count <= maximumBytes, !text.contains("\0") else { throw Failure.tooLarge }
    let manager = FileManager.default
    if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      do { try manager.removeItem(at: url) } catch CocoaError.fileNoSuchFile {} catch { throw Failure.storage }
      return
    }
    do {
      try manager.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
      try Data(text.utf8).write(to: url, options: .atomic)
    } catch { throw Failure.storage }
  }
}
