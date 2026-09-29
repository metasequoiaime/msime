import SwiftUI

/// The four-step onboarding from the design: add the keyboard, pick a scheme, meet the translation line under candidates, and sign in for sync. Full screen on a phone; on an iPad `isCard` lays it out for the 480pt modal card (see OnboardingModalCard).
struct WelcomeFlowView: View {
  var onFinish: (() -> Void)? = nil
  var isCard = false

  @Environment(\.dismiss) private var dismiss
  @State private var page = 0
  @State private var scheme = InputSchemePreference.scheme
  @State private var showsGloss = InputHabitPreference.mirrored.glossEnabled
  @State private var glossSaveFailed = false
  @State private var signedIn = false
  @State private var showsLogin = false

  private static let pageCount = 4

  private struct Step {
    let symbol: String
    let kicker: String
    let title: String
    let body: String
  }

  private let steps = [
    Step(symbol: "keyboard", kicker: "第一步 · 约 30 秒", title: "把水杉加进键盘",
         body: "在系统设置里添加水杉键盘，再打开「允许完全访问」，云候选、云剪贴板和同步才能工作。"),
    Step(symbol: "textformat", kicker: "第二步 · 随时可以改", title: "选一套输入方案",
         body: "全拼、9 键、双拼和五笔用的是同一套引擎，词库和自造词通用。"),
    Step(symbol: "translate", kicker: "第三步 · 水杉的特点", title: "候选下方就是译文",
         body: "每个候选词下面那行小字是英文释义，本地词库优先，没命中时才联网查询。"),
    Step(symbol: "arrow.triangle.2.circlepath", kicker: "最后一步", title: "登录后多端同步",
         body: "登录后词库、自造词、皮肤和云剪贴板会在手机、平板和电脑之间同步。"),
  ]

  var body: some View {
    NavigationStack {
      VStack(spacing: 0) {
        topBar
        ScrollView {
          content.frame(maxWidth: 560, alignment: .leading).frame(maxWidth: .infinity)
            .padding(.horizontal, isCard ? 28 : 24).padding(.top, 8).padding(.bottom, 16)
        }
        .scrollBounceBehavior(.basedOnSize)
        footer
      }
      .background(background.ignoresSafeArea())
      .toolbar(.hidden, for: .navigationBar)
      .simultaneousGesture(swipe)
      .accessibilityAction(named: "上一步") { if page > 0 { withAnimation { page -= 1 } } }
    }
    .tint(MetasequoiaTheme.accent)
    .task { await refreshSignIn() }
    .sheet(isPresented: $showsLogin, onDismiss: { Task { await refreshSignIn() } }) { AccountLoginSheet() }
  }

  private var background: Color { isCard ? MetasequoiaTheme.surface : MetasequoiaTheme.canvas }

