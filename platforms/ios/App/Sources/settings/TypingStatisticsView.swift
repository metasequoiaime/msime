import SwiftUI

private typealias StatisticsSlice = StatisticsChart.Slice

/// 每一类配一个图标。名字是查出来的意思,不是装饰:九键是九宫格,双拼是两个键,五笔是笔画,手写是笔,语音是波形。
private enum StatisticsSymbol {
  static func source(_ source: TypingSource) -> String {
    switch source {
    case .quanpin: return "keyboard"
    case .nineKey: return "square.grid.3x3"
    case .shuangpin, .ziranma, .microsoft, .shoudao: return "square.on.square"
    case .wubi: return "scribble"
    case .japanese: return "character.bubble"
    case .korean: return "character.bubble.fill"
    case .cantonese: return "character.book.closed"
    case .zhuyin: return "character.phonetic"
    case .vietnamese: return "textformat.abc.dottedunderline"
    case .handwriting: return "hand.draw"
    case .english: return "abc"
    case .local: return "clock.arrow.circlepath"
    case .ai: return "sparkles"
    case .reply: return "bubble.left.and.bubble.right"
    case .voice: return "waveform"
    case .unknown: return "questionmark.circle"
    }
  }
  static func kind(_ kind: TypingCharacterKind) -> String {
    switch kind {
    case .han: return "character.textbox"
    case .latin: return "textformat.abc"
    case .otherLetter: return "globe"
    case .number: return "number"
    case .punctuation: return "quote.opening"
    case .emoji: return "face.smiling"
    case .symbol: return "asterisk"
    case .unknown: return "questionmark.circle"
    }
  }
}

