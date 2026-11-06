import Foundation

enum AppServicesBridge {
  private static let maximumVoiceTextCharacters = 10_000
  private static let maximumPolishTextBytes = 32 * 1024
  private static let maximumVoiceAudioBytes = 2_100_000

  static func polishBody(_ model: String, endpoint: URL, prompt: String, text: String) throws -> Data {
    guard prompt.utf8.count <= maximumPolishTextBytes else {
      throw ServiceFailure(message: "润色提示词过长。")
    }
    guard text.utf8.count <= maximumPolishTextBytes else {
      throw ServiceFailure(message: "润色文本过长。")
    }
    var body: [String: Any] = [
      "model": model,
      "messages": [["role": "system", "content": prompt], ["role": "user", "content": text]],
      "stream": false
    ]
    if disablesArkSeedThinking(endpoint: endpoint, model: model) { body["thinking"] = ["type": "disabled"] }
    return try JSONSerialization.data(withJSONObject: body)
  }

  /// 火山方舟上的豆包 Seed 默认开着深度思考，润色和键盘里的 AI 回复这类短请求要显式发 thinking.type = disabled。方舟深度思考文档（https://docs.volcengine.com/docs/82379/1449737 ，2026-11-05 查）列出的 doubao-seed-* 全部支持 disabled，同平台的别的模型不一定（glm-5-3-flash-260828 只支持 enabled），所以接口地址属于 volces.com 且模型属于这个系列才算。与 client-providers 的 disables_ark_seed_thinking 一致。
  static func disablesArkSeedThinking(endpoint: URL, model: String) -> Bool {
    guard let host = endpoint.host?.lowercased(), host == "volces.com" || host.hasSuffix(".volces.com") else {
      return false
    }
    return model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().hasPrefix("doubao-seed-")
  }

  static func transcriptionBody(_ wav: Data, model: String, language: String? = nil) throws -> [String: Any] {
    guard wav.count <= maximumVoiceAudioBytes else {
      throw ServiceFailure(message: "语音文件过大。")
    }
    let boundary = "Boundary-\(UUID().uuidString)"
    var body = Data()
    func append(_ text: String) { body.append(contentsOf: text.utf8) }
    append("--\(boundary)\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n\(model)\r\n")
    if let language, !language.isEmpty {
      append("--\(boundary)\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\n\(language)\r\n")
    }
    append("--\(boundary)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n")
    body.append(wav)
    append("\r\n--\(boundary)--\r\n")
    return ["body": body, "contentType": "multipart/form-data; boundary=\(boundary)"]
  }

  /// 阿里云百炼的整句识别（chat_audio）：唯一一条 user 消息，内容是录音的 Base64 数据 URL；不带语种，交给模型自动识别。录音上限和 multipart 相同，编码后仍远在百炼 10 MB 的上限以内。
  static func chatAudioBody(_ wav: Data, model: String) throws -> Data {
    guard wav.count <= maximumVoiceAudioBytes else {
      throw ServiceFailure(message: "语音文件过大。")
    }
    let content: [[String: Any]] = [[
      "type": "input_audio",
      "input_audio": ["data": "data:audio/wav;base64," + wav.base64EncodedString()],
    ]]
    return try JSONSerialization.data(withJSONObject: [
      "model": model,
      "stream": false,
      "messages": [["role": "user", "content": content]],
    ] as [String: Any])
  }

  static func parseResponse(_ data: Data, voice: Bool) throws -> String {
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
      throw ServiceFailure(message: "服务返回格式无效。")
    }
    if voice {
      // multipart 接口的文字在 `text`，chat_audio 接口的在 `choices[0].message.content`。
      // 明确失败约束整条回复，文字别名不能绕过聊天候选的完成状态。
      if let error = object["error"], !(error is NSNull) {
        throw ServiceFailure(message: "服务未返回可用文字。")
      }
      if let first = (object["choices"] as? [[String: Any]])?.first,
         let reason = first["finish_reason"] as? String,
         reason == "length" || reason == "content_filter" {
        throw ServiceFailure(message: "服务未返回可用文字。")
      }
      let chat = ((object["choices"] as? [[String: Any]])?.first?["message"] as? [String: Any])?["content"]
      guard let text = (object["text"] ?? chat) as? String,
            text.count <= maximumVoiceTextCharacters else {
        throw ServiceFailure(message: object["error"] as? String ?? "服务未返回可用文字。")
      }
      return text
    }
    // 明确失败或不完整的 AI 回复不能覆盖完整转写；缺省和 null 状态兼容旧服务。
    if let error = object["error"], !(error is NSNull) {
      throw ServiceFailure(message: "服务未返回可用文字。")
    }
    if let first = (object["choices"] as? [[String: Any]])?.first {
      let reason = first["finish_reason"] as? String
      guard reason != "length", reason != "content_filter" else {
        throw ServiceFailure(message: "服务未返回可用文字。")
      }
      if let message = first["message"] as? [String: Any],
         let content = message["content"] as? String,
         content.utf8.count <= maximumPolishTextBytes { return content }
    }
    throw ServiceFailure(message: "服务未返回可用文字。")
  }
}
