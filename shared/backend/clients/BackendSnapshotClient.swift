import Foundation
import CoreFoundation
import CryptoKit
import SQLite3

/// Validates framing, fields, cross-record consistency and checksum. Engine
/// staging must still succeed before any local dictionary activation.
struct BackendSnapshotEnvelope: Sendable {
  let revision: Int64
  let sha256: String
  let records: Int
  let entries: Int
  let overlays: Int
  let positions: Int
  let selections: Int

  static func inspect(_ file: URL) throws -> Self {
    let handle = try FileHandle(forReadingFrom: file)
    defer { try? handle.close() }
    let index = try SnapshotRecordIndex()
    var hash = SHA256()
    var line = Data()
    var count = 0, bytes = 0, category = -1
    var totals = [Int](repeating: 0, count: 5)
    var revision: Int64?
    var finished = false
    var checksum: String?
    let bad = BackendAccountClient.Failure(status: 400)
    func integer(_ value: Any?) throws -> Int64 {
      guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
            !["f", "d"].contains(String(cString: number.objCType)),
            let result = Int64(number.stringValue) else { throw bad }
      return result
    }
    func record(_ data: Data) throws {
      try Task.checkCancellation()
      var syntax = SnapshotJSONSyntax(data)
      try syntax.validate()
      guard !finished, !data.isEmpty, data.count < 65536, String(data: data, encoding: .utf8) != nil,
            let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
            let type = object["type"] as? String else { throw bad }
      if type == "footer" {
        guard revision != nil, Set(object.keys) == ["type", "records", "sha256"],
              try integer(object["records"]) == Int64(count),
              let digest = object["sha256"] as? String,
              digest == hash.finalize().map({ String(format: "%02x", $0) }).joined() else { throw bad }
        checksum = digest; finished = true; return
      }
      let next: Int
      switch type {
      case "header":
        guard count == 0, Set(object.keys) == ["type", "format", "version", "revision"],
              object["format"] as? String == "msime-dictionary-snapshot",
              try integer(object["version"]) == 1 else { throw bad }
        let value = try integer(object["revision"])
        guard value >= 0 else { throw bad }
        revision = value; next = 0
      case "entry", "overlay", "position", "selection":
        guard revision != nil, object["data"] is [String: Any] else { throw bad }
        if type == "overlay" {
          guard Set(object.keys) == ["type", "data", "deleted"], let deleted = object["deleted"] as? NSNumber,
                CFGetTypeID(deleted) == CFBooleanGetTypeID() else { throw bad }
        } else { guard Set(object.keys) == ["type", "data"] else { throw bad } }
        try SnapshotRecordFields.validate(object, revision: revision!)
        try index.insert(object)
        next = ["entry": 1, "overlay": 2, "position": 3, "selection": 4][type]!
      default: throw bad
      }
      guard next >= category else { throw bad }
      category = next; totals[next] += 1; count += 1
      hash.update(data: data); hash.update(data: Data([10]))
    }
    while let chunk = try handle.read(upToCount: 65536), !chunk.isEmpty {
      bytes += chunk.count
      guard bytes <= 512 * 1024 * 1024 else { throw bad }
      var start = chunk.startIndex
      for index in chunk.indices where chunk[index] == 10 {
        line.append(chunk[start..<index])
        try record(line)
        line.removeAll(keepingCapacity: true)
        start = chunk.index(after: index)
      }
      line.append(chunk[start..<chunk.endIndex])
      guard line.count < 65536 else { throw bad }
    }
    if !line.isEmpty { try record(line) }
    guard finished, let revision, let checksum else { throw bad }
    try index.validate()
    return .init(revision: revision, sha256: checksum, records: count, entries: totals[1], overlays: totals[2], positions: totals[3], selections: totals[4])
  }
}

extension BackendAccountClient {
  struct DownloadedSnapshot: Sendable { let url: URL; let envelope: BackendSnapshotEnvelope }
  struct SnapshotRestoreResult: Decodable, Sendable { let revision: Int64; let reset: Bool }

