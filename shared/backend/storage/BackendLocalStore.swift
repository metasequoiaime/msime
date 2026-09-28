import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// App-Group file storage for the anonymous account shared by the app and keyboard extension.
struct BackendLocalStore: BackendSessionStorage {
  static let maximumSessionBytes = 64 * 1024
  private let fileName: String
  private let baseDirectory: URL?
  init(fileName: String = "backend-session.json", directory: URL? = nil) {
    self.fileName = fileName
    self.baseDirectory = directory ?? Self.directory
  }
  private static var directory: URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.msime.ios")
  }
  private var url: URL? { baseDirectory?.appendingPathComponent(fileName, isDirectory: false) }
  private var lockURL: URL? { baseDirectory?.appendingPathComponent("backend-local-store.lock", isDirectory: false) }

  private func withLock<T>(_ body: () throws -> T) throws -> T {
    guard let directory = baseDirectory, let lockURL else { throw BackendAccountClient.Failure(status: 0) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
      attributes: [.posixPermissions: 0o700])
    #if canImport(Darwin)
    let descriptor = open(lockURL.path, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw BackendAccountClient.Failure(status: 0) }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX) == 0 else { throw BackendAccountClient.Failure(status: 0) }
    defer { flock(descriptor, LOCK_UN) }
    #endif
    return try body()
  }

  func load() throws -> BackendSavedSession? {
    guard baseDirectory != nil else { return nil }
    return try withLock { () throws -> BackendSavedSession? in
      guard let url, FileManager.default.fileExists(atPath: url.path) else { return nil }
      let data: Data
      do { data = try Self.readBounded(url, maximumBytes: Self.maximumSessionBytes) }
      catch { throw BackendAccountClient.Failure(status: 0) }
      do { return try JSONDecoder().decode(BackendSavedSession.self, from: data) }
      catch { throw BackendAccountClient.Failure(status: 0) }
    }
  }
  func save(_ session: BackendSavedSession) throws {
    let data = try JSONEncoder().encode(session)
    try withLock {
      guard let url else { throw BackendAccountClient.Failure(status: 0) }
      try data.write(to: url, options: [.atomic])
      try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }
  }
  func clear() throws {
    guard baseDirectory != nil else { return }
    try withLock { if let url { try? FileManager.default.removeItem(at: url) } }
  }
  static func read(_ fileName: String) -> Data? {
    guard let directory else { return nil }
    let url = directory.appendingPathComponent(fileName, isDirectory: false)
    return try? BackendLocalStore(fileName: fileName, directory: directory).withLock {
      try readBounded(url, maximumBytes: maximumSessionBytes)
    }
  }

  private static func readBounded(_ url: URL, maximumBytes: Int) throws -> Data {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var data = Data()
    data.reserveCapacity(min(maximumBytes, 64 * 1024))
    while true {
      let remaining = maximumBytes - data.count
      let chunk = try handle.read(upToCount: min(64 * 1024, remaining + 1)) ?? Data()
      if chunk.isEmpty { return data }
      guard chunk.count <= remaining else { throw BackendAccountClient.Failure(status: 0) }
      data.append(chunk)
    }
  }
  @discardableResult static func write(_ data: Data, to fileName: String) -> Bool {
    guard let directory else { return false }
    do {
      try BackendLocalStore(fileName: fileName, directory: directory).withLock {
        let url = directory.appendingPathComponent(fileName, isDirectory: false)
        try data.write(to: url, options: [.atomic])
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
      }
      return true
    } catch { return false }
  }

  /// Atomically create this store's file only when no process has created it yet.
  /// The result is false when another process already owns the file.
  @discardableResult func writeIfAbsent(_ data: Data) -> Bool {
    do {
      return try withLock {
        guard let url, !FileManager.default.fileExists(atPath: url.path) else { return false }
        try data.write(to: url, options: [.atomic])
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
        return true
      }
    } catch { return false }
  }

  @discardableResult static func writeIfAbsent(_ data: Data, to fileName: String) -> Bool {
    guard let directory else { return false }
    return BackendLocalStore(fileName: fileName, directory: directory).writeIfAbsent(data)
  }
}
