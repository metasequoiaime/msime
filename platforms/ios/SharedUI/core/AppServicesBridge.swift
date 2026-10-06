import Foundation

enum AppServicesBridge {
  private static let maximumVoiceTextCharacters = 10_000
  private static let maximumPolishTextBytes = 32 * 1024
  private static let maximumVoiceAudioBytes = 2_100_000

  static func polishBody(_ model: String, prompt: String, text: String) throws -> Data {
    guard prompt.utf8.count <= maximumPolishTextBytes else {
      throw ServiceFailure(message: "润色提示词过长。")
    }
    guard text.utf8.count <= maximumPolishTextBytes else {
      throw ServiceFailure(message: "润色文本过长。")
    }
    return try JSONSerialization.data(withJSONObject: [
      "model": model,
      "messages": [["role": "system", "content": prompt], ["role": "user", "content": text]],
      "stream": false
    ])
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

  static func parseResponse(_ data: Data, voice: Bool) throws -> String {
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
      throw ServiceFailure(message: "服务返回格式无效。")
    }
    if voice {
      guard let text = object["text"] as? String,
            text.count <= maximumVoiceTextCharacters else {
        throw ServiceFailure(message: object["error"] as? String ?? "服务未返回可用文字。")
      }
      return text
    }
    if let choices = object["choices"] as? [[String: Any]],
       let message = choices.first?["message"] as? [String: Any],
       let content = message["content"] as? String,
       content.utf8.count <= maximumPolishTextBytes { return content }
    throw ServiceFailure(message: object["error"] as? String ?? "服务未返回可用文字。")
  }
}
