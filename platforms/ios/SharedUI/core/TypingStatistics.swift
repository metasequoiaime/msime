import Foundation
import Darwin
import CoreFoundation

enum TypingSource: String, CaseIterable {
  case quanpin, nineKey, shuangpin, ziranma, microsoft, shoudao, wubi, japanese, korean, cantonese, zhuyin, vietnamese, tibetan, stroke, handwriting, english, local, ai, reply, voice, unknown
  var title: String {
    switch self {
    case .quanpin: "全拼 26 键"
    case .nineKey: "全拼 9 键"
    case .shuangpin: "小鹤双拼"
    case .ziranma: "自然码双拼"
    case .microsoft: "微软双拼"
    case .shoudao: "首道双拼"
    // 86 与 98 五笔共用一个统计来源，这里只写「五笔」。
    case .wubi: "五笔"
    case .japanese: "日语"
    case .korean: "韩语"
    case .cantonese: "粤语"
    case .zhuyin: "注音"
    case .vietnamese: "越南语"
    case .tibetan: "藏文"
    case .stroke: "笔画"
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

struct TypingBreakdown: Decodable, Equatable {
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
  /// Key presses per local day, keyed by the ids in `TypingKeyID.all`. Only how many times each key went down that day: no order, no time of day, no text. Documents written before the key heatmap have no such field.
  var dailyKeys: [String: [String: Int]] = [:]

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
    case enabled, total, days, detail, dailyDetails, retention, dailyActiveMs, dailyHours, dailyKeys
  }
  init(from decoder: Decoder) throws {
    let values = try decoder.container(keyedBy: CodingKeys.self)
    enabled = try values.decodeIfPresent(Bool.self, forKey: .enabled) ?? false
    total = try values.decodeIfPresent(Int.self, forKey: .total) ?? 0
    days = try values.decodeIfPresent([String: Int].self, forKey: .days) ?? [:]
    detail = try values.decodeIfPresent(TypingBreakdown.self, forKey: .detail) ?? TypingBreakdown()
    dailyDetails = try values.decodeIfPresent([String: TypingBreakdown].self, forKey: .dailyDetails) ?? [:]
    retentionDays = try values.decodeIfPresent(String.self, forKey: .retention).flatMap(Self.retentionDays)
    dailyActiveMs = try values.decodeIfPresent([String: Int].self, forKey: .dailyActiveMs) ?? [:]
    dailyHours = try values.decodeIfPresent([String: [Int]].self, forKey: .dailyHours) ?? [:]
    dailyKeys = try values.decodeIfPresent([String: [String: Int]].self, forKey: .dailyKeys) ?? [:]
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

/// The key ids the shared store accepts, W3C `KeyboardEvent.code` names plus the on-screen keys a touch keyboard has and a hardware one does not. A copy of `KEY_IDS` in `crates/client-core/src/typing_statistics.rs`: the store rejects a whole batch that carries any other id, so a key that maps to nothing here is left uncounted rather than given a made-up name.
enum TypingKeyID {
  // Built from typed parts: as one chain of `+` over closures and literals, the Xcode 26.2 compiler gives up with
  // "unable to type-check this expression in reasonable time" and the keyboard extension does not build.
  static let all: [String] = {
    let letters: [String] = (UnicodeScalar("A").value...UnicodeScalar("Z").value).map { "Key\(Character(UnicodeScalar($0)!))" }
    let digits: [String] = (0...9).map { "Digit\($0)" }
    let editing: [String] = [
      "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "Semicolon", "Quote", "Comma", "Period", "Slash",
      "IntlBackslash", "IntlRo", "IntlYen", "Lang1", "Lang2", "Convert", "NonConvert", "KanaMode",
      "Space", "Enter", "Backspace", "Tab", "Escape", "Delete", "Insert", "Home", "End", "PageUp", "PageDown",
      "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
      "CapsLock", "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "Fn", "ContextMenu"]
    let functionKeys: [String] = (1...12).map { "F\($0)" }
    let numpadDigits: [String] = (0...9).map { "Numpad\($0)" }
    let numpad: [String] = ["NumpadDecimal", "NumpadEnter", "NumpadAdd", "NumpadSubtract", "NumpadMultiply", "NumpadDivide", "NumLock"]
    let nineKey: [String] = (0...9).map { "Nine\($0)" }
    let soft: [String] = ["SoftPunctuation", "SoftSymbol", "SoftLayer", "SoftLanguage", "SoftGlobe", "SoftEmoji", "SoftVoice"]
    var keys: [String] = letters
    for part in [digits, editing, functionKeys, numpadDigits, numpad, nineKey, soft] {
      keys += part
    }
    return keys
  }()
  static let known = Set(all)

  static let space = "Space"
  static let enter = "Enter"
  static let backspace = "Backspace"
  static let tab = "Tab"
  static let shift = "ShiftLeft"
  /// iPad 键盘的第二个 ⇧，在第三排字母键的最右端。
  static let shiftRight = "ShiftRight"
  static let punctuation = "SoftPunctuation"
  /// The bottom-row quick punctuation key, which sits where a hardware comma does whichever mark it types.
  static let quickPunctuation = "Comma"
  static let symbol = "SoftSymbol"
  static let layer = "SoftLayer"
  static let language = "SoftLanguage"
  static let globe = "SoftGlobe"
  static let emoji = "SoftEmoji"
  static let voice = "SoftVoice"

  /// 九键格里的一格，按格上印的数字编号。1 号格是 @#，点开符号面板；0 是最下一排的那个键。
  static func nineKey(_ digit: Int) -> String? {
    (0...9).contains(digit) ? "Nine\(digit)" : nil
  }

  /// A cell of the Japanese kana grid by its index in `JapaneseNineKeyView.keys`: あ through ら sit where 1 to 9 sit on a phone keypad, わ where 0 sits, and the punctuation cell beside it is side punctuation.
  static func japaneseKana(_ index: Int) -> String? {
    switch index {
    case 0...8: return nineKey(index + 1)
    case 9: return nineKey(0)
    case 10: return punctuation
    default: return nil
    }
  }

  /// The US ANSI key that types `text` on its own, shifted or not, so a soft symbol key lands on the key a hardware keyboard would use for the same mark. Chinese punctuation goes to the key the Chinese layout types it with. Text no single key types, such as an emoji or `€`, has no id.
  static func character(_ text: String) -> String? {
    if let mapped = characterKeys[text] { return mapped }
    guard text.count == 1, let scalar = text.unicodeScalars.first, scalar.isASCII else { return nil }
    let character = Character(scalar)
    if character.isLetter { return "Key\(character.uppercased())" }
    if let digit = character.wholeNumberValue { return "Digit\(digit)" }
    return nil
  }

  private static let characterKeys: [String: String] = [
    " ": "Space", "\n": "Enter", "\t": "Tab",
    "`": "Backquote", "~": "Backquote", "·": "Backquote",
    "!": "Digit1", "！": "Digit1", "@": "Digit2", "#": "Digit3", "$": "Digit4", "￥": "Digit4", "%": "Digit5",
    "^": "Digit6", "……": "Digit6", "&": "Digit7", "*": "Digit8", "(": "Digit9", "（": "Digit9", ")": "Digit0", "）": "Digit0",
    "-": "Minus", "_": "Minus", "——": "Minus", "=": "Equal", "+": "Equal",
    "[": "BracketLeft", "{": "BracketLeft", "【": "BracketLeft", "「": "BracketLeft",
    "]": "BracketRight", "}": "BracketRight", "】": "BracketRight", "」": "BracketRight",
    "\\": "Backslash", "|": "Backslash", "、": "Backslash",
    ";": "Semicolon", ":": "Semicolon", "；": "Semicolon", "：": "Semicolon",
    "'": "Quote", "\"": "Quote", "‘": "Quote", "’": "Quote", "“": "Quote", "”": "Quote",
    ",": "Comma", "<": "Comma", "，": "Comma", "《": "Comma",
    ".": "Period", ">": "Period", "。": "Period", "》": "Period",
    "/": "Slash", "?": "Slash", "？": "Slash",
  ]

  private static let names: [String: String] = [
    "Backquote": "`", "Minus": "-", "Equal": "=", "BracketLeft": "[", "BracketRight": "]", "Backslash": "\\",
    "Semicolon": ";", "Quote": "'", "Comma": ",", "Period": ".", "Slash": "/",
    "Space": "空格", "Enter": "换行", "Backspace": "删除", "Tab": "Tab", "Escape": "Esc", "Delete": "向后删除",
    "ArrowUp": "↑", "ArrowDown": "↓", "ArrowLeft": "←", "ArrowRight": "→",
    "CapsLock": "大写锁定", "ShiftLeft": "Shift", "ShiftRight": "右 Shift",
    "SoftPunctuation": "九键侧栏标点", "SoftSymbol": "符", "SoftLayer": "123", "SoftLanguage": "中/英",
    "SoftGlobe": "切换键盘", "SoftEmoji": "表情", "SoftVoice": "语音",
  ]

  /// What the statistics page prints for a key: the letter or mark on its face, or its name in Chinese.
  static func label(_ id: String) -> String {
    if let name = names[id] { return name }
    if id.hasPrefix("Key") { return String(id.dropFirst(3)) }
    if id.hasPrefix("Digit") { return String(id.dropFirst(5)) }
    if id.hasPrefix("Nine") { return "九键 \(id.dropFirst(4))" }
    return id
  }
}

/// One batch for the store: press counts for the local day they were pressed on.
struct TypingKeyBatch: Equatable {
  let day: String
  let keys: [String: Int]
}

/// The keyboard's in-memory tally between writes. The store locks, rewrites and fsyncs the whole document per call, so presses are counted here and handed over in batches: when this many have piled up, when the day turns, and whenever the keyboard flushes on its timer or on going away. The counts belong to the day they were pressed on, never the day they happen to be written.
struct TypingKeyCounter {
  static let flushThreshold = 256

  private(set) var day: String?
  private(set) var counts: [String: Int] = [:]
  private(set) var presses = 0

  /// Count one press of `id` on `day`, and return what has to be written now: the earlier day's counts when the day changed, then this day's when they reached the threshold. An id the store does not know is dropped, so one stray key never costs the rest of the batch.
  mutating func record(_ id: String, day: String) -> [TypingKeyBatch] {
    guard TypingKeyID.known.contains(id) else { return [] }
    var ready: [TypingKeyBatch] = []
    if self.day != day, let previous = drain() { ready.append(previous) }
    self.day = day
    counts[id, default: 0] += 1
    presses += 1
    if presses >= Self.flushThreshold, let full = drain() { ready.append(full) }
    return ready
  }

  /// Everything counted so far, emptying the tally; `nil` when nothing was.
  mutating func drain() -> TypingKeyBatch? {
    defer {
      day = nil
      counts = [:]
      presses = 0
    }
    guard let day, !counts.isEmpty else { return nil }
    return TypingKeyBatch(day: day, keys: counts)
  }
}

/// The 按键 page's figures: the soft keyboard it draws, the nine-key grid when there are nine-key presses to show, the keys neither has a place for, and the most pressed keys.
struct TypingKeyHeatmap: Equatable {
  struct Key: Equatable {
    let id: String
    let label: String
    let count: Int
  }

  /// The 26-key keyboard as the touch keyboard lays it out, with its bottom row.
  static let keyboardRows: [[String]] = [
    "QWERTYUIOP".map { "Key\($0)" },
    "ASDFGHJKL".map { "Key\($0)" },
    [TypingKeyID.shift] + "ZXCVBNM".map { "Key\($0)" } + [TypingKeyID.backspace],
    [TypingKeyID.layer, TypingKeyID.globe, TypingKeyID.space, TypingKeyID.language, TypingKeyID.enter],
  ]
  /// The nine-key grid, 1 to 9 and the 0 below them.
  static let nineKeyRows: [[String]] = [
    ["Nine1", "Nine2", "Nine3"], ["Nine4", "Nine5", "Nine6"], ["Nine7", "Nine8", "Nine9"], ["Nine0"],
  ]
  /// 按键盘实际排布的九键键盘：左侧是纵贯整个格区的标点侧栏，中间是 3×3 的格子，右侧从上到下叠着删除、句号和 0。最下一排与 26 键相同。
  static let nineKeySidebar = TypingKeyID.punctuation
  static let nineKeyGrid: [[String]] = [["Nine1", "Nine2", "Nine3"], ["Nine4", "Nine5", "Nine6"], ["Nine7", "Nine8", "Nine9"]]
  static let nineKeyControls: [String] = [TypingKeyID.backspace, "Period", "Nine0"]
  /// 两块键盘各自画出的键：26 键是 `keyboardRows`；九键是标点侧栏、3×3 格子、右侧控制键和与 26 键相同的最下一排。
  private static let letterDrawn = Set(keyboardRows.joined())
  private static let nineKeyDrawn = Set([nineKeySidebar] + nineKeyGrid.joined() + nineKeyControls + (keyboardRows.last ?? []))

  /// 共用同一 id 的两套格子上，一个九键格各自承载的内容：拼音字母（1 是 @#，0 没有字母），以及 `TypingKeyID.japaneseKana` 放在这里的假名行首字，1 到 9 是あ到ら，0 是わ。
  static let nineKeyFaces: [String: (pinyin: String, kana: String)] = [
    "Nine1": ("@#", "あ"), "Nine2": ("ABC", "か"), "Nine3": ("DEF", "さ"),
    "Nine4": ("GHI", "た"), "Nine5": ("JKL", "な"), "Nine6": ("MNO", "は"),
    "Nine7": ("PQRS", "ま"), "Nine8": ("TUV", "や"), "Nine9": ("WXYZ", "ら"),
    "Nine0": ("", "わ"),
  ]

  /// The small print under a nine-key cell's digit, pinyin letters then kana: `ABC か`.
  static func nineKeySubtitle(_ id: String) -> String? {
    guard let face = nineKeyFaces[id] else { return nil }
    return [face.pinyin, face.kana].filter { !$0.isEmpty }.joined(separator: " ")
  }

  let counts: [String: Int]

  init(counts: [String: Int]) {
    self.counts = counts.filter { $0.value > 0 }
  }

  var total: Int { counts.values.reduce(0, +) }
  var maximum: Int { counts.values.max() ?? 0 }
  var showsNineKey: Bool { Self.nineKeyRows.joined().contains { counts[$0] != nil } }

  func count(_ id: String) -> Int { counts[id] ?? 0 }

  /// Where a count sits between none and the busiest key, 0 to 1.
  func level(_ id: String) -> Double {
    let maximum = maximum
    return maximum == 0 ? 0 : Double(count(id)) / Double(maximum)
  }

  /// 某个键在窗口内全部按键次数中的占比，单位为百分比，分母也算上没画在任何键盘上的键；一次都没按过时为 0。
  func percent(_ id: String) -> Double {
    let total = total
    return total == 0 ? 0 : Double(count(id)) * 100 / Double(total)
  }

  /// 设计稿按键位占全部按键次数的比例分出的五级热度：26 键键盘上是 ≥7%、≥4.5%、≥2% 和超过 0.5%；九键键盘上一格承载三到四个字母，所以是 ≥12%、≥9%、≥5% 和只要按过。
  static func heatLevel(percent: Double, nineKey: Bool) -> Int {
    let thresholds: [Double] = nineKey ? [12, 9, 5] : [7, 4.5, 2]
    if percent >= thresholds[0] { return 4 }
    if percent >= thresholds[1] { return 3 }
    if percent >= thresholds[2] { return 2 }
    return percent > (nineKey ? 0 : 0.5) ? 1 : 0
  }

  /// 某个键盘上按得最多的字母键：26 键键盘取 A 到 Z，九键键盘取带字母的 2 到 9 号格；一个都没按过时为 nil。并列时按键盘顺序取前者。
  func busiestLetterKey(nineKey: Bool) -> String? {
    let candidates = nineKey ? (2...9).map { "Nine\($0)" } : "QWERTYUIOPASDFGHJKLZXCVBNM".map { "Key\($0)" }
    var best: String?
    var most = 0
    for id in candidates where count(id) > most {
      best = id
      most = count(id)
    }
    return best
  }

  /// 当前显示的那块键盘上没有位置的已按键，按次数从多到少排列。页面一次只画一块键盘，所以 26 键热力图下也列出 。 键、九键侧栏标点和九键格子。
  func others(nineKey: Bool) -> [Key] {
    let drawn = nineKey ? Self.nineKeyDrawn : Self.letterDrawn
    return ranked.filter { !drawn.contains($0.id) }
  }

  /// The most pressed keys; ties keep the id order so the list does not shuffle between loads.
  func top(_ limit: Int = 5) -> [Key] { Array(ranked.prefix(limit)) }

  private var ranked: [Key] {
    counts.map { Key(id: $0.key, label: TypingKeyID.label($0.key), count: $0.value) }
      .sorted { $0.count != $1.count ? $0.count > $1.count : $0.id < $1.id }
  }

  /// `A，123 次`, what VoiceOver reads for a key. A nine-key cell also names what it carries on each grid, since pinyin and kana presses land on the same id: `九键 2，拼音 ABC，日文 か，7 次`.
  static func accessibilityLabel(_ id: String, count: Int) -> String {
    guard let face = nineKeyFaces[id] else { return "\(TypingKeyID.label(id))，\(count) 次" }
    let pinyin = face.pinyin.isEmpty ? "" : "拼音 \(face.pinyin)，"
    return "\(TypingKeyID.label(id))，\(pinyin)日文 \(face.kana)，\(count) 次"
  }
}

extension TypingStatistics {
  /// Key presses summed over `dates`, or over every retained day when `dates` is `nil`, as the page's other figures follow the selected day.
  func keyCounts(on dates: [Date]?, calendar: Calendar = .current) -> [String: Int] {
    let days = dates.map { $0.map { Self.dayKey($0, calendar: calendar) } } ?? Array(dailyKeys.keys)
    var result: [String: Int] = [:]
    for day in days {
      for (key, count) in dailyKeys[day] ?? [:] { result[key, default: 0] += count }
    }
    return result
  }
}

/// 存储层 `summary` 操作的返回：统计页画的概览、习惯、按键和成就。镜像 `crates/client-core/src/typing_statistics/metrics.rs` 里的 `TypingSummary`，所有数字都在那边算好，这边只解码和格式化。Rust 留空的字段（样本太少、没有活跃时长、还没有记录）在这里保持 nil，页面上显示 '—' 而不是 0。
struct TypingSummary: Decodable, Equatable {
  struct DayCount: Decodable, Equatable {
    var day: String
    var count: Int
  }

  struct Overview: Decodable, Equatable {
    var weekTotal: Int
    var previousWeekTotal: Int
    /// 最近七个本地日，从旧到新；最后一个是今天。
    var last7: [DayCount]
    /// 最近七天里每活跃分钟输入的可读字符数；这几天没测到活跃时长时为 nil。
    var averageSpeed: Double?
    var previousAverageSpeed: Double?
    /// 选词时选中首选的比例，0 到 1；不足 50 次选词时为 nil。
    var firstCandidateRate: Double?
    /// 比全拼少按多少键，0 到 1；还没有全拼按键数时为 nil。
    var keystrokesSavedRate: Double?
    var currentStreak: Int
    var longestStreak: Int

    private enum CodingKeys: String, CodingKey {
      case weekTotal = "week_total", previousWeekTotal = "previous_week_total", last7
      case averageSpeed = "average_speed", previousAverageSpeed = "previous_average_speed"
      case firstCandidateRate = "first_candidate_rate", keystrokesSavedRate = "keystrokes_saved_rate"
      case currentStreak = "current_streak", longestStreak = "longest_streak"
    }
  }

  /// 最忙的连续两小时，`[start, end)`；`end` 可以跨过午夜。
  struct PeakWindow: Decodable, Equatable {
    var start: Int
    var end: Int

    /// `hour` 是否落在这个时段内，跨午夜的部分也算。
    func contains(_ hour: Int) -> Bool {
      let hours = TypingActivity.hours
      let start = ((start % hours) + hours) % hours
      let end = ((end % hours) + hours) % hours
      if start == end { return hour == start }
      return start < end ? hour >= start && hour < end : hour >= start || hour < end
    }
  }

  struct Habits: Decodable, Equatable {
    /// 最近 84 个本地日，从旧到新，所以今天落在最后一列的最底部。
    var weeks12: [DayCount]
    /// 最近七天按本地小时累加的字符数，共 24 个桶。
    var hours24: [Int]
    var usualHours: [Double]?
    var peakWindow: PeakWindow?
    /// 最近 12 周里有输入的天数。
    var activeDays: Int
    /// 所有保留天数里的字符种类和来源，未归类的余量记在 `unknown` 下。
    var breakdown: TypingBreakdown

    private enum CodingKeys: String, CodingKey {
      case weeks12, hours24, usualHours = "usual_hours", peakWindow = "peak_window", activeDays = "active_days", breakdown
    }
  }

  /// 不停顿连续输入的最长一段及其开始的日期。只有记录连续段的宿主才会有，所以 iOS 上为 nil。
  struct Run: Decodable, Equatable {
    var characters: Int
    var day: String
  }

  struct Keys: Decodable, Equatable {
    /// 最近七天每个字符的拼写按键数；没有字符或按键数时为 nil。
    var perCharacterKeys: Double?
    var previousPerCharacterKeys: Double?
    /// 最近七天退格键占全部按键的比例，0 到 1。
    var backspaceRate: Double?
    /// 上屏内容中来自联想的比例，0 到 1。
    var predictionRate: Double?
    var longestRun: Run?
    /// 首选、第二、第三候选以及之后所有位置各自的占比，各为 0 到 1；还没有选词时为 nil。
    var positions: [Double]?

    private enum CodingKeys: String, CodingKey {
      case perCharacterKeys = "per_character_keys", previousPerCharacterKeys = "previous_per_character_keys"
      case backspaceRate = "backspace_rate", predictionRate = "prediction_rate", longestRun = "longest_run", positions
    }
  }

  /// 一枚徽章。何时解锁由 Rust 决定，并在 `summary` 内记下解锁日期；这里不计算任何解锁。
  struct Achievement: Decodable, Equatable, Identifiable {
    var id: String
    /// 奖章上的文字。
    var glyph: String
    var title: String
    var description: String
    /// `volume`、`streak`、`skill` 或 `fun`，决定奖章的配色。
    var group: String
    var unlockedDay: String?
    /// 以 `target` 的单位计的进度；可以超过 `target`。
    var current: Int
    var target: Int

    var isUnlocked: Bool { unlockedDay != nil }

    /// 0 到 1 的进度；解锁后恒为 1。
    var progress: Double {
      if isUnlocked { return 1 }
      guard target > 0 else { return 0 }
      return SharedNumber.clamped(Double(current) / Double(target), to: 0...1)
    }

    private enum CodingKeys: String, CodingKey {
      case id, glyph, title, description, group, unlockedDay = "unlocked_day", current, target
    }
  }

  var overview: Overview
  var habits: Habits
  var keys: Keys
  var achievements: [Achievement]

  var unlockedCount: Int { achievements.filter(\.isUnlocked).count }

  /// 进度需要用户自造词数的那枚徽章，而应用只能从键盘上一次的词库快照里读到这个数。
  static let userWordsAchievement = "words_50"

  /// 造词者徽章用的用户自造拼音词数，读自键盘上一次的词库快照：应用自己从不打开 Engine 词库。快照恰好是整个列表完整的第一页时是精确值；页面词数已够、后面还有更多页时，是一个已经达到徽章要求的下限；其余情况一律为 nil，免得把快照担保不了的数交给 summary。
  static func userWords(from state: PersonalDictionaryState) -> Int? {
    guard state.snapshotDate != nil, state.pageKind == nil, state.pageQuery.isEmpty, state.pageOffset == 0 else { return nil }
    let words = state.entries.filter { $0.kind == .pinyin && !$0.isBundled }.count
    if !state.hasMore { return words }
    return words >= 50 ? words : nil
  }
}

/// 统计页的文案规则，移植自 Android 的 `TypingStatisticsSummary`，让两个平台对每个数字的措辞一致。
enum TypingSummaryText {
  /// 占比图里的一段：标题和数量。
  struct Share: Equatable {
    var title: String
    var count: Int
  }

  /// 千位分隔符：`12,846`。
  static func grouped(_ value: Int) -> String {
    groupedFormatter.string(from: NSNumber(value: value)) ?? String(value)
  }

  /// 总数大字下方的周环比一行；上周没有记录可比时为 nil。
  static func weekDelta(_ current: Int, _ previous: Int) -> String? {
    guard previous > 0 else { return nil }
    let percent = javaRound(Double(current - previous) * 100 / Double(previous))
    if percent == 0 { return "和上周持平" }
    return percent > 0 ? "比上周多 \(percent)%" : "比上周少 \(-percent)%"
  }

  /// 整数，nil 时为 '—'。
  static func whole(_ value: Double?) -> String {
    value.map { String(javaRound($0)) } ?? "—"
  }

  /// 把 0–1 的比率写成整数百分比的数字部分，nil 时为 '—'。
  static func percent(_ rate: Double?) -> String {
    rate.map { String(javaRound($0 * 100)) } ?? "—"
  }

  /// 把 0–1 的比率写成保留一位小数的百分比数字（`7.4`），nil 时为 '—'。
  static func percentTenths(_ rate: Double?) -> String {
    rate.map { tenths($0 * 100) } ?? "—"
  }

  /// 保留一位小数（`2.3`），不带末尾的 `.0`，nil 时为 '—'。
  static func decimal(_ value: Double?) -> String {
    value.map(tenths) ?? "—"
  }

  /// 平均速度与上周对比；任一边没有数字时为 nil。
  static func speedDelta(_ current: Double?, _ previous: Double?) -> String? {
    guard let current, let previous else { return nil }
    let difference = javaRound(current) - javaRound(previous)
    if difference == 0 { return "和上周一样快" }
    return difference > 0 ? "比上周快 \(difference) 字" : "比上周慢 \(-difference) 字"
  }

  /// 每字按键数与上周对比；任一边没有数字时为 nil。
  static func perKeyDelta(_ current: Double?, _ previous: Double?) -> String? {
    guard let current, let previous else { return nil }
    let difference = javaRound(current * 10) - javaRound(previous * 10)
    if difference == 0 { return "和上周持平" }
    let amount = tenths(Double(abs(difference)) / 10)
    return difference < 0 ? "比上周少 \(amount) 次" : "比上周多 \(amount) 次"
  }

  /// 把高峰时段写成 `晚上 9–11 点`；没有高峰时段时为 nil。
  static func peakLabel(_ window: TypingSummary.PeakWindow?) -> String? {
    guard let window else { return nil }
    let hours = TypingActivity.hours
    let start = ((window.start % hours) + hours) % hours
    let end = ((window.end % hours) + hours) % hours
    return "\(period(start)) \(clock(start))–\(clock(end)) 点"
  }

  /// 把 `2026-09-28` 写成 `9 月 28 日`；其他输入原样返回。
  static func monthDay(_ day: String) -> String {
    let parts = day.split(separator: "-")
    guard day.count == 10, parts.count == 3, let month = Int(parts[1]), let date = Int(parts[2]) else { return day }
    return "\(month) 月 \(date) 日"
  }

  /// 徽章下方的一行：已解锁时是它的说明；未解锁时，差距能用单位说清就写还差多少，否则仍是说明。`progressKnown` 为 false 表示 summary 拿不到这枚徽章的计数，此时不声称还差多少。
  static func caption(_ badge: TypingSummary.Achievement, progressKnown: Bool = true) -> String {
    if badge.isUnlocked || !progressKnown { return badge.description }
    let missing = max(0, badge.target - badge.current)
    guard let unit = unit(badge.id), missing > 0 else { return badge.description }
    if unit == "字" { return "还差 \(characters(missing))" }
    return "还差 \(missing) \(unit)"
  }

  /// 点击后的提示：已解锁时是 `已解锁「名」· 说明`，否则是 `「名」· 还差 …`。
  static func toast(_ badge: TypingSummary.Achievement, progressKnown: Bool = true) -> String {
    badge.isUnlocked
      ? "已解锁「\(badge.title)」· \(badge.description)"
      : "「\(badge.title)」· \(caption(badge, progressKnown: progressKnown))"
  }

  /// 进度环里的百分比，向下取整，免得差一点完成时显示 100%。
  static func progressLabel(_ badge: TypingSummary.Achievement) -> String {
    "\(Int((badge.progress * 100).rounded(.down)))%"
  }

  /// 输入构成按设计稿分成四部分，顺序固定，让每部分保持自己的颜色：汉字（han）、英文（拉丁及其他文字的字母）、符号（数字、标点和符号）、表情（emoji）。旧记录里未归类的余量不属于任何一部分，不计入。
  static func composition(_ characters: [String: Int]) -> [Share] {
    func value(_ kind: TypingCharacterKind) -> Int { max(0, characters[kind.rawValue] ?? 0) }
    return [
      Share(title: "汉字", count: value(.han)),
      Share(title: "英文", count: value(.latin) + value(.otherLetter)),
      Share(title: "符号", count: value(.number) + value(.punctuation) + value(.symbol)),
      Share(title: "表情", count: value(.emoji)),
    ]
  }

  /// 输入方式：26 键、9 键、语音和手写，只列有输入的部分。除九键外的所有键盘方案都在 26 个字母键上输入；AI 润色、回复和未归类的输入不算输入方式，不计入。
  static func methods(_ sources: [String: Int]) -> [Share] {
    let excluded = Set([TypingSource.nineKey, .voice, .handwriting, .unknown, .ai, .reply].map(\.rawValue))
    let full = sources.filter { !excluded.contains($0.key) }.values.reduce(0) { $0 + max(0, $1) }
    func value(_ source: TypingSource) -> Int { max(0, sources[source.rawValue] ?? 0) }
    return [
      Share(title: "26 键", count: full),
      Share(title: "9 键", count: value(.nineKey)),
      Share(title: "语音", count: value(.voice)),
      Share(title: "手写", count: value(.handwriting)),
    ].filter { $0.count > 0 }
  }

  /// `count / total` 的整数百分比；总数为 0 时为 0。
  static func share(_ count: Int, of total: Int) -> Int {
    total <= 0 ? 0 : javaRound(Double(count) * 100 / Double(total))
  }

  /// Java 的 `Math.round`：0.5 向上进位，让同一个数字的取整方式与 Android 一致。
  static func javaRound(_ value: Double) -> Int {
    guard value.isFinite else { return 0 }
    return Int((value + 0.5).rounded(.down))
  }

  private static let groupedFormatter: NumberFormatter = {
    let formatter = NumberFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.numberStyle = .decimal
    formatter.groupingSeparator = ","
    formatter.usesGroupingSeparator = true
    return formatter
  }()

  private static func tenths(_ value: Double) -> String {
    let rounded = javaRound(value * 10)
    if rounded % 10 == 0 { return String(rounded / 10) }
    return String(format: "%.1f", Double(rounded) / 10)
  }

  /// 徽章差距的计量单位；速度、准确率和早起的小时没有单位。
  private static func unit(_ id: String) -> String? {
    switch id {
    case "chars_10k", "chars_100k", "chars_1m", "night_owl", "shuangpin_10k", "handwriting_500": return "字"
    case "streak_7", "streak_30", "streak_100": return "天"
    case "sentence_1000": return "次"
    case "voice_1h": return "分钟"
    case "words_50": return "个词"
    case "skins_5": return "款皮肤"
    default: return nil
    }
  }

  /// 字符数，一万及以上写成 `51.7 万字`。
  private static func characters(_ count: Int) -> String {
    count >= 10_000 ? "\(tenths(Double(count) / 10_000)) 万字" : "\(count) 字"
  }

  private static func period(_ hour: Int) -> String {
    switch hour {
    case ..<5: return "凌晨"
    case ..<8: return "早上"
    case ..<11: return "上午"
    case ..<13: return "中午"
    case ..<18: return "下午"
    case ..<19: return "傍晚"
    default: return "晚上"
    }
  }

  private static func clock(_ hour: Int) -> Int {
    if hour == 0 { return 12 }
    return hour > 12 ? hour - 12 : hour
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

  init() {
    directory = FileManager.default.containerURL(
      forSecurityApplicationGroupIdentifier: MSIMEAppEdition.appGroupIdentifier)?.appendingPathComponent("MSIME", isDirectory: true)
  }

  init(directory: URL?) {
    self.directory = directory
  }

  /// The request buffer the ABI accepts.
  private static let maximumRequestBytes = 65_536
  /// A commit is split into pieces this size, well inside the store's 40,000-byte commit limit and, even with every byte escaped, inside the request limit.
  private static let maximumChunkBytes = 8_000

  private static let preparedLock = NSLock()
  private static var prepared = Set<String>()

  private static func rejectsSymlinkAncestors(_ path: URL) -> Bool {
    SafePath.hasRefusedSymbolicLink(path)
  }

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

  /// Once per directory and process: create the directory, refusing one reached through a symbolic link.
  private func prepare(_ directory: URL) throws {
    Self.preparedLock.lock()
    defer { Self.preparedLock.unlock() }
    guard !Self.prepared.contains(directory.path) else { return }
    guard !Self.rejectsSymlinkAncestors(directory) else { throw CocoaError(.fileWriteNoPermission) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    Self.prepared.insert(directory.path)
  }

  func load() throws -> TypingStatistics {
    let value = try call(["operation": "load"])
    return try JSONDecoder().decode(TypingStatistics.self, from: JSONSerialization.data(withJSONObject: value))
  }

  /// 本地日 `day` 的统计页派生数字。`userWords` 是造词者徽章用的用户自造词数，未知时为 nil，存储层按 0 处理。统计开启时，存储层会记下本次调用新发现解锁的徽章。
  func summary(day: Date = Date(), userWords: Int?, calendar: Calendar = .current) throws -> TypingSummary {
    var action: [String: Any] = ["operation": "summary", "day": TypingStatistics.dayKey(day, calendar: calendar)]
    if let userWords { action["user_words"] = max(0, userWords) }
    let value = try call(action)
    return try JSONDecoder().decode(TypingSummary.self, from: JSONSerialization.data(withJSONObject: value))
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
    return .neverWritten
  }

  /// 原生统计页「统计没有数据」那张卡的正文，nil 表示不需要这张卡。`recordingOff` 只在确实读到了统计文档且其中记录关闭时为真：此时键盘本来就不写，「记录已关闭」那张卡已经说明了原因，这里再让人去开完全访问，会把人引到错误的地方。
  ///
  /// 文件存在不能说明键盘写过：在 app 里开启记录时，`client-core` 的 `set_enabled` 会无条件写出 `typing-statistics.json`，而键盘没有完全访问权限时不记录任何东西，数字会一直是零。app 读不到键盘是否有完全访问（iOS 没有公开 API，键盘没有完全访问时也写不了 App Group，留下的标记在权限收回后会过期），所以计数为零时始终先提醒这一项，再说其他可能。
  static func emptyStatisticsAdvice(_ availability: Availability, recordingOff: Bool, total: Int) -> String? {
    if recordingOff { return nil }
    switch availability {
    case .containerUnavailable:
      return "无法访问共享存储，键盘与本 app 之间没有可用的数据通道。重装水杉输入法可以重建它。"
    case .neverWritten:
      return "键盘从未写入过统计。请在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中开启“允许完全访问”，"
        + "然后用水杉键盘输入几个字再回来刷新。未开启时仍可正常打字，只是不记录统计。"
    case .ready(let lastWritten):
      guard total == 0 else { return nil }
      let fullAccess = "请先确认已在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中为水杉键盘开启“允许完全访问”，未开启时键盘不记录统计。"
      guard let lastWritten else {
        return "统计文件存在但还没有计数。" + fullAccess + "已开启的话，用水杉键盘输入几个字再刷新。"
      }
      return "统计文件最后写入于 \(lastWritten.formatted(.dateTime.month().day().hour().minute()))，但计数为零。"
        + fullAccess + "已开启的话，若此前清空过统计，这是正常的；否则请附上这条信息反馈。"
    }
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

  /// Add one batch of key press counts to `day`, the local day they were pressed on. Returns how many presses the store took: 0 when statistics are off, which is the keyboard's cue to stop counting.
  func recordKeys(_ keys: [String: Int], day: String) throws -> Int {
    guard !keys.isEmpty else { return 0 }
    let value = try call(["operation": "record_keys", "day": day, "keys": keys])
    let maximum = keys.values.filter { $0 > 0 }.reduce(0) { partial, count in
      partial > Int.max - count ? Int.max : partial + count
    }
    guard let recorded = Self.strictRecordedCount((value as? [String: Any])?["recorded"], maximum: maximum) else {
      throw TypingStatisticsError.invalidResponse
    }
    return recorded
  }

  /// 存储层接受的单次语音输入最长时长（`MAX_VOICE_MS_PER_CALL`）；Android 也同样截断到这个值。
  static let maximumVoiceMilliseconds = 600_000

  /// 为动口不动手徽章给 `day` 加一次时长为 `milliseconds` 的语音输入。时长为 0 不算语音输入，不发送；超过存储层上限的截断处理，与 Android 相同。返回存储层实际记下的毫秒数：统计关闭时为 0。
  @discardableResult
  func recordVoice(milliseconds: Int, day: String) throws -> Int {
    guard milliseconds > 0 else { return 0 }
    let value = try call(["operation": "record_voice", "day": day,
                          "milliseconds": min(milliseconds, Self.maximumVoiceMilliseconds)])
    guard let recorded = Self.strictRecordedCount((value as? [String: Any])?["recorded"],
                                                  maximum: Self.maximumVoiceMilliseconds) else {
      throw TypingStatisticsError.invalidResponse
    }
    return recorded
  }

  /// 为换装达人徽章记下用过皮肤 `id`：内置主题 id、`custom` 或已保存设计的 UUID。返回它对存储层是否是新的；已经计过或统计关闭时为 false。
  @discardableResult
  func recordSkin(id: String) throws -> Bool {
    let value = try call(["operation": "record_skin", "id": id])
    guard let recorded = (value as? [String: Any])?["recorded"] as? Bool else { throw TypingStatisticsError.invalidResponse }
    return recorded
  }

  /// Native JSON must return a non-negative integral count that cannot exceed the submitted batch.
  static func strictRecordedCount(_ value: Any?, maximum: Int) -> Int? {
    guard let integer = SharedNumber.nonnegativeInt(value), integer <= maximum else { return nil }
    return integer
  }

  /// Whether the user has statistics on. The keyboard asks once per appearance so that, while they are off, it does not even keep key counts in memory.
  func isEnabled() throws -> Bool {
    let value = try call(["operation": "load"])
    return try Self.strictEnabled(value)
  }

  /// The shared store always includes `enabled` in a load response. Reject a malformed
  /// field instead of silently disabling collection and hiding a protocol mismatch.
  static func strictEnabled(_ value: Any?) throws -> Bool {
    guard let dictionary = value as? [String: Any],
          let enabled = dictionary["enabled"] as? Bool else {
      throw TypingStatisticsError.invalidResponse
    }
    return enabled
  }
}

/// 打字之外推动徽章的两项记录：识别出的语音输入时长（动口不动手）和换上的皮肤（换装达人）。与 Android 一样尽力而为：在一个串行队列上离开主线程写入，失败只进诊断日志，从不妨碍换皮肤或语音结果。是否记录（隐私模式）由调用方决定，Android 也是如此。
enum TypingStatisticsExtras {
  private static let queue = DispatchQueue(label: "app.msime.ios.typing-statistics.extras", qos: .utility)

  /// 换上了一款皮肤：内置主题记它的 id，已保存或社区的设计记它的 UUID，从键盘面板换上的任何设计则记为 `custom`。
  static func recordSkin(_ id: String) {
    queue.async {
      do { try TypingStatisticsStore().recordSkin(id: id) } catch { DiagnosticLog.shared.write("statistics_skin_failed") }
    }
  }

  /// 识别出一段时长为 `milliseconds` 的语音输入，计在 `date` 所在的本地日。
  static func recordVoice(milliseconds: Int, at date: Date = Date()) {
    guard milliseconds > 0 else { return }
    let day = TypingStatistics.dayKey(date)
    queue.async {
      do { try TypingStatisticsStore().recordVoice(milliseconds: milliseconds, day: day) } catch { DiagnosticLog.shared.write("statistics_voice_failed") }
    }
  }
}
