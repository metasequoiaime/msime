import SwiftUI
import UIKit

/// 设计稿里的开屏动画，按应用主题的季节着色：光晕在由主色混出的深色底上弹出并呼吸，浅色圆盘弹入并泛起两圈涟漪，标志旋入并写出白色笔画，名称和拉丁文一行依次升起，底部进度条在 2.4 秒内填满。2.8 秒后或首次点击时流程继续。它在首次启动的引导之前播放，也可以从 我的 → 开屏动画 再次播放；普通启动在静态启动屏之后直接进入应用。
///
/// 每个元素都是开屏出现以来经过时间的纯函数，由 TimelineView 求值，因此设计稿的 CSS 关键帧（偏移、分段 cubic-bezier 曲线、延迟和迭代次数）被精确复现，而不是用串联的 SwiftUI 动画近似。Android 的 HomeActivity 播放的是同一条时间线。
struct SplashView: View {
  var onFinish: () -> Void

  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @Environment(\.colorScheme) private var colorScheme
  @State private var start: Date?
  @State private var finished = false

  var body: some View {
    let palette = Palette(accent: accent)
    TimelineView(.animation(paused: reduceMotion || start == nil)) { context in
      let elapsed = start.map { context.date.timeIntervalSince($0) } ?? 0
      scene(Frame(elapsed: elapsed, reduceMotion: reduceMotion), palette: palette)
    }
    .contentShape(Rectangle())
    .onTapGesture(perform: finish)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("水杉输入法")
    .accessibilityHint("轻点跳过")
    .accessibilityAddTraits(.isButton)
    .accessibilityAction { finish() }
    .accessibilityIdentifier("splashView")
    .statusBarHidden()
    .onAppear { if start == nil { start = Date() } }
    .task {
      try? await Task.sleep(nanoseconds: 2_800_000_000)
      finish()
    }
  }

  /// 当前外观下的季节主色；应用主题无法解析时为经典绿色。
  private var accent: UIColor {
    let dark = colorScheme == .dark
    if let resolved = AppThemePalette.resolved(dark: dark) { return resolved.accent }
    return MetasequoiaTheme.forestUIColor.resolvedColor(with: UITraitCollection(userInterfaceStyle: dark ? .dark : .light))
  }

  private func scene(_ frame: Frame, palette: Palette) -> some View {
    ZStack {
      palette.background
      // 320pt 的圆，光晕在半径 65% 处淡尽：设计稿的 `radial-gradient(circle, …)` 量到最远角，即 160·√2 ≈ 226pt，所以颜色在 147pt 处消失。
      RadialGradient(colors: [palette.glow, palette.glow.opacity(0)], center: .center, startRadius: 0, endRadius: 147)
        .frame(width: 320, height: 320)
        .scaleEffect(frame.glowScale)
        .opacity(frame.glowOpacity)
      VStack(spacing: 18) {
        disc(frame, palette: palette)
        Text("水杉输入法").font(.system(size: 24, weight: .bold)).tracking(1.44).foregroundStyle(.white)
          .opacity(frame.nameOpacity).offset(y: frame.nameOffset)
        Text("METASEQUOIA IME").font(.system(size: 13)).tracking(2.34).foregroundStyle(.white.opacity(0.62))
          .opacity(frame.latinOpacity).offset(y: frame.latinOffset)
      }
      VStack {
        Spacer()
        Capsule().fill(.white.opacity(0.08))
          .overlay(alignment: .leading) {
            Capsule().fill(palette.bar).scaleEffect(x: frame.progress, y: 1, anchor: .leading)
          }
          .frame(height: 3)
          .padding(.horizontal, 48).padding(.bottom, 48)
      }
    }
    .ignoresSafeArea()
  }

