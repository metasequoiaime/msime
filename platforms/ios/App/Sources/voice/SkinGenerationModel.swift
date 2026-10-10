import SwiftUI

@MainActor
final class SkinGenerationModel: ObservableObject {
  @Published var prompt = ""
  @Published var models: [BackendAccountClient.ChatModels.Model] = []
  @Published var selectedModel = ""
  @Published var loading = false
  @Published var generating = false
  @Published var loginNeeded = false
  @Published var message: String?
  @Published var result: SavedKeyboardSkin?
  private var request: Task<Void, Never>?
  private var generation = UUID()
  private let api = BackendAccountClient()
  private var accountID: String?
  private var accountSessionID: UUID?
  private func switchAccount(to userID: String?, sessionID: UUID? = nil) {
    guard accountID != userID || accountSessionID != sessionID else { return }
    let hadAccount = accountID != nil
    cancel()
    accountID = userID
    accountSessionID = sessionID
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
      guard let userID = try await BackendAccountSession.shared.user()?.id else {
        switchAccount(to: nil); loginNeeded = true; return
      }
      let identity = try await BackendAccountSession.shared.credentials(matchingUserID: userID)
      switchAccount(to: identity.userID, sessionID: identity.sessionID)
      let catalog = try await api.chatModels(session: .shared, matchingUserID: identity.userID,
                                              matchingSessionID: identity.sessionID)
      try Task.checkCancellation()
      models = catalog.data
      if !models.contains(where: { $0.id == selectedModel }) { selectedModel = catalog.default_model }
      loginNeeded = false; message = nil
    } catch is CancellationError { }
    catch { show(error) }
  }
  func generate() {
    let description = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
    guard let userID = accountID, !generating, !description.isEmpty, description.count <= 600,
          models.contains(where: { $0.id == selectedModel }) else { return }
    generating = true; message = nil
    let id = UUID(); generation = id
    let model = selectedModel
    let sessionID = accountSessionID
    request = Task {
      defer { if generation == id { generating = false; request = nil } }
      do {
        let reply: String
        if fixture {
          try await Task.sleep(nanoseconds: ProcessInfo.processInfo.arguments.contains("-skinGenerationSlowFixture") ? 3_600_000_000_000 : 300_000_000)
          reply = ##"{"name":"AI 苔绿庭院","design":{"background":"#E8F0EB","keyBackground":"#FFFFFF","keyForeground":"#17251D","accent":"#185C47","actionBackground":"#185C47","cornerRadius":12,"borderWidth":0.5,"shadow":0.1,"pattern":3}}"##
        } else {
          let messages: [BackendAccountClient.ChatMessage] = [.init(role: "system", content: GeneratedKeyboardSkin.instruction), .init(role: "user", content: description)]
          reply = try await api.chat(messages: messages, model: model,
                                     session: .shared, matchingUserID: userID,
                                     matchingSessionID: sessionID)
        }
        try Task.checkCancellation()
        let parsed = try GeneratedKeyboardSkin.parse(reply)
        guard generation == id else { return }
        result = parsed
      } catch is CancellationError { }
      catch { if generation == id && !Task.isCancelled { show(error) } }
    }
  }
  func cancel() { generation = UUID(); request?.cancel(); request = nil; generating = false }
  private func show(_ error: Error) {
    if let failure = error as? BackendAccountClient.Failure {
      loginNeeded = failure.status == 401
      message = failure.status == 400 ? "模型列表已更新，请刷新模型后重试。" : failure.localizedDescription
    } else { message = error.localizedDescription }
  }
}