struct TypingStatisticsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @State private var statistics = TypingStatistics()
  @State private var errorMessage = ""
  @State private var confirmsReset = false
  @State private var tab = Tab.trend
  @State private var selectedDay: Date?
  /// 占比条和图标的进场动画放过了没有。换标签时先归零再置起,这一块就重放一遍。
  @State private var revealed = false

  /// 三块内容轮流占这一屏,不再一路往下滚。
  private enum Tab: String, CaseIterable {
    case trend, rhythm, kind, mode, scheme, keys
    /// 标签只给两个字 —— 六格分段控件上放「语言模式」「输入方案」会挤成一行小字;全名在下面的分组标题里。
    var title: String {
      switch self {
      case .trend: return "趋势"
      case .rhythm: return "节奏"
      case .kind: return "类型"
      case .mode: return "模式"
      case .scheme: return "方案"
      case .keys: return "按键"
      }
    }
  }

  /// 趋势最多画多少天；这是图表的宽度上限，不是保留期限，更早的每日明细仍在存储里。
  ///
  /// 原来固定三十天:一个月看不出「这个月比上个月多」,而数据本来就攒着一年。有多少画多少 —— 没有记录时退回三十天,免得开一屏空白的年历。
  private static let trendDayLimit = 366
  private static let trendDayFloor = 30
  /// 实际要画的天数:从最早那条记录到今天,上限一年。
  private var trendDays: Int {
    guard let earliest = statistics.days.keys.min(),
          let date = Self.dayKeyFormatter.date(from: earliest)
    else { return Self.trendDayFloor }
    let start = Calendar.current.startOfDay(for: date)
    let span = Calendar.current.dateComponents([.day], from: start, to: Calendar.current.startOfDay(for: Date())).day ?? 0
    return min(Self.trendDayLimit, max(Self.trendDayFloor, span + 1))
  }
  private static let dayKeyFormatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "yyyy-MM-dd"
    return formatter
  }()
  private let store = TypingStatisticsStore()
  /// 扇区颜色:品牌绿的一条明度梯度,不是八个互不相干的色相。
  ///
  /// 原先是 `.teal .blue .indigo .orange .pink .purple .brown .gray`。图表确实需要相邻扇区能分开,但八种色相除了"彼此不同"之外什么都没说,而且这一页因此和应用其余部分不是一套配色。梯度按同一顺序排进图例,所以哪一档对应哪一项仍然读得出来。
  private let colors: [Color] = MetasequoiaTheme.chartRamp(10)
  @State private var availability = TypingStatisticsStore.Availability.neverWritten
  // The old copy asked for Full Access unconditionally, so it said the same thing whether the
  // setting was the problem or not and carried no information. Each case here is a different
  // answer to "why is this empty", and a run that is working says nothing at all.
  private var storageAdvice: String? {
    switch availability {
    case .containerUnavailable:
      return "无法访问共享存储，键盘与本 app 之间没有可用的数据通道。重装水杉输入法可以重建它。"
    case .neverWritten:
      return "键盘从未写入过统计。请在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中开启“允许完全访问”，"
        + "然后用水杉键盘输入几个字再回来刷新。未开启时仍可正常打字，只是不记录统计。"
    case .ready(let lastWritten):
      guard statistics.total == 0 else { return nil }
      guard let lastWritten else { return "统计文件存在但还没有计数，请用水杉键盘输入几个字再刷新。" }
      return "统计文件最后写入于 \(lastWritten.formatted(.dateTime.month().day().hour().minute()))，但计数为零。"
        + "若此前清空过统计，这是正常的；否则请附上这条信息反馈。"
    }
  }
  private var dates: [Date] {
    (0..<trendDays).reversed().compactMap {
      Calendar.current.date(byAdding: .day, value: -$0, to: Calendar.current.startOfDay(for: Date()))
    }
  }
  /// 点了某一天就只看那一天,否则看累计。
  private var scopeDates: [Date]? { selectedDay.map { [$0] } }
  private var breakdown: TypingBreakdown { statistics.breakdown(on: scopeDates) }
  private var keyHeatmap: TypingKeyHeatmap { TypingKeyHeatmap(counts: statistics.keyCounts(on: scopeDates)) }
  private var scopeTotal: Int { scopeDates?.reduce(0) { $0 + statistics.count(on: $1) } ?? statistics.total }
  private var scopeTitle: String {
    if let selectedDay { return selectedDay.formatted(.dateTime.month().day()) }
    return "累计输入"
  }
  private var characterSlices: [StatisticsSlice] {
    TypingCharacterKind.allCases.enumerated().map { index, kind in
      StatisticsSlice(id: kind.rawValue, title: kind.title, count: breakdown.characters[kind.rawValue] ?? 0,
                      color: colors[index % colors.count], symbol: StatisticsSymbol.kind(kind))
    }
  }
  private var sourceSlices: [StatisticsSlice] {
    TypingSource.allCases.enumerated().map { index, source in
      StatisticsSlice(id: source.rawValue, title: source.title, count: breakdown.sources[source.rawValue] ?? 0,
                      color: colors[index % colors.count], symbol: StatisticsSymbol.source(source))
    }
  }
  private var languageSlices: [StatisticsSlice] {
    let sources = breakdown.sources
    return [
      StatisticsSlice(id: "chinese", title: "中文模式", count: ["quanpin", "nineKey", "shuangpin", "ziranma", "microsoft", "shoudao", "wubi", "cantonese", "zhuyin"].reduce(0) { $0 + (sources[$1] ?? 0) }, color: colors[0], symbol: "character.textbox"),
      StatisticsSlice(id: "japanese", title: "日语模式", count: sources["japanese"] ?? 0, color: colors[1], symbol: "character.bubble"),
      StatisticsSlice(id: "korean", title: "韩语模式", count: sources["korean"] ?? 0, color: colors[2], symbol: "character.bubble.fill"),
      StatisticsSlice(id: "vietnamese", title: "越南语模式", count: sources["vietnamese"] ?? 0, color: colors[3], symbol: "textformat.abc.dottedunderline"),
      StatisticsSlice(id: "english", title: "英文模式", count: sources["english"] ?? 0, color: colors[4], symbol: "abc"),
      StatisticsSlice(id: "local", title: "本地输入", count: sources["local"] ?? 0, color: colors[5], symbol: "clock.arrow.circlepath"),
      StatisticsSlice(id: "ai", title: "AI 润色", count: sources["ai"] ?? 0, color: colors[6], symbol: "sparkles"),
      StatisticsSlice(id: "reply", title: "高情商回复", count: sources["reply"] ?? 0, color: colors[7], symbol: "bubble.left.and.bubble.right"),
      StatisticsSlice(id: "voice", title: "语音输入", count: sources["voice"] ?? 0, color: colors[8], symbol: "waveform"),
      StatisticsSlice(id: "unknown", title: "历史未分类", count: sources["unknown"] ?? 0, color: colors[9], symbol: "questionmark.circle"),
    ]
  }

  var body: some View {
    Form {
      Section {
        Picker("统计内容", selection: $tab) {
          ForEach(Tab.allCases, id: \.self) { Text($0.title).tag($0) }
        }.pickerStyle(.segmented).accessibilityIdentifier("statisticsTab")
          .onChange(of: tab) { _ in replayReveal() }
        HStack {
          metric("今日输入", count: statistics.count(on: Date()), identifier: "typingToday")
          Spacer()
          metric(scopeTitle, count: scopeTotal, identifier: "typingTotal")
        }.padding(.vertical, 8)
      }
      switch tab {
      case .trend:
        Section {
          trendChart
          if selectedDay != nil {
            Button("返回累计") { selectedDay = nil }
          }
          NavigationLink {
            TypingDailyDetailView(statistics: statistics)
          } label: {
            Label("按日明细", systemImage: "tablecells")
          }.accessibilityIdentifier("typingDailyDetails")
        } header: { Text(trendDays >= 360 ? "每日趋势 · 近一年" : "每日趋势 · 近 \(trendDays) 天") }
          footer: { Text("折线画到最早那条记录，最多一年。方块每天一格、一列一周，铺满一屏后可以左右拖，没有记录的日子是最浅的一档；点一个方块只看那一天的分类与占比。") }
      case .rhythm:
        rhythmSections
      case .kind:
        Section {
          distribution(characterSlices, chart: .pie)
        } header: { Text("字符类型") }
      case .mode:
        Section {
          distribution(languageSlices, chart: .donut)
        } header: { Text("语言模式") }
          footer: { Text("按提交时使用的键盘模式统计，不推测文本语言；粤拼和大千注音计入中文模式，中文模式下输入的数字仍计入中文模式。AI 润色和语音输入单独按来源统计。") }
      case .scheme:
        Section {
          distribution(sourceSlices, chart: .rank)
        } header: { Text("输入方案") }
          footer: { Text("拼音方案统计其上屏字符数，拼写时的按键另计在“按键”页。旧版本总数保留为历史未分类，新输入开始记录细分。") }
      case .keys:
        keySections
      }
      // 开关、刷新和清空挪到了右上角的菜单:这一页是给人看数的,三个管理项挂在每一屏下面,每换一个标签都要再滚过它们一次。说明留在原处 —— 它解释的是屏幕上这些数字怎么来的。
      Section {
      } footer: {
        Text("字数只统计水杉键盘提交的字符，含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图另计每个键的按下次数，拼音拼写、删除和功能键都算，密码框里的按键不计；按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。所有统计仅在本机保存计数，不保存输入内容。每日明细默认永久保留，可在右上角菜单里缩短为 30 至 365 天，超期的每日记录随即删除，并同时从累计总数和分类中扣除；要全部删除请用“清空统计”。")
      }
      if let advice = storageAdvice {
        Section("统计没有数据") {
          Text(advice)
          Button("前往系统设置") {
            guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
            UIApplication.shared.open(url)
          }
        }
      }
      if !errorMessage.isEmpty { Section { Text(errorMessage).foregroundStyle(.secondary) } }
    }
    .navigationTitle("统计").navigationBarTitleDisplayMode(.large)
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) {
        Menu {
          Button {
            update { try store.setEnabled(!statistics.enabled) }
          } label: {
            // 菜单里的开关用对勾表示开着 —— Toggle 放进 Menu 在 iOS 15 上画不出来。
            if statistics.enabled { Label("记录打字统计", systemImage: "checkmark") }
            else { Text("记录打字统计") }
          }
          .accessibilityIdentifier("typingStatisticsEnabled")
          Picker(selection: Binding(get: { statistics.retentionDays ?? 0 }, set: { days in
            update { try store.setRetention(days == 0 ? nil : days) }
          })) {
            Text("永久保留").tag(0)
            ForEach(TypingStatistics.retentionChoices, id: \.self) { Text("保留最近 \($0) 天").tag($0) }
          } label: {
            Label("每日记录", systemImage: "calendar.badge.clock")
          }
          .pickerStyle(.menu)
          .accessibilityIdentifier("typingStatisticsRetention")
          Button("刷新统计") { reload() }
          Button("清空统计", role: .destructive) { confirmsReset = true }
            .accessibilityIdentifier("resetTypingStatistics")
        } label: {
          Image(systemName: "ellipsis.circle")
        }
        .accessibilityLabel("统计选项")
        .accessibilityIdentifier("statisticsMenu")
      }
    }
    .onAppear { reload(); replayReveal() }
    .onChange(of: scenePhase) { if $0 == .active { reload() } }
    .alert("清空所有打字统计？", isPresented: $confirmsReset) {
      Button("取消", role: .cancel) {}
      Button("清空", role: .destructive) { update { try store.reset() }; selectedDay = nil }
    } message: { Text("累计字数、分类、按键次数和每日记录将被删除，无法恢复。") }
  }

  private var trendChart: some View {
    let days = dates.map { StatisticsChart.Day(date: $0, count: statistics.count(on: $0)) }

    let maximum = days.map(\.count).max() ?? 0
    return VStack(alignment: .leading, spacing: 14) {
      Text("最高 \(maximum) 字符 / 天").font(.caption).foregroundStyle(.secondary)
      StatisticsTrendChart(days: days, selected: selectedDay,
                           accent: MetasequoiaTheme.accent, progress: revealed ? 1 : 0)
        .animation(.easeOut(duration: 0.7), value: revealed)
      // 折线看走势,热力图看「哪天在打字」—— 同一份数据的两个问题,一条线回答不了第二个。
      Text("每天一格，一列一周").font(.caption).foregroundStyle(.secondary)
      StatisticsHeatmap(count: { statistics.count(on: $0) }, selected: selectedDay,
                        accent: MetasequoiaTheme.accent) { date in
        selectedDay = selectedDay == date ? nil : date
      }
    }.padding(.vertical, 8).accessibilityElement(children: .contain).accessibilityIdentifier("statisticsTrend")
  }

  /// 按键:键盘热力图、按得最多的五个键,以及键盘上画不出位置的键。和其他标签一样跟着选中的那一天走。
  @ViewBuilder private var keySections: some View {
    let heatmap = keyHeatmap
    Section {
      StatisticsKeyboardHeatmap(heatmap: heatmap, accent: MetasequoiaTheme.accent)
        .padding(.vertical, 8)
      if heatmap.total == 0 {
        Text("暂无按键记录").font(.subheadline).foregroundStyle(.secondary)
      }
    } header: { Text(selectedDay == nil ? "按键热力 · 累计" : "按键热力 · \(scopeTitle)") }
      footer: { Text("每个键按下一次计一次，按住删除键连删也只算一次。只保存每个键每天被按下的次数，不保存按键顺序和输入内容。") }
    if heatmap.total > 0 {
      Section {
        keyRows(heatmap.top())
      } header: { Text("最常按的键") }
    }
    if !heatmap.others.isEmpty {
      Section {
        keyRows(heatmap.others)
      } header: { Text("其他键") }
        footer: { Text("数字、标点、符号键等键盘图上没有位置的键。符号按产生它的英文键盘按键计，例如“！”计入 1 键。") }
    }
  }

  private func keyRows(_ keys: [TypingKeyHeatmap.Key]) -> some View {
    ForEach(keys, id: \.id) { key in
      HStack {
        Text(key.label)
        Spacer()
        Text("\(key.count.formatted()) 次").monospacedDigit().foregroundStyle(.secondary)
      }
      .accessibilityElement(children: .ignore)
      .accessibilityLabel(TypingKeyHeatmap.accessibilityLabel(key.id, count: key.count))
    }
  }

  private var activity: TypingActivity { statistics.activity(todayKey: TypingStatistics.dayKey(Date())) }

  /// 输入节奏:速度、活跃时长、连续天数和今日时段,和共享统计页的「输入节奏」同一套算法。手机上两列,iPad 的宽窗口一行放下四格。
  @ViewBuilder private var rhythmSections: some View {
    let activity = activity
    Section {
      LazyVGrid(columns: Array(repeating: GridItem(.flexible(), alignment: .leading),
                               count: horizontalSizeClass == .regular ? 4 : 2), spacing: 18) {
        rhythmMetric("今日速度", value: "\(Int(activity.todaySpeed.rounded()))", unit: "字 / 分钟", identifier: "typingTodaySpeed")
        rhythmMetric("平均速度", value: "\(Int(activity.averageSpeed.rounded()))", unit: "字 / 分钟", identifier: "typingAverageSpeed")
        rhythmMetric("今日活跃", value: TypingActivity.formatActiveTime(activity.todayActiveMs), unit: "连续打字的时间", identifier: "typingTodayActive")
        rhythmMetric("连续天数", value: "\(activity.currentStreak)", unit: "最长 \(activity.longestStreak) 天", identifier: "typingStreak")
      }.padding(.vertical, 8)
      VStack(alignment: .leading, spacing: 4) {
        Text("日均 \(Int(activity.averagePerDay.rounded())) 字符 · \(activity.recordedDays) 天有记录")
        if let best = activity.bestDay {
          Text("最多 \(dayLabel(best))，\(activity.bestDayCharacters) 字符")
        }
        if let fastest = activity.fastestDay {
          Text("最快 \(dayLabel(fastest))，\(Int(activity.fastestSpeed.rounded())) 字 / 分钟")
        }
      }.font(.caption).foregroundStyle(.secondary).accessibilityIdentifier("typingRhythmSummary")
    } header: { Text("输入节奏") }
      footer: {
        Text(activity.hasActivity
          ? "速度按连续打字的时间计算，两次上屏间隔超过 10 秒算休息、不计入；只统计汉字与各种文字的字母，数字、标点和表情不参与。"
          : "还没有测量到活跃时长。这项从本次更新后开始记录，之前的输入只有字数。")
      }
    if let hours = activity.todayHours {
      Section {
        StatisticsHourlyChart(hours: hours, accent: MetasequoiaTheme.accent, progress: revealed ? 1 : 0)
          .animation(.easeOut(duration: 0.6), value: revealed)
          .padding(.vertical, 8)
      } header: { Text("今日时段") }
    }
  }

  private func rhythmMetric(_ title: String, value: String, unit: String, identifier: String) -> some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(title).font(.subheadline).foregroundStyle(.secondary)
      Text(value).font(.system(size: 26, weight: .semibold, design: .rounded))
        .foregroundStyle(MetasequoiaTheme.accent).lineLimit(1).minimumScaleFactor(0.6)
        .accessibilityIdentifier(identifier)
      Text(unit).font(.caption).foregroundStyle(.secondary)
    }
  }

  /// `9月21日`,和趋势轴的标签一样。
  private func dayLabel(_ key: String) -> String {
    guard let date = Self.dayKeyFormatter.date(from: key) else { return key }
    return date.formatted(.dateTime.month().day())
  }

  /// 一块分布 = 一张图 + 一份图例。图形按这一块回答的问题选,图例给准确数字。
  private func distribution(_ slices: [StatisticsSlice], chart: DistributionChart) -> some View {
    let total = slices.reduce(0) { $0 + $1.count }
    let visible = slices.filter { $0.count > 0 || $0.id != "unknown" }
    return VStack(spacing: 14) {
      switch chart {
      case .pie:
        StatisticsPieChart(slices: slices, progress: revealed ? 1 : 0)
      case .donut:
        StatisticsDonutChart(slices: slices, total: total, progress: revealed ? 1 : 0)
      case .rank:
        StatisticsRankChart(slices: slices, progress: revealed ? 1 : 0)
      }
      if total == 0, chart != .rank {
        Text("暂无输入记录").font(.subheadline).foregroundStyle(.secondary)
      }
      ForEach(Array(visible.enumerated()), id: \.element.id) { index, slice in
        HStack(spacing: 10) {
          Image(systemName: slice.symbol)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(slice.color)
            .frame(width: 28, height: 28)
            .background(slice.color.opacity(0.14), in: RoundedRectangle(cornerRadius: 8))
            .scaleEffect(revealed ? 1 : 0.6)
            .opacity(revealed ? 1 : 0)
            .animation(.spring(response: 0.42, dampingFraction: 0.72).delay(Double(index) * 0.03), value: revealed)
          Text(slice.title).font(.subheadline)
          Spacer()
          Text("\(slice.count)").monospacedDigit()
          Text(total == 0 ? "—" : "\(Double(slice.count) / Double(total) * 100, specifier: "%.1f")%")
            .font(.caption).foregroundStyle(.secondary).monospacedDigit().frame(width: 54, alignment: .trailing)
        }.accessibilityElement(children: .combine)
      }
    }.padding(.vertical, 8)
    .animation(.easeOut(duration: 0.6), value: revealed)
  }

  /// 这一块用哪种图。
  private enum DistributionChart { case pie, donut, rank }

  private func metric(_ title: String, count: Int, identifier: String) -> some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(title).font(.subheadline).foregroundStyle(.secondary)
      Text(count.formatted()).font(.system(size: 36, weight: .bold)).monospacedDigit()
        .foregroundStyle(MetasequoiaTheme.accent).lineLimit(1).minimumScaleFactor(0.6)
        .accessibilityIdentifier(identifier)
      Text("字符").font(.caption).foregroundStyle(.secondary)
    }
  }
  /// 动画从头放一遍。SwiftUI 只在值真的变了的时候动,所以要先落回起点。
  private func replayReveal() {
    revealed = false
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) { revealed = true }
  }

  private func reload() { update {} }
  private func update(_ operation: () throws -> Void) {
    availability = store.availability()
    do {
      try operation()
      statistics = try store.load()
      errorMessage = ""
    } catch {
      // A locked device is one reason among several, and naming only that one sent a reader
      // looking in the wrong place. Carry what actually failed.
      errorMessage = "无法读取或保存统计：\(error.localizedDescription)"
    }
  }
}