  private func disc(_ frame: Frame, palette: Palette) -> some View {
    let logoSize: CGFloat = 100
    let logoScale = MSIMELogo.scale(for: CGRect(x: 0, y: 0, width: logoSize, height: logoSize))
    return ZStack {
      // 设计稿的 msRipple 让白色 box-shadow 的扩展从 0 长到 36pt，同时淡出；在圆盘后面放一个从 180pt 缩放到 252pt 的白色圆，画出的是同样的圆环。
      Circle().fill(.white)
        .scaleEffect(frame.rippleScale)
        .opacity(frame.rippleOpacity)
      Circle().fill(palette.disc)
        .shadow(color: .black.opacity(0.35), radius: 20, y: 12)
      ZStack {
        MSIMELogoFrame().fill(palette.logo)
        MSIMELogoStroke()
          .trim(from: 0, to: frame.strokeDrawn)
          .stroke(.white, style: StrokeStyle(lineWidth: MSIMELogo.strokeWidth * logoScale, lineCap: .round, lineJoin: .round))
      }
      .frame(width: logoSize, height: logoSize)
      .scaleEffect(frame.logoScale)
      .rotationEffect(.degrees(frame.logoRotation))
      .opacity(frame.logoOpacity)
    }
    .frame(width: 180, height: 180)
    .scaleEffect(frame.discScale)
    .opacity(frame.discOpacity)
  }

  private func finish() {
    guard !finished else { return }
    finished = true
    onFinish()
  }
}

/// 由同一个主色混出的开屏颜色，定义与设计稿 token 和 Android 的 `AppThemePalette.splash*` 一致。
private struct Palette {
  let background: Color
  let glow: Color
  let disc: Color
  let logo: Color
  let bar: Color

  init(accent: UIColor) {
    background = Color(uiColor: AppThemePalette.mix(accent, 20, UIColor(red: 10 / 255, green: 11 / 255, blue: 10 / 255, alpha: 1)))
    // color-mix(accent 34%, transparent) 按预乘 alpha 插值，结果就是 34% 不透明度的主色（Android 的 `withAlpha(accent, 0x57)`）；直接和透明色逐通道混合反而会让它变暗。
    glow = Color(uiColor: accent.withAlphaComponent(CGFloat(0x57) / 255))
    disc = Color(uiColor: AppThemePalette.mix(accent, 12, .white))
    logo = Color(uiColor: AppThemePalette.mix(accent, 82, .black))
    bar = Color(uiColor: AppThemePalette.mix(accent, 70, .white))
  }
}

/// 开屏在某一时刻的全部动画值。开启减弱动态效果（Reduce Motion）时，一切停在最终状态，不做任何运动。
private struct Frame {
  var glowScale: CGFloat = 1
  var glowOpacity: Double = 1
  var discScale: CGFloat = 1
  var discOpacity: Double = 1
  var rippleScale: CGFloat = 1
  var rippleOpacity: Double = 0
  var logoScale: CGFloat = 1
  var logoRotation: Double = 0
  var logoOpacity: Double = 1
  var strokeDrawn: CGFloat = 1
  var nameOpacity: Double = 1
  var nameOffset: CGFloat = 0
  var latinOpacity: Double = 1
  var latinOffset: CGFloat = 0
  var progress: CGFloat = 1

  // CSS 时间函数：关键帧动画没有指定时默认为 `ease`。
  private static let ease = UnitCurve.bezier(startControlPoint: UnitPoint(x: 0.25, y: 0.1), endControlPoint: UnitPoint(x: 0.25, y: 1))
  private static let easeInOut = UnitCurve.bezier(startControlPoint: UnitPoint(x: 0.42, y: 0), endControlPoint: UnitPoint(x: 0.58, y: 1))
  private static let easeOut = UnitCurve.bezier(startControlPoint: UnitPoint(x: 0, y: 0), endControlPoint: UnitPoint(x: 0.58, y: 1))
  private static let spring = UnitCurve.bezier(startControlPoint: UnitPoint(x: 0.2, y: 0.8), endControlPoint: UnitPoint(x: 0.3, y: 1))
  private static let draw = UnitCurve.bezier(startControlPoint: UnitPoint(x: 0.65, y: 0), endControlPoint: UnitPoint(x: 0.35, y: 1))

