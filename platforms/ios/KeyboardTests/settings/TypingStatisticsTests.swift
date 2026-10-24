import XCTest
import UIKit

final class TypingStatisticsTests: XCTestCase {
  private var savedSharedPreferences: [String: Any]?
  private var savedAppGroupValues: [String: Any?] = [:]
  private let appGroupKeys = [InputSchemePreference.enabledSchemesKey, "chineseInputScheme",
                              ImeModeMemoryPreference.enabledKey, ImeModeMemoryPreference.lastChineseKey]

  override func setUp() {
    super.setUp()
    savedSharedPreferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
    let defaults = ImeModeMemoryPreference.defaults
    for key in appGroupKeys { savedAppGroupValues[key] = defaults.object(forKey: key) }

    // `KeyboardViewController.viewWillAppear` 先把共享文档镜像到 App Group，再读取方案；两处都固定为同一个中文方案，避免热力图测试依赖前一项测试或模拟器里的用户偏好。
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .quanpin
    _ = InputSchemePreference.update { selection in
      selection.enabled = ChineseInputScheme.allCases
      selection.scheme = .quanpin
    }
    defaults.set(false, forKey: ImeModeMemoryPreference.enabledKey)
    defaults.removeObject(forKey: ImeModeMemoryPreference.lastChineseKey)
  }