  private var topBar: some View {
    HStack {
      Text("\(page + 1) / \(Self.pageCount)").font(.system(size: 13).monospacedDigit()).foregroundStyle(.secondary)
        .accessibilityIdentifier("onboardingProgress")
      Spacer()
      Button("跳过") { finish() }.font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.accent)
        .accessibilityIdentifier("skipOnboardingButton")
    }
    .padding(.horizontal, isCard ? 28 : 24).padding(.top, isCard ? 24 : 16).padding(.bottom, 8)
  }

  private var content: some View {
    let step = steps[page]
    return VStack(alignment: .leading, spacing: 0) {
      Image(systemName: step.symbol).font(.system(size: 32, weight: .medium)).foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 72, height: 72)
        .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .accessibilityHidden(true)
      Text(step.kicker).font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
        .padding(.top, 24)
      Text(step.title).font(.system(size: 28, weight: .bold)).padding(.top, 6)
        .accessibilityAddTraits(.isHeader).accessibilityIdentifier("onboardingTitle")
      Text(step.body).font(.system(size: 15)).foregroundStyle(.secondary).padding(.top, 10)
        .fixedSize(horizontal: false, vertical: true)
      Group {
        switch page {
        case 0: keyboardStep
        case 1: schemeStep
        case 2: translationStep
        default: syncStep
        }
      }.padding(.top, 24)
    }
    .id(page)
    .transition(.opacity)
  }

  private var keyboardStep: some View {
    VStack(alignment: .leading, spacing: 12) {
      card {
        VStack(alignment: .leading, spacing: 12) {
          instruction(1, "打开「设置 → 通用 → 键盘 → 键盘」，选择「添加新键盘」里的水杉输入法")
          instruction(2, "点进水杉输入法，打开「允许完全访问」")
          instruction(3, "在输入框长按地球键，切换到水杉")
        }
      }
      Button {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
      } label: {
        Text("打开系统设置").font(.system(size: 15, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity).frame(height: 44)
          .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .accessibilityIdentifier("openKeyboardSettingsButton")
      .accessibilityHint("打开水杉输入法的系统设置页面")
    }
  }

  private var schemeStep: some View {
    VStack(alignment: .leading, spacing: 10) {
      ForEach(Self.schemeChoices, id: \.scheme) { choice in
        Button { select(choice.scheme) } label: {
          HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
              Text(choice.title).font(.system(size: 17, weight: .semibold)).foregroundStyle(.primary)
              Text(choice.detail).font(.system(size: 13)).foregroundStyle(.secondary)
            }
            Spacer()
            Circle()
              .strokeBorder(scheme == choice.scheme ? MetasequoiaTheme.accent : Color.secondary.opacity(0.5),
                            lineWidth: scheme == choice.scheme ? 6 : 1.5)
              .frame(width: 22, height: 22)
          }
          .padding(16)
          .background(MetasequoiaTheme.cardBackground(isCard), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
          .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous)
            .strokeBorder(scheme == choice.scheme ? MetasequoiaTheme.accent : .clear, lineWidth: 2))
          .contentShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("welcomeScheme_\(choice.scheme.rawValue)")
        .accessibilityValue(scheme == choice.scheme ? "已选择" : "未选择")
        .accessibilityAddTraits(scheme == choice.scheme ? [.isSelected] : [])
      }
      NavigationLink(destination: InputSettingsView()) {
        Text("查看全部输入方案").font(.system(size: 15))
      }.padding(.top, 4).accessibilityIdentifier("welcomeAllSchemesLink")
    }
    .onAppear { scheme = InputSchemePreference.scheme }
  }

  private var translationStep: some View {
    VStack(alignment: .leading, spacing: 12) {
      card {
        HStack(alignment: .top, spacing: 0) {
          ForEach(Array(Self.glossSample.enumerated()), id: \.offset) { index, item in
            VStack(spacing: 3) {
              Text(item.0).font(.system(size: 18, weight: index == 0 ? .semibold : .regular))
                .foregroundStyle(index == 0 ? MetasequoiaTheme.accent : Color.primary)
              if showsGloss {
                Text(item.1).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
              }
            }
            .frame(maxWidth: .infinity)
          }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(showsGloss ? "候选示例：候选 candidate，后选 choice，侯选 option，候 wait" : "候选示例：候选，后选，侯选，候")
      }
      card {
        Toggle("显示译文", isOn: Binding(get: { showsGloss }, set: saveGloss))
          .font(.system(size: 17))
          .accessibilityIdentifier("welcomeGlossToggle")
      }
      if glossSaveFailed {
        Text("设置没有保存，键盘可能正在写入同一份设置，请再试一次。").font(.footnote).foregroundStyle(.red)
      }
      NavigationLink(destination: KeyboardTryoutView()) {
        Text("先试试键盘").font(.system(size: 15))
      }.padding(.top, 4).accessibilityIdentifier("welcomeTryoutLink")
    }
    .onAppear { showsGloss = InputHabitPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences()).glossEnabled }
  }

  private var syncStep: some View {
    card {
      VStack(alignment: .leading, spacing: 14) {
        perk("books.vertical", "词库和自造词")
        perk("paintpalette", "皮肤与主题")
        perk("doc.on.clipboard", "云剪贴板")
      }
    }
  }

  private var footer: some View {
    VStack(spacing: 16) {
      HStack(spacing: 6) {
        ForEach(0..<Self.pageCount, id: \.self) { index in
          RoundedRectangle(cornerRadius: 3)
            .fill(index == page ? MetasequoiaTheme.accent : Color.secondary.opacity(0.3))
            .frame(width: index == page ? 20 : 6, height: 6)
        }
      }
      .accessibilityElement(children: .ignore)
      .accessibilityLabel("第 \(page + 1) 步，共 \(Self.pageCount) 步")
      if page < Self.pageCount - 1 {
        primaryButton("下一步", identifier: "nextOnboardingButton") { withAnimation { page += 1 } }
      } else if signedIn {
        primaryButton("开始使用", identifier: "finishOnboardingButton") { finish() }
      } else {
        primaryButton("登录", identifier: "onboardingSignInButton") { showsLogin = true }
        // The whole row answers the tap, not only the two words drawn in it.
        Button { finish() } label: {
          Text("稍后再说").font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.accent)
            .frame(maxWidth: .infinity).frame(height: 36).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("finishOnboardingButton")
      }
    }
    .frame(maxWidth: 560)
    .padding(.horizontal, isCard ? 28 : 24).padding(.top, 8).padding(.bottom, isCard ? 24 : 12)
  }

  private func primaryButton(_ title: String, identifier: String, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      Text(title).font(.system(size: 17, weight: .semibold)).foregroundStyle(MetasequoiaTheme.onAccent)
        .frame(maxWidth: .infinity).frame(height: 50)
        .background(MetasequoiaTheme.accent, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
    .buttonStyle(.plain)
    .accessibilityIdentifier(identifier)
  }

  private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
    content().padding(16).frame(maxWidth: .infinity, alignment: .leading)
      .background(MetasequoiaTheme.cardBackground(isCard), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
  }

  private func instruction(_ number: Int, _ text: String) -> some View {
    HStack(alignment: .top, spacing: 10) {
      Text("\(number)").font(.system(size: 13, weight: .semibold)).foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 22, height: 22).background(MetasequoiaTheme.accentSoft, in: Circle())
      Text(text).font(.system(size: 15)).fixedSize(horizontal: false, vertical: true)
    }
    .accessibilityElement(children: .combine)
  }

  private func perk(_ symbol: String, _ title: String) -> some View {
    HStack(spacing: 12) {
      Image(systemName: symbol).font(.system(size: 15, weight: .medium)).foregroundStyle(MetasequoiaTheme.accent)
        .frame(width: 32, height: 32)
        .background(MetasequoiaTheme.accentSoft, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .accessibilityHidden(true)
      Text(title).font(.system(size: 17))
    }
  }

  private var swipe: some Gesture {
    DragGesture(minimumDistance: 20).onEnded { value in
      let dx = value.translation.width
      guard abs(dx) >= 50, abs(dx) > abs(value.translation.height) else { return }
      withAnimation {
        if dx < 0, page < Self.pageCount - 1 { page += 1 }
        else if dx > 0, page > 0 { page -= 1 }
      }
    }
  }

  private static let schemeChoices: [(scheme: ChineseInputScheme, title: String, detail: String)] = [
    (.quanpin, "全拼 26 键", "最常用，完整拼音"),
    (.nineKey, "全拼 9 键", "单手更顺手"),
    (.shuangpin, "双拼", "每字两键 · 默认小鹤"),
    (.wubi, "五笔", "形码 · 默认 86 版"),
  ]

  private static let glossSample = [("候选", "candidate"), ("后选", "choice"), ("侯选", "option"), ("候", "wait")]

  private func select(_ choice: ChineseInputScheme) {
    var enabled = InputSchemePreference.enabledSchemes
    if !enabled.contains(choice) { enabled.append(choice) }
    // The first run has no page to report a failed document write on; the App Group still holds the choice until the keyboard first records a scheme of its own.
    if !InputSchemePreference.save(scheme: choice, enabled: enabled) {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = choice
    }
    scheme = choice
  }

  /// The same setting as 词库 → 显示英文释义, written through the shared document the keyboard reloads.
  private func saveGloss(_ enabled: Bool) {
    if let saved = InputHabitPreference.update({ $0.glossEnabled = enabled }) {
      showsGloss = saved.glossEnabled
      glossSaveFailed = false
    } else {
      glossSaveFailed = true
    }
  }

  private func refreshSignIn() async {
    signedIn = (try? await SkinCommunityAPI.shared.signedIn()) ?? false
  }

  private func finish() {
    if let onFinish { onFinish() } else { dismiss() }
  }
}

