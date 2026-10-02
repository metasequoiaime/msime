import XCTest

final class TypingStatisticsTests: XCTestCase {
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

  /// Retention prunes daily records older than the window and deducts them from the running total and breakdown, as the source recomputes both from the retained days; it survives a reset like the pause does.
  func testRetentionPrunesDailyRecordsAndDeductsThemFromTheTotals() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let today = calendar.date(from: DateComponents(year: 2026, month: 9, day: 23))!
    func day(_ offset: Int) -> Date { calendar.date(byAdding: .day, value: offset, to: today)! }
    for offset in [-40, -30, -29, 0] { try store.record("字", at: day(offset), calendar: calendar) }

    try store.setRetention(30, today: today, calendar: calendar)
    var snapshot = try store.load()
    XCTAssertEqual(snapshot.retentionDays, 30)
    XCTAssertEqual(snapshot.days.keys.sorted(), ["2026-08-24", "2026-08-25", "2026-09-23"], "The shared store keeps the day 30 days back and everything after it")
    XCTAssertEqual(Set(snapshot.dailyDetails.keys), Set(snapshot.days.keys))
    XCTAssertEqual(snapshot.total, 3)
    XCTAssertEqual(snapshot.detail.characters["han"], 3)

    // Recording applies the window from the day being recorded.
    try store.record("字", at: day(30), calendar: calendar)
    XCTAssertEqual(try store.load().days.keys.sorted(), ["2026-09-23", "2026-10-23"])

