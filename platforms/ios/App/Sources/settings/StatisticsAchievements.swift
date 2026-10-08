import SwiftUI
import UIKit

/// 成就：先是已解锁徽章的数量，再以三列网格列出全部徽章。所有解锁都由 Rust 在 `summary` 里判定，并按显示顺序返回徽章；这个视图只负责绘制。已解锁的徽章是带所属系列渐变的旋转方形奖章，未解锁的是进度环；点按会在 toast 里显示徽章，已解锁的奖章还会晃动一下。
struct StatisticsAchievementsView: View {
  let achievements: [TypingSummary.Achievement]
  /// 已解锁的徽章数，即 `TypingSummary.unlockedCount`。
  let unlockedCount: Int
  /// 汇总时是否拿到了用户的自造词数量；没有的话造词者徽章不显示还差多少。
  let userWordsKnown: Bool

  var body: some View {
    VStack(spacing: 10) {
      StatisticsAchievementsSummary(unlocked: unlockedCount, total: achievements.count)
      LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10, alignment: .top), count: 3), spacing: 10) {
        ForEach(Array(achievements.enumerated()), id: \.element.id) { index, badge in
          StatisticsBadgeCell(badge: badge, index: index,
                              progressKnown: userWordsKnown || badge.id != TypingSummary.userWordsAchievement)
        }
      }
      .accessibilityIdentifier("statisticsBadges")
    }
  }
}

/// `9 / 16 已解锁`，下面是一条 6pt 的强调色进度条，短暂停顿后才填充。
private struct StatisticsAchievementsSummary: View {
  let unlocked: Int
  let total: Int
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var grown = false

  var body: some View {
    let share = total == 0 ? 0 : CGFloat(unlocked) / CGFloat(total)
    VStack(alignment: .leading, spacing: 8) {
      HStack(alignment: .firstTextBaseline, spacing: 4) {
        Text("\(unlocked)").font(.system(size: 28, weight: .bold)).monospacedDigit()
        Text("/ \(total) 已解锁").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
      }
      GeometryReader { geometry in
        ZStack(alignment: .leading) {
          RoundedRectangle(cornerRadius: 3, style: .continuous).fill(StatisticsStyle.track)
          RoundedRectangle(cornerRadius: 3, style: .continuous).fill(MetasequoiaTheme.accent)
            .frame(width: grown ? geometry.size.width * share : 0)
        }
      }
      .frame(height: 6)
    }
    .statisticsCard()
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("\(unlocked) / \(total) 枚成就已解锁")
    .accessibilityIdentifier("statisticsBadgeSummary")
    .onAppear {
      guard !grown else { return }
      if reduceMotion { grown = true } else { withAnimation(StatisticsStyle.curve(1).delay(0.15)) { grown = true } }
    }
  }
}

/// 按系列区分的徽章双色，同设计稿原型：数量类用强调色，连续类用加深的强调色，技巧类用混入蓝色的强调色，趣味类用混入紫色的强调色。`deep` 是实色端，`light` 是高光。
private enum BadgeFamily {
  case volume, streak, skill, fun

  init(group: String) {
    switch group {
    case "streak": self = .streak
    case "skill": self = .skill
    case "fun": self = .fun
    default: self = .volume
    }
  }

  private static func mixed(_ percent: Double, _ other: UIColor) -> Color {
    Color(uiColor: UIColor { traits in
      AppThemePalette.mix(MetasequoiaTheme.accentUIColor.resolvedColor(with: traits), percent, other)
    })
  }

  private static let blue = UIColor(red: 0x3A / 255, green: 0x6E / 255, blue: 0xA5 / 255, alpha: 1)
  private static let purple = UIColor(red: 0x7A / 255, green: 0x5B / 255, blue: 0xA8 / 255, alpha: 1)
  private static let streakDeep = mixed(82, .black)
  private static let skillDeep = mixed(70, blue)
  private static let funDeep = mixed(68, purple)
  private static let volumeLight = mixed(70, .white)
  private static let streakLight = mixed(80, .white)
  private static let otherLight = mixed(55, .white)

