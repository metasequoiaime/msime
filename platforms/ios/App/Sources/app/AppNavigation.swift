import AuthenticationServices
import SwiftUI

// Each tab owns its stack. Cross-tab discovery always opens the community root.
final class AppNavigation: ObservableObject {
  enum Tab: Hashable { case settings, community, statistics, account }
  @Published var tab: Tab = .settings
  @Published var communityCategory = 0
  @Published var communityRoot = UUID()
  /// 替换它即可把设置导航栈重建回根页面。
  @Published var settingsRoot = UUID()
  /// The keyboard's voice panel sends msime://voice: the keyboard cannot record, so the app opens straight onto the recording screen.
  @Published var recordsVoice = false
  /// iPad 设置侧栏显示的页面。放在这里而不放在侧栏视图里，是为了在 app 因换主题或换季（`paletteVersion`）重建内容时保住它。
  @Published var tabletSettingsSelection: TabletSettingsView.Destination? = .page(.input)

  func discoverSkins() {
    communityRoot = UUID()
    communityCategory = 0
    tab = .community
  }

  /// 点了标签栏的某一项。设置正显示时再点它就回到根页面，与 Android 标签栏重选时弹出该标签的页面一样；切换标签则各自保留导航栈，与 iOS 惯例一致。
  func select(_ next: Tab, router: SettingsRouter) {
    if next == tab && next == .settings {
      router.settingsPage = nil
      settingsRoot = UUID()
    }
    tab = next
  }
}

/// 登录水杉：设计稿里的底部面板（页面底色、圆角 20、系统拖动条、30pt 的圆形关闭按钮），里面是通过 Apple 登录、内嵌的邮箱验证码流程，以及后端提供时的手机号登录。登录是一个任务面板，绝不是「我的」标签页的又一份拷贝。登录成功后面板关闭，并用 toast 说明是用哪种方式登录的；调用方在面板的 `onDismiss` 里刷新各自的状态。
struct AccountLoginSheet: View {
  @Environment(\.dismiss) private var dismiss
  @StateObject private var model = CodeLoginModel()
  /// 面板内容的高度，布局完成后测得；在那之前面板以 `.medium` 打开。
  @State private var contentHeight: CGFloat?
  /// 面板处于 `.large`：用户把它拖了上去，或者邮箱表单展开了。
  @State private var expanded = false

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 12) {
        header
        AccountLoginOptions(model: model, onExpand: { expanded = true }) { via in
          dismiss()
          ToastCenter.shared.show("已通过 \(via) 登录")
        }
        .padding(.top, 6)
        Text("登录即表示同意[《隐私政策》](https://msime.app/privacy/)")
          .font(.system(size: 12)).foregroundStyle(MetasequoiaTheme.sub)
          .multilineTextAlignment(.center)
          .frame(maxWidth: .infinity)
          .padding(.top, 4)
          .accessibilityIdentifier("accountLoginPrivacy")
      }
      // 系统拖动指示条代替了设计稿里 5pt 的拖动条和它下方 12pt 的间距，所以内容起点比设计稿的 8pt 顶部内边距低出这么多。
      .padding(.top, 25)
      .padding(.horizontal, 20)
      .padding(.bottom, 34)
      .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = ceil($0) }
    }
    .scrollBounceBehavior(.basedOnSize)
    .tint(MetasequoiaTheme.accent)
    // 收起的面板和设计稿一样贴合内容高度，而不是停在 `.medium` 空着半截。
    .presentationDetents(detents, selection: detent)
    .presentationDragIndicator(.visible)
    .presentationCornerRadius(20)
    .presentationBackground(MetasequoiaTheme.canvas)
    .accessibilityIdentifier("accountLoginSheet")
  }

  private var compactDetent: PresentationDetent { contentHeight.map { .height($0) } ?? .medium }
  private var detents: Set<PresentationDetent> { [compactDetent, .large] }
  private var detent: Binding<PresentationDetent> {
    Binding(get: { expanded ? .large : compactDetent }, set: { expanded = $0 == .large })
  }

  private var header: some View {
    HStack(alignment: .top, spacing: 12) {
      VStack(alignment: .leading, spacing: 4) {
        Text("登录水杉").font(.system(size: 22, weight: .bold)).foregroundStyle(.primary)
          .accessibilityAddTraits(.isHeader)
        Text("在手机、平板和电脑之间同步词库、皮肤和云剪贴板")
          .font(.system(size: 14)).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
      }
      Spacer(minLength: 0)
      Button { dismiss() } label: {
        Image(systemName: "xmark").font(.system(size: 12, weight: .bold))
          .foregroundStyle(MetasequoiaTheme.sub)
          .frame(width: 30, height: 30)
          .background(MetasequoiaTheme.segBg, in: Circle())
          // 30pt 圆形按钮外围 44pt 的点击区域。
          .frame(width: 44, height: 44)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .padding(.top, -7)
      .padding(.trailing, -7)
      .accessibilityLabel("关闭")
      .accessibilityIdentifier("accountLoginClose")
    }
    .padding(.top, 6)
  }
}

