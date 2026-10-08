import SwiftUI
import UIKit

/// 「统计」标签页的构件：分区标题、卡片、指标块，以及「概览」「习惯」「按键」三部分的图表。所有数字都来自 Rust 的 `summary`（按键热力图则来自按天存下的按键计数），这些视图只负责画出来。配色用季节强调色及其 `accentMix` 梯度，对应设计稿的 `aM(p)`。
enum StatisticsStyle {
  /// 设计稿的 `cubic-bezier(.2,.8,.2,1)`，所有生长动画都用它。
  static func curve(_ duration: Double) -> Animation { .timingCurve(0.2, 0.8, 0.2, 1, duration: duration) }

  /// 非高亮柱的颜色：浅色 aM18，深色 aM24。
  static let mutedBar = MetasequoiaTheme.accentMix(18, 24)

  /// 进度条的未填充部分：rgba(0,0,0,.07) / rgba(255,255,255,.1)。
  static let track = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark ? UIColor(white: 1, alpha: 0.1) : UIColor(white: 0, alpha: 0.07)
  })

  /// 小号分段控件的轨道：浅色 aM9，深色 rgba(255,255,255,.08)。
  static let segmentTrack = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(white: 1, alpha: 0.08)
      : MetasequoiaTheme.mixUIColor(9).resolvedColor(with: traits)
  })

  /// 分段控件选中项的滑块：白色 / #636366。
  static let segmentThumb = Color(uiColor: UIColor { traits in
    traits.userInterfaceStyle == .dark ? UIColor(red: 99 / 255, green: 99 / 255, blue: 102 / 255, alpha: 1) : .white
  })

  /// 12 周热力图的五档颜色：aM 6 / 18 / 38 / 62 / 90。
  static let dayHeat: [Color] = [6, 18, 38, 62, 90].map { MetasequoiaTheme.accentMix($0) }
  /// 按键热力图的五档颜色：aM 6 / 16 / 32 / 56 / 88。
  static let keyHeat: [Color] = [6, 16, 32, 56, 88].map { MetasequoiaTheme.accentMix($0) }
  /// 占比图各部分依次使用的颜色：强调色，然后是 aM 52 / 28 / 15。
  static let shares: [Color] = [MetasequoiaTheme.accent, MetasequoiaTheme.accentMix(52), MetasequoiaTheme.accentMix(28), MetasequoiaTheme.accentMix(15)]
  /// 「选词位置」的填充色：强调色，然后是 aM 60 / 40 / 25。
  static let positions: [Color] = [MetasequoiaTheme.accent, MetasequoiaTheme.accentMix(60), MetasequoiaTheme.accentMix(40), MetasequoiaTheme.accentMix(25)]
}

extension View {
  /// 统计卡片：内容背后衬季节卡片色，形状是 10pt 的连续圆角矩形。
  func statisticsCard(top: CGFloat = 16, horizontal: CGFloat = 16, bottom: CGFloat = 16) -> some View {
    padding(.top, top)
      .padding(.horizontal, horizontal)
      .padding(.bottom, bottom)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
  }
}

/// 页面里带标题的一块：13pt 小标题，可带尾部说明或控件，内容在其下方 8pt。
struct StatisticsSection<Trailing: View, Content: View>: View {
  let title: String
  private let trailing: Trailing
  private let content: Content

  init(_ title: String, @ViewBuilder trailing: () -> Trailing, @ViewBuilder content: () -> Content) {
    self.title = title
    self.trailing = trailing()
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      HStack(alignment: .center, spacing: 8) {
        Text(title).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
          .accessibilityAddTraits(.isHeader)
        Spacer(minLength: 8)
        trailing
      }
      .padding(.horizontal, 4)
      .frame(minHeight: 24)
      content
    }
  }
}

extension StatisticsSection where Trailing == Text {
  init(_ title: String, note: String?, @ViewBuilder content: () -> Content) {
    self.init(title, trailing: { Text(note ?? "").font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub) }, content: content)
  }
}

