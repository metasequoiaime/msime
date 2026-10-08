import Foundation
import Darwin

enum PersonalWordKind: String, Codable, CaseIterable, Identifiable, Sendable {
  /// `wubi` 是 86 五笔的词条，`wubi98` 是 98 五笔的，两套码表的个人词条分开存。
  case pinyin, wubi, wubi98, quickPhrase, english
  var id: String { rawValue }
  var title: String {
    switch self {
    case .pinyin: return "拼音"
    case .wubi: return "86 五笔"
    case .wubi98: return "98 五笔"
    case .quickPhrase: return "快捷短语"
    case .english: return "英文"
    }
  }
}

/// Where a listed row comes from, as the Engine list says. Only `bundled` is kept: a user row is what every word without a source already is.
enum PersonalWordSource: String, Codable, Sendable {
  case bundled
}

struct PersonalWord: Codable, Hashable, Sendable, Identifiable {
  var kind: PersonalWordKind = .pinyin
  var key: String
  var value: String
  var weight: Int64 = Self.defaultWeight
  /// Set on a row the dictionary shipped or learned rather than one the user added. Its code and word are fixed, so it can only be re-weighted or deleted, and the edit goes back to the keyboard carrying this mark.
  var source: PersonalWordSource?
  var isBundled: Bool { source == .bundled }
  /// The weights the Engine stores (`validate_personal_dictionary_entry`); anything outside is refused.
  static let weightRange: ClosedRange<Int64> = 1...100_000_000
  static let defaultWeight: Int64 = 100_000
  // Length prefixes keep arbitrary phrase text from colliding with an input-code separator.
  var id: String { "\(kind.rawValue):\(key.utf8.count):\(key)\(value)" }
}

struct PersonalWordRequest: Codable, Identifiable, Sendable {
  enum Status: String, Codable, Sendable { case pending, applied, failed }
  // Keep this as a string because the Rust host queue accepts caller-owned
  // receipts (for example, an import prefix plus row index). Native Swift
  // callers still use UUID strings by default.
  var id = UUID().uuidString
  var previous: PersonalWord?
  var replacement: PersonalWord?
  var status: Status = .pending
  var error: String?
  var createdAt: Date? = Date()
}

struct PersonalDictionaryState: Codable, Sendable {
  // The Rust queue uses serde's camelCase conversion (`refreshId`), while Swift's conventional acronym spelling would otherwise encode `refreshID`. Keep the on-disk contract explicit so the Tauri host and keyboard can acknowledge the same refresh cycle instead of silently resetting it.
  enum CodingKeys: String, CodingKey {
    case version, requests, entries, hasMore, snapshotDate, snapshotError
    case pageOffset, requestedPageOffset, requestedKind, requestedQuery, pageKind, pageQuery
    case exportRequest, exportResult
    case refreshID = "refreshId"
    case completedRefreshID = "completedRefreshId"
  }
  var version = 1
  var requests: [PersonalWordRequest] = []
  var entries: [PersonalWord] = []
  var hasMore = false
  var snapshotDate: Date?
  var snapshotError: String?
  var pageOffset = 0
  var requestedPageOffset = 0
  /// The dictionary and code prefix the host asked for; the keyboard answers them from the user's whole store. `pageKind` and `pageQuery` describe the entries it last confirmed.
  var requestedKind: PersonalWordKind?
  var requestedQuery = ""
  var pageKind: PersonalWordKind?
  var pageQuery = ""
  var refreshID = UUID()
  var completedRefreshID: UUID?
  /// The export the host asked for, and the one the keyboard last wrote to `PersonalDictionaryStore.exportFile`. They match by `id` once the file is ready.
  var exportRequest: PersonalExportRequest?
  var exportResult: PersonalExportResult?
  var pendingCount: Int { requests.filter { $0.status == .pending }.count }