/// 登录面板上的登录按钮，`AppleAccountSection` 也会把它放进 Form 里显示：通过 Apple 登录、通过 Google 登录、展开成内嵌邮箱和验证码输入框的「使用邮箱登录」，以及打开手机验证码流程的「手机号登录」文字按钮。显示哪些按钮取决于后端的登录方式列表；Google 还要这个构建能发起 Google 登录（`GoogleSignInSupport`）。登录成功、且本视图弹出的面板都已关闭后，才运行 `onFinish`。
struct AccountLoginOptions: View {
  private enum Field: Hashable { case email, code }

  @ObservedObject var model: CodeLoginModel
  /// 邮箱表单展开时调用，好让面板长高以容纳键盘。
  var onExpand: () -> Void = {}
  let onFinish: (String) -> Void
  @Environment(\.colorScheme) private var colorScheme
  @FocusState private var focus: Field?
  @State private var emailExpanded = false
  @State private var email = ""
  @State private var code = ""
  @State private var challenge: BackendAccountClient.Challenge?
  @State private var sentTo = ""
  @State private var expiresAt = Date.distantPast
  @State private var resendAt = Date.distantPast
  @State private var phoneChannel: CodeLoginChannel?
  @State private var pending: Task<Void, Never>?

  init(model: CodeLoginModel, onExpand: @escaping () -> Void = {}, onFinish: @escaping (String) -> Void) {
    self.model = model
    self.onExpand = onExpand
    self.onFinish = onFinish
  }

