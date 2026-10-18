import SwiftUI

/// 用一句描述生成一套键盘皮肤：模型目录（`chat_models`）和对话（`chat`）都经 Rust 的 `msime_client_account`（#8201 迁移计划 4c），只给登录账号用。
@MainActor
final class SkinGenerationModel: ObservableObject {
  /// 模型目录里的一项。
  struct Model: Identifiable, Equatable, Sendable { let id: String }
  @Published var prompt = ""
  @Published var models: [Model] = []
  @Published var selectedModel = ""
  @Published var loading = false
  @Published var generating = false
  @Published var loginNeeded = false
  @Published var message: String?
  @Published var result: SavedKeyboardSkin?
  private var request: Task<Void, Never>?
  private var generation = OperationGeneration()
  private var accountID: String?
  /// 默认是 App 启动时注册的那一份，测试传入替身函数表上的账号。
  private let rust: @Sendable () throws -> BackendRustAccount
  init(rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() }) {
    self.rust = rust
  }
  private func switchAccount(to userID: String?) {
    guard accountID != userID else { return }
    let hadAccount = accountID != nil
    cancel()
    accountID = userID
    models = []
    selectedModel = ""
    if hadAccount { prompt = "" }
    result = nil
    message = nil
  }
  private var fixture: Bool {
    #if DEBUG && targetEnvironment(simulator)
    ProcessInfo.processInfo.arguments.contains("-skinGenerationFixture")
    #else
    false
    #endif
  }
  func loadModels() async {
    guard !loading else { return }; loading = true
    defer { loading = false }
    if fixture {
      switchAccount(to: "fixture-skin-account")
      models = [.init(id: "gpt-5.6-luna")]; selectedModel = "gpt-5.6-luna"; return
    }
    do {
      let account = try rust()
      guard let userID = try await account.status()?.id else {
        switchAccount(to: nil); loginNeeded = true; return
      }
      switchAccount(to: userID)
      let catalog = try BackendRustAccount.object(try await account.account(["operation": "chat_models"]))
      guard let data = catalog["data"] as? [[String: Any]], let defaultModel = catalog["defaultModel"] as? String else {
        throw BackendRustClient.Failure.invalidResponse
      }
      try Task.checkCancellation()
      models = data.compactMap { ($0["id"] as? String).map(Model.init(id:)) }
      if !models.contains(where: { $0.id == selectedModel }) { selectedModel = defaultModel }
      loginNeeded = false; message = nil
    } catch is CancellationError { }
    catch { show(error) }
  }
  func generate() {
    let description = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
    guard let userID = accountID, !generating, !description.isEmpty, description.count <= 600,
          models.contains(where: { $0.id == selectedModel }) else { return }
    generating = true; message = nil
    let id = generation.begin()
    let model = selectedModel
    let rust = self.rust
    request = Task {
      defer { if generation.isCurrent(id) { generating = false; request = nil } }
      do {
        let reply: String
        if fixture {
          try await Task.sleep(nanoseconds: ProcessInfo.processInfo.arguments.contains("-skinGenerationSlowFixture") ? 3_600_000_000_000 : 300_000_000)
          reply = ##"{"name":"AI 苔绿庭院","design":{"background":"#E8F0EB","keyBackground":"#FFFFFF","keyForeground":"#17251D","accent":"#185C47","actionBackground":"#185C47","cornerRadius":12,"borderWidth":0.5,"shadow":0.1,"pattern":3}}"##
        } else {
          let account = try rust()
          let messages = [["role": "system", "content": GeneratedKeyboardSkin.instruction], ["role": "user", "content": description]]
          let answer = try BackendRustAccount.object(try await account.account(["operation": "chat", "messages": messages, "model": model]))
          guard let content = answer["content"] as? String else { throw BackendRustClient.Failure.invalidResponse }
          // 回答期间退出或换了号时 Rust 报 `account_cancelled`；这里再核对页面绑定的账号。
          guard try await account.status()?.id == userID else { throw CancellationError() }
          reply = content
        }
        try Task.checkCancellation()
        let parsed = try GeneratedKeyboardSkin.parse(reply)
        guard generation.isCurrent(id) else { return }
        result = parsed
      } catch is CancellationError { }
      catch { if generation.isCurrent(id) && !Task.isCancelled { show(error) } }
    }
  }
  func cancel() { generation.cancel(task: &request); generating = false }
  private func show(_ error: Error) {
    if let failure = error as? BackendRustClient.Failure {
      loginNeeded = failure.code == "account_unauthorized"
      message = failure.code == "account_invalid" ? "模型列表已更新，请刷新模型后重试。" : failure.localizedDescription
    } else { message = error.localizedDescription }
  }
}
