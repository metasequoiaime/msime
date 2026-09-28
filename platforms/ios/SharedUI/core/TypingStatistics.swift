import Foundation
import Darwin

enum TypingSource: String, CaseIterable {
  case quanpin, nineKey, shuangpin, ziranma, microsoft, shoudao, wubi, japanese, handwriting, english, local, ai, reply, voice, unknown
  var title: String {
    switch self {
    case .quanpin: "全拼 26 键"
    case .nineKey: "全拼 9 键"
    case .shuangpin: "小鹤双拼"
    case .ziranma: "自然码双拼"
    case .microsoft: "微软双拼"
    case .shoudao: "首道双拼"
    case .wubi: "86 五笔"
    case .japanese: "日语"
    case .handwriting: "手写"
    case .english: "英文键盘"
    case .local: "本地输入"
    case .ai: "AI 润色"
    case .reply: "高情商回复"
    case .voice: "语音输入"
    case .unknown: "历史未分类"
    }
  }
}

enum TypingCharacterKind: String, CaseIterable {
  case han, latin, otherLetter, number, punctuation, emoji, symbol, unknown
  var title: String {
    switch self {
    case .han: "汉字"
    case .latin: "拉丁字母"
    case .otherLetter: "其他文字"
    case .number: "数字"
    case .punctuation: "标点"
    case .emoji: "表情"
    case .symbol: "其他符号"
    case .unknown: "历史未分类"
    }
  }
}

struct TypingBreakdown: Decodable {
  var characters: [String: Int] = [:]
  var sources: [String: Int] = [:]
  init() {}
  private enum CodingKeys: String, CodingKey { case characters, sources }
  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    characters = try values.decodeIfPresent([String: Int].self, forKey: .characters) ?? [:]
    sources = try values.decodeIfPresent([String: Int].self, forKey: .sources) ?? [:]
  }
  mutating func merge(_ other: Self) {
    for (key, count) in other.characters { characters[key, default: 0] += count }
    for (key, count) in other.sources { sources[key, default: 0] += count }
  }
  func includingUnclassified(total: Int) -> Self {
    var result = self
    result.characters[TypingCharacterKind.unknown.rawValue, default: 0] += max(0, total - characters.values.reduce(0, +))
    result.sources[TypingSource.unknown.rawValue, default: 0] += max(0, total - sources.values.reduce(0, +))
    return result
  }
}

/// The statistics document as the shared Rust store (`crates/client-core/src/typing_statistics.rs`) writes it; this side only reads it.
struct TypingStatistics: Decodable {
  /// Off until the user turns it on, as the shared store ships it; a document that says otherwise keeps what it says.
  var enabled = false
  var total = 0
  var days: [String: Int] = [:]
  var detail = TypingBreakdown()
  var dailyDetails: [String: TypingBreakdown] = [:]
  /// How many days of daily records to keep, today included; `nil` keeps them all. Days a narrower window removes are also deducted from the running total and breakdown, as the source recomputes its totals from the retained days.
  var retentionDays: Int?
  /// Active typing time per day in milliseconds. A day absent here predates the measurement, which means unknown rather than zero.
  var dailyActiveMs: [String: Int] = [:]
  /// Characters per local hour, 24 buckets per day.
  var dailyHours: [String: [Int]] = [:]

  /// The retention choices the statistics page offers, as on Windows.
  static let retentionChoices = [30, 90, 180, 365]

  /// The shared document's retention id for a window: `30d`, `90d`, `180d`, `365d`, or `forever`.
  static func retentionID(_ days: Int?) -> String {
    guard let days, retentionChoices.contains(days) else { return "forever" }
    return "\(days)d"
  }

  static func retentionDays(_ id: String) -> Int? {
    retentionChoices.first { retentionID($0) == id }
  }