  private var trimmedEmail: String { email.trimmingCharacters(in: .whitespacesAndNewlines) }
  private var emailValid: Bool {
    trimmedEmail.range(of: "^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$", options: .regularExpression) != nil
  }
  private var codeValid: Bool { code.utf8.count == 6 && code.utf8.allSatisfy { (48...57).contains($0) } }
  private var offersGoogle: Bool { model.providers["google"] == true && GoogleSignInSupport.available }
  private var offersAny: Bool { offersGoogle || ["apple", "email", "phone"].contains { model.providers[$0] == true } }

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      if !model.providersLoaded {
        ProgressView().frame(maxWidth: .infinity, minHeight: 50)
      } else if !offersAny {
        Text(model.message ?? "暂时没有可用的登录方式，请稍后重试。")
          .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
          .fixedSize(horizontal: false, vertical: true)
        Button { pending = Task { await model.prepare() } } label: { Text("刷新登录方式") }
          .buttonStyle(AccountLoginButtonStyle(fill: MetasequoiaTheme.accentSoft, foreground: MetasequoiaTheme.accent))
          .disabled(model.busy)
          .accessibilityIdentifier("accountLoginRetry")
      } else {
        if model.providers["apple"] == true { appleButton }
        if offersGoogle { googleButton }
        if model.providers["email"] == true {
          emailButton
          if emailExpanded { emailForm.transition(.opacity) }
        }
        if model.providers["phone"] == true {
          Button {
            model.message = nil
            model.user = nil
            phoneChannel = .phone
          } label: {
            Text(CodeLoginChannel.phone.title).font(.system(size: 14, weight: .medium))
              .foregroundStyle(MetasequoiaTheme.accent)
              .frame(maxWidth: .infinity, minHeight: 40)
              .contentShape(Rectangle())
          }
          .buttonStyle(.plain)
          .disabled(model.busy)
          .accessibilityIdentifier("backendCodeLogin_\(CodeLoginChannel.phone.rawValue)")
        }
        if let message = model.message, phoneChannel == nil {
          Text(message).font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .center)
            .multilineTextAlignment(.center)
            .accessibilityIdentifier("accountLoginStatus")
        }
      }
    }
    .task { if !model.providersLoaded { await model.prepare() } }
    .sheet(item: $phoneChannel, onDismiss: {
      if let via = model.signedInVia { onFinish(via) }
    }) { channel in
      CodeLoginView(model: model, channel: channel)
    }
    .onChange(of: model.signedInVia) { _, via in
      // 手机号登录在它自己的面板里完成；那个面板关掉后由它的 `onDismiss` 上报。
      if let via, phoneChannel == nil { onFinish(via) }
    }
    .onDisappear { pending?.cancel() }
  }

  @ViewBuilder private var appleButton: some View {
    if let appleChallenge = model.appleChallenge {
      SignInWithAppleButton(.signIn) { request in
        request.nonce = appleChallenge.nonce
        request.state = appleChallenge.challenge_id
      } onCompletion: { result in
        pending = Task { await model.signInWithApple(result) }
      }
      .signInWithAppleButtonStyle(colorScheme == .dark ? .white : .black)
      .frame(height: 50)
      .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
      // 新的 challenge 需要新的按钮：请求闭包是在创建按钮时读取的。
      .id(appleChallenge.challenge_id)
      .disabled(model.busy)
      .accessibilityIdentifier("backendAppleSignIn")
    } else {
      Button { pending = Task { await model.prepareApple() } } label: {
        HStack(spacing: 8) {
          if model.preparingApple {
            ProgressView().tint(colorScheme == .dark ? .black : .white)
          } else {
            Image(systemName: "apple.logo")
          }
          Text(model.preparingApple ? "正在准备 Apple 登录" : "重新准备 Apple 登录")
        }
      }
      .buttonStyle(AccountLoginButtonStyle(fill: colorScheme == .dark ? .white : .black,
                                           foreground: colorScheme == .dark ? .black : .white))
      .disabled(model.preparingApple || model.busy)
      .accessibilityIdentifier("backendApplePrepare")
    }
  }

  /// 按 Google 的品牌规范画：彩色 G 标加「通过 Google 登录」，浅色下白底细描边，深色下深灰底。
  private var googleButton: some View {
    Button { pending = Task { await model.signInWithGoogle() } } label: {
      HStack(spacing: 8) {
        Image("GoogleMark").resizable().frame(width: 18, height: 18)
        Text("通过 Google 登录")
      }
    }
    .buttonStyle(AccountLoginButtonStyle(fill: colorScheme == .dark ? Color(white: 0.075) : .white,
                                         foreground: colorScheme == .dark ? Color(white: 0.89) : Color(white: 0.12),
                                         stroke: colorScheme == .dark ? Color(white: 0.56) : Color(white: 0.45)))
    .disabled(model.busy)
    .accessibilityIdentifier("backendGoogleSignIn")
  }

  private var emailButton: some View {
    Button {
      withAnimation(.easeOut(duration: 0.2)) { emailExpanded.toggle() }
      if emailExpanded {
        onExpand()
        focus = challenge == nil ? .email : .code
      } else {
        focus = nil
      }
    } label: {
      HStack(spacing: 8) {
        Image(systemName: "envelope")
        Text("使用邮箱登录")
      }
    }
    .buttonStyle(AccountLoginButtonStyle(fill: MetasequoiaTheme.accentSoft, foreground: MetasequoiaTheme.accent))
    .accessibilityIdentifier("backendCodeLogin_\(CodeLoginChannel.email.rawValue)")
  }

  @ViewBuilder private var emailForm: some View {
    if let challenge {
      Text("验证码已发到 \(sentTo)，\(max(1, challenge.expires_in / 60)) 分钟内有效")
        .font(.system(size: 13)).foregroundStyle(MetasequoiaTheme.sub)
        .fixedSize(horizontal: false, vertical: true)
      TextField("6 位验证码", text: $code)
        .keyboardType(.numberPad)
        .textContentType(.oneTimeCode)
        .modifier(AccountLoginFieldStyle())
        .focused($focus, equals: .code)
        .accessibilityIdentifier("backendVerificationCode")
      TimelineView(.periodic(from: .now, by: 1)) { timeline in
        let expired = expiresAt <= timeline.date
        let ready = expired || (codeValid && !model.busy)
        Button {
          if expired { restartEmail() } else { signIn(challenge) }
        } label: {
          HStack(spacing: 8) {
            if model.busy && !expired { ProgressView().tint(MetasequoiaTheme.onAccent) }
            Text(expired ? "验证码已过期，请重新获取" : "登录")
          }
        }
        .buttonStyle(AccountLoginButtonStyle(fill: ready ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg,
                                             foreground: ready ? MetasequoiaTheme.onAccent : MetasequoiaTheme.sub))
        .disabled(!ready)
        .accessibilityIdentifier("backendCodeSignIn")
      }
      Button(action: restartEmail) {
        Text("换个邮箱或重新发送").font(.system(size: 14, weight: .medium))
          .foregroundStyle(MetasequoiaTheme.accent)
          .frame(maxWidth: .infinity, minHeight: 40)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .disabled(model.busy)
      .accessibilityIdentifier("backendCodeRestart")
    } else {
      TextField("name@example.com", text: $email)
        .keyboardType(.emailAddress)
        .textContentType(.emailAddress)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .submitLabel(.send)
        .onSubmit(sendCode)
        .modifier(AccountLoginFieldStyle())
        .focused($focus, equals: .email)
        .accessibilityIdentifier("backendCodeTarget")
      TimelineView(.periodic(from: .now, by: 1)) { timeline in
        let seconds = max(0, Int(ceil(resendAt.timeIntervalSince(timeline.date))))
        let ready = emailValid && seconds == 0 && !model.busy
        Button(action: sendCode) {
          HStack(spacing: 8) {
            if model.busy { ProgressView().tint(MetasequoiaTheme.sub) }
            Text(seconds == 0 ? "发送验证码" : "\(seconds) 秒后可重新发送")
          }
        }
        .buttonStyle(AccountLoginButtonStyle(fill: ready ? MetasequoiaTheme.accent : MetasequoiaTheme.segBg,
                                             foreground: ready ? MetasequoiaTheme.onAccent : MetasequoiaTheme.sub))
        .disabled(!ready)
        .accessibilityIdentifier("backendSendCode")
      }
    }
  }

  private func sendCode() {
    guard emailValid, resendAt <= Date(), !model.busy else { return }
    let target = trimmedEmail
    pending = Task {
      let response = await model.requestCode(provider: CodeLoginChannel.email.rawValue, target: target)
      guard !Task.isCancelled, let response else { return }
      sentTo = target
      code = ""
      challenge = response
      expiresAt = Date().addingTimeInterval(TimeInterval(response.expires_in))
      resendAt = Date().addingTimeInterval(60)
      focus = .code
    }
  }

  private func signIn(_ challenge: BackendAccountClient.Challenge) {
    guard codeValid, !model.busy else { return }
    focus = nil
    pending = Task { await model.signInWithCode(challenge: challenge.challenge_id, code: code, channel: .email) }
  }

  /// 回到邮箱地址输入框；60 秒的重发等待仍然有效。
  private func restartEmail() {
    challenge = nil
    code = ""
    model.message = nil
    focus = .email
  }
}