/// 按日明细:Windows 统计页那张九列表格。
///
/// iPad 的宽窗口照原样画九列;手机竖屏放不下九列,每天一行字数、下面一行小字列分类和速度 —— 挤成九列只会每格一个数字、谁也读不清。完整数据导成 CSV 交给分享面板,手机上真要逐列比对,在表格 app 里比在这里顺手。
struct TypingDailyDetailView: View {
  let statistics: TypingStatistics
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @State private var exportFile: URL?

  /// 和 Windows 一样只列最近 30 个有记录的日子;更早的在导出的 CSV 里。
  static let dayLimit = 30

  private var rows: [TypingDailyRow] { statistics.dailyRows(limit: Self.dayLimit) }

  var body: some View {
    Form {
      if rows.isEmpty {
        Section { Text("暂无输入记录").foregroundStyle(.secondary) }
      } else if horizontalSizeClass == .regular {
        Section { table } footer: { footer }
      } else {
        Section {
          ForEach(rows, id: \.day) { compactRow($0) }
        } footer: { footer }
      }
    }
    .navigationTitle("按日明细").navigationBarTitleDisplayMode(.inline)
    .toolbar {
      if let exportFile {
        ToolbarItem(placement: .navigationBarTrailing) {
          ShareLink(item: exportFile) { Label("导出 CSV", systemImage: "square.and.arrow.up") }
            .accessibilityIdentifier("typingDailyExport")
        }
      }
    }
    .onAppear { exportFile = Self.writeExport(statistics) }
  }

