import SwiftUI

@MainActor
final class KeyboardChatModel: ObservableObject {
  struct Message: Identifiable {
    let id = UUID()
    let role: String
    let text: String
  }
  @Published var messages: [Message] = []
  @Published var sending = false
  @Published var loadingAccount = false
  @Published var error: String?
  @Published var loginNeeded = false
  /// 已确认的账号；为 nil 时还不能发送。用哪个模型由后端决定，这里不加载模型目录，也不让用户选。
  @Published private(set) var accountID: String?
  private var request: Task<Void, Never>?
  private let api = BackendAccountClient()
  private var generation = OperationGeneration()
  private var accountSessionID: UUID?

  private func switchAccount(to userID: String?, sessionID: UUID? = nil) {
    guard accountID != userID || accountSessionID != sessionID else { return }
    clear()
    accountID = userID
    accountSessionID = sessionID
  }

  private var isFixture: Bool {
    #if DEBUG && targetEnvironment(simulator)
    ProcessInfo.processInfo.arguments.contains("--keyboard-chat-ui-fixture")
    #else
    false
    #endif
  }

  func loadAccount() async {
    if isFixture { switchAccount(to: "fixture-chat-account"); return }
    guard !loadingAccount else { return }
    loadingAccount = true
    defer { loadingAccount = false }
    do {
      guard let userID = try await BackendAccountSession.shared.user()?.id else {
        switchAccount(to: nil); loginNeeded = true; return
      }
      let identity = try await BackendAccountSession.shared.credentials(matchingUserID: userID)
      try Task.checkCancellation()
      switchAccount(to: identity.userID, sessionID: identity.sessionID)
      loginNeeded = false
      error = nil
    } catch is CancellationError { }
    catch { show(error) }
  }

  func send(_ text: String) {
    guard !sending, accountID != nil else { return }
    messages.append(Message(role: "user", text: text))
    submit()
  }
  func retry() { guard !sending, messages.last?.role == "user" else { return }; submit() }
  private func submit() {
    guard let userID = accountID else { loginNeeded = true; return }
    let sessionID = accountSessionID
    error = nil; sending = true
    let version = generation.begin()
    // The system prompt first, then the newest complete messages that still fit the service contract.
    let context = KeyboardChatHistoryPolicy.request(messages.map { (role: $0.role, text: $0.text) })
      .map { BackendAccountClient.ChatMessage(role: $0.role, content: $0.text) }
    let history = context
    request = Task { [weak self] in
      guard let self else { return }
      defer { if generation.isCurrent(version) { sending = false; request = nil } }
      do {
        if isFixture {
          try await Task.sleep(nanoseconds: 150_000_000)
          guard generation.isCurrent(version) else { return }
          messages.append(Message(role: "assistant", text: "已收到：" + (history.last?.content ?? "")))
          return
        }
        let reply = try await api.chat(messages: history,
                                       session: .shared, matchingUserID: userID,
                                       matchingSessionID: sessionID)
        try Task.checkCancellation()
        guard generation.isCurrent(version) else { return }
        messages.append(Message(role: "assistant", text: reply))
      } catch is CancellationError { }
      catch { if !Task.isCancelled && generation.isCurrent(version) { show(error) } }
    }
  }
  func cancel() { generation.cancel(task: &request); sending = false }
  func clear() { cancel(); messages = []; error = nil }
  private func show(_ failure: Error) {
    if let failure = failure as? BackendAccountClient.Failure {
      loginNeeded = failure.status == 401
      switch failure.status {
      case 401: error = "登录后即可与 AI 对话，输入框仍可试用键盘。"
      case 404: error = "聊天服务正在更新，请稍后重试。"
      case 400: error = "这条消息暂不受支持，请修改后重试。"
      default: error = failure.localizedDescription
      }
    } else { error = "连接失败，请检查网络后重试。" }
  }
}