/// KPI 指标块：13pt 标签，24pt 粗体数字带单位，12pt 说明。数字为 '—' 时不显示单位。
struct StatisticsTile: View {
  let label: String
  let value: String
  let unit: String
  let note: String
  var noteAccent = false
  let identifier: String

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(label).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
      HStack(alignment: .firstTextBaseline, spacing: 3) {
        Text(value).font(.system(size: 24, weight: .bold)).monospacedDigit().foregroundStyle(.primary)
          .lineLimit(1).minimumScaleFactor(0.6)
        if value != "—" {
          Text(unit).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1)
        }
      }
      Text(note).font(.system(size: 12)).foregroundStyle(noteAccent ? MetasequoiaTheme.accent : MetasequoiaTheme.sub)
        .lineLimit(2).fixedSize(horizontal: false, vertical: true)
    }
    .padding(14)
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("\(label) \(value)\(value == "—" ? "" : " \(unit)")，\(note)")
    .accessibilityIdentifier(identifier)
  }
}

/// 指标块两列排布，常规宽度窗口下四列，间距 10pt。
struct StatisticsTileGrid<Content: View>: View {
  @Environment(\.horizontalSizeClass) private var sizeClass
  private let content: Content

  init(@ViewBuilder content: () -> Content) {
    self.content = content()
  }

  var body: some View {
    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10, alignment: .top),
                             count: sizeClass == .regular ? 4 : 2), spacing: 10) {
      content
    }
  }
}

/// 小号的 `少 □□□□□ 多` 图例，使用给定的五档颜色。
struct StatisticsHeatLegend: View {
  let levels: [Color]

  var body: some View {
    HStack(spacing: 3) {
      Text("少").font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub).padding(.trailing, 1)
      ForEach(levels.indices, id: \.self) { index in
        RoundedRectangle(cornerRadius: 2).fill(levels[index]).frame(width: 10, height: 10)
      }
      Text("多").font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub).padding(.leading, 1)
    }
    .accessibilityHidden(true)
  }
}

// MARK: - 概览

/// 主卡片的七根柱，每天一根，最早的在前：今天用强调色并用主文字色标注，其余淡化。高度按最忙的一天缩放，空白日 4pt，最高 118pt，从零开始长出。
struct StatisticsWeekBars: View {
  let days: [TypingSummary.DayCount]
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  private static let maximum: CGFloat = 118
  private static let minimum: CGFloat = 4
  private static let weekdays = ["一", "二", "三", "四", "五", "六", "日"]

  var body: some View {
    let peak = max(1, days.map(\.count).max() ?? 0)
    HStack(alignment: .bottom, spacing: 10) {
      ForEach(Array(days.enumerated()), id: \.offset) { index, day in
        let isToday = index == days.count - 1
        let height = max(Self.minimum, (Self.maximum * CGFloat(day.count) / CGFloat(peak)).rounded())
        VStack(spacing: 8) {
          RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(isToday ? MetasequoiaTheme.accent : StatisticsStyle.mutedBar)
            .frame(maxWidth: 30)
            .frame(height: grown ? height : 0)
            .frame(height: Self.maximum, alignment: .bottom)
          Text(Self.weekday(day.day)).font(.system(size: 11))
            .foregroundStyle(isToday ? Color.primary : MetasequoiaTheme.sub)
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("星期\(Self.weekday(day.day))，\(day.count) 字")
      }
    }
    .frame(height: 140, alignment: .bottom)
    .accessibilityElement(children: .contain)
    .accessibilityLabel("近 7 天每日字数")
    .accessibilityIdentifier("statisticsWeekBars")
    .onAppear {
      guard !grown else { return }
      if reduceMotion { grown = true } else { withAnimation(StatisticsStyle.curve(0.7)) { grown = true } }
    }
  }

  /// `YYYY-MM-DD` 日期键对应的星期字，周一为一周之首。
  static func weekday(_ key: String) -> String {
    guard let date = TypingStatistics.parseDayKey(key) else { return "" }
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(secondsFromGMT: 0)!
    let weekday = calendar.component(.weekday, from: date)
    return weekdays[(weekday + 5) % 7]
  }
}

// MARK: - 习惯

/// 12 周热力图：十二列、每列七天，一列一周，最早的在左上、今天在右下。每天的档位按它占最忙一天的比例取，量化方式与 Android 的 `HeatmapView` 相同。
struct StatisticsWeeksHeatmap: View {
  let days: [TypingSummary.DayCount]