    try store.reset()
    XCTAssertEqual(try store.load().retentionDays, 30)
    try store.setRetention(nil, today: today, calendar: calendar)
    for offset in [-100, 0] { try store.record("字", at: day(offset), calendar: calendar) }
    snapshot = try store.load()
    XCTAssertNil(snapshot.retentionDays)
    XCTAssertEqual(snapshot.days.count, 2)
  }

  func testActiveTimeAndHoursAreRecordedByTheSharedStore() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let date = calendar.date(from: DateComponents(year: 2026, month: 9, day: 23, hour: 14))!
    try store.record("水杉", source: .quanpin, at: date, calendar: calendar)
    Thread.sleep(forTimeInterval: 0.05)
    try store.record("输入法", source: .quanpin, at: date, calendar: calendar)
    let snapshot = try store.load()
    XCTAssertGreaterThan(snapshot.dailyActiveMs["2026-09-23"] ?? 0, 0, "间隔不到 10 秒的两次上屏算活跃时间")
    XCTAssertLessThan(snapshot.dailyActiveMs["2026-09-23"] ?? 0, 10_000)
    XCTAssertEqual(snapshot.dailyHours["2026-09-23"]?.count, 24)
    XCTAssertEqual(snapshot.dailyHours["2026-09-23"]?[14], 5)

    try store.reset()
    let cleared = try store.load()
    XCTAssertTrue(cleared.dailyActiveMs.isEmpty)
    XCTAssertTrue(cleared.dailyHours.isEmpty)
  }

  func testLongCommitsAreSplitOnCharacterBoundaries() throws {
    let family = "👨‍👩‍👧‍👦"
    let text = String(repeating: family, count: 1_000)
    let chunks = TypingStatisticsStore.chunks(text)
    XCTAssertGreaterThan(chunks.count, 1)
    XCTAssertEqual(chunks.joined(), text)
    XCTAssertTrue(chunks.allSatisfy { $0.utf8.count <= 8_000 && $0.allSatisfy { $0 == Character(family) } })

    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    try store.record(String(repeating: "字", count: 20_000))
    XCTAssertEqual(try store.load().total, 20_000)
  }

  func testRhythmMatchesTheSharedPage() throws {
    let fixture: [String: Any] = [
      "total": 1_000,
      "days": ["2026-08-30": 100, "2026-08-31": 300, "2026-09-01": 200, "2026-09-03": 400],
      "dailyDetails": [
        "2026-08-31": ["characters": ["han": 200, "latin": 40, "otherLetter": 20, "number": 40]],
        "2026-09-01": ["characters": ["han": 100, "punctuation": 100]],
        "2026-09-03": ["characters": ["han": 300, "emoji": 100]],
      ],
      "dailyActiveMs": ["2026-08-31": 120_000, "2026-09-01": 30_000, "2026-09-03": 240_000],
      "dailyHours": ["2026-09-03": Array(repeating: 0, count: 23) + [400], "2026-09-01": [1]],
      "retention": "180d",
    ]
    let statistics = try JSONDecoder().decode(
      TypingStatistics.self, from: JSONSerialization.data(withJSONObject: fixture))
    XCTAssertEqual(statistics.retentionDays, 180)

    let today = statistics.activity(todayKey: "2026-09-03")
    XCTAssertEqual(today.recordedDays, 4)
    XCTAssertEqual(today.averagePerDay, 250)
    XCTAssertEqual(today.currentStreak, 1)
    XCTAssertEqual(today.longestStreak, 3, "跨月的连续天数不能按字符串数字比较")
    XCTAssertEqual(today.bestDay, "2026-09-03")
    XCTAssertEqual(today.bestDayCharacters, 400)
    XCTAssertEqual(today.todayActiveMs, 240_000)
    XCTAssertEqual(today.todaySpeed, 75, accuracy: 0.001, "表情不算进速度")
    XCTAssertEqual(today.averageSpeed, Double(260 + 100 + 300) / 6.5, accuracy: 0.001)
    XCTAssertEqual(today.fastestDay, "2026-08-31", "活跃不到一分钟的那天不参与最快")
    XCTAssertEqual(today.fastestSpeed, 130, accuracy: 0.001)
    XCTAssertEqual(today.todayHours?.last, 400)
    XCTAssertTrue(today.hasActivity)

    // Today with nothing typed yet keeps yesterday's streak; a malformed hour list is not drawn.
    XCTAssertEqual(statistics.activity(todayKey: "2026-09-04").currentStreak, 1)
    XCTAssertEqual(statistics.activity(todayKey: "2026-09-05").currentStreak, 0)
    XCTAssertEqual(statistics.activity(todayKey: "2026-09-02").currentStreak, 3)

    // A document pruned by an older build keeps a running total above its remaining day rows; the average divides the rows, as the shared page does.
    let outlived = try JSONDecoder().decode(
      TypingStatistics.self,
      from: JSONSerialization.data(withJSONObject: ["total": 900, "days": ["2026-09-19": 200, "2026-09-20": 300]]))
    XCTAssertEqual(outlived.activity(todayKey: "2026-09-21").averagePerDay, 250)
    XCTAssertNil(statistics.activity(todayKey: "2026-09-01").todayHours)
    XCTAssertFalse(TypingStatistics().activity(todayKey: "2026-09-01").hasActivity)

    XCTAssertEqual(TypingStatistics.addDays("2028-02-28", 1), "2028-02-29")
    XCTAssertEqual(TypingStatistics.addDays("2026-02-28", 1), "2026-03-01")
    XCTAssertEqual(TypingStatistics.addDays("2026-01-01", -1), "2025-12-31")
    XCTAssertEqual(TypingStatistics.addDays("not-a-day", 1), "not-a-day")
    XCTAssertEqual(TypingActivity.formatActiveTime(0), "0分")
    XCTAssertEqual(TypingActivity.formatActiveTime(45_000), "45秒")
    XCTAssertEqual(TypingActivity.formatActiveTime(12 * 60_000), "12分")
    XCTAssertEqual(TypingActivity.formatActiveTime(60 * 60_000), "1小时")
    XCTAssertEqual(TypingActivity.formatActiveTime(83 * 60_000), "1小时23分")
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

  /// The daily table uses the Windows columns: the four named kinds, everything else under 其他 so a row adds up, and speed over prose only.
  func testDailyRowsFollowTheWindowsColumnsAndExportAsCSV() throws {
    let document = """
    {"enabled":true,"total":230,
     "days":{"2026-09-20":120,"2026-09-21":80,"2026-09-22":30,"not-a-day":5},
     "dailyDetails":{
       "2026-09-20":{"characters":{"han":60,"latin":30,"number":10,"punctuation":8,"emoji":2,"symbol":1,"otherLetter":4},"sources":{}},
       "2026-09-22":{"characters":{"han":20},"sources":{}}},
     "dailyActiveMs":{"2026-09-20":180000,"2026-09-22":30000}}
    """
    let statistics = try JSONDecoder().decode(TypingStatistics.self, from: Data(document.utf8))
    let rows = statistics.dailyRows()
    XCTAssertEqual(rows.map(\.day), ["2026-09-22", "2026-09-21", "2026-09-20"])
    let busy = try XCTUnwrap(rows.last)
    XCTAssertEqual([busy.total, busy.han, busy.latin, busy.number, busy.punctuation, busy.other], [120, 60, 30, 10, 8, 12])
    // 94 prose characters (Han, Latin, other scripts) over three minutes.
    XCTAssertEqual(busy.speed, 94.0 / 3, accuracy: 0.001)
    // A day recorded before the breakdown existed lands entirely under 其他 and has no speed.
    XCTAssertEqual(rows[1].other, 80)
    XCTAssertEqual(rows[1].speed, 0)
    XCTAssertEqual(statistics.dailyRows(limit: 2).map(\.day), ["2026-09-22", "2026-09-21"])

    let csv = statistics.dailyCSV()
    XCTAssertTrue(csv.hasPrefix("\u{FEFF}日期,字数,中文,英文,数字,标点,其他,活跃分钟,速度(字/分)\r\n"))
    let lines = csv.dropFirst().split(separator: "\r\n").map(String.init)
    XCTAssertEqual(lines.count, 4)
    XCTAssertEqual(lines[1], "2026-09-22,30,20,0,0,0,10,0.5,40")
    XCTAssertEqual(lines[3], "2026-09-20,120,60,30,10,8,12,3.0,31")
  }

  /// The ids are a copy of the shared store's whitelist, which rejects a whole batch for one id it does not know, so every one of them has to be accepted.
  func testKeyIDsAreTheSharedWhitelist() throws {
    XCTAssertEqual(TypingKeyID.all.count, 127)
    XCTAssertEqual(TypingKeyID.known.count, 127)
    for id in TypingKeyID.all {
      XCTAssertTrue(id.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber) }, id)
    }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    let everyKey = Dictionary(uniqueKeysWithValues: TypingKeyID.all.map { ($0, 1) })
    XCTAssertEqual(try store.recordKeys(everyKey, day: "2026-09-30"), 127)
    XCTAssertThrowsError(try store.recordKeys(["KeyA": 1, "NineComma": 1], day: "2026-09-30"))
    XCTAssertEqual(try store.load().dailyKeys["2026-09-30"]?["KeyA"], 1)
  }

  func testSoftKeysMapToTheKeyThatTypesTheirCharacter() {
    XCTAssertEqual(TypingKeyID.character("q"), "KeyQ")
    XCTAssertEqual(TypingKeyID.character("Q"), "KeyQ")
    XCTAssertEqual(TypingKeyID.character("7"), "Digit7")
    XCTAssertEqual(TypingKeyID.character("!"), "Digit1")
    XCTAssertEqual(TypingKeyID.character("@"), "Digit2")
    XCTAssertEqual(TypingKeyID.character(","), "Comma")
    XCTAssertEqual(TypingKeyID.character("<"), "Comma")
    XCTAssertEqual(TypingKeyID.character("？"), "Slash")
    XCTAssertEqual(TypingKeyID.character("、"), "Backslash")
    XCTAssertEqual(TypingKeyID.character("——"), "Minus")
    XCTAssertEqual(TypingKeyID.character("“"), "Quote")
    XCTAssertEqual(TypingKeyID.character(" "), "Space")
    // Nothing on a hardware keyboard types these on its own, so they are not counted.
    XCTAssertNil(TypingKeyID.character("€"))
    XCTAssertNil(TypingKeyID.character("😀"))
    XCTAssertNil(TypingKeyID.character("ab"))
    XCTAssertEqual(TypingKeyID.nineKey(1), "Nine1")
    XCTAssertEqual(TypingKeyID.nineKey(0), "Nine0")
    XCTAssertNil(TypingKeyID.nineKey(10))
    // The quick punctuation key counts as the comma key it sits on, whatever mark it types.
    XCTAssertEqual(TypingKeyID.quickPunctuation, "Comma")
    XCTAssertTrue(TypingKeyID.known.contains(TypingKeyID.quickPunctuation))
    XCTAssertEqual((0...10).map(TypingKeyID.japaneseKana),
                   ["Nine1", "Nine2", "Nine3", "Nine4", "Nine5", "Nine6", "Nine7", "Nine8", "Nine9", "Nine0", "SoftPunctuation"])
    XCTAssertNil(TypingKeyID.japaneseKana(11))
    let mapped = (Array("abcdefghijklmnopqrstuvwxyz0123456789").map(String.init)
      + [",", ".", "?", "!", ";", ":", "'", "\"", "@", "/", "(", ")", "[", "]", "<", ">", "\\", "-", "_", "="]
      + ["，", "。", "？", "！", "、", "；", "：", "「", "」"]).map(TypingKeyID.character)
    // Every key the soft keyboard and its symbol layer draw lands on an id the store accepts.
    for id in mapped { XCTAssertTrue(TypingKeyID.known.contains(id ?? ""), String(describing: id)) }
    XCTAssertEqual(TypingKeyID.label("KeyA"), "A")
    XCTAssertEqual(TypingKeyID.label("Digit0"), "0")
    XCTAssertEqual(TypingKeyID.label("Nine2"), "九键 2")
    XCTAssertEqual(TypingKeyID.label("Space"), "空格")
  }

  /// Counts go out in batches, and a press before midnight stays on the day it was pressed even when the batch is written after it.
  func testKeyCounterBatchesByDayAndThreshold() {
    var counter = TypingKeyCounter()
    XCTAssertEqual(counter.record("KeyA", day: "2026-09-30"), [])
    XCTAssertEqual(counter.record("KeyA", day: "2026-09-30"), [])
    XCTAssertEqual(counter.record("Space", day: "2026-09-30"), [])
    // An id the store would reject the whole batch for is dropped on its own.
    XCTAssertEqual(counter.record("NotAKey", day: "2026-09-30"), [])
    XCTAssertEqual(counter.presses, 3)
    XCTAssertEqual(counter.record("KeyB", day: "2026-10-01"),
                   [TypingKeyBatch(day: "2026-09-30", keys: ["KeyA": 2, "Space": 1])])
    XCTAssertEqual(counter.day, "2026-10-01")
    XCTAssertEqual(counter.counts, ["KeyB": 1])
    var ready: [TypingKeyBatch] = []
    for _ in 1..<TypingKeyCounter.flushThreshold { ready += counter.record("Backspace", day: "2026-10-01") }
    XCTAssertEqual(ready, [TypingKeyBatch(day: "2026-10-01", keys: ["KeyB": 1, "Backspace": TypingKeyCounter.flushThreshold - 1])])
    XCTAssertEqual(counter.presses, 0)
    XCTAssertNil(counter.drain())
    _ = counter.record("Enter", day: "2026-10-01")
    XCTAssertEqual(counter.drain(), TypingKeyBatch(day: "2026-10-01", keys: ["Enter": 1]))
    XCTAssertNil(counter.day)
  }

  func testKeyPressCountsFollowTheSwitchAndReset() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    XCTAssertFalse(try store.isEnabled())
    XCTAssertEqual(try store.recordKeys(["KeyA": 3], day: "2026-09-30"), 0)
    XCTAssertTrue(try store.load().dailyKeys.isEmpty)
    try store.setEnabled(true)
    XCTAssertTrue(try store.isEnabled())
    XCTAssertEqual(try store.recordKeys([:], day: "2026-09-30"), 0)
    XCTAssertEqual(try store.recordKeys(["KeyA": 3, "Nine2": 2], day: "2026-09-30"), 5)
    XCTAssertEqual(try store.recordKeys(["KeyA": 1], day: "2026-09-30"), 1)
    XCTAssertEqual(try store.recordKeys(["Space": 4], day: "2026-10-01"), 4)
    let snapshot = try store.load()
    XCTAssertEqual(snapshot.dailyKeys, ["2026-09-30": ["KeyA": 4, "Nine2": 2], "2026-10-01": ["Space": 4]])
    // Key presses are not characters: the character total is untouched.
    XCTAssertEqual(snapshot.total, 0)
    try store.reset()
    XCTAssertTrue(try store.load().dailyKeys.isEmpty)
  }

  /// The 按键 page sums the selected day, or every day, and splits the keys into the drawn keyboard, the nine-key grid, the rest and the top five.
  func testKeyHeatmapFollowsTheScope() throws {
    let old = try JSONDecoder().decode(TypingStatistics.self, from: Data(#"{"enabled":true,"total":0}"#.utf8))
    XCTAssertTrue(old.dailyKeys.isEmpty)
    let document = """
    {"enabled":true,"total":0,
     "dailyKeys":{"2026-09-29":{"KeyA":5,"Space":2,"Digit1":1},
                  "2026-09-30":{"KeyA":118,"KeyB":40,"Nine5":7,"SoftSymbol":3,"Comma":9,"Backspace":40}}}
    """
    let statistics = try JSONDecoder().decode(TypingStatistics.self, from: Data(document.utf8))
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let day = calendar.date(from: DateComponents(year: 2026, month: 9, day: 29))!
    XCTAssertEqual(statistics.keyCounts(on: [day], calendar: calendar), ["KeyA": 5, "Space": 2, "Digit1": 1])
    let all = statistics.keyCounts(on: nil)
    XCTAssertEqual(all["KeyA"], 123)
    XCTAssertEqual(all["Space"], 2)

    let heatmap = TypingKeyHeatmap(counts: all)
    XCTAssertEqual(heatmap.total, 225)
    XCTAssertEqual(heatmap.maximum, 123)
    XCTAssertEqual(heatmap.level("KeyA"), 1)
    XCTAssertEqual(heatmap.level("KeyZ"), 0)
    XCTAssertTrue(heatmap.showsNineKey)
    XCTAssertFalse(TypingKeyHeatmap(counts: statistics.keyCounts(on: [day], calendar: calendar)).showsNineKey)
    // Ties keep id order, so Backspace comes before KeyB.
    XCTAssertEqual(heatmap.top().map(\.id), ["KeyA", "Backspace", "KeyB", "Comma", "Nine5"])
    XCTAssertEqual(heatmap.others.map(\.id), ["Comma", "SoftSymbol", "Digit1"])
    XCTAssertEqual(heatmap.others.map(\.label), [",", "符", "1"])
    XCTAssertEqual(TypingKeyHeatmap.accessibilityLabel("KeyA", count: 123), "A，123 次")
    // Pinyin and kana presses share the nine-key ids, so a cell names both.
    XCTAssertEqual(TypingKeyHeatmap.nineKeySubtitle("Nine2"), "ABC か")
    XCTAssertEqual(TypingKeyHeatmap.nineKeySubtitle("Nine0"), "わ")
    XCTAssertNil(TypingKeyHeatmap.nineKeySubtitle("KeyA"))
    XCTAssertEqual(TypingKeyHeatmap.accessibilityLabel("Nine2", count: 7), "九键 2，拼音 ABC，日文 か，7 次")
    XCTAssertEqual(TypingKeyHeatmap.accessibilityLabel("Nine0", count: 1), "九键 0，日文 わ，1 次")
    XCTAssertEqual(TypingKeyHeatmap.nineKeyRows.joined().map { TypingKeyHeatmap.nineKeyFaces[$0]?.kana },
                   ["あ", "か", "さ", "た", "な", "は", "ま", "や", "ら", "わ"])
    XCTAssertEqual(TypingKeyHeatmap(counts: [:]).level("KeyA"), 0)
  }
}