  private var footer: some View {
    Text("最近 \(Self.dayLimit) 个有记录的日子，新的在上。“其他”含其他文字、表情和符号。速度只按汉字与字母计算，和“节奏”页同一口径。导出的 CSV 含全部保留的日子。")
  }

  private var table: some View {
    Grid(alignment: .trailing, horizontalSpacing: 14, verticalSpacing: 10) {
      GridRow {
        ForEach(["日期", "字数", "中文", "英文", "数字", "标点", "其他", "活跃", "速度"], id: \.self) {
          Text($0).font(.caption).foregroundStyle(.secondary)
        }
      }
      Divider()
      ForEach(rows, id: \.day) { row in
        GridRow {
          Text(Self.dayLabel(row.day)).gridColumnAlignment(.leading)
          Text("\(row.total)").fontWeight(.semibold)
          Text("\(row.han)").foregroundStyle(.secondary)
          Text("\(row.latin)").foregroundStyle(.secondary)
          Text("\(row.number)").foregroundStyle(.secondary)
          Text("\(row.punctuation)").foregroundStyle(.secondary)
          Text("\(row.other)").foregroundStyle(.secondary)
          Text(TypingActivity.formatActiveTime(row.activeMs))
          Text("\(Int(row.speed.rounded())) 字/分")
        }.monospacedDigit().accessibilityElement(children: .combine)
      }
    }.padding(.vertical, 6).accessibilityIdentifier("typingDailyTable")
  }

