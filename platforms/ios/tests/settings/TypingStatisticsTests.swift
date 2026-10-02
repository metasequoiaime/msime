import XCTest

final class TypingStatisticsTests: XCTestCase {
  func testPrepareRejectsASymlinkedStatisticsLock() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-lock-test-\(UUID().uuidString)")
    let outsideDirectory = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-lock-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: directory)
      try? FileManager.default.removeItem(at: outsideDirectory)
    }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outsideDirectory, withIntermediateDirectories: true)
    let outsideLock = outsideDirectory.appendingPathComponent("outside.lock")
    try Data("synthetic-lock-target".utf8).write(to: outsideLock)
    try FileManager.default.createSymbolicLink(
      at: directory.appendingPathComponent("typing-statistics.lock"), withDestinationURL: outsideLock)

    XCTAssertThrowsError(try TypingStatisticsStore(directory: directory).setEnabled(true))
    XCTAssertEqual(try Data(contentsOf: outsideLock), Data("synthetic-lock-target".utf8))
    XCTAssertFalse(FileManager.default.fileExists(
      atPath: directory.appendingPathComponent("typing-statistics.json").path))
  }

  func testPrepareRejectsASymlinkedStatisticsDirectory() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-directory-symlink-root-(UUID().uuidString)")
    let outside = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-directory-symlink-target-(UUID().uuidString)")
    let linked = root.appendingPathComponent("linked", isDirectory: true)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)

    XCTAssertThrowsError(try TypingStatisticsStore(directory: linked).setEnabled(true))
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(at: outside, includingPropertiesForKeys: nil).isEmpty)
  }

  func testCountsCommittedCharactersAcrossDaysAndPreservesPauseOnReset() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let today = calendar.date(from: DateComponents(year: 2026, month: 9, day: 7))!
    let tomorrow = calendar.date(byAdding: .day, value: 1, to: today)!
    try store.record("你好 A1，👨‍👩‍👧‍👦e\u{301}\n\t", at: today, calendar: calendar)
    try store.record("次日", at: tomorrow, calendar: calendar)
    var snapshot = try store.load()
    XCTAssertEqual(snapshot.total, 9)
    XCTAssertEqual(snapshot.count(on: today, calendar: calendar), 7)
    XCTAssertEqual(snapshot.count(on: tomorrow, calendar: calendar), 2)
    try store.setEnabled(false)
    try store.record("不计入", at: today, calendar: calendar)
    XCTAssertEqual(try store.load().total, 9)
    try store.reset()
    snapshot = try store.load()
    XCTAssertEqual(snapshot.total, 0)
    XCTAssertTrue(snapshot.days.isEmpty)
    XCTAssertFalse(snapshot.enabled)
    try store.setEnabled(true)
    try store.record("重新开始", at: today, calendar: calendar)
    XCTAssertEqual(try TypingStatisticsStore(directory: directory).load().total, 4)
    let persisted = try String(contentsOf: directory.appendingPathComponent("typing-statistics.json"), encoding: .utf8)
    XCTAssertFalse(persisted.contains("重新开始"))
  }

  func testUnicodeCategoriesAndLegacyMigration() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    let date = Date()
    let key = TypingStatistics.dayKey(date)
    let old: [String: Any] = ["enabled": true, "total": 12, "days": [key: 12]]
    try JSONSerialization.data(withJSONObject: old).write(to: directory.appendingPathComponent("typing-statistics.json"))
    XCTAssertEqual(try store.load().breakdown(on: nil).characters["unknown"], 12)
    try store.record("汉𠮷Aée\u{301}９1，!👨‍👩‍👧‍👦1️⃣あЖ+ \n", source: .nineKey, at: date)
    var snapshot = try store.load()
    let detail = snapshot.breakdown(on: [date])
    XCTAssertEqual(detail.characters["han"], 2)
    XCTAssertEqual(detail.characters["latin"], 3)
    XCTAssertEqual(detail.characters["number"], 2)
    XCTAssertEqual(detail.characters["punctuation"], 2)
    XCTAssertEqual(detail.characters["emoji"], 2)
    XCTAssertEqual(detail.characters["otherLetter"], 2)
    XCTAssertEqual(detail.characters["symbol"], 1)
    XCTAssertEqual(detail.characters["unknown"], 12)
    XCTAssertEqual(detail.sources["nineKey"], 14)
    XCTAssertEqual(detail.sources["unknown"], 12)
    XCTAssertEqual(snapshot.total, 26)
    try store.record("abc", source: .english, at: date)
    try store.record("日期", source: .local, at: date)
    snapshot = try store.load()
    XCTAssertEqual(snapshot.detail.sources["english"], 3)
    XCTAssertEqual(snapshot.detail.sources["local"], 2)
    XCTAssertEqual(snapshot.breakdown(on: nil).characters.values.reduce(0, +), snapshot.total)
    XCTAssertEqual(snapshot.breakdown(on: nil).sources.values.reduce(0, +), snapshot.total)
    try store.setEnabled(false)
    try store.record("暂停", source: .shuangpin)
    XCTAssertEqual(try store.load().total, 31)
    try store.reset()
    snapshot = try store.load()
    XCTAssertTrue(snapshot.detail.characters.isEmpty)
    XCTAssertTrue(snapshot.dailyDetails.isEmpty)
    XCTAssertFalse(snapshot.enabled)
  }

  func testConcurrentWritersAndForeverKeepsEveryDay() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    try TypingStatisticsStore(directory: directory).setEnabled(true)
    DispatchQueue.concurrentPerform(iterations: 100) { _ in
      try! TypingStatisticsStore(directory: directory).record("字")
    }
    let store = TypingStatisticsStore(directory: directory)
    XCTAssertEqual(try store.load().total, 100)
    for offset in 1...370 {
      try store.record("字", at: Calendar.current.date(byAdding: .day, value: offset, to: Date())!)
    }
    let snapshot = try store.load()
    // Forever, the default, keeps every day: today plus 370 later ones, or one fewer if the loop crosses midnight.
    XCTAssertGreaterThanOrEqual(snapshot.days.count, 370)
    XCTAssertEqual(snapshot.dailyDetails.count, snapshot.days.count)
    XCTAssertEqual(snapshot.total, 470)
    XCTAssertEqual(snapshot.detail.characters["han"], snapshot.total)
    XCTAssertEqual(snapshot.detail.sources["unknown"], snapshot.total)
  }

  func testAvailabilityTellsAnEmptyRunApartFromABrokenOne() throws {
    // The three answers to "why is this empty" are different actions for the reader, so the store
    // has to distinguish them rather than return one emptiness.
    XCTAssertEqual(TypingStatisticsStore(directory: nil).availability(), .containerUnavailable)

    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("stats-availability-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }

    let store = TypingStatisticsStore(directory: directory)
    XCTAssertEqual(store.availability(), .neverWritten)

    try store.setEnabled(true)
    try store.record("水杉")
    guard case .ready(let lastWritten) = store.availability() else {
      return XCTFail("A written store still reported that the keyboard had never written.")
    }
    XCTAssertNotNil(lastWritten)
  }
}