  init() {}
  private enum CodingKeys: String, CodingKey {
    case enabled, total, days, detail, dailyDetails, retention, retentionDays, dailyActiveMs, dailyHours
  }
  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    enabled = try values.decodeIfPresent(Bool.self, forKey: .enabled) ?? false
    total = try values.decodeIfPresent(Int.self, forKey: .total) ?? 0
    days = try values.decodeIfPresent([String: Int].self, forKey: .days) ?? [:]
    detail = try values.decodeIfPresent(TypingBreakdown.self, forKey: .detail) ?? TypingBreakdown()
    dailyDetails = try values.decodeIfPresent([String: TypingBreakdown].self, forKey: .dailyDetails) ?? [:]
    if let retention = try values.decodeIfPresent(String.self, forKey: .retention) {
      retentionDays = Self.retentionDays(retention)
    } else {
      // Written by the Swift store this one replaced, before the shared document was the only format.
      retentionDays = try values.decodeIfPresent(Int.self, forKey: .retentionDays)
    }
    dailyActiveMs = try values.decodeIfPresent([String: Int].self, forKey: .dailyActiveMs) ?? [:]
    dailyHours = try values.decodeIfPresent([String: [Int]].self, forKey: .dailyHours) ?? [:]
  }

  static func dayKey(_ date: Date, calendar: Calendar = .current) -> String {
    let parts = calendar.dateComponents([.year, .month, .day], from: date)
    return String(format: "%04d-%02d-%02d", parts.year!, parts.month!, parts.day!)
  }
  func count(on date: Date, calendar: Calendar = .current) -> Int {
    days[Self.dayKey(date, calendar: calendar)] ?? 0
  }
  func breakdown(on dates: [Date]?, calendar: Calendar = .current) -> TypingBreakdown {
    guard let dates else { return detail.includingUnclassified(total: total) }
    var result = TypingBreakdown()
    for date in dates {
      let key = Self.dayKey(date, calendar: calendar)
      result.merge((dailyDetails[key] ?? TypingBreakdown()).includingUnclassified(total: days[key] ?? 0))
    }
    return result
  }

  /// Speed, streak and hour figures, derived exactly as the shared page's `activityMetrics` does so the two screens never disagree.
  func activity(todayKey: String) -> TypingActivity {
    let recorded = days.keys.filter { Self.parseDayKey($0) != nil }.sorted()
    var result = TypingActivity()
    var totalReadable = 0
    var recordedCharacters = 0
    for key in recorded {
      let activeMs = dailyActiveMs[key] ?? 0
      let readable = TypingActivity.readableCharacters(dailyDetails[key])
      if activeMs > 0 {
        result.totalActiveMs += activeMs
        totalReadable += readable
        if activeMs >= TypingActivity.fastestDayMinimumActiveMs {
          let speed = TypingActivity.charactersPerMinute(readable, activeMs)
          // Strictly greater, so the earliest day keeps a tie.
          if speed > result.fastestSpeed {
            result.fastestSpeed = speed
            result.fastestDay = key
          }
        }
      }
      let characters = days[key] ?? 0
      recordedCharacters += characters
      if characters > result.bestDayCharacters {
        result.bestDayCharacters = characters
        result.bestDay = key
      }
    }
    result.recordedDays = recorded.count
    result.averagePerDay = recorded.isEmpty ? 0 : Double(recordedCharacters) / Double(recorded.count)
    result.todayActiveMs = dailyActiveMs[todayKey] ?? 0
    result.todaySpeed = TypingActivity.charactersPerMinute(
      TypingActivity.readableCharacters(dailyDetails[todayKey]), result.todayActiveMs)
    result.averageSpeed = TypingActivity.charactersPerMinute(totalReadable, result.totalActiveMs)
    result.currentStreak = TypingActivity.currentStreak(recorded, todayKey: todayKey)
    result.longestStreak = TypingActivity.longestStreak(recorded)
    if let hours = dailyHours[todayKey], hours.count == TypingActivity.hours { result.todayHours = hours }
    return result
  }

  private static let dayKeyCalendar: Calendar = {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    return calendar
  }()

  /// A `YYYY-MM-DD` key as a UTC midnight. The keys are calendar labels rather than instants, and local-time arithmetic would lose or repeat a day at a daylight-saving boundary.
  static func parseDayKey(_ key: String) -> Date? {
    let parts = key.split(separator: "-", omittingEmptySubsequences: false)
    guard parts.count == 3, parts[0].count == 4, parts[1].count == 2, parts[2].count == 2,
          let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2]),
          let date = dayKeyCalendar.date(from: DateComponents(year: year, month: month, day: day)),
          dayKey(date, calendar: dayKeyCalendar) == key else { return nil }
    return date
  }

  /// Offsets a day key by whole days; a key that is not a date comes back unchanged.
  static func addDays(_ key: String, _ offset: Int) -> String {
    guard let date = parseDayKey(key),
          let shifted = dayKeyCalendar.date(byAdding: .day, value: offset, to: date) else { return key }
    return dayKey(shifted, calendar: dayKeyCalendar)
  }
}