  private func compactRow(_ row: TypingDailyRow) -> some View {
    VStack(alignment: .leading, spacing: 4) {
      HStack {
        Text(Self.dayLabel(row.day))
        Spacer()
        Text("\(row.total) 字符").fontWeight(.semibold).monospacedDigit()
      }
      Text(Self.compactSummary(row)).font(.caption).foregroundStyle(.secondary).monospacedDigit()
    }.accessibilityElement(children: .combine)
  }

  /// 手机上那一行小字。为零的分类不列,不然每天都是一串「数字 0 · 标点 0」。
  static func compactSummary(_ row: TypingDailyRow) -> String {
    var parts = [("中文", row.han), ("英文", row.latin), ("数字", row.number), ("标点", row.punctuation), ("其他", row.other)]
      .filter { $0.1 > 0 }.map { "\($0.0) \($0.1)" }
    if row.activeMs > 0 {
      parts.append("活跃 \(TypingActivity.formatActiveTime(row.activeMs))")
      parts.append("\(Int(row.speed.rounded())) 字/分")
    }
    return parts.joined(separator: " · ")
  }

  /// `9月21日 周一`。明细按天读,星期几比年份有用。
  static func dayLabel(_ key: String) -> String {
    guard let date = TypingStatistics.parseDayKey(key) else { return key }
    var style = Date.FormatStyle.dateTime.month().day().weekday(.abbreviated)
    style.timeZone = TimeZone(secondsFromGMT: 0)!
    return date.formatted(style)
  }

  /// 写到临时目录,文件名给分享面板和「存储到文件」用。写不了就不给导出按钮,而不是给一个点了没反应的按钮。
  static func writeExport(_ statistics: TypingStatistics) -> URL? {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("水杉IME-打字统计.csv")
    guard (try? Data(statistics.dailyCSV().utf8).write(to: url, options: .atomic)) != nil else { return nil }
    return url
  }
}