  init() {}

  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    version = try values.decodeIfPresent(Int.self, forKey: .version) ?? 1
    requests = try values.decodeIfPresent([PersonalWordRequest].self, forKey: .requests) ?? []
    entries = try values.decodeIfPresent([PersonalWord].self, forKey: .entries) ?? []
    hasMore = try values.decodeIfPresent(Bool.self, forKey: .hasMore) ?? false
    snapshotDate = try values.decodeIfPresent(Date.self, forKey: .snapshotDate)
    snapshotError = try values.decodeIfPresent(String.self, forKey: .snapshotError)
    pageOffset = try values.decodeIfPresent(Int.self, forKey: .pageOffset) ?? 0
    requestedPageOffset = try values.decodeIfPresent(Int.self, forKey: .requestedPageOffset) ?? 0
    requestedKind = try values.decodeIfPresent(PersonalWordKind.self, forKey: .requestedKind)
    requestedQuery = try values.decodeIfPresent(String.self, forKey: .requestedQuery) ?? ""
    pageKind = try values.decodeIfPresent(PersonalWordKind.self, forKey: .pageKind)
    pageQuery = try values.decodeIfPresent(String.self, forKey: .pageQuery) ?? ""
    refreshID = try values.decodeIfPresent(UUID.self, forKey: .refreshID) ?? UUID()
    completedRefreshID = try values.decodeIfPresent(UUID.self, forKey: .completedRefreshID)
    exportRequest = try values.decodeIfPresent(PersonalExportRequest.self, forKey: .exportRequest)
    exportResult = try values.decodeIfPresent(PersonalExportResult.self, forKey: .exportResult)
  }
}

/// One dictionary written out in a shared text layout, as the desktop settings page exports it.
///
/// The host cannot open the Engine's dictionary while the keyboard may hold it, so the keyboard writes the file the next time it synchronizes, the same way it applies edits.
struct PersonalExportRequest: Codable, Equatable, Sendable {
  static let formats = ["standard", "windows"]
  var id = UUID()
  var kind: PersonalWordKind
  var format: String
  /// The name the desktop settings page gives the same export.
  var fileName: String { "水杉IME-\(kind.title)用户词库.txt" }
}

struct PersonalExportResult: Codable, Equatable, Sendable {
  var request: PersonalExportRequest
  var rows = 0
  /// The dictionary had more rows than one export writes.
  var truncated = false
  var error: String?
  var date = Date()
}

/// What the keyboard's Engine returned for an export request.
struct PersonalExportText: Sendable {
  var text: String
  var complete: Bool
}

/// One page of the user's own words, optionally within one dictionary and under one code prefix.
struct PersonalPageRequest: Equatable, Sendable {
  var offset: Int
  var kind: PersonalWordKind?
  var query: String
}

struct PersonalWordPage: Sendable {
  var entries: [PersonalWord]
  var hasMore: Bool
}

// This file is local host/keyboard transport, not the dictionary database. The Engine owns
// validation, candidate changes and durable edit receipts. Call keyboard synchronization only
// with Full Access and with no active input session touching the dictionaries.
final class PersonalDictionaryStore: @unchecked Sendable {
  enum StoreError: LocalizedError {
    case unavailable, invalidState, busy, tooManyRequests, conflict, exportTooLarge
    var errorDescription: String? {
      switch self {
      case .unavailable: return "无法访问个人词库共享目录。"
      case .invalidState: return "个人词库同步文件无法读取，已保留原文件。"
      case .busy: return "个人词库正在同步，请稍后重试。"
      case .tooManyRequests: return "等待同步的操作过多，请先打开键盘完成同步。"
      case .conflict: return "这个词条已有等待同步的操作，请同步后再编辑。"
      case .exportTooLarge: return "词库超过 8 MB，键盘无法一次导出。"
      }
    }
  }
  private let directory: URL?
  private static let processLock = NSLock()
  private let maximumBytes = 8 * 1024 * 1024
  /// The code prefix a page may be filtered by, as the Engine list accepts it.
  static let maximumQueryBytes = 256
  /// The keyboard extension builds the export in memory, and its memory limit is far below the app's.
  static let maximumExportBytes = 8 * 1024 * 1024

  /// Where the keyboard writes the requested export. Only one is kept: a new request replaces it.
  var exportFile: URL? { directory?.appendingPathComponent("export.txt") }