  private static let columns = 12
  private static let rows = 7

  /// 没有输入为 0，否则按占最忙一天的比例取 1 到 4。
  static func level(_ count: Int, peak: Int) -> Int {
    guard count > 0, peak > 0 else { return 0 }
    let share = Double(count) / Double(peak)
    if share > 0.75 { return 4 }
    if share > 0.5 { return 3 }
    if share > 0.25 { return 2 }
    return 1
  }

  var body: some View {
    let cells = Self.columns * Self.rows
    let shown = Array(days.suffix(cells))
    let offset = cells - shown.count
    let peak = shown.map(\.count).max() ?? 0
    VStack(alignment: .trailing, spacing: 10) {
      HStack(alignment: .top, spacing: 3) {
        ForEach(0..<Self.columns, id: \.self) { column in
          VStack(spacing: 3) {
            ForEach(0..<Self.rows, id: \.self) { row in
              let index = column * Self.rows + row - offset
              let count = index >= 0 ? shown[index].count : 0
              RoundedRectangle(cornerRadius: 3, style: .continuous)
                .fill(StatisticsStyle.dayHeat[Self.level(count, peak: peak)])
                .aspectRatio(1, contentMode: .fit)
            }
          }
          .frame(maxWidth: .infinity)
        }
      }
      StatisticsHeatLegend(levels: StatisticsStyle.dayHeat)
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("近 12 周输入热力图，\(shown.filter { $0.count > 0 }.count) 天有输入，最多一天 \(peak) 字")
    .accessibilityIdentifier("statisticsWeeksHeatmap")
  }
}

/// 活跃时段：最近七天每小时输入字数的 24 根细柱，排在 56pt 高的一行里，峰值时段用强调色，下方是 0 时 · 6 · 12 · 18 · 24 坐标轴。
struct StatisticsHourBars: View {
  let hours: [Int]
  let peak: TypingSummary.PeakWindow?
  let peakLabel: String?
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  private static let height: CGFloat = 56

