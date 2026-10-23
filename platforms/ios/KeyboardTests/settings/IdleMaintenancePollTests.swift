import Foundation
import XCTest

@MainActor
final class IdleMaintenancePollTests: XCTestCase {
  func testReadsOffTheMainThreadWithoutOverlapAndDropsCancelledRounds() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = MetasequoiaInputSessionBridge(stateRoot: root.appendingPathComponent("State"))
    let worker = DictionarySnapshotWorker(session: session,
                                          queue: DictionarySnapshotQueue(directory: root.appendingPathComponent("Queue")))
    let poll = IdleMaintenancePoll(store: PersonalDictionaryStore(directory: root))
    let reader = worker.stateReader()
    let entered = expectation(description: "后台读取已开始")
    let gate = DispatchSemaphore(value: 0)
    defer { gate.signal() }
    var cancelledDeliveries = 0
    XCTAssertTrue(poll.start(readSnapshotState: {
      XCTAssertFalse(Thread.isMainThread)
      entered.fulfill()
      XCTAssertEqual(gate.wait(timeout: .now() + 10), .success)
      return reader()
    }) { _ in cancelledDeliveries += 1 })
    await fulfillment(of: [entered], timeout: 5)
    // 后台读取被阻塞时主线程照常运行，上一轮没回来不开新的一轮。
    XCTAssertTrue(poll.isRunning)
    XCTAssertFalse(poll.start(readSnapshotState: reader) { _ in XCTFail("上一轮还在读时开了新的一轮") })

    // 键盘收起后才回来的那一轮不交付；再次出现后的新一轮排在它后面，照常交付。
    poll.cancel()
    XCTAssertFalse(poll.isRunning)
    let delivered = expectation(description: "新一轮交付")
    XCTAssertTrue(poll.start(readSnapshotState: reader) { outcome in
      XCTAssertTrue(Thread.isMainThread)
      // 还没有个人词库同步文件，键盘要同步一次把它建出来。
      XCTAssertEqual(try? outcome.personalDictionary.get(), true)
      delivered.fulfill()
    })
    gate.signal()
    await fulfillment(of: [delivered], timeout: 5)
    XCTAssertEqual(cancelledDeliveries, 0)
    XCTAssertFalse(poll.isRunning)
  }

  func testRemembersAnIdlePersonalDictionaryUntilTheFileIsReplaced() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let session = MetasequoiaInputSessionBridge(stateRoot: root.appendingPathComponent("State"))
    let worker = DictionarySnapshotWorker(session: session,
                                          queue: DictionarySnapshotQueue(directory: root.appendingPathComponent("Queue")))
    let store = PersonalDictionaryStore(directory: root)
    try store.synchronize(apply: { _ in }, page: { _ in .init(entries: [], hasMore: false) })
    let poll = IdleMaintenancePoll(store: store)
    func round() async -> Result<Bool, Error> {
      await withCheckedContinuation { continuation in
        XCTAssertTrue(poll.start(readSnapshotState: worker.stateReader()) { continuation.resume(returning: $0.personalDictionary) })
      }
    }
    let first = await round()
    XCTAssertEqual(try? first.get(), false)
    // 记住了无事可做的文件：读不出来也不再去读。
    let file = root.appendingPathComponent("PersonalDictionary/sync.json")
    try FileManager.default.setAttributes([.posixPermissions: 0], ofItemAtPath: file.path)
    let unchanged = await round()
    try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    XCTAssertEqual(try? unchanged.get(), false)
    // 宿主入队一条编辑后文件被替换，下一轮读到它。
    try store.enqueue(previous: nil, replacement: PersonalWord(key: "ni'hao", value: "拟好"))
    let replaced = await round()
    XCTAssertEqual(try? replaced.get(), true)
  }
}
