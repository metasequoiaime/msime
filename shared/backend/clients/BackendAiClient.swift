import Foundation

/// OpenAI-compatible AI candidate transport. The caller owns token storage and
/// injects it for one request; this client never persists or logs credentials.
struct BackendAiClient: Sendable {
  struct Candidate: Decodable, Sendable { let text: String }
  struct Result: Sendable { let candidates: [Candidate] }
  private let session: URLSession
  private static let maxResponseBytes = 1024 * 1024

  final class Redirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask,
                    willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest,
                    completionHandler: @escaping (URLRequest?) -> Void) {
      completionHandler(nil)
    }
  }

  init(configuration: URLSessionConfiguration = .ephemeral) {
    let configuration = configuration.copy() as! URLSessionConfiguration
    configuration.httpCookieStorage = nil
    configuration.urlCache = nil
    session = URLSession(configuration: configuration, delegate: Redirects(), delegateQueue: nil)
  }

  func suggest(endpoint: URL, model: String, token: String,
               segmentedPinyin: [String], context: String,
               candidateLimit: Int) async throws -> Result {
    guard endpoint.scheme == "https", endpoint.user == nil, endpoint.password == nil,
          endpoint.fragment == nil, !model.isEmpty, model.utf8.count <= 256,
          !token.isEmpty, !token.contains(where: { $0.isWhitespace }),
          !segmentedPinyin.isEmpty, segmentedPinyin.count <= 128,
          segmentedPinyin.allSatisfy({ !$0.isEmpty && $0.utf8.count <= 32 }),
          context.utf8.count <= 16 * 1024, (1...10).contains(candidateLimit)
    else { throw URLError(.badURL) }
    let prompt = "Return only JSON: {\"candidates\":[{\"text\":\"...\"}]}. Input pinyin: " + segmentedPinyin.joined(separator: " ") + " Context: " + context
    let body: [String: Any] = ["model": model, "temperature": 0, "max_tokens": 256,
                                "messages": [["role": "user", "content": prompt]]]
    let encodedBody = try JSONSerialization.data(withJSONObject: body)
    guard encodedBody.count <= 65_536 else { throw URLError(.dataLengthExceedsMaximum) }
    var request = URLRequest(url: endpoint)
    request.httpMethod = "POST"
    request.timeoutInterval = 30
    request.httpBody = encodedBody
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    let (bytes, response) = try await session.bytes(for: request)
    guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
          http.expectedContentLength < 0 || http.expectedContentLength <= Self.maxResponseBytes else {
      throw URLError(.badServerResponse)
    }
    var data = Data()
    for try await byte in bytes {
      guard data.count < Self.maxResponseBytes else { throw URLError(.dataLengthExceedsMaximum) }
      data.append(byte)
    }
    let envelope = try JSONDecoder().decode(Envelope.self, from: data)
    guard let content = envelope.choices.first?.message.content.data(using: .utf8) else { throw URLError(.cannotParseResponse) }
    let result = try JSONDecoder().decode(ResultPayload.self, from: content)
    guard result.candidates.count <= candidateLimit,
          result.candidates.allSatisfy({ candidate in
            !candidate.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
              && candidate.text.utf8.count <= 4096
              && !candidate.text.unicodeScalars.contains { $0.properties.generalCategory == .control }
          }) else { throw URLError(.cannotParseResponse) }
    return Result(candidates: result.candidates)
  }

  private struct Envelope: Decodable { struct Choice: Decodable { struct Message: Decodable { let content: String }; let message: Message }; let choices: [Choice] }
  private struct ResultPayload: Decodable { let candidates: [Candidate] }
}