/// The 输入节奏 figures. Mirrors `ActivityMetrics` in `packages/ui/src/settings/typing-statistics.tsx`.
struct TypingActivity: Equatable {
  static let hours = 24
  /// A day needs this much active time before it can win the fastest day; otherwise a dozen characters typed in two seconds would top it forever.
  static let fastestDayMinimumActiveMs = 60_000
  /// Speed counts prose only: Han, Latin and every other script's letters, so the Japanese mode measures too. Digits, punctuation, emoji and symbols would read as a burst of speed for someone entering a phone number.
  static let speedKinds: [TypingCharacterKind] = [.han, .latin, .otherLetter]

  var recordedDays = 0
  var averagePerDay = 0.0
  var todayActiveMs = 0
  var totalActiveMs = 0
  var todaySpeed = 0.0
  var averageSpeed = 0.0
  var fastestSpeed = 0.0
  var fastestDay: String?
  var currentStreak = 0
  var longestStreak = 0
  var bestDay: String?
  var bestDayCharacters = 0
  /// Today's 24 hourly buckets, or `nil` when no hour was recorded today.
  var todayHours: [Int]?
  /// False when nothing has ever measured active time, which is not the same as zero speed.
  var hasActivity: Bool { totalActiveMs > 0 }

  static func readableCharacters(_ detail: TypingBreakdown?) -> Int {
    speedKinds.reduce(0) { $0 + (detail?.characters[$1.rawValue] ?? 0) }
  }

  static func charactersPerMinute(_ characters: Int, _ activeMs: Int) -> Double {
    guard characters > 0, activeMs > 0 else { return 0 }
    return Double(characters) / (Double(activeMs) / 60_000)
  }

  /// Consecutive recorded days ending today, or ending yesterday while today has no record yet: today is still in progress and must not break a streak.
  static func currentStreak(_ recorded: [String], todayKey: String) -> Int {
    let present = Set(recorded)
    var cursor = present.contains(todayKey) ? todayKey : TypingStatistics.addDays(todayKey, -1)
    var streak = 0
    while present.contains(cursor) {
      streak += 1
      cursor = TypingStatistics.addDays(cursor, -1)
    }
    return streak
  }

  /// The longest run of consecutive recorded days; `recorded` is sorted ascending.
  static func longestStreak(_ recorded: [String]) -> Int {
    guard !recorded.isEmpty else { return 0 }
    var longest = 1
    var run = 1
    for index in recorded.indices.dropFirst() where recorded[index] != recorded[index - 1] {
      run = recorded[index] == TypingStatistics.addDays(recorded[index - 1], 1) ? run + 1 : 1
      longest = max(longest, run)
    }
    return longest
  }

  /// `1小时23分` / `12分` / `45秒`, as the shared page formats it.
  static func formatActiveTime(_ milliseconds: Int) -> String {
    guard milliseconds > 0 else { return "0分" }
    let totalMinutes = milliseconds / 60_000
    if totalMinutes == 0 { return "\(max(1, Int((Double(milliseconds) / 1000).rounded())))秒" }
    let hours = totalMinutes / 60
    let minutes = totalMinutes % 60
    if hours == 0 { return "\(minutes)分" }
    return minutes == 0 ? "\(hours)小时" : "\(hours)小时\(minutes)分"
  }
}