  var deep: Color {
    switch self {
    case .volume: return MetasequoiaTheme.accent
    case .streak: return Self.streakDeep
    case .skill: return Self.skillDeep
    case .fun: return Self.funDeep
    }
  }

  var light: Color {
    switch self {
    case .volume: return Self.volumeLight
    case .streak: return Self.streakLight
    case .skill, .fun: return Self.otherLight
    }
  }
}

/// 点按奖章时做动画的数值：额外旋转的角度和缩放比例。
private struct WiggleFrame {
  var rotation: Double = 0
  var scale: Double = 1
}

private struct BadgePressStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .scaleEffect(configuration.isPressed ? 0.96 : 1)
      .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
  }
}

/// 一张徽章卡片：奖章或进度环、13pt 标题和下面一行 11pt 小字。卡片依次淡入并上浮；奖章在卡片之后弹出，每 3.6 秒有一道亮光扫过。开启减弱动态效果时只保留淡入。
private struct StatisticsBadgeCell: View {
  let badge: TypingSummary.Achievement
  let index: Int
  let progressKnown: Bool
  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var appeared = false
  @State private var popped = false
  @State private var wiggles = 0

  private static let medal: CGFloat = 54

  private var family: BadgeFamily { BadgeFamily(group: badge.group) }

  /// 按字形长度取字号：19、15 或 13pt。
  private var glyphSize: CGFloat {
    switch badge.glyph.count {
    case ...1: return 19
    case 2: return 15
    default: return 13
    }
  }

  var body: some View {
    Button(action: tap) {
      VStack(spacing: 8) {
        Group {
          if badge.isUnlocked { medal } else { ring }
        }
        .frame(width: Self.medal, height: Self.medal)
        Text(badge.title).font(.system(size: 13, weight: .semibold))
          .foregroundStyle(badge.isUnlocked ? Color.primary : MetasequoiaTheme.sub)
          .lineLimit(1).minimumScaleFactor(0.8)
          .padding(.top, 2)
        Text(TypingSummaryText.caption(badge, progressKnown: progressKnown)).font(.system(size: 11))
          .foregroundStyle(MetasequoiaTheme.sub)
          .multilineTextAlignment(.center).lineLimit(2).lineSpacing(1)
          .fixedSize(horizontal: false, vertical: true)
      }
      .padding(.top, 16).padding(.horizontal, 6).padding(.bottom, 12)
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
      .contentShape(RoundedRectangle(cornerRadius: MetasequoiaTheme.tabCardRadius, style: .continuous))
    }
    .buttonStyle(BadgePressStyle())
    .opacity(appeared ? 1 : 0)
    .offset(y: appeared || reduceMotion ? 0 : 10)
    .scaleEffect(appeared || reduceMotion ? 1 : 0.86)
    .accessibilityLabel("\(badge.title)，\(badge.isUnlocked ? "已解锁" : "未解锁")，\(TypingSummaryText.caption(badge, progressKnown: progressKnown))")
    .accessibilityIdentifier("statisticsBadge_\(badge.id)")
    .onAppear {
      guard !appeared else { return }
      withAnimation(reduceMotion ? .easeOut(duration: 0.3) : StatisticsStyle.curve(0.45).delay(Double(index) * 0.045)) {
        appeared = true
      }
      if !reduceMotion { popped = true }
    }
  }

  private func tap() {
    if badge.isUnlocked && !reduceMotion { wiggles += 1 }
    ToastCenter.shared.show(TypingSummaryText.toast(badge, progressKnown: progressKnown))
  }