  var body: some View {
    let values = (0..<TypingActivity.hours).map { $0 < hours.count ? max(0, hours[$0]) : 0 }
    let highest = max(1, values.max() ?? 0)
    VStack(spacing: 6) {
      HStack(alignment: .bottom, spacing: 2) {
        ForEach(values.indices, id: \.self) { hour in
          let height = max(3, (Self.height * CGFloat(values[hour]) / CGFloat(highest)).rounded())
          RoundedRectangle(cornerRadius: 2, style: .continuous)
            .fill(peak?.contains(hour) == true ? MetasequoiaTheme.accent : StatisticsStyle.mutedBar)
            .frame(maxWidth: .infinity)
            .frame(height: grown ? height : 3)
        }
      }
      .frame(height: Self.height, alignment: .bottom)
      HStack {
        ForEach(["0 时", "6", "12", "18", "24"], id: \.self) { tick in
          Text(tick).font(.system(size: 11)).foregroundStyle(MetasequoiaTheme.sub)
          if tick != "24" { Spacer(minLength: 0) }
        }
      }
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(peakLabel.map { "24 小时输入分布，最常在\($0)" } ?? "24 小时输入分布，近 7 天还没有记录")
    .accessibilityIdentifier("statisticsHours")
    .onAppear {
      guard !grown else { return }
      if reduceMotion { grown = true } else { withAnimation(StatisticsStyle.curve(0.7)) { grown = true } }
    }
  }
}

/// 输入构成：各部分组成的 12pt 堆叠条，间隔 2pt，下方是两列图例，带 8pt 圆点和整数百分比。每部分的颜色按位置固定，所以没有输入的部分在图例里也保持同样的颜色。
struct StatisticsCompositionBar: View {
  let shares: [TypingSummaryText.Share]
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  var body: some View {
    let total = shares.reduce(0) { $0 + $1.count }
    VStack(alignment: .leading, spacing: 14) {
      GeometryReader { geometry in
        let visible = shares.indices.filter { shares[$0].count > 0 }
        let usable = geometry.size.width - CGFloat(max(0, visible.count - 1)) * 2
        HStack(spacing: 2) {
          ForEach(visible, id: \.self) { index in
            Rectangle().fill(StatisticsStyle.shares[index % StatisticsStyle.shares.count])
              .frame(width: total == 0 ? 0 : usable * CGFloat(shares[index].count) / CGFloat(total))
          }
        }
        .frame(width: geometry.size.width, alignment: .leading)
        .mask(alignment: .leading) {
          Rectangle().frame(width: grown ? geometry.size.width : 0)
        }
      }
      .frame(height: 12)
      .background(StatisticsStyle.track)
      .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
      LazyVGrid(columns: [GridItem(.flexible(), spacing: 16), GridItem(.flexible(), spacing: 16)], alignment: .leading, spacing: 10) {
        ForEach(shares.indices, id: \.self) { index in
          let share = shares[index]
          HStack(spacing: 8) {
            Circle().fill(StatisticsStyle.shares[index % StatisticsStyle.shares.count]).frame(width: 8, height: 8)
            Text(share.title).font(.system(size: 14))
            Spacer(minLength: 4)
            Text("\(TypingSummaryText.share(share.count, of: total))%").font(.system(size: 14)).monospacedDigit()
              .foregroundStyle(MetasequoiaTheme.sub)
          }
          .accessibilityElement(children: .combine)
        }
      }
    }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("statisticsComposition")
    .onAppear {
      guard !grown else { return }
      if reduceMotion { grown = true } else { withAnimation(StatisticsStyle.curve(0.9)) { grown = true } }
    }
  }
}

// MARK: - 按键

/// 按键热力图标题旁的 26 键 / 9 键小切换：轨道 8pt，选项 24pt 配 6pt 滑块，文字 12pt。
struct StatisticsLayoutSwitch: View {
  @Binding var nineKey: Bool

