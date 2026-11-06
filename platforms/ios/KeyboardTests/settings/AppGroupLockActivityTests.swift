import XCTest

/// `AppGroupLockActivity` 按 `performExpiringActivity` 的约定工作（#10910）：活动开到第一次调用的块返回为止；到期时系统在另一条线程上以 `expired == true` 再调一次，那次调用返回之前锁必须已经在主线程上放掉。这里换上一个假的系统入口，按真机上的线程关系去调块。
@MainActor
final class AppGroupLockActivityTests: XCTestCase {
  /// 假的系统入口：记下每次交来的块，由测试决定在哪条线程上、以什么参数调它。
  private final class FakeSystem: @unchecked Sendable {
    private let lock = NSLock()
    private var blocks: [@Sendable (Bool) -> Void] = []
    var count: Int { lock.lock(); defer { lock.unlock() }; return blocks.count }
    func record(_ block: @escaping @Sendable (Bool) -> Void) { lock.lock(); blocks.append(block); lock.unlock() }
    func block(_ index: Int) -> @Sendable (Bool) -> Void { lock.lock(); defer { lock.unlock() }; return blocks[index] }
  }

  private final class Flag: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var isSet: Bool { lock.lock(); defer { lock.unlock() }; return value }
    func set() { lock.lock(); value = true; lock.unlock() }
  }

  private var previous: AppGroupLockActivity.Perform!
  private let system = FakeSystem()

  override func setUp() {
    super.setUp()
    previous = AppGroupLockActivity.perform
    let system = system
    AppGroupLockActivity.perform = { _, block in system.record(block) }
  }

  override func tearDown() {
    AppGroupLockActivity.perform = previous
    super.tearDown()
  }

  /// 在后台线程上调块，返回它结束时会满足的期望。
  private func call(_ index: Int, expired: Bool, returned flag: Flag? = nil) -> XCTestExpectation {
    let done = expectation(description: "block \(index) expired=\(expired) returned")
    let block = system.block(index)
    DispatchQueue.global().async {
      block(expired)
      flag?.set()
      done.fulfill()
    }
    return done
  }

  func testExpiryReleasesOnTheMainThreadBeforeTheExpiredCallReturns() {
    let activity = AppGroupLockActivity(reason: "test")
    let released = Flag()
    var releasedOnMain = false
    activity.begin {
      releasedOnMain = Thread.isMainThread
      released.set()
    }
    XCTAssertEqual(system.count, 1)
    XCTAssertTrue(activity.isActive)
    let running = Flag()
    let first = call(0, expired: false, returned: running)
    // 第一次调用一直停着：活动开着，系统不挂起。
    RunLoop.current.run(until: Date().addingTimeInterval(0.2))
    XCTAssertFalse(running.isSet, "the activity ended before anything asked it to")

    let expiredReturned = Flag()
    let sawReleaseFirst = Flag()
    let expiredDone = expectation(description: "expired call returned")
    let expiredBlock = system.block(0)
    DispatchQueue.global().async {
      expiredBlock(true)
      if released.isSet { sawReleaseFirst.set() }
      expiredReturned.set()
      expiredDone.fulfill()
    }
    wait(for: [expiredDone, first], timeout: 5)
    XCTAssertTrue(sawReleaseFirst.isSet, "the expired call returned before the locks were released")
    XCTAssertTrue(releasedOnMain, "the release ran off the main thread")
    XCTAssertFalse(activity.isActive)

    // 到期之后再开是新的一段活动。
    activity.begin {}
    XCTAssertEqual(system.count, 2)
    XCTAssertTrue(activity.isActive)
    let second = call(1, expired: false)
    activity.end()
    wait(for: [second], timeout: 5)
  }

  func testEndFinishesTheActivityAndBeginIsIdempotent() {
    let activity = AppGroupLockActivity(reason: "test")
    var expired = false
    activity.begin { expired = true }
    activity.begin { expired = true }
    XCTAssertEqual(system.count, 1, "a second begin opened a second activity")
    let first = call(0, expired: false)
    activity.end()
    wait(for: [first], timeout: 5)
    XCTAssertFalse(activity.isActive)
    XCTAssertFalse(expired)
    activity.end()
  }

  /// 系统一开始就不给活动（进程正要被挂起）时只调一次，`expired` 就是 true：同样要先放锁。
  func testARefusedActivityReleasesAtOnce() {
    let activity = AppGroupLockActivity(reason: "test")
    let released = Flag()
    activity.begin { released.set() }
    let refused = call(0, expired: true)
    wait(for: [refused], timeout: 5)
    XCTAssertTrue(released.isSet)
    XCTAssertFalse(activity.isActive)
  }

  /// 活动的持有者被释放时活动跟着结束，不会留下一条永远停着的线程把进程撑着不挂起。
  func testReleasingTheOwnerEndsTheActivity() {
    var activity: AppGroupLockActivity? = AppGroupLockActivity(reason: "test")
    activity?.begin {}
    let first = call(0, expired: false)
    activity = nil
    wait(for: [first], timeout: 5)
  }
}
