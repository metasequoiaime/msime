import Foundation

private typealias MSIMEVocabularyByte = UInt8

@_silgen_name("msime_client_vocabulary_review")
private func msimeClientVocabularyReview(
  _ request: UnsafePointer<MSIMEVocabularyByte>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientVocabularyStringFree(_ value: UnsafeMutablePointer<CChar>?)

struct VocabularyWordbook: Equatable, Identifiable {
  var id: String
  var name: String
  var total: Int
  /// A bundled book cannot be deleted; an imported one can.
  var builtin: Bool
}

struct VocabularyCard: Equatable {
  var word: String
  /// May be empty: a user's own list often carries no transcription.
  var phonetic: String
  var meaning: String
}

/// Everything the page draws, as one answer.
///
/// The shared layer returns the whole status from every action, so the page never follows a change
/// with a read of its own.
struct VocabularyReviewStatus: Equatable {
  var wordbooks: [VocabularyWordbook] = []
  var wordbook = ""
  var newPerDay = 0
  var sessionLimit = 0
  /// 今日待复习.
  var due = 0
  /// 已完成.
  var answeredToday = 0
  var introducing = 0
  var remaining = 0
  var queue: [VocabularyCard] = []

  var current: VocabularyCard? { queue.first }

  /// Whether the page should ask for a book rather than deal a card.
  ///
  /// Distinct from an empty queue: with no book chosen there is nothing to offer, while an empty
  /// queue with a book chosen means the day is finished. Telling someone who has never picked a
  /// book that they have finished would be a lie.
  var needsWordbook: Bool { wordbook.isEmpty || !wordbooks.contains { $0.id == wordbook } }
}

/// 背单词 storage, through the shared entry point.
///
/// Every rule — the intervals, the ease, what a lapse costs, how the queue is ordered — belongs to
/// `client-core::vocabulary`. Hand-rolling any of it in Swift would give this host a second
/// scheduler that quietly disagrees with the five others, which is what the shared layer exists to
/// prevent. This type only moves JSON across the boundary.
struct VocabularyReviewStore {
  static let maximumImportBytes = 8 * 1024 * 1024

  let root: URL
  /// The verified Engine resources, whose `wordbooks/` sibling would hold bundled books. The shared entry point requires an absolute path, so a host without packaged resources passes its own directory, where no bundled book is found and only imported books are offered.
  let resources: URL

  init(directory: URL? = nil, resources: URL? = PersonalDictionaryBridge.packagedResources) {
    root = directory ?? FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: InputSchemePreference.appGroupIdentifier)?
      .appendingPathComponent("MSIME", isDirectory: true)
      ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    self.resources = resources ?? root
  }

  func load() throws -> VocabularyReviewStatus { try call(["operation": "load"]) }

  func answer(_ word: String, known: Bool) throws -> VocabularyReviewStatus {
    try call(["operation": "answer", "word": word, "known": known])
  }

  func setSettings(wordbook: String, newPerDay: Int, sessionLimit: Int)
    throws -> VocabularyReviewStatus
  {
    try call([
      "operation": "set_settings",
      "wordbook": wordbook,
      "new_per_day": newPerDay,
      "session_limit": sessionLimit,
    ])
  }

  func importWordbook(name: String, text: String) throws -> VocabularyReviewStatus {
    try call(["operation": "import", "name": name, "text": text])
  }

  func removeWordbook(_ wordbook: String) throws -> VocabularyReviewStatus {
    try call(["operation": "remove", "wordbook": wordbook])
  }

  func reset() throws -> VocabularyReviewStatus { try call(["operation": "reset"]) }

  /// Read a picked wordbook without allocating more than the shared import limit.
  static func readWordbookData(from url: URL) throws -> Data {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var data = Data()
    data.reserveCapacity(min(maximumImportBytes, 64 * 1024))
    while data.count <= maximumImportBytes {
      let chunk = try handle.read(
        upToCount: min(64 * 1024, maximumImportBytes + 1 - data.count)) ?? Data()
      if chunk.isEmpty { return data }
      data.append(chunk)
    }
    throw Failure.unreadableWordbook
  }

  /// The device's local day, as the shared layer spells one.
  ///
  /// Resolved here because this process is the one that knows the device's calendar and timezone;
  /// the shared layer never derives a day from either.
  static func today(_ date: Date = Date(), calendar: Calendar = .current) -> String {
    let parts = calendar.dateComponents([.year, .month, .day], from: date)
    return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
  }

  private func call(_ action: [String: Any]) throws -> VocabularyReviewStatus {
    let request = try JSONSerialization.data(withJSONObject: [
      "directory": root.path,
      "resources": resources.path,
      "day": Self.today(),
      "action": action,
    ])
    let pointer = request.withUnsafeBytes { bytes in
      msimeClientVocabularyReview(
        bytes.bindMemory(to: MSIMEVocabularyByte.self).baseAddress, UInt(request.count))
    }
    guard let pointer else { throw Failure.unavailable }
    let response = String(cString: pointer)
    msimeClientVocabularyStringFree(pointer)
    guard let data = response.data(using: .utf8),
          let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any]
    else { throw Failure.unavailable }
    guard envelope["ok"] as? Bool == true, let value = envelope["value"] as? [String: Any] else {
      throw Self.failure(for: envelope["error"] as? String ?? "")
    }
    protectSharedState()
    return Self.status(from: value)
  }

  /// The shared error string, turned into something a user can act on.
  ///
  /// A file that turned out not to be a word list is its own answer: reporting it as a storage
  /// failure would read as the application being broken rather than the file being wrong.
  static func failure(for error: String) -> Failure {
    if error.contains("import") { return .unreadableWordbook }
    if error.contains("library is full") { return .libraryFull }
    if error.contains("not in the library") { return .missingWordbook }
    return .unavailable
  }

  static func status(from value: [String: Any]) -> VocabularyReviewStatus {
    var status = VocabularyReviewStatus()
    if let books = value["wordbooks"] as? [[String: Any]] {
      status.wordbooks = books.compactMap { book in
        guard let id = book["id"] as? String, !id.isEmpty else { return nil }
        return VocabularyWordbook(
          id: id,
          name: book["name"] as? String ?? id,
          total: book["total"] as? Int ?? 0,
          builtin: book["builtin"] as? Bool ?? false)
      }
    }
    if let settings = value["settings"] as? [String: Any] {
      status.wordbook = settings["wordbook"] as? String ?? ""
      status.newPerDay = settings["newPerDay"] as? Int ?? 0
      status.sessionLimit = settings["sessionLimit"] as? Int ?? 0
    }
    status.due = value["due"] as? Int ?? 0
    status.answeredToday = value["answeredToday"] as? Int ?? 0
    status.introducing = value["introducing"] as? Int ?? 0
    status.remaining = value["remaining"] as? Int ?? 0
    if let cards = value["queue"] as? [[String: Any]] {
      status.queue = cards.compactMap { card in
        // A card with no word could never be answered: the answer is keyed by it.
        guard let word = card["word"] as? String, !word.isEmpty else { return nil }
        return VocabularyCard(
          word: word,
          phonetic: card["phonetic"] as? String ?? "",
          meaning: card["meaning"] as? String ?? "")
      }
    }
    return status
  }

  private func protectSharedState() {
    guard FileManager.default.fileExists(atPath: root.path) else { return }
    var directory = root
    var values = URLResourceValues()
    values.isExcludedFromBackup = true
    try? directory.setResourceValues(values)
  }

  enum Failure: Error, LocalizedError, Equatable {
    case unavailable, unreadableWordbook, libraryFull, missingWordbook
    var errorDescription: String? {
      switch self {
      case .unavailable: "背单词进度无法读取或保存；已有的进度不会被自动清空。"
      case .unreadableWordbook: "这个文件不是可用的词表，请检查每行是否为「单词,释义」。"
      case .libraryFull: "词表数量已达上限，请先删除一个再导入。"
      case .missingWordbook: "这本词表已不在词库中，请重新选择。"
      }
    }
  }
}