struct KeyboardTryoutView: View {
  /// 从设置打开的 AI 对话二级页叫「AI 对话」；引导页和皮肤试用里仍叫「试用键盘」。
  var title = "试用键盘"
  var focusOnAppear = false
  /// `AppNavigation.chatFocusRequest`：与上次出现时不同，说明是从「试用键盘」过来的，出现时把焦点放进输入框。
  var focusRequest = 0
  @State private var handledFocusRequest = 0
  @StateObject private var chat = KeyboardChatModel()
  @State private var draft = ""
  @State private var showAccount = false
  @State private var clearConfirmation = false
  @FocusState private var focused: Bool

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        LazyVStack(alignment: .leading, spacing: 16) {
          if chat.messages.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
              Label("边聊天，边试键盘", systemImage: "keyboard").font(.title3.bold())
              Text("长按地球键切换到水杉输入法。试试你的皮肤、输入方案和模糊音，再发一条消息给 AI。").foregroundStyle(.secondary)
              Text("发送后，本次对话会经水杉后端交由 EveryAPI 处理。").font(.footnote).foregroundStyle(.secondary)
            }.padding(20).frame(maxWidth: .infinity, alignment: .leading)
              .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
          }
          ForEach(chat.messages) { message in
            HStack {
              if message.role == "user" { Spacer(minLength: 36) }
              Text(message.text).textSelection(.enabled).padding(13)
                .foregroundStyle(message.role == "user" ? MetasequoiaTheme.onAccent : Color.primary)
                .background(message.role == "user" ? MetasequoiaTheme.accent : MetasequoiaTheme.surface,
                            in: RoundedRectangle(cornerRadius: 18, style: .continuous))
              if message.role != "user" { Spacer(minLength: 36) }
            }.id(message.id)
          }
          if chat.sending { HStack { ProgressView(); Text("正在回复…").font(.footnote).foregroundStyle(.secondary) } }
          if let error = chat.error {
            VStack(alignment: .leading, spacing: 8) {
              Text(error).font(.footnote).foregroundStyle(.secondary)
              if chat.messages.last?.role == "user" && !chat.loginNeeded { Button("重试") { chat.retry() } }
            }
          } else if !chat.sending && chat.messages.last?.role == "user" {
            Button("重新发送") { chat.retry() }
          }
          Color.clear.frame(height: 1).id("bottom")
        }.padding(16)
      }
      .onChange(of: chat.messages.count) { withAnimation { proxy.scrollTo("bottom", anchor: .bottom) } }
      .onChange(of: chat.sending) { withAnimation { proxy.scrollTo("bottom", anchor: .bottom) } }
    }
    .designCanvasBackground()
    .chatBars(header: { header }, composer: { composer })
    .navigationTitle(title).navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .navigationBarTrailing) {
          if focused { Button("收起键盘") { focused = false }.accessibilityIdentifier("dismissKeyboardButton") }
          else { Button { clearConfirmation = true } label: { Image(systemName: "square.and.pencil") }.accessibilityLabel("新对话").disabled(chat.messages.isEmpty) }
        }
      }
      .task {
        // 真机连续点按验收期间保持本编辑器亮屏，不更改手机的自动锁屏设置。
        if ProcessInfo.processInfo.arguments.contains("--keyboard-toolbar-ui-test") {
          UIApplication.shared.isIdleTimerDisabled = true
        }
        if focusOnAppear || focusRequest != handledFocusRequest { focused = true }
        handledFocusRequest = focusRequest
        await chat.loadAccount()
      }
      .onDisappear {
        chat.cancel(); focused = false
        if ProcessInfo.processInfo.arguments.contains("--keyboard-toolbar-ui-test") {
          UIApplication.shared.isIdleTimerDisabled = false
        }
      }
      .sheet(isPresented: $showAccount, onDismiss: { Task { await chat.loadAccount() } }) {
        AccountLoginSheet()
      }
      .confirmationDialog("开始新对话？当前消息将被清空。", isPresented: $clearConfirmation, titleVisibility: .visible) {
        Button("新对话", role: .destructive) { chat.clear() }
      }
  }

  /// 服务商和登录入口。
  private var header: some View {
    HStack(spacing: 8) {
      Image("EveryAPI").resizable().scaledToFit().frame(width: 22, height: 22)
      Text("EveryAPI").font(.subheadline.weight(.semibold))
      Spacer()
      if chat.loadingAccount { ProgressView() }
      else if chat.loginNeeded || chat.accountID == nil {
        Button(chat.loginNeeded ? "登录使用 AI" : "重新连接") {
          if chat.loginNeeded { showAccount = true }
          else { Task { await chat.loadAccount() } }
        }.font(.caption).accessibilityIdentifier("keyboardChatAccountButton")
      }
    }.padding(.horizontal, 16).padding(.vertical, 10)
  }

  private var sendDisabled: Bool {
    draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || (!chat.loginNeeded && chat.accountID == nil)
  }

  private func send() {
    if chat.loginNeeded { showAccount = true; return }
    let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return }
    chat.send(text); draft = ""
  }

  /// 输入栏。iOS 26 起是浮在对话上的玻璃胶囊输入框加一枚强调色玻璃圆按钮，与系统「信息」的输入栏同一种做法；更早的系统沿用圆角输入框和图标按钮。
  @ViewBuilder private var composer: some View {
    if #available(iOS 26, *) {
      HStack(spacing: 10) {
        field.padding(.horizontal, 16).frame(minHeight: 44)
          .glassEffect(.regular.interactive(), in: Capsule())
        Group {
          if chat.sending {
            Button { chat.cancel() } label: { Image(systemName: "stop.fill").font(.system(size: 15, weight: .semibold)).frame(width: 30, height: 30) }
              .accessibilityLabel("停止回复")
          } else {
            Button(action: send) { Image(systemName: "arrow.up").font(.system(size: 17, weight: .semibold)).frame(width: 30, height: 30) }
              .disabled(sendDisabled)
              .accessibilityLabel("发送消息").accessibilityIdentifier("keyboardChatSend")
          }
        }
        .buttonStyle(.glassProminent).buttonBorderShape(.circle).tint(MetasequoiaTheme.accent)
      }.padding(.horizontal, 16).padding(.vertical, 8)
    } else {
      HStack(spacing: 10) {
        field.padding(12).background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 14))
        if chat.sending {
          Button { chat.cancel() } label: { Image(systemName: "stop.circle.fill").font(.title) }
            .accessibilityLabel("停止回复")
        } else {
          Button(action: send) { Image(systemName: "arrow.up.circle.fill").font(.title) }
            .disabled(sendDisabled)
            .accessibilityLabel("发送消息").accessibilityIdentifier("keyboardChatSend")
        }
      }.padding(12)
    }
  }

  private var field: some View {
    TextField("输入消息，试试键盘", text: $draft).focused($focused)
      .accessibilityIdentifier("keyboardTryoutField")
      .onChange(of: draft) { _, newValue in if newValue.count > 2000 { draft = String(newValue.prefix(2000)) } }
  }
}

private extension View {
  /// 对话页上下两条栏。iOS 26 起放进 `safeAreaBar`，对话滚到栏下面时与导航栏一样按滚动边缘效果渐隐，不画分隔线和实色底；更早的系统用分隔线隔开，垫页面色。
  @ViewBuilder func chatBars<Header: View, Composer: View>(@ViewBuilder header: () -> Header,
                                                          @ViewBuilder composer: () -> Composer) -> some View {
    if #available(iOS 26, *) {
      safeAreaBar(edge: .top, spacing: 0) { header() }
        .safeAreaBar(edge: .bottom, spacing: 0) { composer() }
    } else {
      safeAreaInset(edge: .top, spacing: 0) {
        VStack(spacing: 0) { header(); Divider() }.background(MetasequoiaTheme.canvas, ignoresSafeAreaEdges: [])
      }
      .safeAreaInset(edge: .bottom, spacing: 0) {
        VStack(spacing: 0) { Divider(); composer() }.background(MetasequoiaTheme.canvas)
      }
    }
  }
}
