import SwiftUI

/// 统计标签页：概览 / 习惯 / 按键 / 成就，以卡片排在季节页面上。每个数字都来自共享存储的 `summary` 操作，规则在 Rust 里；页面只负责排版，Rust 没给出数字的地方显示 '—'。统计文档本身仍要读，用于开关、保留期、热力图的按键计数和按日明细表。
///
/// 统计只保留聚合计数，所以设计稿里的常用词和最常打错两张卡片没有做：它们需要用户打过的词本身。
struct TypingStatisticsView: View {
  @Environment(\.scenePhase) private var scenePhase
  @Environment(\.horizontalSizeClass) private var horizontalSizeClass
  @State private var statistics = TypingStatistics()
  /// 统计文档是否至少读到过一次，这样关闭提示说的是存储里的开关，而不是空的默认值。
  @State private var loaded = false
  @State private var summary: TypingSummary?
  /// 交给 `summary` 的用户自造词数量；键盘的词库快照给不出可靠数字时为 nil。
  @State private var userWords: Int?
  @State private var errorMessage = ""
  @State private var confirmsReset = false
  @State private var tab = Tab.overview
  /// 读者选过之后按键热力图所用的键盘布局；nil 表示跟随当前输入方案。
  @State private var nineKeyChoice: Bool?
  @State private var showsDailyDetail = false
  @State private var showsAbout = false
  @State private var availability = TypingStatisticsStore.Availability.neverWritten
  /// 至少有一次读取结果（不论成败）已经回到页面；在此之前 availability 与 summary 还是初始值，不能据此提示存储不可用或读不到。之后的重新读取不再清掉它，页面保留上一次的内容。
  @State private var settled = false
  /// 正在进行的存储操作。每次更新都等前一次完成，所以重新读取永远不会抢在先发起的写入前面落地。
  @State private var pending: Task<Void, Never>?
  private let store = TypingStatisticsStore()

  private enum Tab: CaseIterable {
    case overview, habits, keys, achievements

    var title: String {
      switch self {
      case .overview: return "概览"
      case .habits: return "习惯"
      case .keys: return "按键"
      case .achievements: return "成就"
      }
    }
  }

  // 旧文案无条件要求开启完全访问，不管问题是不是出在这个设置上都说同一句话，等于没有信息。这里每种情况都是对「为什么是空的」的不同回答，正常运行时则什么都不说。
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

  /// 最近七个本地日，今天排最后：按键热力图的时间窗口，和按键小卡片一致。
  private var weekDates: [Date] {
    let today = Calendar.current.startOfDay(for: Date())
    return (0..<7).reversed().compactMap { Calendar.current.date(byAdding: .day, value: -$0, to: today) }
  }

  private var keyHeatmap: TypingKeyHeatmap { TypingKeyHeatmap(counts: statistics.keyCounts(on: weekDates)) }

