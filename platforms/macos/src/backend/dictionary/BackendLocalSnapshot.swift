import Foundation

// Each prepared object has a single owner; staging runs off the main thread,
// and only the finished result is handed back for main-thread publication.
final class MacPreparedLocalSnapshot: @unchecked Sendable {
  let context: NSDictionary
  let snapshot: BackendPreparedSnapshot
  let identifier = UUID().uuidString
  private var prepared: UInt64?
  private var stagingRoot: URL?
  init(context: NSDictionary, snapshot: BackendPreparedSnapshot) { self.context = context; self.snapshot = snapshot }
  static func strictHandle(_ raw: Any?) -> UInt64? {
    guard !(raw is Bool), let number = raw as? NSNumber else { return nil }
    switch String(cString: number.objCType) {
    case "C", "S", "I", "L", "Q": return number.uint64Value
    case "c", "s", "i", "l", "q":
      let value = number.int64Value
      return value >= 0 ? UInt64(value) : nil
    default: return nil
    }
  }
  static func invoke(_ selector: String, _ parameters: NSDictionary? = nil) throws -> NSDictionary {
    guard let type = NSClassFromString("MSIMEClientSession") as? NSObject.Type,
          let result = type.perform(NSSelectorFromString(selector), with: parameters)?.takeUnretainedValue() as? NSDictionary else {
      throw BackendAccountClient.Failure(status: 503)
    }
    if let error = result["error"] as? NSError { throw error }
    return result
  }
  func stage() throws {
    let stream = try BackendSnapshotRecordStream(snapshot: snapshot)
    let next: @convention(block) (AutoreleasingUnsafeMutablePointer<NSError?>?) -> NSDictionary? = { failure in
      do { return try stream.next().map { $0 as NSDictionary } }
      catch { failure?.pointee = error as NSError; return nil }
    }
    let versionResult = try Self.invoke("snapshotVersion:", context)
    guard let version = versionResult["version"] as? String else { throw BackendAccountClient.Failure(status: 409) }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-snapshot-stage-" + identifier, isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    stagingRoot = root
    let request: NSDictionary = ["options": context, "staging_root": root.path,
      "expected_version": version, "records": snapshot.envelope.records,
      "activation_id": identifier]
    let result = try Self.invoke("prepareSnapshot:", ["request": request, "nextRecord": next])
    guard let handle = Self.strictHandle(result["handle"]) else {
      throw BackendAccountClient.Failure(status: 500)
    }
    prepared = handle
  }
  @MainActor func activate() throws {
    guard let prepared else { throw BackendAccountClient.Failure(status: 400) }
    let versionResult = try Self.invoke("snapshotVersion:", context)
    guard let version = versionResult["version"] as? String else { throw BackendAccountClient.Failure(status: 409) }
    _ = try Self.invoke("applySnapshot:", ["handle": prepared, "expectedVersion": version])
  }
  deinit {
    if let prepared { _ = try? Self.invoke("discardSnapshot:", ["handle": prepared]) }
    if let stagingRoot { try? FileManager.default.removeItem(at: stagingRoot) }
  }
}
