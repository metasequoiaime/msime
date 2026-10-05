import Foundation
import CoreFoundation

private typealias SnapshotByte = UInt8
private typealias SnapshotNext = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<SnapshotByte>?, Int) -> Int

@_silgen_name("msime_client_snapshot_prepare")
private func msimeClientSnapshotPrepare(_ request: UnsafePointer<SnapshotByte>?, _ length: UInt,
                                        _ next: SnapshotNext?, _ context: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_discard")
private func msimeClientSnapshotDiscard(_ handle: UInt64) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_snapshot_version")
private func msimeClientSnapshotVersionForBridge(_ options: UnsafePointer<SnapshotByte>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientSnapshotStringFree(_ value: UnsafeMutablePointer<CChar>?)

final class MSIMEPreparedDictionarySnapshot: @unchecked Sendable {
  let handle: UInt64
  let identifier: String
  private let sourceVersion: String
  private var consumed = false

  fileprivate init(handle: UInt64, identifier: String, sourceVersion: String) {
    self.handle = handle
    self.identifier = identifier
    self.sourceVersion = sourceVersion
  }

  func stateRevision() throws -> String { sourceVersion }
  func markConsumed() { consumed = true }
  var isConsumed: Bool { consumed }
}

enum DictionarySnapshotBridge {
  typealias NextRecord = (UnsafeMutablePointer<NSError?>?) -> [String: Any]?
  private static var handles: [String: UInt64] = [:]
  private static let lock = NSLock()

  static func forget(_ identifier: String) {
    lock.lock(); handles.removeValue(forKey: identifier); lock.unlock()
  }

  static func discardInactive(identifier: String, user: URL) throws {
    _ = user
    lock.lock(); let handle = handles[identifier]; lock.unlock()
    guard let handle else { return }
    let value = try decode(msimeClientSnapshotDiscard(handle))
    guard value["discarded"] as? Bool == true else { throw SnapshotBridgeFailure.invalid }
    lock.lock(); handles.removeValue(forKey: identifier); lock.unlock()
  }

  static func prepare(resources: URL, user: URL, identifier: String, contentIdentifier: String,
                      maximumRecords: UInt, preparedOptions: Data? = nil,
                      nextRecord: @escaping NextRecord) throws -> MSIMEPreparedDictionarySnapshot {
    guard resources.isFileURL, user.isFileURL, !identifier.isEmpty, maximumRecords > 0 else {
      throw SnapshotBridgeFailure.invalid
    }
    let options: [String: Any]
    if let preparedOptions {
      guard let decoded = try JSONSerialization.jsonObject(with: preparedOptions) as? [String: Any],
            decoded["resources"] as? String == resources.path,
            decoded["user_data"] as? String == user.path else { throw SnapshotBridgeFailure.invalid }
      options = decoded
    } else {
      options = snapshotOptions(resources: resources, user: user)
    }
    let expected = try version(options)
    let staging = user.deletingLastPathComponent().appendingPathComponent("SnapshotStaging", isDirectory: true)
    try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)
    let request: [String: Any] = ["options": options, "staging_root": staging.path,
                                  "expected_version": expected, "records": maximumRecords,
                                  "activation_id": identifier]
    let data = try JSONSerialization.data(withJSONObject: request)
    let box = SnapshotRecordBox(nextRecord)
    let opaque = Unmanaged.passRetained(box).toOpaque()
    defer { Unmanaged<SnapshotRecordBox>.fromOpaque(opaque).release() }
    let response = try data.withUnsafeBytes { bytes in
      try decode(msimeClientSnapshotPrepare(bytes.bindMemory(to: SnapshotByte.self).baseAddress,
                                            UInt(data.count), snapshotNext, opaque))
    }
    guard let handle = unsignedIntegerValue(response["handle"]),
          let sourceVersion = response["source_version"] as? String else {
      throw SnapshotBridgeFailure.invalid
    }
    lock.lock(); handles[identifier] = handle; lock.unlock()
    _ = contentIdentifier
    return MSIMEPreparedDictionarySnapshot(handle: handle, identifier: identifier, sourceVersion: sourceVersion)
  }

  static func unsignedIntegerValue(_ value: Any?) -> UInt64? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = UInt64(number.stringValue),
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }

  private static func snapshotOptions(resources: URL, user: URL) -> [String: Any] {
    let root = user.deletingLastPathComponent()
    for path in [user, root.appendingPathComponent("cache", isDirectory: true),
                 root.appendingPathComponent("dictionaries", isDirectory: true)] {
      try? FileManager.default.createDirectory(at: path, withIntermediateDirectories: true)
    }
    var options: [String: Any] = ["api_version": 1, "resources": resources.path, "user_data": user.path,
            "cache": root.appendingPathComponent("cache", isDirectory: true).path,
            "dictionaries": root.appendingPathComponent("dictionaries", isDirectory: true).path,
            "preferences": ["scheme": MSIMEAppEdition.defaultScheme, "candidate_page_size": 9,
                             "learning": true, "chinese_punctuation": true]]
    // 与键盘的 HostOptions 一样：full 不带版本，与引入版本之前相同；其他版本让 host-api 按本版本的方案和资源锁处理。
    if !MSIMEAppEdition.isFull { options["edition"] = MSIMEAppEdition.identifier }
    return options
  }

  private static func version(_ options: [String: Any]) throws -> String {
    let data = try JSONSerialization.data(withJSONObject: options)
    let value = try data.withUnsafeBytes { bytes in
      try decode(msimeClientSnapshotVersionForBridge(bytes.bindMemory(to: SnapshotByte.self).baseAddress,
                                                     UInt(data.count)))
    }
    guard let result = value["version"] as? String else { throw SnapshotBridgeFailure.invalid }
    return result
  }

  private static func decode(_ pointer: UnsafeMutablePointer<CChar>?) throws -> [String: Any] {
    guard let pointer else { throw SnapshotBridgeFailure.unavailable }
    let text = String(cString: pointer)
    msimeClientSnapshotStringFree(pointer)
    guard let data = text.data(using: .utf8), let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          envelope["ok"] as? Bool == true, let value = envelope["value"] as? [String: Any] else {
      throw SnapshotBridgeFailure.invalid
    }
    return value
  }
}

private enum SnapshotBridgeFailure: LocalizedError {
  case invalid, unavailable
  var errorDescription: String? {
    switch self { case .invalid: return "词库快照参数无效。"; case .unavailable: return "词库快照服务不可用。" }
  }
}

private final class SnapshotRecordBox {
  let nextRecord: DictionarySnapshotBridge.NextRecord
  init(_ nextRecord: @escaping DictionarySnapshotBridge.NextRecord) { self.nextRecord = nextRecord }
}

// One pool per record; otherwise every record's temporaries live until the import ends.
private let snapshotNext: SnapshotNext = { context, buffer, capacity in
  autoreleasepool { () -> Int in
    guard let context, let buffer, capacity > 0 else { return -1 }
    let box = Unmanaged<SnapshotRecordBox>.fromOpaque(context).takeUnretainedValue()
    var error: NSError?
    let record = withUnsafeMutablePointer(to: &error) { box.nextRecord($0) }
    if error != nil { return -1 }
    guard let record, let data = try? JSONSerialization.data(withJSONObject: record), data.count <= capacity else {
      return record == nil ? 0 : -1
    }
    data.copyBytes(to: buffer, count: data.count)
    return data.count
  }
}
