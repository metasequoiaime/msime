import Foundation

/// One bounded HTTPS request for a cloud or AI candidate provider.
struct OnlineCandidateRequest: Sendable {
  var urlRequest: URLRequest
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

/// HTTPS only, no redirects, no cookies or cache, and nothing kept between requests.
struct URLSessionOnlineCandidateTransport: OnlineCandidateTransport {
  static func validURL(_ url: URL?) -> Bool {
    guard let url, url.scheme == "https",
          let host = url.host, !host.isEmpty,
          url.user == nil, url.password == nil, url.fragment == nil else { return false }
    return true
  }

  func fetch(_ request: OnlineCandidateRequest) async -> Data? {
    guard Self.validURL(request.urlRequest.url) else { return nil }
    let configuration = URLSessionConfiguration.ephemeral
    configuration.timeoutIntervalForRequest = request.connectTimeout
    configuration.timeoutIntervalForResource = request.timeout
    configuration.waitsForConnectivity = false
    configuration.urlCache = nil
    configuration.httpCookieStorage = nil
    let session = URLSession(configuration: configuration, delegate: RedirectRefusal(), delegateQueue: nil)
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