  override func tearDown() {
    if let savedSharedPreferences {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in document = savedSharedPreferences }
    }
    let defaults = ImeModeMemoryPreference.defaults
    for key in appGroupKeys {
      if let value = savedAppGroupValues[key] { defaults.set(value, forKey: key) }
      else { defaults.removeObject(forKey: key) }
    }
    savedAppGroupValues.removeAll()
    savedSharedPreferences = nil
    super.tearDown()
  }

  func testEnabledResponseRequiresABooleanField() throws {
    XCTAssertTrue(try TypingStatisticsStore.strictEnabled(["enabled": true]))
    XCTAssertFalse(try TypingStatisticsStore.strictEnabled(["enabled": false]))
    for invalid: Any in [1, 0, "true", NSNull()] {
      XCTAssertThrowsError(try TypingStatisticsStore.strictEnabled(["enabled": invalid]),
                           "enabled: \(invalid)")
    }
    XCTAssertThrowsError(try TypingStatisticsStore.strictEnabled([:]))
    XCTAssertThrowsError(try TypingStatisticsStore.strictEnabled(NSNull()))
  }

  /// 「上传到云端」的本机部分走共享层：没打开时不发；打开后有字数才发，请求体带 `ios` 和今天的计数；上传成功后十分钟内不再发（`force` 不看间隔）；关掉后又不发。
  func testUploadRequestsComeFromTheSharedStore() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    let today = Date()
    XCTAssertEqual(try store.uploadStatus(), .init(enabled: false, lastUploadMs: nil))
    XCTAssertEqual(try store.uploadRequest(day: today, force: true), .skip("disabled"))
    XCTAssertEqual(try store.setUploadEnabled(true), .init(enabled: true, lastUploadMs: nil))
    XCTAssertEqual(try store.uploadRequest(day: today, force: true), .skip("nothing"))
    try store.setEnabled(true)
    try store.record("你好", at: today)
    guard case let .send(path, device, body) = try store.uploadRequest(day: today, force: false) else {
      return XCTFail("expected a request")
    }
    XCTAssertEqual(path, "/v1/users/me/typing-statistics")
    let root = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
    XCTAssertEqual(root["platform"] as? String, "ios")
    XCTAssertEqual(root["device"] as? String, device)
    let days = try XCTUnwrap(root["days"] as? [[String: Any]])
    XCTAssertEqual(days.first?["day"] as? String, TypingStatistics.dayKey(today))
    XCTAssertEqual(days.first?["characters"] as? Int, 2)
    let uploaded = try store.uploadSucceeded(device: device)
    XCTAssertTrue(uploaded.enabled)
    XCTAssertNotNil(uploaded.lastUploadMs)
    XCTAssertEqual(try store.uploadRequest(day: today, force: false), .skip("not_due"))
    if case .skip = try store.uploadRequest(day: today, force: true) { XCTFail("force ignores the interval") }
    XCTAssertEqual(try store.setUploadEnabled(false), .init(enabled: false, lastUploadMs: nil))
    XCTAssertEqual(try store.uploadRequest(day: today, force: true), .skip("disabled"))
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

  /// 键位与共享存储白名单一致；未知键位会拒绝整批，因此每个已知键位都必须可记录。
  func testKeyIDsAreTheSharedWhitelist() throws {
    XCTAssertEqual(TypingKeyID.all.count, 159)
    XCTAssertEqual(TypingKeyID.known.count, 159)
    for id in TypingKeyID.all {
      XCTAssertTrue(id.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber) }, id)
    }
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    try store.setEnabled(true)
    let everyKey = Dictionary(uniqueKeysWithValues: TypingKeyID.all.map { ($0, 1) })
    XCTAssertEqual(try store.recordKeys(everyKey, day: "2026-09-30"), 159)
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
    // 全拼 14 键按键面上的字母记，不记到 26 键的字母上。
    XCTAssertEqual(TypingKeyID.fourteenKey("QW"), "FourteenQW")
    XCTAssertEqual(TypingKeyID.fourteenKey("L"), "FourteenL")
    XCTAssertNil(TypingKeyID.fourteenKey("Q"))
    XCTAssertEqual(TypingKeyID.label("FourteenBN"), "14 键 BN")
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

  /// 动口不动手和换装达人读取存储层的语音时长和试过的皮肤，这两项现在由应用和键盘写入。
  func testVoiceTimeAndSkinsReachTheirBadges() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    XCTAssertEqual(try store.recordVoice(milliseconds: 1_000, day: "2026-10-08"), 0, "nothing is kept while statistics are off")
    XCTAssertFalse(try store.recordSkin(id: "paper"))
    try store.setEnabled(true)
    XCTAssertEqual(try store.recordVoice(milliseconds: 0, day: "2026-10-08"), 0, "an empty recording is not sent")
    XCTAssertEqual(try store.recordVoice(milliseconds: 90_000, day: "2026-10-08"), 90_000)
    XCTAssertEqual(try store.recordVoice(milliseconds: 3_600_000, day: "2026-10-08"),
                   TypingStatisticsStore.maximumVoiceMilliseconds, "one input is clamped to the store's limit, as on Android")
    XCTAssertTrue(try store.recordSkin(id: "paper"))
    XCTAssertFalse(try store.recordSkin(id: "paper"), "a skin counts once")
    XCTAssertTrue(try store.recordSkin(id: GlobalThemeCatalog.customId))
    XCTAssertTrue(try store.recordSkin(id: UUID().uuidString))
    XCTAssertThrowsError(try store.recordSkin(id: "../paper"))
    var utc = Calendar(identifier: .gregorian)
    utc.timeZone = try XCTUnwrap(TimeZone(secondsFromGMT: 0))
    let day = try XCTUnwrap(TypingStatistics.parseDayKey("2026-10-08"))
    let badges = try store.summary(day: day, userWords: nil, calendar: utc).achievements
    XCTAssertEqual(badges.first { $0.id == "voice_1h" }?.current, 11)
    XCTAssertEqual(badges.first { $0.id == "skins_5" }?.current, 3)
  }

  func testRecordCountRejectsMalformedNativeNumbers() {
    XCTAssertEqual(TypingStatisticsStore.strictRecordedCount(NSNumber(value: 3), maximum: 5), 3)
    XCTAssertNil(TypingStatisticsStore.strictRecordedCount(NSNumber(value: true), maximum: 5))
    XCTAssertNil(TypingStatisticsStore.strictRecordedCount(NSNumber(value: 3.5), maximum: 5))
    XCTAssertNil(TypingStatisticsStore.strictRecordedCount(NSNumber(value: -1), maximum: 5))
    XCTAssertNil(TypingStatisticsStore.strictRecordedCount(NSNumber(value: 6), maximum: 5))
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
    // 页面一次只画一块键盘，其他键按当前那块列出：26 键热力图下列出九键格子，九键热力图下列出字母。两块键盘的底行都有 符，所以它不在其他键里。
    XCTAssertEqual(heatmap.others(nineKey: false).map(\.id), ["Nine5", "Digit1"])
    XCTAssertEqual(heatmap.others(nineKey: false).map(\.label), ["九键 5", "1"])
    XCTAssertEqual(heatmap.others(nineKey: true).map(\.id), ["KeyA", "KeyB", "Comma", "Digit1"])
    XCTAssertEqual(TypingKeyHeatmap.accessibilityLabel("KeyA", count: 123), "A，123 次")
    // 画在键盘图上的键读它的键面：分词键记在 `'` 上，读出来仍是「分词」。
    XCTAssertEqual(TypingKeyHeatmap.accessibilityLabel(TypingKeyID.pinyinSeparator, count: 4, face: "分词"), "分词，4 次")
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

  /// 26 键热力图画的就是 iPhone 竖屏的中文全拼 26 键：每一排的键、顺序、键面和相对宽度都从真的键盘上量出来比对。
  @MainActor
  func testLetterHeatmapMatchesThePhoneQuanpinKeyboard() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = Self.phoneKeyboard()
    let views = Self.descendants(controller.view)
    let shift = try XCTUnwrap(views.first { $0.accessibilityIdentifier == "shiftButton" } as? UIButton)
    let thirdRow = try XCTUnwrap(shift.superview as? UIStackView)
    func letterRow(_ first: String) throws -> UIStackView {
      try XCTUnwrap(views.first {
        ($0 as? UIButton)?.configuration?.title?.uppercased() == first && !$0.isHidden
      }?.superview as? UIStackView, "No letter row starting with \(first)")
    }
    let space = try XCTUnwrap(views.first { $0.accessibilityIdentifier == "spaceKey" })
    let bottomRow = try XCTUnwrap(space.superview as? UIStackView)
    let rows = [try letterRow("Q"), try letterRow("A"), thirdRow, bottomRow]
    XCTAssertEqual(rows.count, TypingKeyHeatmap.keyboardRows.count)
    for (row, cells) in zip(rows, TypingKeyHeatmap.keyboardRows) {
      Self.assertRow(row, draws: cells)
    }
    // 第二排两侧各缩进本行宽度的 5%。
    XCTAssertEqual(rows[1].layoutMargins.left / rows[1].bounds.width, TypingKeyHeatmap.keyboardMiddleRowInset, accuracy: 0.005)
    XCTAssertEqual(rows[1].layoutMargins.right / rows[1].bounds.width, TypingKeyHeatmap.keyboardMiddleRowInset, accuracy: 0.005)
  }

  /// 九键热力图画的就是 iPhone 竖屏的拼音九键：侧栏、3×3 格子、右列和底行都从真的键盘上量出来比对。
  @MainActor
  func testNineKeyHeatmapMatchesThePhoneNineKeyKeyboard() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    _ = InputSchemePreference.update { selection in
      selection.enabled = ChineseInputScheme.allCases
      selection.scheme = .nineKey
    }
    let controller = Self.phoneKeyboard()
    let views = Self.descendants(controller.view)
    func view(_ identifier: String) throws -> UIView {
      try XCTUnwrap(views.first { $0.accessibilityIdentifier == identifier }, "No view \(identifier)")
    }
    let sidebar = try view("nineKeySidebar")
    let block = try XCTUnwrap(sidebar.superview)
    // 侧栏里显示的是 ，。？ 三个键，自上而下。
    let marks = Self.descendants(sidebar).compactMap { $0 as? UIButton }.filter { !Self.hidden($0, below: sidebar) }
      .sorted { $0.convert($0.bounds, to: sidebar).minY < $1.convert($1.bounds, to: sidebar).minY }
      .compactMap { $0.configuration?.title }
    XCTAssertEqual(marks.joined(), TypingKeyHeatmap.nineKeySidebar.face)
    XCTAssertEqual(sidebar.bounds.width / block.bounds.width, TypingKeyHeatmap.nineKeySideRatio, accuracy: 0.005)
    // 3×3 格子：键面是 @# 和各组拼音字母。
    for (row, cells) in TypingKeyHeatmap.nineKeyGrid.enumerated() {
      for (column, cell) in cells.enumerated() {
        let digit = row * 3 + column + 1
        XCTAssertEqual(cell.id, TypingKeyID.nineKey(digit))
        let key = try XCTUnwrap(try view("nineKey\(digit)") as? UIButton)
        XCTAssertEqual(key.configuration?.title, cell.face)
      }
    }
    // 右列自上而下：⌫、分词、！，宽度与侧栏相同。
    let controls = ["nineKeyDelete", "nineKeyMiddleKey", "nineKeyClosingMark"]
    let column = try XCTUnwrap(try view(controls[0]).superview as? UIStackView)
    XCTAssertEqual(column.arrangedSubviews.filter { !$0.isHidden }.compactMap(\.accessibilityIdentifier), controls)
    XCTAssertEqual(column.bounds.width, sidebar.bounds.width, accuracy: 0.5)
    XCTAssertEqual(controls.map { Self.recordedID[$0] }, TypingKeyHeatmap.nineKeyControls.map(\.id))
    for (identifier, cell) in zip(controls, TypingKeyHeatmap.nineKeyControls) {
      let title = try XCTUnwrap(try view(identifier) as? UIButton).configuration?.title
      if let title, !title.isEmpty { XCTAssertEqual(title, cell.face, identifier) }
    }
    let bottomRow = try XCTUnwrap(try view("spaceKey").superview as? UIStackView)
    Self.assertRow(bottomRow, draws: TypingKeyHeatmap.nineKeyBottomRow)
  }

  /// 统计页画的键盘没有画面以外的键，也没有存储层不认的 id。
  func testKeyHeatmapDrawsOnlyKnownKeys() {
    let letters = TypingKeyHeatmap.cells(nineKey: false), nine = TypingKeyHeatmap.cells(nineKey: true)
    XCTAssertEqual((letters + nine).map(\.id).filter { !TypingKeyID.known.contains($0) }, [])
    XCTAssertEqual(nine.count, 1 + 9 + 3 + 5)
    // 26 键画的键各不相同；九键上只有侧栏 ，。？ 和右列的 ！ 记同一个 id，这两个键都不印占比。
    XCTAssertTrue(letters.allSatisfy { TypingKeyHeatmap.sharedFaces($0, nineKey: false).isEmpty })
    XCTAssertEqual(nine.filter { !TypingKeyHeatmap.sharedFaces($0, nineKey: true).isEmpty }.map(\.face), ["，。？", "！"])
    XCTAssertEqual(TypingKeyHeatmap.sharedFaces(TypingKeyHeatmap.nineKeySidebar, nineKey: true), ["！"])
    // 两块键盘都没画的键列在其他键里：英文模式的 Shift、九键数字层的 0 和句点。
    let others = TypingKeyHeatmap(counts: [TypingKeyID.shift: 5, "Nine0": 4, "Period": 3, TypingKeyID.pinyinSeparator: 2])
    XCTAssertEqual(others.others(nineKey: false).map(\.id), [TypingKeyID.shift, "Nine0", "Period"])
    XCTAssertEqual(others.others(nineKey: true).map(\.id), [TypingKeyID.shift, "Nine0", "Period"])
  }

  /// 键盘按下各键时 `countKeyPress` 记的 id，按无障碍标识对应。地球键也列出来，好让比对时把它当作热力图不画的键滤掉。
  private static let recordedID: [String: String] = [
    "shiftButton": TypingKeyID.pinyinSeparator, "letterDeleteKey": TypingKeyID.backspace,
    "symbolPanelKey": TypingKeyID.symbol, "layoutToggleButton": TypingKeyID.layer, "inputModeSwitchButton": TypingKeyID.globe,
    "spaceKey": TypingKeyID.space, "bottomLanguageKey": TypingKeyID.language, "returnKey": TypingKeyID.enter,
    "quickPunctuationKey": TypingKeyID.quickPunctuation,
    "nineKeyDelete": TypingKeyID.backspace, "nineKeyMiddleKey": TypingKeyID.pinyinSeparator,
    "nineKeyClosingMark": TypingKeyID.punctuation,
  ]

  /// iPhone 17 竖屏宽度的键盘，默认设置。
  @MainActor
  private static func phoneKeyboard() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 402, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  /// 比对一排：显示着的键依次记的 id、印着字的键的键面（空格印的是方案名，不比），以及每个键占本行键宽之和的比例。iPhone 自带地球键时键盘不画自己的，热力图也不画，所以地球键不算在内。
  @MainActor
  private static func assertRow(_ row: UIStackView, draws cells: [TypingKeyHeatmap.Cell], file: StaticString = #filePath, line: UInt = #line) {
    let keys = row.arrangedSubviews.filter { !$0.isHidden && $0.accessibilityIdentifier != "inputModeSwitchButton" }
    let ids = keys.map { key -> String in
      if let id = key.accessibilityIdentifier.flatMap({ recordedID[$0] }) { return id }
      return "Key\((key as? UIButton)?.configuration?.title?.uppercased() ?? "?")"
    }
    XCTAssertEqual(ids, cells.map(\.id), file: file, line: line)
    guard ids.count == cells.count else { return }
    let keyboardTotal = keys.reduce(0) { $0 + $1.bounds.width }
    let weights = cells.reduce(0) { $0 + $1.width }
    for (key, cell) in zip(keys, cells) {
      if let title = (key as? UIButton)?.configuration?.title, !title.isEmpty, cell.id != TypingKeyID.space {
        XCTAssertEqual(title, cell.face, cell.id, file: file, line: line)
      }
      XCTAssertEqual(Double(key.bounds.width / keyboardTotal), cell.width / weights, accuracy: 0.01, cell.id, file: file, line: line)
    }
  }

  private static func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  /// 视图自己或它到 `root` 之间的某一层是否隐藏。
  private static func hidden(_ view: UIView, below root: UIView) -> Bool {
    var node: UIView? = view
    while let current = node, current !== root {
      if current.isHidden { return true }
      node = current.superview
    }
    return false
  }
}
