import AppKit
import Foundation

protocol DesktopSnapshotAPI {
  func dictionarySnapshot(token: String) async throws -> BackendAccountClient.DownloadedSnapshot
  func dictionaryCatalog(_ kind: BackendAccountClient.DictionaryKind, code: String, offset: Int, scheme: String, profile: String, token: String) async throws -> BackendAccountClient.DictionaryCatalog
  func restoreDictionarySnapshot(file: URL, expectedSHA256: String, revision: Int64, token: String) async throws -> BackendAccountClient.SnapshotRestoreResult
}
extension BackendAccountClient: DesktopSnapshotAPI {}

@MainActor protocol DesktopSnapshotTarget {
  var version: String { get }
  func currentVersion() throws -> String
  func stage(_ snapshot: BackendPreparedSnapshot) async throws -> any DesktopSnapshotActivation
}
@MainActor protocol DesktopSnapshotActivation {
  func ready() throws -> Bool
  func activate() throws
}

@MainActor final class MacDesktopSnapshotTarget: DesktopSnapshotTarget {
  let context: NSDictionary
  let version: String
  init() throws {
    context = try MacPreparedLocalSnapshot.invoke("activeHostOptions")
    guard let version = try MacPreparedLocalSnapshot.invoke("snapshotVersion:", context)["version"] as? String else { throw BackendAccountClient.Failure(status: 503) }
    self.version = version
  }
  func currentVersion() throws -> String {
    let active = try MacPreparedLocalSnapshot.invoke("activeHostOptions")
    guard let version = try MacPreparedLocalSnapshot.invoke("snapshotVersion:", active)["version"] as? String else { throw BackendAccountClient.Failure(status: 503) }
    return version
  }
  func stage(_ snapshot: BackendPreparedSnapshot) async throws -> any DesktopSnapshotActivation {
    guard try currentVersion() == version else { throw BackendAccountClient.Failure(status: 409) }
    let prepared = MacPreparedLocalSnapshot(context: context, snapshot: snapshot)
    let work = Task.detached(priority: .utility) { try prepared.stage() }
    try await withTaskCancellationHandler(operation: { try await work.value }, onCancel: { work.cancel() })
    return MacDesktopSnapshotActivation(prepared: prepared, target: self)
  }
}
@MainActor final class MacDesktopSnapshotActivation: DesktopSnapshotActivation {
  let prepared: MacPreparedLocalSnapshot
  let target: MacDesktopSnapshotTarget
  init(prepared: MacPreparedLocalSnapshot, target: MacDesktopSnapshotTarget) { self.prepared = prepared; self.target = target }
  func ready() throws -> Bool {
    guard try target.currentVersion() == target.version else { throw BackendAccountClient.Failure(status: 409) }
    return try MacPreparedLocalSnapshot.invoke("snapshotActivationReady")["ready"] as? Bool == true
  }
  func activate() throws {
    guard try ready() else { throw BackendAccountClient.Failure(status: 409) }
    try prepared.activate()
  }
}

@MainActor final class DesktopSnapshotFileDialogs {
  func choose(saving: Bool) async throws -> URL? {
    let panel: NSSavePanel = saving ? NSSavePanel() : NSOpenPanel()
    if let open = panel as? NSOpenPanel { open.allowsMultipleSelection = false; open.canChooseDirectories = false }
    panel.nameFieldStringValue = "msime-dictionary-snapshot.ndjson"
    return try await withTaskCancellationHandler(operation: {
      try Task.checkCancellation()
      let result: URL? = await withCheckedContinuation { continuation in
        panel.begin { response in continuation.resume(returning: response == .OK ? panel.url : nil) }
      }
      try Task.checkCancellation()
      return result
    }, onCancel: { Task { @MainActor in panel.cancel(nil) } })
  }
}