/// The iPad presentation of the onboarding: a 480pt card, at most 600pt or 94% of the height, over a 38% scrim that skips the flow when tapped.
struct OnboardingModalCard: View {
  let onFinish: () -> Void

  var body: some View {
    GeometryReader { proxy in
      ZStack {
        Color.black.opacity(0.38).ignoresSafeArea()
          .onTapGesture(perform: onFinish)
          .accessibilityHidden(true)
        WelcomeFlowView(onFinish: onFinish, isCard: true)
          .frame(width: min(480, proxy.size.width - 32), height: min(600, proxy.size.height * 0.94))
          .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
          .shadow(color: .black.opacity(0.4), radius: 40, y: 30)
          // The tabs behind are hidden from accessibility while the card is up (FirstRunContainer), so VoiceOver stays in the card; the escape gesture closes it the way a tap on the scrim does.
          .accessibilityElement(children: .contain)
          .accessibilityAddTraits(.isModal)
          .accessibilityAction(.escape, onFinish)
      }
      .frame(width: proxy.size.width, height: proxy.size.height)
    }
  }
}

private extension MetasequoiaTheme {
  /// Option cards sit on the page canvas when full screen, and on the grey grouped canvas inside the white iPad card.
  static func cardBackground(_ inCard: Bool) -> Color { inCard ? canvas : surface }
}