  var body: some View {
    HStack(spacing: 2) {
      item("26 键", nine: false, index: 0)
      item("9 键", nine: true, index: 1)
    }
    .padding(2)
    .background(StatisticsStyle.segmentTrack, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    .animation(.easeInOut(duration: 0.2), value: nineKey)
  }

  private func item(_ title: String, nine: Bool, index: Int) -> some View {
    let isSelected = nineKey == nine
    return Button { nineKey = nine } label: {
      Text(title)
        .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
        .foregroundStyle(.primary)
        .padding(.horizontal, 10)
        .frame(height: 24)
        .background {
          if isSelected {
            RoundedRectangle(cornerRadius: 6, style: .continuous).fill(StatisticsStyle.segmentThumb)
              .shadow(color: .black.opacity(0.12), radius: 1, y: 1)
          }
        }
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier("statisticsKeyLayout-\(index)")
    .accessibilityAddTraits(isSelected ? .isSelected : [])
  }
}

/// 按键热力图：按键盘的画法画出 26 键或九键键盘，每个键按它在统计窗口内全部按键中的占比填色（`TypingKeyHeatmap.heatLevel`），占比印在键面下方。下面是按得最多的字母键和图例。各键依次出现。
///
/// 只画在键盘上有位置的键；其余的键由页面另外列出。
struct StatisticsKeyboardHeatmap: View {
  let heatmap: TypingKeyHeatmap
  let nineKey: Bool
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var appeared = false

  private static let gap: CGFloat = 4
  private static let rowGap: CGFloat = 6
  /// 一行以字母键宽度为单位排布：26 键键盘一行十个单位。
  private static let rowUnits: CGFloat = 10

  private var keyHeight: CGFloat { nineKey ? 52 : 44 }

  private static func units(_ id: String) -> CGFloat {
    switch id {
    case TypingKeyID.space: return 5
    case TypingKeyID.shift, TypingKeyID.backspace: return 1.5
    case TypingKeyID.layer, TypingKeyID.globe, TypingKeyID.language, TypingKeyID.enter: return 1.25
    default: return 1
    }
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      GeometryReader { geometry in
        if nineKey { nineKeyBoard(width: geometry.size.width) } else { letterBoard(width: geometry.size.width) }
      }
      .frame(height: boardHeight)
      .accessibilityElement(children: .contain)
      .accessibilityLabel(nineKey ? "9 键按键热力图" : "26 键按键热力图")
      .accessibilityIdentifier(nineKey ? "statisticsNineKey" : "statisticsKeyboard")
      HStack(alignment: .center, spacing: 8) {
        Text(headline).font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub).lineLimit(1).minimumScaleFactor(0.8)
        Spacer(minLength: 4)
        StatisticsHeatLegend(levels: StatisticsStyle.keyHeat)
      }
      .padding(.horizontal, 4)
    }
    .onAppear { reveal() }
    .onChange(of: nineKey) {
      appeared = false
      DispatchQueue.main.async { reveal() }
    }
  }

  private var boardHeight: CGFloat {
    let rows: CGFloat = nineKey ? 4 : CGFloat(TypingKeyHeatmap.keyboardRows.count)
    return keyHeight * rows + Self.rowGap * (rows - 1)
  }

  /// 26 键键盘显示 `字母键最常按 N · 9.6%`，九键键盘显示 `最常按 GHI · 16.2%`。
  private var headline: String {
    guard let id = heatmap.busiestLetterKey(nineKey: nineKey) else { return "近 7 天还没有按键记录" }
    let share = TypingSummaryText.percentTenths(heatmap.percent(id) / 100)
    return nineKey ? "最常按 \(face(id)) · \(share)%" : "字母键最常按 \(face(id)) · \(share)%"
  }

  private func reveal() {
    if reduceMotion { withAnimation(.easeOut(duration: 0.2)) { appeared = true } } else { appeared = true }
  }

  private func letterBoard(width: CGFloat) -> some View {
    let unit = (width - Self.gap * (Self.rowUnits - 1)) / Self.rowUnits
    let rows = TypingKeyHeatmap.keyboardRows
    return VStack(spacing: Self.rowGap) {
      ForEach(rows.indices, id: \.self) { row in
        HStack(spacing: Self.gap) {
          ForEach(rows[row], id: \.self) { id in
            let units = Self.units(id)
            key(id, order: order(row: row, id: id, in: rows))
              .frame(width: unit * units + Self.gap * (units - 1), height: keyHeight)
          }
        }
        .frame(maxWidth: .infinity)
      }
    }
  }

