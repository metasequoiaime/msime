import Foundation
import Darwin

enum TypingSource: String, CaseIterable {
  case quanpin, nineKey, shuangpin, ziranma, microsoft, shoudao, wubi, japanese, korean, cantonese, zhuyin, vietnamese, handwriting, english, local, ai, reply, voice, unknown
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
    case .korean: "韩语"
    case .cantonese: "粤语"
    case .zhuyin: "注音"
    case .vietnamese: "越南语"
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
    case enabled, total, days, detail, dailyDetails, retention, retentionDays, dailyActiveMs, dailyHours, dailyKeys
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
  static let punctuation = "SoftPunctuation"
  /// The bottom-row quick punctuation key, which sits where a hardware comma does whichever mark it types.
  static let quickPunctuation = "Comma"
  static let symbol = "SoftSymbol"
  static let layer = "SoftLayer"
  static let language = "SoftLanguage"
  static let globe = "SoftGlobe"
  static let emoji = "SoftEmoji"
  static let voice = "SoftVoice"

  /// A cell of the nine-key grid, by the digit printed on it. Cell 1 is the pinyin separator.
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
  private static let drawn = Set((keyboardRows + nineKeyRows).flatMap { $0 })

  /// What a nine-key cell carries on the two grids that share its id: the pinyin letters (1 is the syllable separator, 0 has none) and the head of the kana row `TypingKeyID.japaneseKana` puts there, あ through ら on 1 to 9 and わ on 0.
  static let nineKeyFaces: [String: (pinyin: String, kana: String)] = [
    "Nine1": ("分词", "あ"), "Nine2": ("ABC", "か"), "Nine3": ("DEF", "さ"),
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

  /// Pressed keys with no place on either drawing, most pressed first.
  var others: [Key] { ranked.filter { !Self.drawn.contains($0.id) } }

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
    let descriptor = open(lockURL.path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
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
    let descriptor = open(legacyLockURL.path, O_CREAT | O_RDWR | O_NOFOLLOW | O_CLOEXEC, S_IRUSR | S_IWUSR)
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

  /// Add one batch of key press counts to `day`, the local day they were pressed on. Returns how many presses the store took: 0 when statistics are off, which is the keyboard's cue to stop counting.
  func recordKeys(_ keys: [String: Int], day: String) throws -> Int {
    guard !keys.isEmpty else { return 0 }
    let value = try call(["operation": "record_keys", "day": day, "keys": keys])
    guard let recorded = (value as? [String: Any])?["recorded"] as? NSNumber else { throw TypingStatisticsError.invalidResponse }
    return recorded.intValue
  }

  /// Whether the user has statistics on. The keyboard asks once per appearance so that, while they are off, it does not even keep key counts in memory.
  func isEnabled() throws -> Bool {
    let value = try call(["operation": "load"])
    return (value as? [String: Any])?["enabled"] as? Bool ?? false
  }
}