  /// 热力图默认显示的键盘布局：键盘当前方案是九键时用九键。
  private var showsNineKey: Bool {
    nineKeyChoice ?? [.nineKey, .japaneseNineKey].contains(InputSchemePreference.scheme)
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 22) {
        DesignSegmentedControl(items: Tab.allCases.map { ($0.title, $0) }, selection: $tab, style: .rounded,
                               identifierPrefix: "statisticsTab")
        notices
        if let summary {
          switch tab {
          case .overview: overview(summary.overview)
          case .habits: habits(summary.habits)
          case .keys: keys(summary)
          case .achievements:
            StatisticsAchievementsView(achievements: summary.achievements, unlockedCount: summary.unlockedCount,
                                       userWordsKnown: userWords != nil)
          }
        }
        footer
      }
      .padding(.horizontal, 16)
      .padding(.top, 4)
      .padding(.bottom, 24)
      .frame(maxWidth: horizontalSizeClass == .regular ? 760 : .infinity)
      .frame(maxWidth: .infinity)
    }
    .background(MetasequoiaTheme.canvas.ignoresSafeArea())
    .navigationTitle("统计").navigationBarTitleDisplayMode(.large)
    .toolbar {
      ToolbarItem(placement: .navigationBarTrailing) { menu }
    }
    .navigationDestination(isPresented: $showsDailyDetail) { TypingDailyDetailView(statistics: statistics) }
    .sheet(isPresented: $showsAbout) { about }
    .onAppear { reload() }
    .onChange(of: scenePhase) { _, phase in if phase == .active { reload() } }
    .alert("清空所有打字统计？", isPresented: $confirmsReset) {
      Button("取消", role: .cancel) {}
      Button("清空", role: .destructive) { update { try $0.reset() } }
    } message: { Text("累计字数、分类、按键次数、每日记录和已解锁的成就将被删除，无法恢复。") }
  }

  // MARK: - 菜单、提示与页脚

  // 开关、保留期、刷新和重置放在菜单里：这个页面是用来看数字的，管理项放在页面上会出现在每个分段下面。
  private var menu: some View {
    Menu {
      Button {
        let enabled = !statistics.enabled
        update { try $0.setEnabled(enabled) }
      } label: {
        // iOS 15 上 Menu 里的 Toggle 画不出来，所以用对勾表示开关已打开。
        if statistics.enabled { Label("记录打字统计", systemImage: "checkmark") }
        else { Text("记录打字统计") }
      }
      .accessibilityIdentifier("typingStatisticsEnabled")
      Picker(selection: Binding(get: { statistics.retentionDays ?? 0 }, set: { days in
        update { try $0.setRetention(days == 0 ? nil : days) }
      })) {
        Text("永久保留").tag(0)
        ForEach(TypingStatistics.retentionChoices, id: \.self) { Text("保留最近 \($0) 天").tag($0) }
      } label: {
        Label("每日记录", systemImage: "calendar.badge.clock")
      }
      .pickerStyle(.menu)
      .accessibilityIdentifier("typingStatisticsRetention")
      Button { showsDailyDetail = true } label: { Label("按日明细", systemImage: "tablecells") }
        .accessibilityIdentifier("typingDailyDetailsMenu")
      Button { showsAbout = true } label: { Label("关于统计", systemImage: "info.circle") }
        .accessibilityIdentifier("typingStatisticsAbout")
      Button("刷新统计") { reload() }
      Button("清空统计", role: .destructive) { confirmsReset = true }
        .accessibilityIdentifier("resetTypingStatistics")
    } label: {
      Image(systemName: "ellipsis.circle")
    }
    .accessibilityLabel("统计选项")
    .accessibilityIdentifier("statisticsMenu")
  }

  /// 页面可能为空或过时的原因：统计已关闭（默认）、键盘从未写到存储，或读取失败。
  @ViewBuilder private var notices: some View {
    if !settled {
      HStack(spacing: 8) {
        ProgressView()
        Text("正在读取统计…").font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .statisticsCard()
    }
    if loaded && !statistics.enabled {
      notice("记录已关闭", text: "已有的计数保留在本机，新的输入不再计入。统计默认关闭，打开后键盘只记录字数和按键次数，不记录输入内容。") {
        Button("开启记录") { update { try $0.setEnabled(true) } }
          .font(.system(size: 15, weight: .semibold))
          .foregroundStyle(MetasequoiaTheme.accent)
          .accessibilityIdentifier("typingStatisticsEnableNotice")
      }
    }
    if settled, let advice = storageAdvice {
      notice("统计没有数据", text: advice) {
        Button("前往系统设置") {
          guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
          UIApplication.shared.open(url)
        }
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(MetasequoiaTheme.accent)
      }
    }
    if !errorMessage.isEmpty {
      notice(nil, text: errorMessage) { EmptyView() }
    } else if settled && summary == nil {
      notice(nil, text: "统计暂时读不到，可以在右上角菜单里刷新。") { EmptyView() }
    }
  }

  private func notice<Action: View>(_ title: String?, text: String, @ViewBuilder action: () -> Action) -> some View {
    VStack(alignment: .leading, spacing: 8) {
      if let title { Text(title).font(.system(size: 15, weight: .semibold)) }
      Text(text).font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub).fixedSize(horizontal: false, vertical: true)
      action()
    }
    .statisticsCard()
    .accessibilityElement(children: .contain)
  }

  private var footer: some View {
    HStack(spacing: 4) {
      Image(systemName: "lock").font(.system(size: 13))
      Text("统计只保存在本机，不包含输入内容").font(.system(size: 12))
    }
    .foregroundStyle(MetasequoiaTheme.sub)
    .frame(maxWidth: .infinity)
    .accessibilityElement(children: .combine)
    .accessibilityIdentifier("statisticsPrivacyFooter")
  }

  /// 原先放在页面底部的说明：每个数字怎么统计、保留了什么。
  private var about: some View {
    NavigationStack {
      ScrollView {
        VStack(alignment: .leading, spacing: 14) {
          ForEach(Self.aboutParagraphs, id: \.self) { paragraph in
            Text(paragraph).font(.system(size: 15)).fixedSize(horizontal: false, vertical: true)
          }
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
      }
      .background(MetasequoiaTheme.canvas.ignoresSafeArea())
      .navigationTitle("关于统计").navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .confirmationAction) { Button("完成") { showsAbout = false } }
      }
    }
    .presentationDetents([.medium, .large])
  }

  private static let aboutParagraphs = [
    "字数只统计水杉键盘提交的字符，含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。",
    "平均速度按连续打字的时间计算，两次上屏间隔超过 10 秒算休息、不计入；只统计汉字与各种文字的字母，数字、标点和表情不参与。首选命中在选词满 50 次后显示。",
    "按键热力图另计每个键的按下次数，拼音拼写、删除和功能键都算，密码框里的按键不计；只保存每个键每天被按下的次数，不保存按键顺序和输入内容。",
    "成就由统计数据推算，清空统计时一并清除。",
    "所有统计仅在本机保存计数，不保存输入内容。每日明细默认永久保留，可在右上角菜单里缩短为 30 至 365 天，超期的每日记录随即删除，并同时从累计总数和分类中扣除；要全部删除请用“清空统计”。",
  ]

  // MARK: - 概览

  private func overview(_ overview: TypingSummary.Overview) -> some View {
    VStack(alignment: .leading, spacing: 22) {
      VStack(alignment: .leading, spacing: 18) {
        VStack(alignment: .leading, spacing: 4) {
          Text("近 7 天共输入").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
          HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(TypingSummaryText.grouped(overview.weekTotal))
              .font(.system(size: 40, weight: .bold)).monospacedDigit().tracking(-0.8)
              .lineLimit(1).minimumScaleFactor(0.6)
              .accessibilityIdentifier("typingWeekTotal")
            Text("字").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
          }
          if let delta = TypingSummaryText.weekDelta(overview.weekTotal, overview.previousWeekTotal) {
            Text(delta).font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
          }
        }
        .accessibilityElement(children: .combine)
        StatisticsWeekBars(days: overview.last7)
      }
      .statisticsCard(top: 18, horizontal: 16, bottom: 14)
      let speedNote = TypingSummaryText.speedDelta(overview.averageSpeed, overview.previousAverageSpeed)
      StatisticsTileGrid {
        StatisticsTile(label: "平均速度", value: TypingSummaryText.whole(overview.averageSpeed), unit: "字/分",
                       note: speedNote ?? "近 7 天的活跃时间里", identifier: "typingAverageSpeed")
        StatisticsTile(label: "首选命中", value: TypingSummaryText.percent(overview.firstCandidateRate), unit: "%",
                       note: overview.firstCandidateRate == nil ? "选词满 50 次后显示" : "第一个候选就是你要的",
                       identifier: "typingFirstCandidate")
        StatisticsTile(label: "少按键", value: TypingSummaryText.percent(overview.keystrokesSavedRate), unit: "%",
                       note: "联想和整句帮你省下", identifier: "typingKeystrokesSaved")
        StatisticsTile(label: "连续使用", value: "\(overview.currentStreak)", unit: "天",
                       note: "最长 \(overview.longestStreak) 天", identifier: "typingStreak")
      }
    }
  }

  // MARK: - 习惯

  private func habits(_ habits: TypingSummary.Habits) -> some View {
    let peak = TypingSummaryText.peakLabel(habits.peakWindow)
    return VStack(alignment: .leading, spacing: 22) {
      StatisticsSection("近 12 周", note: "活跃 \(habits.activeDays) 天") {
        StatisticsWeeksHeatmap(days: habits.weeks12).statisticsCard(top: 14, horizontal: 14, bottom: 12)
      }
      StatisticsSection("活跃时段", note: peak.map { "最常在 \($0)" }) {
        StatisticsHourBars(hours: habits.hours24, peak: habits.peakWindow, peakLabel: peak).statisticsCard(top: 14, horizontal: 14, bottom: 12)
      }
      StatisticsSection("输入构成", note: nil) {
        let composition = TypingSummaryText.composition(habits.breakdown.characters)
        Group {
          if composition.allSatisfy({ $0.count == 0 }) {
            Text("还没有记录").font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
          } else {
            StatisticsCompositionBar(shares: composition)
          }
        }
        .statisticsCard()
      }
      NavigationLink {
        TypingDailyDetailView(statistics: statistics)
      } label: {
        DesignNavRowLabel(title: "按日明细", symbol: "tablecells")
      }
      .buttonStyle(PressFillButtonStyle())
      .background(MetasequoiaTheme.surface)
      .clipShape(RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
      .accessibilityIdentifier("typingDailyDetails")
    }
  }

  // MARK: - 按键

  private func keys(_ summary: TypingSummary) -> some View {
    let keys = summary.keys
    let heatmap = keyHeatmap
    let nineKey = showsNineKey
    let perKeyNote = TypingSummaryText.perKeyDelta(keys.perCharacterKeys, keys.previousPerCharacterKeys)
    let methods = TypingSummaryText.methods(summary.habits.breakdown.sources)
    return VStack(alignment: .leading, spacing: 22) {
      StatisticsSection("按键热力图", trailing: {
        StatisticsLayoutSwitch(nineKey: Binding(get: { nineKey }, set: { nineKeyChoice = $0 }))
      }) {
        VStack(alignment: .leading, spacing: 10) {
          StatisticsKeyboardHeatmap(heatmap: heatmap, nineKey: nineKey)
            .statisticsCard(top: 10, horizontal: 4, bottom: 10)
          let others = heatmap.others(nineKey: nineKey)
          if !others.isEmpty {
            DisclosureGroup {
              VStack(spacing: 0) {
                ForEach(others, id: \.id) { key in
                  HStack {
                    Text(key.label).font(.system(size: 15))
                    Spacer()
                    Text("\(key.count.formatted()) 次").font(.system(size: 15)).monospacedDigit().foregroundStyle(MetasequoiaTheme.sub)
                  }
                  .frame(minHeight: 36)
                  .accessibilityElement(children: .ignore)
                  .accessibilityLabel(TypingKeyHeatmap.accessibilityLabel(key.id, count: key.count))
                }
                Text("数字、标点、符号键等键盘图上没有位置的键。符号按产生它的英文键盘按键计，例如“！”计入 1 键。")
                  .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
                  .fixedSize(horizontal: false, vertical: true)
                  .padding(.top, 6)
              }
              .padding(.top, 6)
            } label: {
              Text("其他键").font(.system(size: 15)).foregroundStyle(.primary)
            }
            .tint(MetasequoiaTheme.sub)
            .statisticsCard(top: 12, horizontal: 16, bottom: 12)
            .accessibilityIdentifier("statisticsOtherKeys")
          }
        }
      }
      StatisticsTileGrid {
        StatisticsTile(label: "每字按键", value: TypingSummaryText.decimal(keys.perCharacterKeys), unit: "次",
                       note: perKeyNote ?? "近 7 天平均", noteAccent: true, identifier: "typingKeysPerCharacter")
        StatisticsTile(label: "退格占比", value: TypingSummaryText.percentTenths(keys.backspaceRate), unit: "%",
                       note: "近 7 天全部按键里", noteAccent: true, identifier: "typingBackspaceRate")
        StatisticsTile(label: "联想上屏", value: TypingSummaryText.percent(keys.predictionRate), unit: "%",
                       note: "不用打完就上屏的词", identifier: "typingPredictionRate")
        StatisticsTile(label: "单次最长", value: keys.longestRun.map { "\($0.characters)" } ?? "—", unit: "字",
                       note: keys.longestRun.map { "\(TypingSummaryText.monthDay($0.day)) · 不停顿" } ?? "还没有记录",
                       identifier: "typingLongestRun")
      }
      StatisticsSection("选词位置", note: nil) {
        Group {
          // 寥寥几次选词算出的比例没有意义；Rust 要到 50 次选词才给出首选率，候选位置分布也等到同样的门槛。
          if let positions = keys.positions, summary.overview.firstCandidateRate != nil {
            StatisticsPositionBars(positions: positions)
          } else {
            Text("选词满 50 次后显示").font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
          }
        }
        .statisticsCard(top: 10, horizontal: 16, bottom: 10)
      }
      if !methods.isEmpty {
        StatisticsSection("输入方式", note: nil) {
          StatisticsMethodDonut(shares: methods).statisticsCard()
        }
      }
    }
  }

  // MARK: - 加载

  private func reload() { update { _ in } }

  /// 先执行 `operation`，再读取统计、词库快照和汇总，全部在主线程之外进行：每次存储调用都要拿统计文件锁，键盘落盘时会持有这把锁，而且 `summary` 会写入新解锁的徽章。结果按更新发起的顺序回到 main actor 上赋值。
  private func update(_ operation: @escaping @Sendable (TypingStatisticsStore) throws -> Void) {
    let store = store
    let previous = pending
    pending = Task {
      await previous?.value
      let result = await Task.detached(priority: .userInitiated) { Self.load(store, after: operation) }.value
      availability = result.availability
      if let loadedStatistics = result.statistics {
        statistics = loadedStatistics
        loaded = true
        userWords = result.userWords
      }
      if let loadedSummary = result.summary { summary = loadedSummary }
      errorMessage = result.error ?? ""
      settled = true
    }
  }

  /// 一次更新读到的内容。产出某个字段的步骤没有执行或失败时，该字段保持 nil，页面继续显示原来的内容。
  private struct LoadResult {
    var availability: TypingStatisticsStore.Availability
    var statistics: TypingStatistics?
    var userWords: Int?
    var summary: TypingSummary?
    var error: String?
  }

  private nonisolated static func load(_ store: TypingStatisticsStore,
                                       after operation: (TypingStatisticsStore) throws -> Void) -> LoadResult {
    var result = LoadResult(availability: store.availability())
    do {
      try operation(store)
      result.statistics = try store.load()
      result.userWords = (try? PersonalDictionaryStore.forSettingsPages().read()).flatMap(TypingSummary.userWords(from:))
      result.summary = try store.summary(userWords: result.userWords)
    } catch {
      // 设备锁定只是几种原因之一，只点出这一种会把读者引到错误的地方去找。这里带上实际失败的原因。
      result.error = "无法读取或保存统计：\(error.localizedDescription)"
    }
    return result
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
    Text("最近 \(Self.dayLimit) 个有记录的日子，新的在上。“其他”含其他文字、表情和符号。速度只按汉字与字母计算，和概览里的平均速度同一口径。导出的 CSV 含全部保留的日子。")
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
