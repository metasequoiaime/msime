import Foundation

/// 统一在线请求的毫秒超时范围，避免不同提供方使用不同边界。
enum OnlineRequestPolicy {
  static func timeout(milliseconds: Int) -> TimeInterval {
    TimeInterval(SharedNumber.clamped(milliseconds, to: 1_000...10_000)) / 1000
  }
}

/// One bounded HTTPS request for a cloud or AI candidate provider.
struct OnlineCandidateRequest: Sendable {
  var urlRequest: URLRequest
  /// 只有 AI 候选可以走本机或局域网的 http（`AIEndpointPolicy`）；云候选和翻译始终只走 https。
  var allowsLocalHTTP = false
  /// How long the connection may sit idle, connecting included.
  var connectTimeout: TimeInterval
  /// How long the whole exchange may take.
  var timeout: TimeInterval
  /// A reply larger than this is dropped rather than parsed.
  var maxBytes: Int
}

protocol OnlineCandidateTransport: Sendable {
  /// The body of a 2xx reply, or nil for anything else; an unavailable provider is an ordinary outcome, not an error.
  func fetch(_ request: OnlineCandidateRequest) async -> Data?
}

/// HTTPS only (AI candidates may also use HTTP on the local network), no redirects, no cookies or cache, and nothing kept between requests.
struct URLSessionOnlineCandidateTransport: OnlineCandidateTransport {
  static func validURL(_ url: URL?, allowsLocalHTTP: Bool = false) -> Bool {
    guard let url else { return false }
    if allowsLocalHTTP { return AIEndpointPolicy.validatedURL(url.absoluteString) != nil }
    guard url.scheme == "https",
          let host = url.host, !host.isEmpty,
          url.user == nil, url.password == nil, url.fragment == nil else { return false }
    return true
  }

  func fetch(_ request: OnlineCandidateRequest) async -> Data? {
    guard Self.validURL(request.urlRequest.url, allowsLocalHTTP: request.allowsLocalHTTP) else { return nil }
    let configuration = URLSessionConfiguration.ephemeral
    configuration.timeoutIntervalForRequest = request.connectTimeout
    configuration.timeoutIntervalForResource = request.timeout
    configuration.waitsForConnectivity = false
    configuration.urlCache = nil
    configuration.httpCookieStorage = nil
    let session = URLSession(configuration: AIEndpointPolicy.sessionConfiguration(configuration, for: request.urlRequest.url),
                             delegate: RedirectRefusal(), delegateQueue: nil)
    defer { session.invalidateAndCancel() }
    do {
      let (bytes, response) = try await session.bytes(for: request.urlRequest)
      guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
            http.expectedContentLength <= Int64(request.maxBytes) else { return nil }
      var body = Data()
      for try await byte in bytes {
        guard body.count < request.maxBytes else { return nil }
        body.append(byte)
      }
      return body.isEmpty ? nil : body
    } catch {
      return nil
    }
  }

  /// The shared host built the URL and must be the only one choosing where the request goes; a 3xx comes back as the reply and is refused by its status.
  private final class RedirectRefusal: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
  }
}
