import Foundation

/// 「AI 对话」：经 Rust 的流式对话（`msime_client_account_chat_stream`，#8201 迁移计划 4c），回复边生成边显示，「停止」立刻结束。用哪个模型由后端决定：进页时读一次 `chat_models` 的默认模型，不让用户选。只给登录账号用，与原来相同（Rust 也接受匿名账号，这一页不放开）。
@MainActor
final class KeyboardChatModel: ObservableObject {
  struct Message: Identifiable {
    let id = UUID()
    let role: String
    var text: String
  }
  @Published var messages: [Message] = []
  @Published var sending = false
  @Published var loadingAccount = false
  @Published var error: String?
  @Published var loginNeeded = false
  /// 已确认的账号；为 nil 时还不能发送。
  @Published private(set) var accountID: String?
  private var request: Task<Void, Never>?
  private var generation = OperationGeneration()
  /// 后端的默认模型（`chat_models` 的 `defaultModel`）。
  private var model: String?
  /// 默认是 App 启动时注册的那一份，测试传入替身函数表上的账号。
  private let rust: @Sendable () throws -> BackendRustAccount

  init(rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }) {
    self.rust = rust
  }

  private func switchAccount(to userID: String?) {
    guard accountID != userID else { return }
    clear()
    accountID = userID
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
      let account = try rust()
      guard let userID = try await account.status()?.id else {
        switchAccount(to: nil); loginNeeded = true; return
      }
      let catalog = try BackendRustAccount.object(try await account.account(["operation": "chat_models"]))
      guard let model = catalog["defaultModel"] as? String, !model.isEmpty else { throw BackendRustClient.Failure.invalidResponse }
      try Task.checkCancellation()
      switchAccount(to: userID)
      self.model = model
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
    error = nil; sending = true
    let version = generation.begin()
    // 先放系统提示词，再放服务契约还装得下的最新几条完整消息。
    let history = KeyboardChatHistoryPolicy.request(messages.map { (role: $0.role, text: $0.text) })
      .map { ["role": $0.role, "content": $0.text] }
    let model = self.model
    request = Task { [weak self] in
      guard let self else { return }
      defer { if generation.isCurrent(version) { sending = false; request = nil } }
      if isFixture {
        try? await Task.sleep(nanoseconds: 150_000_000)
        guard generation.isCurrent(version), !Task.isCancelled else { return }
        messages.append(Message(role: "assistant", text: "已收到：" + (history.last?["content"] ?? "")))
        return
      }
      // 这一轮回复在 `messages` 里的位置；第一段增量到达时才加进去。
      var reply: UUID?
      do {
        guard let model else { throw BackendRustClient.Failure.invalidResponse }
        let account = try rust()
        let requestID = UUID().uuidString
        let body = try BackendRustClient.request(["session_store": account.store, "request_id": requestID,
                                                  "messages": history, "model": model])
        for try await delta in account.client.chatStream(requestID: requestID, request: body) {
          guard generation.isCurrent(version) else { return }
          if let reply, let index = messages.firstIndex(where: { $0.id == reply }) {
            messages[index].text += delta
          } else {
            let message = Message(role: "assistant", text: delta)
            reply = message.id
            messages.append(message)
          }
        }
        // 「停止」：留下已经显示的部分。
        guard !Task.isCancelled, generation.isCurrent(version) else { return }
        // 回答期间退出或换了号：Rust 已经核对过给令牌的账号，这里再核对页面绑定的那个。
        guard try await account.status()?.id == userID else { drop(reply); switchAccount(to: nil); loginNeeded = true; return }
      } catch {
        guard !Task.isCancelled, generation.isCurrent(version) else { return }
        // `account_cancelled`：回答期间给令牌的账号退出或换了号，已经交出的增量要丢掉。
        if (error as? BackendRustClient.Failure)?.code == "account_cancelled" { drop(reply) }
        show(error)
      }
    }
  }
  private func drop(_ reply: UUID?) {
    guard let reply else { return }
    messages.removeAll { $0.id == reply }
  }
  func cancel() { generation.cancel(task: &request); sending = false }
  func clear() { cancel(); messages = []; error = nil }
  private func show(_ failure: Error) {
    guard let failure = failure as? BackendRustClient.Failure else { error = "连接失败，请检查网络后重试。"; return }
    loginNeeded = failure.code == "account_unauthorized"
    switch failure.code {
    case "account_unauthorized": error = "登录后即可与 AI 对话，输入框仍可试用键盘。"
    case "account_invalid": error = "这条消息暂不受支持，请修改后重试。"
    case "account_too_large": error = "对话内容太长，请开一个新对话或删减后重试。"
    case "account_cancelled": error = "账号已变化，这次回复没有保留。"
    default: error = failure.localizedDescription
    }
  }
}