  init(elapsed t: Double, reduceMotion: Bool) {
    guard !reduceMotion else { return }
    // 光晕先播 msPop .8s，从 1.4s 起接 msBreath。与 Android 一样，从弹出结束时的状态（全亮、满尺寸）起，用一个 1.2s 的半周期来回往复，膨胀时变暗，这样光晕不会在 1.4s 时跳到关键帧的 55%。
    let pop = Self.track(t, begin: 0, duration: 0.8, Self.ease, [(0, 0), (1, 1)])
    glowScale = 0.86 + 0.14 * pop
    glowOpacity = pop
    if t > 1.4 {
      let cycle = (t - 1.4) / 1.2
      let phase = cycle.truncatingRemainder(dividingBy: 1)
      let swell = Int(cycle) % 2 == 0 ? Self.easeInOut.value(at: phase) : 1 - Self.easeInOut.value(at: phase)
      glowOpacity = 1 - 0.45 * swell
      glowScale = 1 + 0.04 * swell
    }
    // msCircIn .65s：缩放 .4 → 60% 处 1.06 → 1，不透明度 0 → 60% 处 1。
    discScale = Self.track(t, begin: 0, duration: 0.65, Self.spring, [(0, 0.4), (0.6, 1.06), (1, 1)])
    discOpacity = Self.track(t, begin: 0, duration: 0.65, Self.spring, [(0, 0), (0.6, 1), (1, 1)])
    // msRipple 1.2s ease-out，从 1.45s 开始，播放两次：180pt 圆盘上一圈 36pt 宽的白环（缩放 1 → 1.4），从 35% 白色淡到透明。
    if t >= 1.45, t < 1.45 + 2 * 1.2 {
      let run = Self.easeOut.value(at: ((t - 1.45) / 1.2).truncatingRemainder(dividingBy: 1))
      rippleScale = 1 + 0.4 * run
      rippleOpacity = 0.35 * (1 - run)
    }
    // msLogoIn .6s，从 .25s 开始：缩放 .5 → 1.08 → 1，旋转 -12° → 2° → 0，不透明度 0 → 70% 处 1。
    logoScale = Self.track(t, begin: 0.25, duration: 0.6, Self.spring, [(0, 0.5), (0.7, 1.08), (1, 1)])
    logoRotation = Self.track(t, begin: 0.25, duration: 0.6, Self.spring, [(0, -12), (0.7, 2), (1, 0)])
    logoOpacity = Self.track(t, begin: 0.25, duration: 0.6, Self.spring, [(0, 0), (0.7, 1), (1, 1)])
    // msDraw .8s，从 .65s 开始。
    strokeDrawn = Self.track(t, begin: 0.65, duration: 0.8, Self.draw, [(0, 0), (1, 1)])
    // msFadeUp .5s：名称从 1.1s 开始，拉丁文一行从 1.35s 开始，各自上升 10pt。
    let name = Self.track(t, begin: 1.1, duration: 0.5, Self.ease, [(0, 0), (1, 1)])
    nameOpacity = name
    nameOffset = 10 * (1 - name)
    let latin = Self.track(t, begin: 1.35, duration: 0.5, Self.ease, [(0, 0), (1, 1)])
    latinOpacity = latin
    latinOffset = 10 * (1 - latin)
    // msLoad 2.4s ease-out，从头开始。
    progress = Self.track(t, begin: 0, duration: 2.4, Self.easeOut, [(0, 0), (1, 1)])
  }

  /// 一个 `fill-mode: both` 的 CSS 关键帧动画：`stops` 是从 0 到 1 的 (offset, value) 对，`curve` 为相邻两个 stop 之间的每一段做缓动，与 `animation-timing-function` 的作用相同。
  private static func track(_ t: Double, begin: Double, duration: Double, _ curve: UnitCurve, _ stops: [(Double, Double)]) -> Double {
    let x = min(max((t - begin) / duration, 0), 1)
    for (from, to) in zip(stops, stops.dropFirst()) where x <= to.0 {
      let local = to.0 > from.0 ? (x - from.0) / (to.0 - from.0) : 1
      return from.1 + (to.1 - from.1) * curve.value(at: local)
    }
    return stops.last?.1 ?? 0
  }
}