/// 面板里 50pt 高的按钮：圆角 12 的填充底上是 17pt semibold 文字，可选一像素描边，按下时不透明度降到 .7。
private struct AccountLoginButtonStyle: ButtonStyle {
  let fill: Color
  let foreground: Color
  /// 一像素描边的颜色；nil 时不描边。
  var stroke: Color? = nil
  @Environment(\.displayScale) private var displayScale

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(.system(size: 17, weight: .semibold))
      .foregroundStyle(foreground)
      .lineLimit(1).minimumScaleFactor(0.8)
      .frame(maxWidth: .infinity, minHeight: 50)
      .background(fill, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
      .overlay {
        if let stroke {
          RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(stroke, lineWidth: 1 / displayScale)
        }
      }
      .opacity(configuration.isPressed ? 0.7 : 1)
      .contentShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
  }
}

/// 面板里 50pt 高的输入框：卡片底色上的 17pt 文字，外加一像素的细描边。
private struct AccountLoginFieldStyle: ViewModifier {
  @Environment(\.displayScale) private var displayScale

  func body(content: Content) -> some View {
    content
      .font(.system(size: 17))
      .padding(.horizontal, 14)
      .frame(height: 50)
      .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
      .overlay {
        RoundedRectangle(cornerRadius: 12, style: .continuous)
          .strokeBorder(MetasequoiaTheme.hair, lineWidth: 1 / displayScale)
      }
  }
}
