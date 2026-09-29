import SwiftUI

/// The opening animation from the design: the logo's white stroke draws itself over a breathing green glow, then the flow moves on after 2.8 seconds or on the first tap. It plays before the first-run onboarding and again from 我的 → 开屏动画; ordinary launches go straight to the app behind the static launch screen.
struct SplashView: View {
  var onFinish: () -> Void

  @Environment(\.accessibilityReduceMotion) private var reduceMotion
  @State private var drawn: CGFloat = 0
  @State private var breathes = false
  @State private var showsText = false
  @State private var finished = false

  private static let background = Color(red: 14 / 255, green: 16 / 255, blue: 14 / 255)
  private static let glow = Color(red: 127 / 255, green: 224 / 255, blue: 142 / 255)

  var body: some View {
    ZStack {
      Self.background.ignoresSafeArea()
      VStack(spacing: 0) {
        ZStack {
          // A 320pt circle whose glow fades out at 65% of the ray, like the design's radial-gradient.
          RadialGradient(colors: [Self.glow.opacity(0.28), Self.glow.opacity(0)], center: .center,
                         startRadius: 0, endRadius: 147)
            .frame(width: 320, height: 320)
            .scaleEffect(breathes ? 1.08 : 0.92)
            .opacity(breathes ? 1 : 0.7)
          SplashLogo(progress: drawn).frame(width: 109, height: 124)
        }
        .frame(height: 200)
        VStack(spacing: 8) {
          Text("水杉输入法").font(.system(size: 24, weight: .bold)).tracking(1.44).foregroundStyle(.white)
          Text("METASEQUOIA IME").font(.system(size: 13)).tracking(2.34).foregroundStyle(.white.opacity(0.62))
        }
        .opacity(showsText ? 1 : 0)
      }
      VStack {
        Spacer()
        Text("轻点跳过").font(.system(size: 12)).foregroundStyle(.white.opacity(0.4)).padding(.bottom, 36)
      }
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
    .onAppear(perform: animate)
    .task {
      try? await Task.sleep(nanoseconds: 2_800_000_000)
      finish()
    }
  }

  private func animate() {
    guard !reduceMotion else {
      drawn = 1
      showsText = true
      return
    }
    withAnimation(.easeInOut(duration: 1.05).delay(0.4)) { drawn = 1 }
    withAnimation(.easeOut(duration: 0.6).delay(1.1)) { showsText = true }
    withAnimation(.easeInOut(duration: 2.4).repeatForever(autoreverses: true)) { breathes = true }
  }

  private func finish() {
    guard !finished else { return }
    finished = true
    onFinish()
  }
}

/// The app mark in the design's 116 x 132 viewBox: a green frame around a dark panel, and the white zigzag stroke that `progress` draws from its start. The brush texture of the bitmap logo is left out; the splash draws clean vector shapes so the stroke can animate.
private struct SplashLogo: View {
  var progress: CGFloat

  var body: some View {
    GeometryReader { proxy in
      let scale = min(proxy.size.width / 116, proxy.size.height / 132)
      ZStack(alignment: .topLeading) {
        RoundedRectangle(cornerRadius: 4 * scale, style: .continuous)
          .fill(Color(red: 168 / 255, green: 223 / 255, blue: 142 / 255))
          .frame(width: 116 * scale, height: 132 * scale)
        Rectangle()
          .fill(Color(red: 37 / 255, green: 37 / 255, blue: 37 / 255))
          .frame(width: 104 * scale, height: 120 * scale)
          .offset(x: 5.84 * scale, y: 5.83 * scale)
        SplashStroke()
          .trim(from: 0, to: progress)
          .stroke(.white, style: StrokeStyle(lineWidth: 9 * scale, lineCap: .round, lineJoin: .round))
          .frame(width: 116 * scale, height: 132 * scale)
      }
      .frame(width: proxy.size.width, height: proxy.size.height)
    }
    .accessibilityHidden(true)
  }
}

private struct SplashStroke: Shape {
  func path(in rect: CGRect) -> Path {
    let sx = rect.width / 116, sy = rect.height / 132
    func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x * sx, y: rect.minY + y * sy) }
    var path = Path()
    path.move(to: point(80.394, 18.8335))
    path.addLine(to: point(34.3451, 36.489))
    path.addLine(to: point(80.394, 49.7306))
    path.addLine(to: point(34.3451, 71.7999))
    path.addCurve(to: point(31.8431, 113.088), control1: point(77.8789, 79.1564), control2: point(118.8, 85.1887))
    return path
  }
}