  func restoreDictionarySnapshot(file: URL, expectedSHA256: String, revision: Int64, token: String) async throws -> SnapshotRestoreResult {
    guard revision >= 0 else { throw Failure(status: 400) }
    // Freeze the file before validating it. A document provider or another
    // process may replace the originally selected file after the preview.
    let prepared = try BackendPreparedSnapshot(copying: file)
    defer { withExtendedLifetime(prepared) {} }
    guard prepared.envelope.sha256 == expectedSHA256 else { throw Failure(status: 400) }
    let data = try await uploadSnapshot(prepared.url, revision: revision, token: token)
    let result: SnapshotRestoreResult
    do { result = try JSONDecoder().decode(SnapshotRestoreResult.self, from: data) }
    catch { throw Failure(status: 0) }
    guard result.reset, result.revision > revision else { throw Failure(status: 0) }
    return result
  }

  func dictionarySnapshot(token: String) async throws -> DownloadedSnapshot {
    let url = try await download("/v1/users/me/dictionary/snapshot", token: token,
      filename: "msime-dictionary-snapshot.ndjson", maximumBytes: 512 * 1024 * 1024, mediaType: "application/x-ndjson")
    do {
      let envelope = try BackendSnapshotEnvelope.inspect(url)
      return .init(url: url, envelope: envelope)
    } catch {
      try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
      throw error
    }
  }
}


/// The wire format contains only objects and scalar values. Foundation otherwise
/// silently accepts repeated keys, including keys written with Unicode escapes.
private struct SnapshotJSONSyntax {
  private let bytes: [UInt8]
  private var offset = 0
  private let bad = BackendAccountClient.Failure(status: 400)
  init(_ data: Data) { bytes = Array(data) }
  mutating func validate() throws {
    guard !bytes.isEmpty, bytes.count < 65536 else { throw bad }
    try object(depth: 0)
    whitespace()
    guard offset == bytes.count else { throw bad }
  }
  private mutating func whitespace() {
    while offset < bytes.count, [9, 10, 13, 32].contains(bytes[offset]) { offset += 1 }
  }
  private mutating func consume(_ byte: UInt8) throws {
    whitespace()
    guard offset < bytes.count, bytes[offset] == byte else { throw bad }
    offset += 1
  }
  private mutating func string() throws -> String {
    whitespace()
    let start = offset
    try consume(34)
    while offset < bytes.count {
      let byte = bytes[offset]; offset += 1
      if byte == 34 {
        return try JSONDecoder().decode(String.self, from: Data(bytes[start..<offset]))
      }
      if byte == 92 { offset += 1 }
    }
    throw bad
  }
  private mutating func object(depth: Int) throws {
    guard depth <= 1 else { throw bad }
    try consume(123)
    whitespace()
    if offset < bytes.count, bytes[offset] == 125 { offset += 1; return }
    var keys = Set<String>()
    while true {
      guard keys.insert(try string()).inserted else { throw bad }
      try consume(58)
      whitespace()
      guard offset < bytes.count else { throw bad }
      switch bytes[offset] {
      case 123: try object(depth: depth + 1)
      case 34: _ = try string()
      case 91: throw bad
      default:
        let start = offset
        while offset < bytes.count, ![9, 10, 13, 32, 44, 125].contains(bytes[offset]) { offset += 1 }
        let token = String(decoding: bytes[start..<offset], as: UTF8.self)
        // All numeric fields in this protocol are integers. Reject 1.0 and 1e0
        // even when NSNumber happens to represent either as an integer.
        guard ["true", "false", "null"].contains(token) ||
          (!token.isEmpty && !token.contains(".") && !token.contains("e") && !token.contains("E") && Int64(token) != nil) else { throw bad }
      }
      whitespace()
      guard offset < bytes.count else { throw bad }
      if bytes[offset] == 125 { offset += 1; return }
      try consume(44)
    }
  }
}