/// The webview sees only metadata and one-use preview tokens. Snapshot contents,
/// paths, credentials and Engine handles stay with this native account owner.
@MainActor final class BackendDesktopSnapshots {
  private let client: any DesktopSnapshotAPI
  private let credentials: () async throws -> String
  private let choose: (Bool) async throws -> URL?
  private let capture: @MainActor () throws -> any DesktopSnapshotTarget
  private var preview: (token: String, file: BackendPreparedSnapshot, target: (any DesktopSnapshotTarget)?, revision: Int64)?
  private var job: Task<Void, Never>?
  private var status: [String: Any]?
  private var busy = false
  init(client: any DesktopSnapshotAPI = BackendAccountClient(), credentials: @escaping () async throws -> String,
       choose: @escaping (Bool) async throws -> URL? = { try await DesktopSnapshotFileDialogs().choose(saving: $0) },
       capture: @escaping @MainActor () throws -> any DesktopSnapshotTarget = { try MacDesktopSnapshotTarget() }) {
    self.client = client; self.credentials = credentials; self.choose = choose; self.capture = capture
  }
  deinit { job?.cancel() }
  private func authorize() async throws -> String {
    try Task.checkCancellation()
    let token = try await credentials()
    try Task.checkCancellation()
    return token
  }
  private func metadata(_ file: BackendPreparedSnapshot) throws -> [String: Any] {
    let e = file.envelope
    let size = try FileManager.default.attributesOfItem(atPath: file.url.path)[.size] as? NSNumber
    guard let size else { throw BackendAccountClient.Failure(status: 0) }
    return ["cloudRevision":e.revision, "sha256":e.sha256, "bytes":size, "records":e.records,
      "entries":e.entries, "overlays":e.overlays, "positions":e.positions, "selections":e.selections]
  }
  private func cloudRevision(_ token: String) async throws -> Int64 {
    try await client.dictionaryCatalog(.quick, code: "", offset: 0, scheme: "pinyin", profile: "xiaohe", token: token).revision
  }
  private func freeze(_ url: URL) async throws -> BackendPreparedSnapshot {
    let work = Task.detached(priority: .utility) { try BackendPreparedSnapshot(copying: url) }
    return try await withTaskCancellationHandler(operation: { try await work.value }, onCancel: { work.cancel() })
  }
  func execute(_ request: NSDictionary) async throws -> [String: Any] {
    guard let operation = request["operation"] as? String else { throw BackendAccountClient.Failure(status: 400) }
    if operation == "snapshot_status" {
      return ["nativeFiles":true, "localVersion":try capture().version, "request":status as Any? ?? NSNull()]
    }
    if operation == "snapshot_cancel" {
      // Keep the handle until the task's defer path observes cancellation and
      // releases its prepared snapshot. A synchronous staging call may still
      // be unwinding; clearing job here would allow a second enqueue to race
      // that cleanup and create two native snapshot lifecycles at once.
      job?.cancel()
      if status?["status"] as? String == "preparing" || status?["status"] as? String == "queued" { status?["status"] = "cancelled" }
      return ["request":status as Any? ?? NSNull()]
    }
    if operation == "snapshot_restore_cancel" {
      if preview?.target == nil { preview = nil }
      return ["cancelled":true]
    }
    let token = try await authorize()
    guard !busy else { throw BackendAccountClient.Failure(status: 409) }
    busy = true; defer { busy = false }
    switch operation {
    case "snapshot_preview":
      guard job == nil else { throw BackendAccountClient.Failure(status: 409) }
      preview = nil
      let target = try capture()
      let downloaded = try await client.dictionarySnapshot(token: token)
      defer { try? FileManager.default.removeItem(at: downloaded.url.deletingLastPathComponent()) }
      let frozen = try await freeze(downloaded.url)
      _ = try await authorize()
      guard try target.currentVersion() == target.version else { throw BackendAccountClient.Failure(status: 409) }
      let id = UUID().uuidString
      preview = (id, frozen, target, frozen.envelope.revision)
      return ["previewToken":id, "snapshot":try metadata(frozen), "localVersion":target.version]
    case "snapshot_restore_preview", "snapshot_choose_restore":
      guard job == nil else { throw BackendAccountClient.Failure(status: 409) }
      preview = nil
      guard let source = try await choose(false) else { return ["saved":false] }
      let scoped = source.startAccessingSecurityScopedResource()
      defer { if scoped { source.stopAccessingSecurityScopedResource() } }
      let frozen = try await freeze(source)
      let revision = try await cloudRevision(try await authorize())
      _ = try await authorize()
      let id = UUID().uuidString
      preview = (id, frozen, nil, revision)
      return ["previewToken":id, "snapshot":try metadata(frozen), "expectedRevision":revision]
    case "snapshot_export", "snapshot_save":
      guard let destination = try await choose(true) else { return ["cancelled":true] }
      let downloaded = try await client.dictionarySnapshot(token: try await authorize())
      defer { try? FileManager.default.removeItem(at: downloaded.url.deletingLastPathComponent()) }
      try await MacCloudFileTransfer.save(downloaded.url, to: destination) { _ = try await self.authorize() }
      return ["saved":true]
    case "snapshot_discard":
      guard let id = request["token"] as? String, id == preview?.token else { throw BackendAccountClient.Failure(status: 400) }
      preview = nil; return ["discarded":true]
    case "snapshot_restore_native", "snapshot_restore", "snapshot_restore_prepared":
      guard let selected = preview, selected.target == nil, request["token"] as? String == selected.token else { throw BackendAccountClient.Failure(status: 400) }
      // Consume before network mutation: an uncertain acknowledgement is never retried.
      preview = nil
      let result = try await client.restoreDictionarySnapshot(file: selected.file.url, expectedSHA256: selected.file.envelope.sha256, revision: selected.revision, token: token)
      _ = try await authorize()
      return ["revision":result.revision]
    case "snapshot_enqueue":
      guard job == nil, let selected = preview, let target = selected.target, request["token"] as? String == selected.token else { throw BackendAccountClient.Failure(status: 400) }
      guard try target.currentVersion() == target.version else { throw BackendAccountClient.Failure(status: 409) }
      preview = nil
      let id = UUID().uuidString
      status = ["id":id, "cloudRevision":selected.file.envelope.revision, "expectedLocalVersion":target.version, "fileSha256":selected.file.fileSHA256, "status":"queued"]
      let credentials = self.credentials, client = self.client
      job = Task { [weak self] in
        self?.status?["status"] = "preparing"
        let outcome: String
        do {
          let activation = try await target.stage(selected.file)
          while !(try activation.ready()) { try await Task.sleep(nanoseconds: 100_000_000) }
          let token = try await credentials()
          let cloud = try await client.dictionaryCatalog(.quick, code:"", offset:0, scheme:"pinyin", profile:"xiaohe", token:token)
          _ = try await credentials()
          try Task.checkCancellation()
          guard cloud.revision == selected.file.envelope.revision, try target.currentVersion() == target.version else { throw BackendAccountClient.Failure(status:409) }
          try activation.activate()
          outcome = "applied"
        } catch is CancellationError { outcome = "cancelled" }
        catch let failure as BackendAccountClient.Failure { outcome = failure.status == 409 ? "conflict" : "failed" }
        catch { outcome = "failed" }
        if self?.status?["id"] as? String == id { self?.status?["status"] = outcome; self?.job = nil }
      }
      return ["request":status!]
    default: throw BackendAccountClient.Failure(status: 400)
    }
  }
}
