import SwiftUI

@MainActor
final class KeyboardChatModel: ObservableObject {
  struct Message: Identifiable {
    let id = UUID()
    let role: String
    let text: String
  }
  @Published var messages: [Message] = []
  @Published var models: [BackendAccountClient.ChatModels.Model] = []
  @Published var selectedModel = ""
  @Published var sending = false
  @Published var loadingModels = false
  @Published var error: String?
  @Published var loginNeeded = false
  private var request: Task<Void, Never>?
  private let api = BackendAccountClient()
  private var generation = 0

  private var isFixture: Bool {
    #if DEBUG && targetEnvironment(simulator)
    ProcessInfo.processInfo.arguments.contains("--keyboard-chat-ui-fixture")
    #else
    false
    #endif
  }

  func loadModels() async {
    if isFixture {
      models = [.init(id: "fixture-chat"), .init(id: "fixture-fast")]
      selectedModel = "fixture-chat"
      return
    }
    guard !loadingModels else { return }
    loadingModels = true
    defer { loadingModels = false }
    do {
      guard try await BackendAccountSession.shared.user() != nil else { loginNeeded = true; return }
      let token = try await BackendAccountSession.shared.accessToken()
      let catalog = try await api.chatModels(token: token)
      try Task.checkCancellation()
      models = catalog.data
      if !models.contains(where: { $0.id == selectedModel }) { selectedModel = catalog.default_model }
      loginNeeded = false
      error = nil
    } catch is CancellationError { }
    catch { show(error) }
  }

  func send(_ text: String) {
    guard !sending, !selectedModel.isEmpty else { return }
    messages.append(Message(role: "user", text: text))
    submit()
  }
  func retry() { guard !sending, messages.last?.role == "user" else { return }; submit() }
  private func submit() {
    error = nil; sending = true
    generation += 1
    let version = generation, model = selectedModel
    // Bound context to the service contract while retaining the newest complete messages.
    var context: [BackendAccountClient.ChatMessage] = []
    var bytes = 0
    for item in messages.reversed() {
      guard context.count < 14, bytes + item.text.utf8.count < 48000 else { break }
      context.insert(.init(role: item.role, content: item.text), at: 0)
      bytes += item.text.utf8.count
    }
    let history = context
    request = Task { [weak self] in
      guard let self else { return }
      defer { if generation == version { sending = false; request = nil } }
      do {
        if isFixture {
          try await Task.sleep(nanoseconds: 150_000_000)
          guard generation == version else { return }
          messages.append(Message(role: "assistant", text: "已收到：" + (history.last?.content ?? "")))
          return
        }
        var token = try await BackendAccountSession.shared.accessToken()
        let reply: String
        do { reply = try await api.chat(messages: history, model: model, token: token) }
        catch let failure as BackendAccountClient.Failure where failure.status == 401 {
          token = try await BackendAccountSession.shared.accessToken(retrying: token)
          reply = try await api.chat(messages: history, model: model, token: token)
        }
        try Task.checkCancellation()
        guard generation == version else { return }
        messages.append(Message(role: "assistant", text: reply))
      } catch is CancellationError { }
      catch { if !Task.isCancelled && generation == version { show(error) } }
    }
  }
  func cancel() { generation += 1; request?.cancel(); request = nil; sending = false }
  func clear() { cancel(); messages = []; error = nil }
  private func show(_ failure: Error) {
    if let failure = failure as? BackendAccountClient.Failure {
      loginNeeded = failure.status == 401
      switch failure.status {
      case 401: error = "登录后即可与 AI 对话，输入框仍可试用键盘。"
      case 404: error = "聊天服务正在更新，请稍后重新加载模型。"
      case 400: error = "模型或消息暂不受支持，请刷新模型列表后重试。"
      default: error = failure.localizedDescription
      }
    } else { error = "连接失败，请检查网络后重试。" }
  }
}

struct KeyboardTryoutView: View {
  var focusOnAppear = false
  @StateObject private var chat = KeyboardChatModel()
  @State private var draft = ""
  @State private var showAccount = false
  @State private var clearConfirmation = false
  @FocusState private var focused: Bool