private enum SnapshotRecordFields {
  private static let timestampPattern = try! NSRegularExpression(
    pattern: #"\A[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:[.,][0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})\z"#)
  private static func validTimestamp(_ timestamp: String) -> Bool {
    let range = NSRange(timestamp.startIndex..<timestamp.endIndex, in: timestamp)
    guard timestampPattern.firstMatch(in: timestamp, range: range) != nil else { return false }
    let bytes = Array(timestamp.utf8)
    func number(_ start: Int, _ end: Int) -> Int { Int(String(decoding: bytes[start..<end], as: UTF8.self))! }
    let year = number(0, 4), month = number(5, 7), day = number(8, 10)
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    let days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    guard (1...12).contains(month), (1...days[month - 1]).contains(day),
          number(11, 13) < 24, number(14, 16) < 60, number(17, 19) < 60 else { return false }
    if bytes.last != 90 {
      guard number(bytes.count - 5, bytes.count - 3) < 24, number(bytes.count - 2, bytes.count) < 60 else { return false }
    }
    // ISO8601DateFormatter alone accepts trailing text and normalizes February
    // 30; validate syntax and calendar fields before asking it to parse the zone.
    let formatter = ISO8601DateFormatter()
    let normalized = timestamp.replacingOccurrences(of: ",", with: ".")
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    let fractional = formatter.date(from: normalized)
    formatter.formatOptions = [.withInternetDateTime]
    guard let date = fractional ?? formatter.date(from: normalized) else { return false }
    return date != formatter.date(from: "0001-01-01T00:00:00Z")
  }

  static func validate(_ object: [String: Any], revision: Int64) throws {
    let bad = BackendAccountClient.Failure(status: 400)
    guard let type = object["type"] as? String, let data = object["data"] as? [String: Any] else { throw bad }
    func integer(_ key: String) throws -> Int64 {
      guard let number = data[key] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
            let value = Int64(number.stringValue) else { throw bad }
      return value
    }
    func text(_ key: String, maximum: Int) throws -> String {
      guard let value = data[key] as? String, !value.isEmpty, value.utf8.count <= maximum,
            !value.utf8.contains(where: { [0, 9, 10, 13].contains($0) }) else { throw bad }
      return value
    }
    let code = try text("code", maximum: 512)
    let word = try text("word", maximum: 2048)
    if type == "entry" || type == "overlay" {
      var keys = Set(data.keys)
      if keys.remove("user_inserted") != nil {
        guard let value = data["user_inserted"] as? NSNumber, CFGetTypeID(value) == CFBooleanGetTypeID(),
              type != "entry" || value.boolValue else { throw bad }
      }
      guard keys == ["id", "kind", "code", "word", "weight", "revision", "updated_at"],
            let kind = data["kind"] as? String, ["pinyin", "wubi", "wubi98", "english", "quick"].contains(kind),
            data["id"] is String else { throw bad }
      if type == "entry" { _ = try text("id", maximum: 128) }
      let weight = try integer("weight"), recordRevision = try integer("revision")
      guard (0...100000000).contains(weight), weight > 0 || object["deleted"] as? Bool == true,
            recordRevision >= 1, recordRevision <= revision,
            let timestamp = data["updated_at"] as? String else { throw bad }
      guard validTimestamp(timestamp) else { throw bad }
    } else {
      let field = type == "position" ? "position" : "count"
      guard Set(data.keys) == ["context", "code", "word", field] else { throw bad }
      let context = try text("context", maximum: 512)
      guard context.utf8.count + code.utf8.count + word.utf8.count <= 2048 else { throw bad }
      let value = try integer(field)
      guard (type == "position" ? (1...5) : (0...10)).contains(Int(value)) else { throw bad }
    }
  }
}


/// A disposable wire-format index, never a local Engine dictionary. Keeping
/// identities on disk bounds memory even for large overlay/counter snapshots.
private final class SnapshotRecordIndex {
  private var database: OpaquePointer?
  private var insertion: OpaquePointer?
  private let directory: URL
  private var entries = 0
  private let bad = BackendAccountClient.Failure(status: 400)
  init() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-index-" + UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    do {
      let file = directory.appendingPathComponent("records.sqlite")
      guard sqlite3_open_v2(file.path, &database, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_NOMUTEX, nil) == SQLITE_OK else { throw bad }
      sqlite3_progress_handler(database, 1000, { _ in Task.isCancelled ? 1 : 0 }, nil)
      try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
      #if os(iOS)
      try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: file.path)
      #endif
      try execute("""
        PRAGMA cache_size=-2048;
        PRAGMA temp_store=FILE;
        CREATE TABLE records(type TEXT, scope TEXT, code TEXT, word TEXT, id TEXT, weight INTEGER, owned INTEGER, deleted INTEGER, slot INTEGER,
          PRIMARY KEY(type,scope,code,word)) WITHOUT ROWID;
        CREATE UNIQUE INDEX entry_ids ON records(id) WHERE type='entry';
        CREATE UNIQUE INDEX position_slots ON records(scope,slot) WHERE type='position';
        BEGIN;
        """)
      guard sqlite3_prepare_v2(database, "INSERT INTO records VALUES(?,?,?,?,?,?,?,?,?)", -1, &insertion, nil) == SQLITE_OK else { throw bad }
    } catch { cleanup(); throw error }
  }
  deinit { cleanup() }
  private func cleanup() {
    sqlite3_finalize(insertion); insertion = nil
    sqlite3_close(database); database = nil
    try? FileManager.default.removeItem(at: directory)
  }
  private func execute(_ sql: String) throws {
    guard sqlite3_exec(database, sql, nil, nil, nil) == SQLITE_OK else { throw bad }
  }
  func insert(_ object: [String: Any]) throws {
    guard let type = object["type"] as? String, let data = object["data"] as? [String: Any],
          let scope = data["kind"] as? String ?? data["context"] as? String,
          let code = data["code"] as? String, let word = data["word"] as? String else { throw bad }
    if type == "entry" { entries += 1; guard entries <= 100000 else { throw bad } }
    sqlite3_reset(insertion); sqlite3_clear_bindings(insertion)
    let strings = [type, scope, code, word, data["id"] as? String ?? ""]
    for (offset, value) in strings.enumerated() {
      let result = value.withCString { sqlite3_bind_text(insertion, Int32(offset + 1), $0, -1, unsafeBitCast(-1, to: sqlite3_destructor_type.self)) }
      guard result == SQLITE_OK else { throw bad }
    }
    let numbers: [Int64] = [(data["weight"] as? NSNumber)?.int64Value ?? 0,
      (data["user_inserted"] as? Bool ?? true) ? 1 : 0,
      (object["deleted"] as? Bool ?? false) ? 1 : 0,
      (data["position"] as? NSNumber)?.int64Value ?? 0]
    for (offset, value) in numbers.enumerated() {
      guard sqlite3_bind_int64(insertion, Int32(offset + 6), value) == SQLITE_OK else { throw bad }
    }
    guard sqlite3_step(insertion) == SQLITE_DONE else { throw bad }
  }
  func validate() throws {
    try Task.checkCancellation()
    // Same invariant as server restore: every personal entry has a matching
    // live, user-owned overlay with equal weight, and vice versa. Base ranking
    // overrides and tombstones do not require a personal entry.
    let sql = """
      SELECT EXISTS(
        SELECT 1 FROM records e LEFT JOIN records o
          ON o.type='overlay' AND e.scope=o.scope AND e.code=o.code AND e.word=o.word
        WHERE e.type='entry' AND (o.type IS NULL OR o.deleted=1 OR o.owned=0 OR e.weight<>o.weight)
        UNION ALL
        SELECT 1 FROM records o LEFT JOIN records e
          ON e.type='entry' AND e.scope=o.scope AND e.code=o.code AND e.word=o.word
        WHERE o.type='overlay' AND o.deleted=0 AND o.owned=1 AND e.type IS NULL
      )
      """
    var statement: OpaquePointer?
    guard sqlite3_prepare_v2(database, sql, -1, &statement, nil) == SQLITE_OK else { throw bad }
    defer { sqlite3_finalize(statement) }
    guard sqlite3_step(statement) == SQLITE_ROW, sqlite3_column_int(statement, 0) == 0 else { throw bad }
    try Task.checkCancellation()
  }
}


