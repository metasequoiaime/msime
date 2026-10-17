import Foundation

@_silgen_name("msime_client_ai_skin_plan")
private func msimeClientAISkinPlan(_ request: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?

struct AISkinProposal: Identifiable, Sendable {
  let id = UUID()
  let name: String
  let description: String
  let design: CustomKeyboardSkin
  var artworkPrompt: String? = nil
}

enum AISkinService {
  static func drawPrompt() -> String {
    let scenes = ["月光森林里的狐狸茶屋", "云朵之间的鲸鱼邮局", "雨夜街角的猫咪书店", "星际列车上的花园", "蘑菇村的秋日集市", "珊瑚海里的水母舞会", "雪山小屋与极光", "竹林里的熊猫茶会", "沙漠星空下的旅店", "复古街机里的糖果世界", "樱花河畔的兔子野餐", "漂浮岛屿上的灯塔"]
    let selected = scenes.shuffled().prefix(3).joined(separator: "；")
    return "这是一次随机皮肤抽卡。分别围绕以下三个灵感创作三套主题，每套对应一个场景：\(selected)。自由设计原创角色、插画风格和配色，三套键帽造型与材质都要不同，文字清晰。不要使用已有品牌或角色。"
  }

  /// 系统提示词与答案的解析都在 Rust（`msime_client_ai_skin_plan`）：提示词点名的文档格式就是解析器认的格式，两者不能分开。HTTP 暂时仍由 Swift 发。
  static func messages(prompt: String, model: String) throws -> [BackendAccountClient.ChatMessage] {
    struct Compose: Decodable {
      struct Body: Decodable { let messages: [BackendAccountClient.ChatMessage] }
      let path: String
      let body: Body
    }
    guard let compose = plan(["operation": "compose", "prompt": prompt, "model": model], as: Compose.self),
          compose.path == "/v1/chat/completions" else {
      throw ServiceFailure(message: "无法生成抽卡灵感，请重试。")
    }
    return compose.body.messages
  }

  /// 模型的回答经 Rust 校验成三套可用的设计，配色对比度不足的已修正；不合格（不是三套、造型材质或背景场景重复、参数越界、多出字段）时整批拒绝。
  static func parse(_ text: String) throws -> [AISkinProposal] {
    struct Plan: Decodable {
      let name: String
      let description: String
      let artworkPrompt: String
      let design: CustomKeyboardSkin
    }
    guard let plans = plan(["operation": "parse", "text": text], as: [Plan].self), plans.count == 3 else {
      throw ServiceFailure(message: "AI 返回的皮肤方案不可用，请重新生成。")
    }
    return plans.map { .init(name: $0.name, description: $0.description, design: $0.design, artworkPrompt: $0.artworkPrompt) }
  }

  /// 调 `msime_client_ai_skin_plan`，成功时把 value 解成 `type`；被拒绝或读不懂时为 nil。
  private static func plan<Value: Decodable>(_ request: [String: Any], as type: Value.Type) -> Value? {
    guard let value = ClientRequestEnvelope.call(msimeClientAISkinPlan, request),
          JSONSerialization.isValidJSONObject(value),
          let data = try? JSONSerialization.data(withJSONObject: value) else { return nil }
    return try? JSONDecoder().decode(type, from: data)
  }
  static func generate(_ prompt: String, client: BackendAccountClient = BackendAccountClient(),
                       account: BackendAccountSession = .shared,
                       progress: @MainActor @Sendable (Int) -> Void = { _ in }) async throws -> [AISkinProposal] {
    let prompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
    guard (1...500).contains(prompt.count) else { throw ServiceFailure(message: "无法生成抽卡灵感，请重试。") }
    let identity = try await account.credentials()
    let catalog = try await client.chatModels(session: account, matchingUserID: identity.userID,
                                              matchingSessionID: identity.sessionID)
    try Task.checkCancellation()
    let chatMessages = try messages(prompt: prompt, model: catalog.default_model)
    let result = try await account.authenticated(matchingUserID: identity.userID,
                                                 matchingSessionID: identity.sessionID) { token in
      try await client.chat(messages: chatMessages, model: catalog.default_model, token: token)
    }.value
    try await account.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
    try Task.checkCancellation()
    let plans = try parse(result)
    return try await withThrowingTaskGroup(of: (Int, AISkinProposal).self) { group in
      // The validated response has exactly three independent illustrations.
      for (index, plan) in plans.enumerated() {
        group.addTask {
          (index, try await illustrate(plan, prompt: prompt, client: client, account: account,
                                       userID: identity.userID, sessionID: identity.sessionID))
        }
      }
      var completed: [(Int, AISkinProposal)] = []
      for try await value in group {
        completed.append(value)
        try Task.checkCancellation()
        await progress(completed.count)
      }
      try await account.requireSession(matchingUserID: identity.userID, matchingSessionID: identity.sessionID)
      try Task.checkCancellation()
      return completed.sorted { $0.0 < $1.0 }.map { $0.1 }
    }
  }
  private static func illustrate(_ plan: AISkinProposal, prompt: String, client: BackendAccountClient,
                                 account: BackendAccountSession, userID: String,
                                 sessionID: UUID) async throws -> AISkinProposal {
      try await account.requireSession(matchingUserID: userID, matchingSessionID: sessionID)
      try Task.checkCancellation()
      guard let scene = plan.artworkPrompt else { throw ServiceFailure(message: "缺少背景场景，请重新抽取。") }
      let artwork = try await client.skinArtwork(prompt: scene,
        account: account, userID: userID, matchingSessionID: sessionID)
      try await account.requireSession(matchingUserID: userID, matchingSessionID: sessionID)
      try Task.checkCancellation()
      guard ["image/png", "image/jpeg"].contains(artwork.mime_type), (1...2048).contains(artwork.width),
            (1...2048).contains(artwork.height), let data = Data(base64Encoded:artwork.b64_json),
            !data.isEmpty, data.count <= 8 * 1024 * 1024 else {
        throw ServiceFailure(message: "AI 插画格式无效，请重新生成。")
      }
      let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
      defer { try? FileManager.default.removeItem(at:temporary) }
      try data.write(to:temporary,options:.atomic)
      guard let photo = SkinPhotoData.thumbnail(at:temporary) else {
        throw ServiceFailure(message: "无法处理 AI 插画，请重新生成。")
      }
      var design = plan.design
      design.photo = photo; design.photoShade = 0.08; design.photoPosition = 0.5
      design.keyOpacity = 0.92; design.pattern = 0
      return .init(name:plan.name,description:plan.description,design:design)
  }
}