/// One row of the 按日明细 table, in the columns of the Windows statistics page: 日期 字数 中文 英文 数字 标点 其他 活跃 速度.
struct TypingDailyRow: Equatable {
  var day: String
  var total: Int
  var han: Int
  var latin: Int
  var number: Int
  var punctuation: Int
  /// Everything the four named columns leave out: other scripts, emoji, symbols and the unclassified remainder of older records, so the columns always add up to the total.
  var other: Int
  var activeMs: Int
  /// Characters per minute of active time, counted over the same prose kinds as 输入节奏.
  var speed: Double
}

extension TypingStatistics {
  /// Recorded days, newest first. `limit` keeps the most recent ones, as the Windows page lists the last 30.
  func dailyRows(limit: Int? = nil) -> [TypingDailyRow] {
    let keys = days.keys.filter { Self.parseDayKey($0) != nil }.sorted(by: >)
    return keys.prefix(limit ?? keys.count).map { key in
      let total = days[key] ?? 0
      let characters = dailyDetails[key]?.characters ?? [:]
      let han = characters[TypingCharacterKind.han.rawValue] ?? 0
      let latin = characters[TypingCharacterKind.latin.rawValue] ?? 0
      let number = characters[TypingCharacterKind.number.rawValue] ?? 0
      let punctuation = characters[TypingCharacterKind.punctuation.rawValue] ?? 0
      let activeMs = dailyActiveMs[key] ?? 0
      return TypingDailyRow(
        day: key, total: total, han: han, latin: latin, number: number, punctuation: punctuation,
        other: max(0, total - han - latin - number - punctuation), activeMs: activeMs,
        speed: TypingActivity.charactersPerMinute(TypingActivity.readableCharacters(dailyDetails[key]), activeMs))
    }
  }

  /// Every recorded day as CSV, newest first. A byte order mark leads so that Numbers and Excel read the Chinese header as UTF-8; active time is in minutes so a spreadsheet can sum it.
  func dailyCSV() -> String {
    var lines = ["日期,字数,中文,英文,数字,标点,其他,活跃分钟,速度(字/分)"]
    for row in dailyRows() {
      let minutes = String(format: "%.1f", Double(row.activeMs) / 60_000)
      let fields: [String] = [row.day, "\(row.total)", "\(row.han)", "\(row.latin)", "\(row.number)", "\(row.punctuation)", "\(row.other)", minutes, "\(Int(row.speed.rounded()))"]
      lines.append(fields.joined(separator: ","))
    }
    return "\u{FEFF}" + lines.joined(separator: "\r\n") + "\r\n"
  }
}