  /// 侧栏和右侧控制键宽 0.9 个单位，每个格子宽 1.3 个单位，与设计稿一致。
  private func nineKeyBoard(width: CGFloat) -> some View {
    let unit = (width - Self.gap * 4) / (0.9 + 1.3 * 3 + 0.9)
    let side = unit * 0.9
    let cell = unit * 1.3
    let gridHeight = keyHeight * 3 + Self.rowGap * 2
    let bottom = TypingKeyHeatmap.keyboardRows[TypingKeyHeatmap.keyboardRows.count - 1]
    let bottomUnit = (width - Self.gap * (Self.rowUnits - 1)) / Self.rowUnits
    return VStack(spacing: Self.rowGap) {
      HStack(alignment: .top, spacing: Self.gap) {
        key(TypingKeyHeatmap.nineKeySidebar, order: 0).frame(width: side, height: gridHeight)
        VStack(spacing: Self.rowGap) {
          ForEach(TypingKeyHeatmap.nineKeyGrid.indices, id: \.self) { row in
            HStack(spacing: Self.gap) {
              ForEach(Array(TypingKeyHeatmap.nineKeyGrid[row].enumerated()), id: \.element) { column, id in
                key(id, order: 1 + row * 5 + column).frame(width: cell, height: keyHeight)
              }
            }
          }
        }
        VStack(spacing: Self.rowGap) {
          ForEach(Array(TypingKeyHeatmap.nineKeyControls.enumerated()), id: \.element) { row, id in
            key(id, order: 4 + row * 5).frame(width: side, height: keyHeight)
          }
        }
      }
      HStack(spacing: Self.gap) {
        ForEach(Array(bottom.enumerated()), id: \.element) { column, id in
          let units = Self.units(id)
          key(id, order: 15 + column).frame(width: bottomUnit * units + Self.gap * (units - 1), height: keyHeight)
        }
      }
    }
    .frame(width: width)
  }

  private func order(row: Int, id: String, in rows: [[String]]) -> Int {
    rows.prefix(row).reduce(0) { $0 + $1.count } + (rows[row].firstIndex(of: id) ?? 0)
  }

  /// 键面显示的内容：字母、九键的字母组，或功能键的图标。
  private func face(_ id: String) -> String {
    switch id {
    case TypingKeyID.space: return "空格"
    case TypingKeyID.layer: return "123"
    case TypingKeyID.language: return "中/英"
    case TypingKeyID.punctuation: return "标点"
    case "Period": return "."
    case "Nine0": return "0"
    default:
      if let face = TypingKeyHeatmap.nineKeyFaces[id] { return face.pinyin }
      return TypingKeyID.label(id)
    }
  }

  private func symbol(_ id: String) -> String? {
    switch id {
    case TypingKeyID.shift: return "shift"
    case TypingKeyID.backspace: return "delete.left"
    case TypingKeyID.enter: return "return"
    case TypingKeyID.globe: return "globe"
    default: return nil
    }
  }

  private func key(_ id: String, order: Int) -> some View {
    let percent = heatmap.percent(id)
    let level = TypingKeyHeatmap.heatLevel(percent: percent, nineKey: nineKey)
    let ink = level >= 4 ? Color.white : Color.primary
    return RoundedRectangle(cornerRadius: 6, style: .continuous)
      .fill(StatisticsStyle.keyHeat[level])
      .overlay {
        VStack(spacing: 1) {
          if let symbol = symbol(id) {
            Image(systemName: symbol).font(.system(size: 13, weight: .semibold))
          } else {
            Text(face(id)).font(.system(size: nineKey ? 15 : 14, weight: .semibold))
              .lineLimit(1).minimumScaleFactor(0.5)
          }
          if heatmap.total > 0 {
            Text("\(TypingSummaryText.percentTenths(percent / 100))%").font(.system(size: 9)).monospacedDigit()
              .lineLimit(1).minimumScaleFactor(0.7)
              .opacity(0.85)
          }
        }
        .foregroundStyle(ink)
        .padding(.horizontal, 2)
      }
      .opacity(appeared ? 1 : 0)
      .offset(y: appeared || reduceMotion ? 0 : 10)
      .scaleEffect(appeared || reduceMotion ? 1 : 0.86)
      .animation(reduceMotion ? nil : StatisticsStyle.curve(0.45).delay(Double(order) * 0.018), value: appeared)
      .accessibilityElement(children: .ignore)
      .accessibilityLabel(TypingKeyHeatmap.accessibilityLabel(id, count: heatmap.count(id)))
      .accessibilityIdentifier("statisticsKey_\(id)")
  }
}

/// 选词位置：每个位置一行，54pt 标签、按占比填充的 10pt 进度条和整数百分比。各条依次长出。
struct StatisticsPositionBars: View {
  let positions: [Double]
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  private static let titles = ["第 1 个", "第 2 个", "第 3 个", "翻页后"]

