import Foundation

enum CustomServiceKind { case ai, voice }

enum AIProviderPreset: String, Codable, Sendable {
  case everyAPI, openAI, anthropic, gemini, deepSeek, qwen, kimi, zhipu, siliconFlow, openRouter, custom
}

struct ServiceFailure: LocalizedError {
  let message: String
  var errorDescription: String? { message }
}

private final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
  func urlSession(_ session: URLSession, task: URLSessionTask,
                  willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
                  completionHandler: @escaping (URLRequest?) -> Void) {
    completionHandler(nil)
  }
}

struct CustomServiceConfiguration: Codable, Sendable, Equatable {
  private static let maximumModelBytes = 256
  var provider: AIProviderPreset = .custom
  var endpoint = ""
  var model = ""
  var prompt = "请润色以下文字，保持原意，只返回修改后的文字。"

  /// 键盘扩展里只有 AI 这一种服务，按 `AIEndpointPolicy` 放行本机和局域网的 http 地址；参数只为与 App 里同名方法的签名一致，`KeyboardAIService` 和 `KeyboardAIView` 两边共用。
  func validatedURL(allowsLocalHTTP: Bool = true) throws -> URL {
    let trimmedEndpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
    let trimmedModel = model.trimmingCharacters(in: .whitespacesAndNewlines)
    let problem = AIEndpointPolicy.problem(trimmedEndpoint)
    if allowsLocalHTTP && problem == .cleartextPublicHost {
      throw ServiceFailure(message: AIEndpointPolicy.cleartextMessage)
    }
    guard problem == nil, let url = AIEndpointPolicy.validatedURL(trimmedEndpoint),
          allowsLocalHTTP || url.scheme?.lowercased() == "https",
          !trimmedModel.isEmpty, trimmedModel.utf8.count <= Self.maximumModelBytes else {
      throw ServiceFailure(message: "请填写完整的接口地址（https，或本机、局域网的 http）和模型名称。")
    }
    return url
  }
}

enum CustomServiceClient {
  private static let maximumResponseBytes = 1024 * 1024

  static func request(kind: CustomServiceKind, configuration: CustomServiceConfiguration,
                      text: String, token: String,
                      sessionConfiguration: URLSessionConfiguration = .ephemeral) async throws -> String {
    guard kind == .ai else { throw ServiceFailure(message: "键盘扩展不支持此服务类型。") }
    var request = URLRequest(url: try configuration.validatedURL(allowsLocalHTTP: true))
    request.httpMethod = "POST"
    request.timeoutInterval = 30
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.httpBody = try JSONSerialization.data(withJSONObject: [
      "model": configuration.model,
      "messages": [["role": "user", "content": configuration.prompt + "\n" + text]],
      "stream": false
    ])
    let session = URLSession(configuration: AIEndpointPolicy.sessionConfiguration(sessionConfiguration, for: request.url),
                             delegate: NoRedirects(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    let (bytes, response) = try await session.bytes(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
      throw ServiceFailure(message: "服务请求失败，请检查键盘 AI 配置。")
    }
    var data = Data()
    for try await byte in bytes {
      guard data.count < maximumResponseBytes else { throw ServiceFailure(message: "服务响应过大。") }
      data.append(byte)
    }
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let choices = object["choices"] as? [[String: Any]],
          let message = choices.first?["message"] as? [String: Any],
          let content = message["content"] as? String else {
      throw ServiceFailure(message: "服务返回格式无效。")
    }
    return content
  }
}