@_silgen_name("msime_client_typing_statistics")
private func msimeTypingStatistics(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

@_silgen_name("msime_client_string_free")
private func msimeTypingStatisticsStringFree(_ value: UnsafeMutablePointer<CChar>?)

enum TypingStatisticsError: LocalizedError {
  case requestTooLarge
  case invalidResponse
  case store(String)

  var errorDescription: String? {
    switch self {
    case .requestTooLarge: return "统计请求过大"
    case .invalidResponse: return "统计存储返回了无法识别的结果"
    case .store(let message): return message
    }
  }
}

// The keyboard writes only aggregate counts, never document text or preedit. Every read and write goes through the shared Rust store behind `msime_client_typing_statistics`, the one macOS and the shared statistics page use, so active time, the hourly buckets and the retention window are recorded by the same rules everywhere and no host rewrites the document without the fields it does not know. The store's lock file serializes the extension and app processes.
struct TypingStatisticsStore {
  let directory: URL?
  private let legacyDirectory: URL?

  init() {
    let container = FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: "group.app.msime.ios")
    directory = container?.appendingPathComponent("MSIME", isDirectory: true)
    legacyDirectory = container
  }

  init(directory: URL?, legacyDirectory: URL? = nil) {
    self.directory = directory
    self.legacyDirectory = legacyDirectory
  }

  /// The request buffer the ABI accepts.
  private static let maximumRequestBytes = 65_536
  /// A commit is split into pieces this size, well inside the store's 40,000-byte commit limit and, even with every byte escaped, inside the request limit.
  private static let maximumChunkBytes = 8_000
  /// Keep migrations aligned with the shared Rust store's document ceiling before JSON decoding allocates.
  static let maximumDocumentBytes = 64 * 1_048_576

  private static let preparedLock = NSLock()
  private static var prepared = Set<String>()

  private func call(_ action: [String: Any]) throws -> Any {
    guard let directory else { throw CocoaError(.fileNoSuchFile) }
    try prepare(directory)
    let request = try JSONSerialization.data(withJSONObject: ["directory": directory.path, "action": action])
    guard request.count <= Self.maximumRequestBytes else { throw TypingStatisticsError.requestTooLarge }
    let pointer = request.withUnsafeBytes { bytes in
      msimeTypingStatistics(bytes.bindMemory(to: UInt8.self).baseAddress, UInt(request.count))
    }
    guard let pointer else { throw TypingStatisticsError.invalidResponse }
    let response = String(cString: pointer)
    msimeTypingStatisticsStringFree(pointer)
    guard let envelope = try JSONSerialization.jsonObject(with: Data(response.utf8)) as? [String: Any] else {
      throw TypingStatisticsError.invalidResponse
    }
    guard envelope["ok"] as? Bool == true else {
      throw TypingStatisticsError.store(envelope["error"] as? String ?? "统计存储失败")
    }
    return envelope["value"] ?? NSNull()
  }

  /// Once per directory and process: move the pre-shared file out of the App Group root, and carry a retention window the Swift store wrote as `retentionDays` over to the shared `retention` field before the shared store rewrites the document without it.
  private func prepare(_ directory: URL) throws {
    Self.preparedLock.lock()
    defer { Self.preparedLock.unlock() }
    guard !Self.prepared.contains(directory.path) else { return }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    // The shared store takes the same lock file, and flock locks per open file, so this must be released before any call into it.
    let lockURL = directory.appendingPathComponent("typing-statistics.lock")
    let descriptor = open(lockURL.path, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw CocoaError(.fileWriteNoPermission) }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX) == 0 else { throw CocoaError(.fileLocking) }
    defer { flock(descriptor, LOCK_UN) }
    let url = directory.appendingPathComponent("typing-statistics.json")
    try migrateLegacyFileIfNeeded(to: url)
    try migrateLegacyRetention(at: url)
    Self.prepared.insert(directory.path)
  }

  private func migrateLegacyRetention(at url: URL) throws {
    guard FileManager.default.fileExists(atPath: url.path),
          var document = try JSONSerialization.jsonObject(with: Self.readBoundedDocument(from: url)) as? [String: Any],
          let legacy = document["retentionDays"] else { return }
    document.removeValue(forKey: "retentionDays")
    if document["retention"] == nil {
      document["retention"] = TypingStatistics.retentionID((legacy as? NSNumber)?.intValue)
    }
    try JSONSerialization.data(withJSONObject: document).write(to: url, options: .atomic)
  }

  private func migrateLegacyFileIfNeeded(to destination: URL) throws {
    guard !FileManager.default.fileExists(atPath: destination.path),
          let legacyDirectory,
          legacyDirectory.standardizedFileURL != directory?.standardizedFileURL else { return }
    let source = legacyDirectory.appendingPathComponent("typing-statistics.json")
    guard FileManager.default.fileExists(atPath: source.path) else { return }

    let legacyLockURL = legacyDirectory.appendingPathComponent("typing-statistics.lock")
    let descriptor = open(legacyLockURL.path, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
    guard descriptor >= 0 else { throw CocoaError(.fileWriteNoPermission) }
    defer { close(descriptor) }
    guard flock(descriptor, LOCK_EX) == 0 else { throw CocoaError(.fileLocking) }
    defer { flock(descriptor, LOCK_UN) }

    guard !FileManager.default.fileExists(atPath: destination.path),
          FileManager.default.fileExists(atPath: source.path) else { return }
    _ = try JSONDecoder().decode(TypingStatistics.self, from: Self.readBoundedDocument(from: source))
    try FileManager.default.moveItem(at: source, to: destination)
  }

  /// Read only the shared store's accepted document size, even if a legacy file grows after inspection.
  private static func readBoundedDocument(from url: URL) throws -> Data {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }
    var data = Data()
    while data.count <= maximumDocumentBytes {
      let chunk = try handle.read(upToCount: min(65_536, maximumDocumentBytes + 1 - data.count)) ?? Data()
      if chunk.isEmpty { break }
      data.append(chunk)
    }
    guard data.count <= maximumDocumentBytes else {
      throw TypingStatisticsError.store("统计文件过大")
    }
    return data
  }

  func load() throws -> TypingStatistics {
    let value = try call(["operation": "load"])
    return try JSONDecoder().decode(TypingStatistics.self, from: JSONSerialization.data(withJSONObject: value))
  }

  // Why the numbers are empty, answered without going through the write path that may be the thing
  // at fault. The app can always reach the group container; the keyboard extension is the side that
  // can be denied, so an unreachable directory or an absent file each mean something different.
  enum Availability: Equatable {
    case ready(lastWritten: Date?)
    case containerUnavailable
    case neverWritten
  }

  func availability() -> Availability {
    guard let directory else { return .containerUnavailable }
    let url = directory.appendingPathComponent("typing-statistics.json")
    if let attributes = try? FileManager.default.attributesOfItem(atPath: url.path) {
      return .ready(lastWritten: attributes[.modificationDate] as? Date)
    }
    if let legacyDirectory,
       let attributes = try? FileManager.default.attributesOfItem(
         atPath: legacyDirectory.appendingPathComponent("typing-statistics.json").path) {
      return .ready(lastWritten: attributes[.modificationDate] as? Date)
    }
    return .neverWritten
  }

  /// Count one commit. `date` places it on a day and an hour; the active time between commits is measured by the shared store against its own clock.
  func record(_ text: String, source: TypingSource = .unknown, at date: Date = Date(), calendar: Calendar = .current) throws {
    guard text.contains(where: { !$0.isWhitespace }) else { return }
    let day = TypingStatistics.dayKey(date, calendar: calendar)
    let hour = calendar.component(.hour, from: date)
    for chunk in Self.chunks(text) {
      _ = try call(["operation": "record", "text": chunk, "source": source.rawValue, "day": day, "hour": hour])
    }
  }

  /// Splits on character boundaries so no grapheme is counted as two.
  static func chunks(_ text: String) -> [String] {
    var chunks: [String] = []
    var current = ""
    var bytes = 0
    for character in text {
      let size = character.utf8.count
      if bytes + size > maximumChunkBytes, !current.isEmpty {
        chunks.append(current)
        current = ""
        bytes = 0
      }
      current.append(character)
      bytes += size
    }
    if !current.isEmpty { chunks.append(current) }
    return chunks
  }

  /// Set how many days of daily records to keep (`nil` for all) and drop the ones already outside it, rather than waiting for the next keystroke as Windows does.
  func setRetention(_ days: Int?, today: Date = Date(), calendar: Calendar = .current) throws {
    _ = try call(["operation": "set_retention", "retention": TypingStatistics.retentionID(days),
                  "day": TypingStatistics.dayKey(today, calendar: calendar)])
  }

  func setEnabled(_ enabled: Bool) throws {
    _ = try call(["operation": "set_enabled", "enabled": enabled])
  }

  func reset() throws {
    _ = try call(["operation": "reset"])
  }
}