  var body: some View {
    VStack(spacing: 0) {
      ForEach(Self.titles.indices, id: \.self) { index in
        let rate = index < positions.count ? SharedNumber.clamped(positions[index], to: 0...1) : 0
        HStack(spacing: 12) {
          Text(Self.titles[index]).font(.system(size: 14)).frame(width: 54, alignment: .leading)
          GeometryReader { geometry in
            ZStack(alignment: .leading) {
              Capsule().fill(StatisticsStyle.track)
              Capsule().fill(StatisticsStyle.positions[index])
                .frame(width: grown ? max(rate > 0 ? 10 : 0, geometry.size.width * rate) : 0)
                .animation(reduceMotion ? nil : StatisticsStyle.curve(0.9).delay(Double(index) * 0.08), value: grown)
            }
          }
          .frame(height: 10)
          Text("\(TypingSummaryText.percent(rate))%").font(.system(size: 14)).monospacedDigit()
            .foregroundStyle(MetasequoiaTheme.sub).frame(width: 44, alignment: .trailing)
        }
        .frame(height: 32)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(Self.titles[index]) \(TypingSummaryText.percent(rate))%")
      }
    }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("statisticsPositions")
    .onAppear { grown = true }
  }
}

/// 输入方式：96pt 环形图（中间 66pt 空心）展示各输入方式，中心显示最大占比及其名称，旁边是图例。
struct StatisticsMethodDonut: View {
  let shares: [TypingSummaryText.Share]
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  var body: some View {
    let total = shares.reduce(0) { $0 + $1.count }
    let top = shares.max { $0.count < $1.count }
    HStack(spacing: 20) {
      ZStack {
        Circle().stroke(StatisticsStyle.track, lineWidth: 15)
        ForEach(shares.indices, id: \.self) { index in
          let start = Double(shares[..<index].reduce(0) { $0 + $1.count }) / Double(max(1, total))
          let end = start + Double(shares[index].count) / Double(max(1, total))
          Circle()
            .trim(from: grown ? start : 0, to: grown ? end : 0)
            .stroke(StatisticsStyle.shares[index % StatisticsStyle.shares.count], lineWidth: 15)
            .rotationEffect(.degrees(-90))
        }
        if let top {
          VStack(spacing: 2) {
            Text("\(TypingSummaryText.share(top.count, of: total))%").font(.system(size: 17, weight: .bold)).monospacedDigit()
            Text(top.title).font(.system(size: 10)).foregroundStyle(MetasequoiaTheme.sub)
          }
        }
      }
      .padding(7.5)
      .frame(width: 96, height: 96)
      VStack(spacing: 0) {
        ForEach(shares.indices, id: \.self) { index in
          HStack(spacing: 8) {
            Circle().fill(StatisticsStyle.shares[index % StatisticsStyle.shares.count]).frame(width: 8, height: 8)
            Text(shares[index].title).font(.system(size: 14))
            Spacer(minLength: 4)
            Text("\(TypingSummaryText.share(shares[index].count, of: total))%").font(.system(size: 14)).monospacedDigit()
              .foregroundStyle(MetasequoiaTheme.sub)
          }
          .frame(height: 30)
        }
      }
    }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("输入方式，" + shares.map { "\($0.title) \(TypingSummaryText.share($0.count, of: total))%" }.joined(separator: "，"))
    .accessibilityIdentifier("statisticsMethods")
    .onAppear {
      guard !grown else { return }
      if reduceMotion { grown = true } else { withAnimation(StatisticsStyle.curve(1.1)) { grown = true } }
    }
  }
}
