import Foundation

/// Apple snapshot restore protocol, independent of any desktop host or Engine.
/// A failed/ambiguous replacement must never be retried automatically.
public final class SnapshotRestoreClient {
  private let session: URLSession

  final class Redirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask,
                    willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest,
                    completionHandler: @escaping (URLRequest?) -> Void) {
      completionHandler(nil)
    }
  }

  public init(configuration: URLSessionConfiguration = .ephemeral) {
    let configuration = configuration.copy() as! URLSessionConfiguration
    configuration.httpCookieStorage = nil
    configuration.urlCache = nil
    configuration.urlCredentialStorage = nil
    configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
    session = URLSession(configuration: configuration, delegate: Redirects(), delegateQueue: nil)
  }

  // Test-only injection keeps URLProtocol fixtures deterministic without making
  // the public default capable of following a credentialed redirect.
  init(session: URLSession) { self.session = session }

  public struct Result: Decodable, Sendable {
    public let revision: Int64
    public let reset: Bool
  }

  public func restore(file: URL, expectedSHA256: String, revision: Int64,
                      token: String) async throws -> Result {
    guard revision >= 0, !token.isEmpty,
          !token.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) || CharacterSet.whitespacesAndNewlines.contains($0) }),
          expectedSHA256.utf8.count == 64,
          expectedSHA256.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) })
    else { throw SnapshotFailure(status: 400) }
    try Task.checkCancellation()
    let prepared = try await BackendPreparedSnapshot.prepareDocument(file)
    defer { withExtendedLifetime(prepared) {} }
    guard prepared.envelope.sha256 == expectedSHA256 else { throw SnapshotFailure(status: 400) }
    guard let stream = InputStream(url: prepared.url),
          let size = try FileManager.default.attributesOfItem(atPath: prepared.url.path)[.size] as? NSNumber,
          size.int64Value > 0, size.int64Value <= 512 * 1024 * 1024
    else { throw SnapshotFailure(status: 400) }
    var request = URLRequest(url: URL(string: "https://api.msime.app/v1/users/me/dictionary/snapshot?revision=\(revision)")!)
    request.httpMethod = "PUT"
    request.timeoutInterval = 130
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    request.setValue("application/json", forHTTPHeaderField: "Accept")
    request.setValue("application/x-ndjson", forHTTPHeaderField: "Content-Type")
    request.setValue(size.stringValue, forHTTPHeaderField: "Content-Length")
    request.httpBodyStream = stream
    let (bytes, response) = try await session.bytes(for: request)
    guard let response = response as? HTTPURLResponse else { throw SnapshotFailure(status: 0) }
    guard response.statusCode == 200 else { throw SnapshotFailure(status: response.statusCode) }
    guard response.mimeType == "application/json", response.expectedContentLength <= 1024 * 1024
    else { throw SnapshotFailure(status: 0) }
    var data = Data()
    for try await byte in bytes {
      guard data.count < 1024 * 1024 else { throw SnapshotFailure(status: 0) }
      data.append(byte)
    }
    try Task.checkCancellation()
    let result: Result
    do { result = try JSONDecoder().decode(Result.self, from: data) }
    catch { throw SnapshotFailure(status: 0) }
    guard result.reset, result.revision > revision else { throw SnapshotFailure(status: 0) }
    return result
  }
}
