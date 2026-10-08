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

  /// 按 `crates/client-core/src/typing_statistics/metrics.rs` 的序列化方式给出的 `summary` 应答，包含 null 字段。
  private static let summaryFixture = """
  {"overview":{"week_total":12846,"previous_week_total":10886,
    "last7":[{"day":"2026-10-01","count":1620},{"day":"2026-10-02","count":2140},{"day":"2026-10-03","count":1880},
             {"day":"2026-10-04","count":2410},{"day":"2026-10-05","count":1990},{"day":"2026-10-06","count":1520},
             {"day":"2026-10-07","count":1286}],
    "average_speed":52.4,"previous_average_speed":48.2,"first_candidate_rate":null,"keystrokes_saved_rate":0.38,
    "current_streak":23,"longest_streak":41},
   "habits":{"weeks12":[{"day":"2026-07-16","count":0},{"day":"2026-07-17","count":12}],
    "hours24":[4,2,1,1,1,2,6,14,30,42,46,40,28,34,44,48,42,36,30,38,52,60,54,22],
    "usual_hours":null,"peak_window":{"start":21,"end":23},"active_days":71,
    "breakdown":{"characters":{"han":820,"latin":90,"otherLetter":20,"number":20,"punctuation":20,"symbol":10,"emoji":20,"unknown":5},
                 "sources":{"quanpin":600,"shuangpin":80,"nineKey":190,"voice":90,"handwriting":40,"ai":30,"unknown":5}}},
   "keys":{"per_character_keys":2.3,"previous_per_character_keys":2.5,"backspace_rate":0.074,"prediction_rate":null,
    "longest_run":null,"positions":[0.71,0.16,0.07,0.06]},
   "achievements":[
    {"id":"chars_10k","glyph":"1万","title":"初出茅庐","description":"累计输入 1 万字","group":"volume","unlocked_day":"2026-09-01","current":15000,"target":10000},
    {"id":"chars_1m","glyph":"百万","title":"著作等身","description":"累计输入 100 万字","group":"volume","unlocked_day":null,"current":483000,"target":1000000},
    {"id":"streak_30","glyph":"30","title":"连续 30 天","description":"连续使用 30 天","group":"streak","unlocked_day":null,"current":23,"target":30},
    {"id":"speed_60","glyph":"60","title":"快手","description":"平均每分钟 60 字","group":"skill","unlocked_day":null,"current":52,"target":60},
    {"id":"words_50","glyph":"词","title":"造词者","description":"添加 50 个自定义词","group":"fun","unlocked_day":null,"current":0,"target":50}]}
  """

  func testSummaryDecodesTheRustShape() throws {
    let summary = try JSONDecoder().decode(TypingSummary.self, from: Data(Self.summaryFixture.utf8))
    XCTAssertEqual(summary.overview.weekTotal, 12846)
    XCTAssertEqual(summary.overview.previousWeekTotal, 10886)
    XCTAssertEqual(summary.overview.last7.count, 7)
    XCTAssertEqual(summary.overview.last7.last, TypingSummary.DayCount(day: "2026-10-07", count: 1286))
    XCTAssertEqual(summary.overview.averageSpeed, 52.4)
    XCTAssertNil(summary.overview.firstCandidateRate)
    XCTAssertEqual(summary.overview.keystrokesSavedRate, 0.38)
    XCTAssertEqual(summary.overview.currentStreak, 23)
    XCTAssertEqual(summary.overview.longestStreak, 41)
    XCTAssertEqual(summary.habits.hours24.count, 24)
    XCTAssertNil(summary.habits.usualHours)
    XCTAssertEqual(summary.habits.peakWindow, TypingSummary.PeakWindow(start: 21, end: 23))
    XCTAssertEqual(summary.habits.activeDays, 71)
    XCTAssertEqual(summary.habits.breakdown.characters["otherLetter"], 20)
    XCTAssertEqual(summary.habits.breakdown.sources["nineKey"], 190)
    XCTAssertEqual(summary.keys.perCharacterKeys, 2.3)
    XCTAssertNil(summary.keys.predictionRate)
    XCTAssertNil(summary.keys.longestRun)
    XCTAssertEqual(summary.keys.positions, [0.71, 0.16, 0.07, 0.06])
    XCTAssertEqual(summary.achievements.map(\.id), ["chars_10k", "chars_1m", "streak_30", "speed_60", "words_50"])
    XCTAssertEqual(summary.unlockedCount, 1)
    XCTAssertTrue(summary.achievements[0].isUnlocked)
    XCTAssertEqual(summary.achievements[0].progress, 1)
    XCTAssertEqual(summary.achievements[1].progress, 0.483, accuracy: 0.0001)
    XCTAssertEqual(summary.achievements[2].group, "streak")

    let run = try JSONDecoder().decode(TypingSummary.Run.self, from: Data(#"{"characters":86,"day":"2026-09-28"}"#.utf8))
    XCTAssertEqual(run, TypingSummary.Run(characters: 86, day: "2026-09-28"))
  }

  func testPeakWindowWrapsPastMidnight() {
    let late = TypingSummary.PeakWindow(start: 23, end: 1)
    XCTAssertTrue(late.contains(23))
    XCTAssertTrue(late.contains(0))
    XCTAssertFalse(late.contains(1))
    XCTAssertFalse(late.contains(22))
    let evening = TypingSummary.PeakWindow(start: 21, end: 23)
    XCTAssertEqual((0..<24).filter(evening.contains), [21, 22])
  }

  /// 文案与 Android 的 `TypingStatisticsSummary` 逐字一致。
  func testSummaryCopyMatchesAndroid() throws {
    XCTAssertEqual(TypingSummaryText.grouped(12846), "12,846")
    XCTAssertEqual(TypingSummaryText.weekDelta(12846, 10886), "比上周多 18%")
    XCTAssertEqual(TypingSummaryText.weekDelta(900, 1000), "比上周少 10%")
    XCTAssertEqual(TypingSummaryText.weekDelta(1000, 1000), "和上周持平")
    XCTAssertNil(TypingSummaryText.weekDelta(1000, 0))
    XCTAssertEqual(TypingSummaryText.whole(52.5), "53")
    XCTAssertEqual(TypingSummaryText.whole(nil), "—")
    XCTAssertEqual(TypingSummaryText.percent(0.386), "39")
    XCTAssertEqual(TypingSummaryText.percent(nil), "—")
    XCTAssertEqual(TypingSummaryText.percentTenths(0.074), "7.4")
    XCTAssertEqual(TypingSummaryText.percentTenths(0.07), "7")
    XCTAssertEqual(TypingSummaryText.decimal(2.3), "2.3")
    XCTAssertEqual(TypingSummaryText.speedDelta(52.4, 48.2), "比上周快 4 字")
    XCTAssertEqual(TypingSummaryText.speedDelta(40, 48), "比上周慢 8 字")
    XCTAssertEqual(TypingSummaryText.speedDelta(48.2, 48.4), "和上周一样快")
    XCTAssertNil(TypingSummaryText.speedDelta(nil, 48))
    XCTAssertEqual(TypingSummaryText.perKeyDelta(2.3, 2.5), "比上周少 0.2 次")
    XCTAssertEqual(TypingSummaryText.perKeyDelta(3, 2), "比上周多 1 次")
    XCTAssertEqual(TypingSummaryText.perKeyDelta(2.31, 2.29), "和上周持平")
    XCTAssertEqual(TypingSummaryText.peakLabel(TypingSummary.PeakWindow(start: 21, end: 23)), "晚上 9–11 点")
    XCTAssertEqual(TypingSummaryText.peakLabel(TypingSummary.PeakWindow(start: 23, end: 1)), "晚上 11–1 点")
    XCTAssertEqual(TypingSummaryText.peakLabel(TypingSummary.PeakWindow(start: 0, end: 2)), "凌晨 12–2 点")
    XCTAssertNil(TypingSummaryText.peakLabel(nil))
    XCTAssertEqual(TypingSummaryText.monthDay("2026-09-28"), "9 月 28 日")

    let summary = try JSONDecoder().decode(TypingSummary.self, from: Data(Self.summaryFixture.utf8))
    let badges = Dictionary(uniqueKeysWithValues: summary.achievements.map { ($0.id, $0) })
    XCTAssertEqual(TypingSummaryText.caption(badges["chars_10k"]!), "累计输入 1 万字")
    XCTAssertEqual(TypingSummaryText.caption(badges["chars_1m"]!), "还差 51.7 万字")
    XCTAssertEqual(TypingSummaryText.caption(badges["streak_30"]!), "还差 7 天")
    XCTAssertEqual(TypingSummaryText.caption(badges["speed_60"]!), "平均每分钟 60 字")
    XCTAssertEqual(TypingSummaryText.caption(badges["words_50"]!), "还差 50 个词")
    XCTAssertEqual(TypingSummaryText.caption(badges["words_50"]!, progressKnown: false), "添加 50 个自定义词")
    XCTAssertEqual(TypingSummaryText.toast(badges["chars_10k"]!), "已解锁「初出茅庐」· 累计输入 1 万字")
    XCTAssertEqual(TypingSummaryText.toast(badges["streak_30"]!), "「连续 30 天」· 还差 7 天")
    XCTAssertEqual(TypingSummaryText.progressLabel(badges["chars_1m"]!), "48%")
    XCTAssertEqual(TypingSummaryText.progressLabel(badges["streak_30"]!), "76%")

    let composition = TypingSummaryText.composition(summary.habits.breakdown.characters)
    XCTAssertEqual(composition.map(\.title), ["汉字", "英文", "符号", "表情"])
    XCTAssertEqual(composition.map(\.count), [820, 110, 50, 20])
    XCTAssertEqual(TypingSummaryText.composition([:]).map(\.count), [0, 0, 0, 0])
    let methods = TypingSummaryText.methods(summary.habits.breakdown.sources)
    XCTAssertEqual(methods, [.init(title: "26 键", count: 680), .init(title: "9 键", count: 190),
                             .init(title: "语音", count: 90), .init(title: "手写", count: 40)])
    XCTAssertEqual(TypingSummaryText.methods(["quanpin": 10]).map(\.title), ["26 键"])
    XCTAssertEqual(TypingSummaryText.share(680, of: 1000), 68)
    XCTAssertEqual(TypingSummaryText.share(1, of: 0), 0)
  }

  func testStoreSummaryAnswersForAnEmptyAndARecordedStore() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("stats-summary-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = TypingStatisticsStore(directory: directory)
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let today = calendar.date(from: DateComponents(year: 2026, month: 10, day: 7, hour: 21))!

    let empty = try store.summary(day: today, userWords: nil, calendar: calendar)
    XCTAssertEqual(empty.overview.weekTotal, 0)
    XCTAssertEqual(empty.overview.last7.map(\.day).last, "2026-10-07")
    XCTAssertEqual(empty.overview.last7.count, 7)
    XCTAssertEqual(empty.habits.weeks12.count, 84)
    XCTAssertEqual(empty.habits.hours24.count, 24)
    XCTAssertNil(empty.overview.averageSpeed)
    XCTAssertNil(empty.keys.positions)
    XCTAssertEqual(empty.achievements.count, 16)
    XCTAssertEqual(empty.unlockedCount, 0)

    try store.setEnabled(true)
    try store.record("水杉输入法", source: .nineKey, at: today, calendar: calendar)
    let recorded = try store.summary(day: today, userWords: 60, calendar: calendar)
    XCTAssertEqual(recorded.overview.weekTotal, 5)
    XCTAssertEqual(recorded.overview.last7.last?.count, 5)
    XCTAssertEqual(recorded.overview.currentStreak, 1)
    XCTAssertEqual(recorded.habits.activeDays, 1)
    XCTAssertEqual(recorded.habits.hours24[21], 5)
    XCTAssertEqual(recorded.habits.breakdown.sources["nineKey"], 5)
    XCTAssertEqual(TypingSummaryText.methods(recorded.habits.breakdown.sources).map(\.title), ["9 键"])
    // 造词者 徽章按宿主传入的词数解锁；由 Rust 判定并记录。
    XCTAssertNotNil(recorded.achievements.first { $0.id == TypingSummary.userWordsAchievement }?.unlockedDay)
  }

  func testUserWordsComeOnlyFromASnapshotThatVouchesForThem() {
    func word(_ kind: PersonalWordKind, _ key: String) -> PersonalWord { PersonalWord(kind: kind, key: key, value: key) }
    var state = PersonalDictionaryState()
    state.entries = [word(.pinyin, "a"), word(.pinyin, "b"), word(.quickPhrase, "c")]
    XCTAssertNil(TypingSummary.userWords(from: state), "the keyboard never listed the dictionary")
    state.snapshotDate = Date()
    XCTAssertEqual(TypingSummary.userWords(from: state), 2)
    state.hasMore = true
    XCTAssertNil(TypingSummary.userWords(from: state), "a partial page is only a lower bound")
    state.entries = (0..<60).map { word(.pinyin, "w\($0)") }
    XCTAssertEqual(TypingSummary.userWords(from: state), 60, "a lower bound that already reaches the badge")
    state.hasMore = false
    state.pageQuery = "w"
    XCTAssertNil(TypingSummary.userWords(from: state), "a search is not the whole list")
    state.pageQuery = ""
    state.pageKind = .pinyin
    XCTAssertNil(TypingSummary.userWords(from: state))
  }

  func testKeyHeatLevelsFollowTheDesignThresholds() {
    XCTAssertEqual([7.0, 6.9, 4.5, 2, 1.9, 0.6, 0.5, 0].map { TypingKeyHeatmap.heatLevel(percent: $0, nineKey: false) },
                   [4, 3, 3, 2, 1, 1, 0, 0])
    XCTAssertEqual([12.0, 11.9, 9, 5, 4.9, 0.1, 0].map { TypingKeyHeatmap.heatLevel(percent: $0, nineKey: true) },
                   [4, 3, 3, 2, 1, 1, 0])
    let heatmap = TypingKeyHeatmap(counts: ["KeyN": 96, "KeyA": 61, "Space": 89, "Nine4": 162, "Nine2": 94, "Nine1": 300, "Digit1": 198])
    XCTAssertEqual(heatmap.percent("KeyN"), 9.6, accuracy: 0.0001)
    XCTAssertEqual(heatmap.busiestLetterKey(nineKey: false), "KeyN")
    // 分隔格 1 不带字母，所以胜出的是带字母的格子里按得最多的那个。
    XCTAssertEqual(heatmap.busiestLetterKey(nineKey: true), "Nine4")
    XCTAssertNil(TypingKeyHeatmap(counts: [:]).busiestLetterKey(nineKey: false))
    // 九键键盘的侧边键在九键热力图上有自己的位置，所以不列在其他键里；26 键热力图上没有它们，就列出来。
    let sides = TypingKeyHeatmap(counts: ["Period": 3, TypingKeyID.punctuation: 2, "Comma": 1])
    XCTAssertEqual(sides.others(nineKey: true).map(\.id), ["Comma"])
    XCTAssertEqual(sides.others(nineKey: false).map(\.id), ["Period", "SoftPunctuation", "Comma"])
  }
}
