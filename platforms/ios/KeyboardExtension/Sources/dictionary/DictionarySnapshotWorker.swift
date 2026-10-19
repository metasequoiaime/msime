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
  private let readVersion: @Sendable (Data) throws -> String
  private var task: Task<Void, Never>?
  private var prepared: Prepared?
  private var lease: DictionarySnapshotQueue.WorkerLease?
  private var lastVersionCheck = Date.distantPast
  var isPreparing: Bool { task != nil }
  var report: ((String) -> Void)?
  var applied: (() -> Void)?

  init(session: MetasequoiaInputSessionBridge, queue: DictionarySnapshotQueue = .init(),
       currentAccountID: @escaping () throws -> String? = { try BackendKeychain().load()?.tokens.user.id },
       readVersion: @escaping @Sendable (Data) throws -> String = {
         try MetasequoiaInputSessionBridge.localDictionaryStateVersion(preparedOptions: $0)
       }) {
    self.session = session; self.queue = queue; self.currentAccountID = currentAccountID
    self.readVersion = readVersion
  }
  func stop() {
    task?.cancel()
    lease?.release()
    prepared = nil
    lease = nil
  }
  func tick(idle: Bool, fullAccess: Bool, force: Bool = false) {
    guard fullAccess else { stop(); prepared = nil; lease = nil; return }
    guard idle, task == nil else { return }
    do {
      let state = try queue.read()
      if let request = state.request, request.status.active {
        // 取消可能遇到 state.lock 争用；后续轮询仍必须确认账号，不能替另一个账号准备或激活。
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
      let options = try session.dictionaryStateVersionOptions()
      let readVersion = self.readVersion
      task = Task { [weak self] in
        defer { self?.task = nil }
        var claimedRequest: UUID?
        let poll = Task.detached(priority: .utility) { try readVersion(options) }
        do {
          let current = try await withTaskCancellationHandler(operation: { try await poll.value }, onCancel: { poll.cancel() })
          try Task.checkCancellation()
          guard let self else { return }
          try queue.publishLocalVersion(current)
          lastVersionCheck = Date()
          // 读取期间账号可能改变；认领之前重新确认归属。实际激活留给下一次空闲 tick。
          guard let pending = try queue.read().request, pending.status.active,
                let owner = try? currentAccountID() else { return }
          guard owner == pending.accountID else {
            try queue.cancel(accountID: pending.accountID)
            return
          }
          let lease = try queue.acquireWorkerLease()
          guard let request = try queue.claim(using: lease) else { return }
          if current != request.expectedLocalVersion {
            _ = try queue.complete(id: request.id, using: lease, currentVersion: current, alreadyApplied: false) { current }
            report?("本地词库已变化，本次快照未应用。请重新确认。")
            return
          }
          claimedRequest = request.id
          let raw = try session.dictionarySnapshotContext()
          guard let resources = raw["resources"] as? URL, let user = raw["user"] as? URL,
                let content = raw["contentIdentifier"] as? String,
                let options = raw["preparedOptions"] as? Data else { throw DictionarySnapshotQueue.Failure.invalid }
          let context = Context(resources: resources, user: user, content: content, options: options)
          let file = try queue.fileURL(for: request)
          self.lease = lease

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
          let result = try await withTaskCancellationHandler(operation: { try await preparation.value }, onCancel: { preparation.cancel() })
          try Task.checkCancellation()
          self.prepared = result
        } catch {
          guard !Task.isCancelled, let owner = self else { return }
          owner.lease = nil
          if MetasequoiaInputSessionBridge.isSnapshotBusy(error) { return }
          if case DictionarySnapshotQueue.Failure.busy = error { return }
          if let claimedRequest, let state = try? owner.queue.read(),
             state.request?.id == claimedRequest, state.request?.status.active == true {
            try? owner.queue.fail(id: claimedRequest)
          }
          owner.report?(error.localizedDescription)
        }
      }
    } catch {
      // 租约争用时保留准备结果，留到下一次空闲轮询重试。
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