  var body: some View {
    VStack(spacing: 0) {
      HStack(spacing: 8) {
        Image("EveryAPI").resizable().scaledToFit().frame(width: 22, height: 22)
        Text("EveryAPI").font(.subheadline.weight(.semibold))
        Spacer()
        if chat.loadingModels { ProgressView() }
        else if chat.models.isEmpty {
          Button(chat.loginNeeded ? "登录使用 AI" : "加载模型") {
            if chat.loginNeeded { showAccount = true }
            else { Task { await chat.loadModels() } }
          }.font(.caption)
        } else {
          Picker("模型", selection: $chat.selectedModel) {
            ForEach(chat.models) { Text($0.id).tag($0.id) }
          }.pickerStyle(.menu).disabled(chat.sending).accessibilityIdentifier("keyboardChatModelPicker")
        }
      }.padding(.horizontal, 16).padding(.vertical, 10)
      Divider()
      ScrollViewReader { proxy in
        ScrollView {
          LazyVStack(alignment: .leading, spacing: 16) {
            if chat.messages.isEmpty {
              VStack(alignment: .leading, spacing: 12) {
                Label("边聊天，边试键盘", systemImage: "keyboard").font(.title3.bold())
                Text("长按地球键切换到水杉输入法。试试你的皮肤、输入方案和模糊音，再发一条消息给 AI。").foregroundStyle(.secondary)
                Text("发送后，本次对话会经水杉后端交由 EveryAPI 处理。").font(.footnote).foregroundStyle(.secondary)
              }.padding(20).frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(uiColor: .secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 18))
            }
            ForEach(chat.messages) { message in
              HStack {
                if message.role == "user" { Spacer(minLength: 36) }
                Text(message.text).textSelection(.enabled).padding(13)
                  .foregroundStyle(message.role == "user" ? MetasequoiaTheme.onAccent : Color.primary)
                  .background(message.role == "user" ? MetasequoiaTheme.accent : Color(uiColor: .secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 16))
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
        }.background(Color(uiColor: .systemGroupedBackground))
          .onChange(of: chat.messages.count) { _ in withAnimation { proxy.scrollTo("bottom", anchor: .bottom) } }
          .onChange(of: chat.sending) { _ in withAnimation { proxy.scrollTo("bottom", anchor: .bottom) } }
      }
      Divider()
      HStack(spacing: 10) {
        TextField("输入消息，试试键盘", text: $draft).focused($focused)
          .padding(12).background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 14))
          .accessibilityIdentifier("keyboardTryoutField")
          .onChange(of: draft) { if $0.count > 2000 { draft = String($0.prefix(2000)) } }
        if chat.sending {
          Button { chat.cancel() } label: { Image(systemName: "stop.circle.fill").font(.title) }
            .accessibilityLabel("停止回复")
        } else {
          Button {
            if chat.loginNeeded { showAccount = true; return }
            let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { return }
            chat.send(text); draft = ""
          } label: { Image(systemName: "arrow.up.circle.fill").font(.title) }
          .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || (!chat.loginNeeded && chat.selectedModel.isEmpty))
          .accessibilityLabel("发送消息").accessibilityIdentifier("keyboardChatSend")
        }
      }.padding(12)
    }.navigationTitle("试用键盘").navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .navigationBarTrailing) {
          if focused { Button("收起键盘") { focused = false }.accessibilityIdentifier("dismissKeyboardButton") }
          else { Button { clearConfirmation = true } label: { Image(systemName: "square.and.pencil") }.accessibilityLabel("新对话").disabled(chat.messages.isEmpty) }
        }
      }
      .task { focused = focusOnAppear; await chat.loadModels() }
      .onDisappear { chat.cancel(); focused = false }
      .sheet(isPresented: $showAccount, onDismiss: { Task { await chat.loadModels() } }) {
        AccountLoginSheet()
      }
      .confirmationDialog("开始新对话？当前消息将被清空。", isPresented: $clearConfirmation, titleVisibility: .visible) {
        Button("新对话", role: .destructive) { chat.clear() }
      }
  }
}