/// Owns a stable, private snapshot copy from preview until the user finishes.
/// Selecting a file performs only local work and does not upload any data.
final class BackendPreparedSnapshot: @unchecked Sendable {
  let url: URL
  let envelope: BackendSnapshotEnvelope
  let fileSHA256: String
  static func prepareDocument(_ source: URL) async throws -> BackendPreparedSnapshot {
    let access = source.startAccessingSecurityScopedResource()
    defer { if access { source.stopAccessingSecurityScopedResource() } }
    return try BackendPreparedSnapshot(copying: source)
  }
  init(copying source: URL) throws {
    guard source.isFileURL else { throw BackendAccountClient.Failure(status: 400) }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-upload-" + UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    url = directory.appendingPathComponent("snapshot.ndjson")
    do {
      var attributes: [FileAttributeKey: Any] = [.posixPermissions: 0o600]
      #if os(iOS)
      attributes[.protectionKey] = FileProtectionType.completeUntilFirstUserAuthentication
      #endif
      guard FileManager.default.createFile(atPath: url.path, contents: nil, attributes: attributes) else { throw BackendAccountClient.Failure(status: 0) }
      let input = try FileHandle(forReadingFrom: source)
      defer { try? input.close() }
      let output = try FileHandle(forWritingTo: url)
      defer { try? output.close() }
      var count = 0
      var fileHash = SHA256()
      while let data = try input.read(upToCount: 65536), !data.isEmpty {
        try Task.checkCancellation()
        count += data.count
        guard count <= 512 * 1024 * 1024 else { throw BackendAccountClient.Failure(status: 400) }
        fileHash.update(data: data)
        try output.write(contentsOf: data)
      }
      try output.synchronize()
      fileSHA256 = fileHash.finalize().map { String(format: "%02x", $0) }.joined()
      envelope = try BackendSnapshotEnvelope.inspect(url)
    } catch {
      try? FileManager.default.removeItem(at: directory)
      throw error
    }
  }
  deinit { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
}


/// A synchronous pull stream for Engine staging callbacks. Records remain
/// provisional until next() reaches verified EOF; any error discards staging.
final class BackendSnapshotRecordStream {
  let snapshot: BackendPreparedSnapshot
  private let input: FileHandle
  private var chunk = Data()
  private var offset = 0
  private var hash = SHA256()
  private var count = 0
  private var bytes = 0
  private var finished = false
  init(snapshot: BackendPreparedSnapshot) throws {
    self.snapshot = snapshot
    input = try FileHandle(forReadingFrom: snapshot.url)
  }
  deinit { try? input.close() }
  func next() throws -> [String: Any]? {
    if finished { return nil }
    let bad = BackendAccountClient.Failure(status: 400)
    while let line = try readLine() {
      try Task.checkCancellation()
      guard let object = try JSONSerialization.jsonObject(with: line) as? [String: Any],
            let type = object["type"] as? String else { throw bad }
      if type == "footer" {
        var syntax = SnapshotJSONSyntax(line)
        try syntax.validate()
        let digest = hash.finalize().map { String(format: "%02x", $0) }.joined()
        guard Set(object.keys) == ["type", "records", "sha256"],
              let footerCount = object["records"] as? NSNumber, CFGetTypeID(footerCount) != CFBooleanGetTypeID(),
              digest == snapshot.envelope.sha256, count == snapshot.envelope.records,
              object["sha256"] as? String == digest,
              (object["records"] as? NSNumber)?.intValue == count,
              try readLine() == nil else { throw bad }
        finished = true
        return nil
      }
      hash.update(data: line); hash.update(data: Data([10])); count += 1
      guard count <= snapshot.envelope.records else { throw bad }
      switch type {
      case "header", "entry": continue
      case "overlay", "position", "selection": return object
      default: throw bad
      }
    }
    throw bad // EOF without the checksum footer is never a successful stream end.
  }
  private func readLine() throws -> Data? {
    var line = Data()
    while true {
      try Task.checkCancellation()
      if offset == chunk.count {
        chunk = try input.read(upToCount: 65536) ?? Data()
        offset = 0; bytes += chunk.count
        guard bytes <= 512 * 1024 * 1024 else { throw BackendAccountClient.Failure(status: 400) }
        if chunk.isEmpty { return line.isEmpty ? nil : line }
      }
      if let newline = chunk[offset...].firstIndex(of: 10) {
        line.append(chunk[offset..<newline]); offset = newline + 1
        guard !line.isEmpty, line.count < 65536 else { throw BackendAccountClient.Failure(status: 400) }
        return line
      }
      line.append(chunk[offset...]); offset = chunk.count
      guard line.count < 65536 else { throw BackendAccountClient.Failure(status: 400) }
    }
  }
}

