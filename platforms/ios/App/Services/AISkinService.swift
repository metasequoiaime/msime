import Foundation

struct AISkinProposal: Identifiable, Sendable {
  let id = UUID()
  let name: String
  let description: String
  let design: CustomKeyboardSkin
}

enum AISkinService {
  static func drawPrompt() -> String {
    let scenes = ["月光森林里的狐狸茶屋", "云朵之间的鲸鱼邮局", "雨夜街角的猫咪书店", "星际列车上的花园", "蘑菇村的秋日集市", "珊瑚海里的水母舞会", "雪山小屋与极光", "竹林里的熊猫茶会", "沙漠星空下的旅店", "复古街机里的糖果世界", "樱花河畔的兔子野餐", "漂浮岛屿上的灯塔"]
    let selected = scenes.shuffled().prefix(3).joined(separator: "；")
    return "这是一次随机皮肤抽卡。分别围绕以下三个灵感创作三套主题，每套对应一个场景：\(selected)。自由设计原创角色、插画风格和配色，三套键帽造型与材质都要不同，文字清晰。不要使用已有品牌或角色。"
  }

  /// 发给 Rust 的描述。Rust 拒收一切控制字符、按 Unicode 标量数限 500 个，而描述来自多行输入框：去掉首尾空白，内部的换行和制表符连同两侧空白折成一个空格；折完仍超过 500 个 Unicode 标量（emoji 常常一个字形占好几个）或含其他控制字符时为 nil，好在联网之前就提示，而不是白发一次模型目录请求再报「无法生成」。
  static func requestPrompt(_ text: String) -> String? {
    let folded = text.components(separatedBy: CharacterSet.newlines.union(CharacterSet(charactersIn: "\t")))
      .map { $0.trimmingCharacters(in: .whitespaces) }
      .filter { !$0.isEmpty }
      .joined(separator: " ")
    guard (1...maximumPromptScalars).contains(folded.unicodeScalars.count),
          !folded.unicodeScalars.contains(where: { $0.properties.generalCategory == .control }) else { return nil }
    return folded
  }

  /// 与 Rust 的上限相同，按 Unicode 标量（Rust 的 char）计。
  static let maximumPromptScalars = 500

  /// 三套带插画的皮肤（Rust 的 `msime_client_ai_skin_generate`，#8201 迁移计划 4c）：提示词、模型的回答、三张插画和账号都在 Rust 里，一次生成从头到尾用开始时的账号；每完成一张插画调一次 `progress`。这一页只给登录账号用（Rust 也接受匿名账号，与原来一样不放开），没登录时是 `account_unauthorized`。任务取消时 Rust 随即以 `ai_skin_cancelled` 结束，这里换成 `CancellationError`。
  static func generate(_ prompt: String,
                       rust: @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() },
                       progress: @escaping @MainActor @Sendable (Int) -> Void = { _ in }) async throws -> [AISkinProposal] {
    guard let prompt = requestPrompt(prompt) else {
      throw ServiceFailure(message: "描述太长或含有无法识别的字符，请删减后再试。")
    }
    let account = try rust()
    guard try await account.status() != nil else { throw BackendRustClient.Failure(code: "account_unauthorized") }
    let requestID = UUID().uuidString
    let body = try BackendRustClient.request(["session_store": account.store, "request_id": requestID, "prompt": prompt])
    let data: Data
    do {
      data = try await account.client.aiSkinGenerate(requestID: requestID, request: body) { completed in
        Task { @MainActor in progress(Int(completed)) }
      }
    } catch let failure as BackendRustClient.Failure where failure.code == "ai_skin_cancelled" {
      throw CancellationError()
    } catch let failure as BackendRustClient.Failure where failure.code == "ai_skin_invalid" {
      throw ServiceFailure(message: "AI 返回的皮肤方案不可用，请重新生成。")
    }
    try Task.checkCancellation()
    struct Artwork: Decodable { let b64_json: String; let mime_type: String; let width: Int; let height: Int }
    struct Plan: Decodable { let name: String; let description: String; let design: CustomKeyboardSkin; let artwork: Artwork }
    guard let plans = try? JSONDecoder().decode([Plan].self, from: data), plans.count == 3 else {
      throw ServiceFailure(message: "AI 返回的皮肤方案不可用，请重新生成。")
    }
    return try plans.map { plan in
      try illustrated(AISkinProposal(name: plan.name, description: plan.description, design: plan.design),
                      artwork: (plan.artwork.b64_json, plan.artwork.mime_type, plan.artwork.width, plan.artwork.height))
    }
  }

  /// 把 Rust 给的插画缩成照片背景放进设计。
  static func illustrated(_ plan: AISkinProposal, artwork: (base64: String, mimeType: String, width: Int, height: Int)) throws -> AISkinProposal {
    guard ["image/png", "image/jpeg"].contains(artwork.mimeType), (1...2048).contains(artwork.width),
          (1...2048).contains(artwork.height), let data = Data(base64Encoded: artwork.base64),
          !data.isEmpty, data.count <= 8 * 1024 * 1024 else {
      throw ServiceFailure(message: "AI 插画格式无效，请重新生成。")
    }
    let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: temporary) }
    try data.write(to: temporary, options: .atomic)
    guard let photo = SkinPhotoData.thumbnail(at: temporary) else {
      throw ServiceFailure(message: "无法处理 AI 插画，请重新生成。")
    }
    var design = plan.design
    design.photo = photo; design.photoShade = 0.08; design.photoPosition = 0.5
    design.keyOpacity = 0.92; design.pattern = 0
    return .init(name: plan.name, description: plan.description, design: design)
  }
}