  /// 已解锁的奖章：旋转 45° 的 54pt 圆角方块，所属系列的渐变从高光端到 70% 处的实色端，带实色柔和阴影，白色字形正立在上面。
  private var medal: some View {
    ZStack {
      RoundedRectangle(cornerRadius: 17, style: .continuous)
        .fill(LinearGradient(stops: [.init(color: family.light, location: 0), .init(color: family.deep, location: 0.7)],
                             startPoint: UnitPoint(x: 0.21, y: 0.09), endPoint: UnitPoint(x: 0.79, y: 0.91)))
        .overlay { if !reduceMotion { shine } }
        .clipShape(RoundedRectangle(cornerRadius: 17, style: .continuous))
        .frame(width: Self.medal, height: Self.medal)
        .rotationEffect(.degrees(45))
        .shadow(color: family.deep.opacity(0.33), radius: 6, x: 0, y: 4)
      Text(badge.glyph)
        .font(.system(size: glyphSize, weight: .heavy))
        .tracking(-0.3)
        .foregroundStyle(.white)
        .shadow(color: .black.opacity(0.15), radius: 0.5, x: 0, y: 1)
        .lineLimit(1).minimumScaleFactor(0.7)
    }
    .keyframeAnimator(initialValue: 1.0, trigger: popped) { content, scale in
      content.scaleEffect(scale)
    } keyframes: { _ in
      KeyframeTrack(\.self) {
        MoveKeyframe(0.3)
        LinearKeyframe(0.3, duration: 0.12 + Double(index) * 0.045)
        CubicKeyframe(1.12, duration: 0.36)
        CubicKeyframe(0.96, duration: 0.12)
        CubicKeyframe(1, duration: 0.12)
      }
    }
    .keyframeAnimator(initialValue: WiggleFrame(), trigger: wiggles) { content, frame in
      content.rotationEffect(.degrees(frame.rotation)).scaleEffect(frame.scale)
    } keyframes: { _ in
      KeyframeTrack(\.rotation) {
        CubicKeyframe(-15, duration: 0.12)
        CubicKeyframe(13, duration: 0.12)
        CubicKeyframe(-7, duration: 0.12)
        CubicKeyframe(5, duration: 0.12)
        CubicKeyframe(0, duration: 0.12)
      }
      KeyframeTrack(\.scale) {
        CubicKeyframe(1.15, duration: 0.12)
        CubicKeyframe(1.1, duration: 0.12)
        CubicKeyframe(1.05, duration: 0.12)
        CubicKeyframe(1, duration: 0.12)
        CubicKeyframe(1, duration: 0.12)
      }
    }
  }

  /// 一道白色亮条，每个 3.6 秒周期的大部分时间停在左边缘之外，然后扫过奖章。
  private var shine: some View {
    GeometryReader { geometry in
      let width = geometry.size.width * 0.34
      LinearGradient(colors: [.white.opacity(0), .white.opacity(0.55), .white.opacity(0)], startPoint: .leading, endPoint: .trailing)
        .frame(width: width, height: geometry.size.height * 1.4)
        .keyframeAnimator(initialValue: -1.2, repeating: true) { content, travel in
          content.rotationEffect(.degrees(25)).offset(x: width * travel, y: -geometry.size.height * 0.2)
        } keyframes: { _ in
          KeyframeTrack(\.self) {
            LinearKeyframe(-1.2, duration: 2.232)
            CubicKeyframe(2.2, duration: 0.72)
            LinearKeyframe(2.2, duration: 0.648)
          }
        }
    }
    .allowsHitTesting(false)
  }

  /// 未解锁的徽章：4pt 圆环，在浅色轨道上用所属系列的实色填到当前进度，中间是卡片色的 46pt 圆心，放字形和百分比。
  private var ring: some View {
    ZStack {
      Circle().inset(by: 2).stroke(StatisticsStyle.track, lineWidth: 4)
      Circle().inset(by: 2)
        .trim(from: 0, to: appeared ? badge.progress : 0)
        .stroke(family.deep, lineWidth: 4)
        .rotationEffect(.degrees(-90))
        .animation(reduceMotion ? nil : StatisticsStyle.curve(1.1).delay(Double(index) * 0.045), value: appeared)
      Circle().fill(MetasequoiaTheme.surface).frame(width: 46, height: 46)
      VStack(spacing: 1) {
        Text(badge.glyph).font(.system(size: glyphSize, weight: .bold))
          .foregroundStyle(MetasequoiaTheme.sub).opacity(0.7)
          .lineLimit(1).minimumScaleFactor(0.6)
        Text(progressKnown ? TypingSummaryText.progressLabel(badge) : "—").font(.system(size: 9, weight: .semibold)).monospacedDigit()
          .foregroundStyle(family.deep)
      }
      .frame(width: 42)
    }
  }
}