  private static func decoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    // Rust serializes queue timestamps as RFC3339 strings. Older native iOS
    // builds wrote Foundation's reference-date number, so accept both forms
    // while converging writes on the cross-platform string contract.
    decoder.dateDecodingStrategy = .custom { decoder in
      let value = try decoder.singleValueContainer()
      if let string = try? value.decode(String.self) {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: string) { return date }
        formatter.formatOptions = [.withInternetDateTime]
        if let date = formatter.date(from: string) { return date }
      }
      if let seconds = try? value.decode(Double.self) {
        return Date(timeIntervalSinceReferenceDate: seconds)
      }
      throw DecodingError.dataCorruptedError(in: value, debugDescription: "invalid queue timestamp")
    }
    return decoder
  }

  private static func encoder() -> JSONEncoder {
    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .iso8601
    return encoder
  }

  /// 共享的 App Group 状态必须留在容器内。在创建任何目录或访问任何文件之前，先拒绝事先存在的符号链接。
  private func rejectSymlinkAncestors(_ path: URL) throws {
    guard !SafePath.hasRefusedSymbolicLink(path) else { throw StoreError.unavailable }
  }

  init(directory: URL? = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)) {
    self.directory = directory?.appendingPathComponent("PersonalDictionary", isDirectory: true)
  }

  func read() throws -> PersonalDictionaryState {
    guard let directory else { throw StoreError.unavailable }
    return try readFile(at: directory.appendingPathComponent("sync.json"))
  }

  private func readFile(at file: URL) throws -> PersonalDictionaryState {
    try rejectSymlinkAncestors(file)
    guard FileManager.default.fileExists(atPath: file.path) else { return PersonalDictionaryState() }
    let size = try file.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
    guard size <= maximumBytes else { throw StoreError.invalidState }
    let data: Data
    do {
      data = try BoundedFileReader.read(from: file, maximumBytes: maximumBytes)
    } catch {
      throw StoreError.invalidState
    }
    guard data.count <= maximumBytes,
          let state = try? Self.decoder().decode(PersonalDictionaryState.self, from: data),
          state.version == 1, state.requests.count <= 160, state.entries.count <= 100,
          state.pageOffset >= 0, state.pageOffset <= 1_000_000,
          state.requestedPageOffset >= 0, state.requestedPageOffset <= 1_000_000,
          state.requestedQuery.utf8.count <= Self.maximumQueryBytes,
          state.pageQuery.utf8.count <= Self.maximumQueryBytes,
          [state.exportRequest?.format, state.exportResult?.request.format].allSatisfy({
            $0.map(PersonalExportRequest.formats.contains) ?? true
          }),
          Set(state.requests.map(\.id)).count == state.requests.count
    else { throw StoreError.invalidState }
    return state
  }

  private func update(_ action: (inout PersonalDictionaryState) throws -> Void) throws {
    guard let directory else { throw StoreError.unavailable }
    Self.processLock.lock()
    defer { Self.processLock.unlock() }
    try rejectSymlinkAncestors(directory)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let descriptor = open(directory.appendingPathComponent("sync.lock").path,
                          O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw StoreError.unavailable }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX | LOCK_NB) == 0 else { throw StoreError.busy }
    defer { flock(descriptor, LOCK_UN) }
    let file = directory.appendingPathComponent("sync.json")
    var state = try readFile(at: file)
    try action(&state)
    let data = try Self.encoder().encode(state)
    guard data.count <= maximumBytes else { throw StoreError.invalidState }
    try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
  }

  private func withSharedLock<T>(_ body: (URL) throws -> T) throws -> T {
    guard let directory else { throw StoreError.unavailable }
    Self.processLock.lock()
    defer { Self.processLock.unlock() }
    try rejectSymlinkAncestors(directory)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let descriptor = open(directory.appendingPathComponent("sync.lock").path,
                          O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw StoreError.unavailable }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_SH | LOCK_NB) == 0 else { throw StoreError.busy }
    defer { flock(descriptor, LOCK_UN) }
    return try body(directory)
  }

  private static func validRequestID(_ id: String) -> Bool {
    !id.isEmpty && id.utf8.count <= 120 && id.utf8.allSatisfy {
      ($0 >= 48 && $0 <= 57) || ($0 >= 65 && $0 <= 90) ||
        ($0 >= 97 && $0 <= 122) || $0 == 45 || $0 == 95
    }
  }

  @discardableResult
  func enqueue(previous: PersonalWord?, replacement: PersonalWord?, requestID: String? = nil) throws -> String {
    let id = requestID ?? UUID().uuidString
    guard Self.validRequestID(id) else { throw StoreError.invalidState }
    let request = PersonalWordRequest(id: id, previous: previous, replacement: replacement)
    try update { state in
      guard previous != nil || replacement != nil else { throw StoreError.invalidState }
      guard state.requests.filter({ $0.status != .applied }).count < 128 else { throw StoreError.tooManyRequests }
      guard !state.requests.contains(where: { $0.id == id }) else { throw StoreError.conflict }
      let identities = Set([previous?.id, replacement?.id].compactMap { $0 })
      guard !state.requests.contains(where: {
        $0.status == .pending && !identities.isDisjoint(with: [$0.previous?.id, $0.replacement?.id].compactMap { $0 })
      }) else { throw StoreError.conflict }
      let finished = Set(state.requests.filter { $0.status == .applied }.suffix(31).map(\.id))
      state.requests.removeAll { $0.status == .applied && !finished.contains($0.id) }
      state.requests.append(request)
      state.refreshID = UUID()
    }
    return request.id
  }

  // Queue the entire validated import in one file replacement. Dictionary edits are later
  // acknowledged individually by the Engine, so failed entries remain independently retryable.
  func enqueueImport(_ words: [PersonalWord], requestID: String? = nil) throws {
    let validated = try words.map { try $0.validated() }
    guard !validated.isEmpty, validated.count <= 128,
          Set(validated.map(\.id)).count == validated.count else { throw StoreError.invalidState }
    let baseID = requestID ?? UUID().uuidString
    guard Self.validRequestID(baseID), baseID.utf8.count <= 116 else { throw StoreError.invalidState }
    try update { state in
      guard state.requests.filter({ $0.status != .applied }).count + validated.count <= 128
      else { throw StoreError.tooManyRequests }
      let requestIDs = Set(validated.indices.map { "\(baseID)-\($0)" })
      guard state.requests.allSatisfy({ !requestIDs.contains($0.id) }) else { throw StoreError.conflict }
      let identities = Set(validated.map(\.id))
      guard !state.requests.contains(where: {
        $0.status != .applied && !identities.isDisjoint(with: [$0.previous?.id, $0.replacement?.id].compactMap { $0 })
      }) else { throw StoreError.conflict }
      let finished = Set(state.requests.filter { $0.status == .applied }.suffix(31).map(\.id))
      state.requests.removeAll { $0.status == .applied && !finished.contains($0.id) }
      state.requests.append(contentsOf: validated.enumerated().map { index, word in
        PersonalWordRequest(id: "\(baseID)-\(index)", replacement: word)
      })
      state.refreshID = UUID()
    }
  }

  func retry(_ id: String) throws {
    try update { state in
      guard let index = state.requests.firstIndex(where: { $0.id == id && $0.status == .failed }) else { return }
      let request = state.requests[index]
      let identities = Set([request.previous?.id, request.replacement?.id].compactMap { $0 })
      guard !state.requests.contains(where: {
        $0.status == .pending && !identities.isDisjoint(with: [$0.previous?.id, $0.replacement?.id].compactMap { $0 })
      }) else { throw StoreError.conflict }
      state.requests[index].status = .pending
      state.requests[index].error = nil
      state.refreshID = UUID()
    }
  }

  func dismissFailure(_ id: String) throws {
    try update { $0.requests.removeAll { $0.id == id && $0.status == .failed } }
  }

  func requestPage(offset: Int, kind: PersonalWordKind? = nil, query: String = "") throws {
    let query = query.trimmingCharacters(in: .whitespaces)
    guard (0...1_000_000).contains(offset), query.utf8.count <= Self.maximumQueryBytes else {
      throw StoreError.invalidState
    }
    try update {
      $0.requestedPageOffset = offset
      $0.requestedKind = kind
      $0.requestedQuery = query
      $0.refreshID = UUID()
    }
  }

  // Keep the queue lock through apply and acknowledgement. If writing sync.json is interrupted,
  // the same UUID is retried; Engine's transaction receipt makes that retry a no-op success.
  func synchronize(apply: (PersonalWordRequest) throws -> Void,
                   page: (PersonalPageRequest) throws -> PersonalWordPage,
                   export: ((PersonalExportRequest) throws -> PersonalExportText)? = nil) throws {
    try update { state in
      // Each edit closes and reopens the Engine session. Limit work per keyboard timer turn
      // so a full import cannot monopolize the main thread or hold the host's queue lock.
      let batch = state.requests.indices.filter { state.requests[$0].status == .pending }.prefix(4)
      for index in batch {
        do {
          try apply(state.requests[index])
          state.requests[index].status = .applied
          state.requests[index].error = nil
        } catch {
          state.requests[index].status = .failed
          state.requests[index].error = String(error.localizedDescription.prefix(500))
        }
      }
      if state.pendingCount == 0 { state.completedRefreshID = state.refreshID }
      do {
        let request = PersonalPageRequest(offset: state.requestedPageOffset, kind: state.requestedKind,
                                          query: state.requestedQuery)
        let snapshot = try page(request)
        guard snapshot.entries.count <= 100 else { throw StoreError.invalidState }
        state.entries = snapshot.entries
        state.hasMore = snapshot.hasMore
        state.pageOffset = request.offset
        state.pageKind = request.kind
        state.pageQuery = request.query
        state.snapshotDate = Date()
        state.snapshotError = nil
      } catch {
        // Preserve the last confirmed list and every edit acknowledgement even if refresh fails.
        state.snapshotError = String(error.localizedDescription.prefix(500))
      }
      // Written after this turn's edits, so an import followed by an export carries the imported words.
      if let export, let request = state.exportRequest, state.exportResult?.request.id != request.id,
         state.pendingCount == 0, let file = exportFile {
        var result = PersonalExportResult(request: request)
        do {
          let exported = try export(request)
          let data = Data(exported.text.utf8)
          guard data.count <= Self.maximumExportBytes else { throw StoreError.exportTooLarge }
          try data.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
          result.rows = exported.text.split(separator: "\n", omittingEmptySubsequences: true).count
          result.truncated = !exported.complete
        } catch {
          try? FileManager.default.removeItem(at: file)
          result.error = String(error.localizedDescription.prefix(500))
        }
        state.exportResult = result
      }
    }
  }

  /// A copy of the written export under its desktop name, for the share sheet. The shared file stays where the keyboard wrote it.
  func exportCopy(for result: PersonalExportResult) throws -> URL {
    guard result.error == nil else { throw StoreError.unavailable }
    return try withSharedLock { directory in
      let state = try readFile(at: directory.appendingPathComponent("sync.json"))
      guard state.exportResult?.request.id == result.request.id,
            state.exportResult?.error == nil else { throw StoreError.conflict }
      let file = directory.appendingPathComponent("export.txt")
      // Keep each request's share copy in its own directory. A later export must not
      // invalidate a URL that a share sheet is still holding from an earlier one.
      let folder = FileManager.default.temporaryDirectory
        .appendingPathComponent("PersonalExport-\(result.request.id.uuidString)", isDirectory: true)
      try? FileManager.default.removeItem(at: folder)
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      let copy = folder.appendingPathComponent(result.request.fileName)
      try FileManager.default.copyItem(at: file, to: copy)
      return copy
    }
  }

  /// Ask the keyboard to write one dictionary out. It replaces any export not yet written, and the keyboard answers it after the edits already queued.
  @discardableResult
  func requestExport(kind: PersonalWordKind, format: String) throws -> PersonalExportRequest {
    guard PersonalExportRequest.formats.contains(format) else { throw StoreError.invalidState }
    let request = PersonalExportRequest(kind: kind, format: format)
    try update {
      $0.exportRequest = request
      $0.refreshID = UUID()
    }
    return request
  }
}
