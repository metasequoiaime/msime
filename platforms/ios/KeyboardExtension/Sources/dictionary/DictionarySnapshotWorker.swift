import Foundation

@MainActor
final class DictionarySnapshotWorker {
  private struct Context: Sendable {
    let resources: URL
    let user: URL
    let content: String
    let options: Data
  }
  private final class Prepared: @unchecked Sendable {
    let value: MSIMEPreparedDictionarySnapshot
    let user: URL
    let request: DictionarySnapshotRequest
    private let lease: DictionarySnapshotQueue.WorkerLease
    init(_ value: MSIMEPreparedDictionarySnapshot, user: URL, request: DictionarySnapshotRequest, lease: DictionarySnapshotQueue.WorkerLease) {
      self.value = value; self.user = user; self.request = request; self.lease = lease
    }
    deinit { try? DictionarySnapshotBridge.discardInactive(identifier: request.id.uuidString, user: user) }
  }
  private let session: MetasequoiaInputSessionBridge
  private let queue: DictionarySnapshotQueue
  private let currentAccountID: () throws -> String?
  private var task: Task<Void, Never>?
  private var prepared: Prepared?
  private var lease: DictionarySnapshotQueue.WorkerLease?
  private var lastVersionCheck = Date.distantPast
  var isPreparing: Bool { task != nil }
  var report: ((String) -> Void)?
  var applied: (() -> Void)?

  init(session: MetasequoiaInputSessionBridge, queue: DictionarySnapshotQueue = .init(),
       currentAccountID: @escaping () throws -> String? = { try BackendKeychain().load()?.tokens.user.id }) {
    self.session = session; self.queue = queue; self.currentAccountID = currentAccountID
  }
  func stop() { task?.cancel(); prepared = nil; lease = nil }
  func tick(idle: Bool, fullAccess: Bool, force: Bool = false) {
    guard fullAccess else { stop(); prepared = nil; lease = nil; return }
    guard idle, task == nil else { return }
    do {
      let state = try queue.read()
      if let request = state.request, request.status.active {
        // Queue cancellation can lose a race with another holder of state.lock. Never prepare
        // or activate that request for a different signed-in account on a later tick.
        guard let owner = try? currentAccountID() else { return }
        if owner != request.accountID {
          try queue.cancel(accountID: request.accountID)
          prepared = nil; lease = nil
          return
        }
      }
      if let prepared {
        guard state.request?.id == prepared.request.id, state.request?.status.active == true else {
          self.prepared = nil; lease = nil; return
        }
        guard let lease else { self.prepared = nil; return }
        let current = try session.localDictionaryStateVersion()
        let success = try queue.complete(id: prepared.request.id, using: lease, currentVersion: current,
          alreadyApplied: current.hasPrefix("local-v1:" + prepared.request.id.uuidString + ":")) {
            try self.session.activateDictionarySnapshot(prepared.value, expectedVersion: prepared.request.expectedLocalVersion)
            return try self.session.localDictionaryStateVersion()
          }
        self.prepared = nil; self.lease = nil
        if success { applied?(); report?("云词库已应用到本机。") }
        else { report?("本地词库已变化，本次快照未应用。请重新确认。") }
        return
      }
      guard force || state.request?.status.active == true || state.localVersion == nil || Date().timeIntervalSince(lastVersionCheck) >= 10 else { return }
      let current = try session.localDictionaryStateVersion()
      try queue.publishLocalVersion(current)
      lastVersionCheck = Date()
      guard try queue.read().request?.status.active == true else { return }
      let lease = try queue.acquireWorkerLease()
      guard let request = try queue.claim(using: lease) else { return }
      if current != request.expectedLocalVersion {
        _ = try queue.complete(id: request.id, using: lease, currentVersion: current, alreadyApplied: false) { current }
        report?("本地词库已变化，本次快照未应用。请重新确认。")
        return
      }
      let raw = try session.dictionarySnapshotContext()
      guard let resources = raw["resources"] as? URL, let user = raw["user"] as? URL,
            let content = raw["contentIdentifier"] as? String,
            let options = raw["preparedOptions"] as? Data else { throw DictionarySnapshotQueue.Failure.invalid }
      let context = Context(resources: resources, user: user, content: content, options: options)
      let file = try queue.fileURL(for: request)
      self.lease = lease
      task = Task { [weak self] in
        let preparation = Task.detached(priority: .utility) {
          try DictionarySnapshotBridge.discardInactive(identifier: request.id.uuidString, user: context.user)
          let snapshot = try BackendPreparedSnapshot(copying: file)
          guard snapshot.fileSHA256 == request.fileSHA256 else { throw DictionarySnapshotQueue.Failure.invalid }
          let stream = try BackendSnapshotRecordStream(snapshot: snapshot)
          let recordCount = snapshot.envelope.overlays + snapshot.envelope.positions + snapshot.envelope.selections
          let value = try DictionarySnapshotBridge.prepare(resources: context.resources, user: context.user,
            identifier: request.id.uuidString, contentIdentifier: context.content,
            maximumRecords: UInt(recordCount), preparedOptions: context.options,
            nextRecord: { failure in
              do { return try stream.next() }
              catch { failure?.pointee = error as NSError; return nil }
            })
          return Prepared(value, user: context.user, request: request, lease: lease)
        }
        do {
          let result = try await withTaskCancellationHandler(operation: { try await preparation.value }, onCancel: { preparation.cancel() })
          try Task.checkCancellation()
          self?.prepared = result
        } catch {
          if !Task.isCancelled, let owner = self, let state = try? owner.queue.read(),
             state.request?.id == request.id, state.request?.status.active == true {
            try? owner.queue.fail(id: request.id)
            owner.report?(error.localizedDescription)
          }
          self?.lease = nil
        }
        self?.task = nil
      }
    } catch {
      // Another holder has the snapshot lease: keep the prepared job and retry next tick.
      if MetasequoiaInputSessionBridge.isSnapshotBusy(error) { return }
      if case DictionarySnapshotQueue.Failure.busy = error { return }
      if let prepared, let version = try? session.localDictionaryStateVersion() {
        if version.hasPrefix("local-v1:" + prepared.request.id.uuidString + ":") {
          try? queue.publishLocalVersion(version)
        } else { try? queue.fail(id: prepared.request.id) }
      }
      prepared = nil; lease = nil
      report?(error.localizedDescription)
    }
  }
}
