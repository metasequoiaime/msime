import Foundation
import XCTest
@testable import MSIMEBackend

private final class ResourceProtocol: URLProtocol {
  static let id = UUID(uuidString: "10000000-0000-0000-0000-000000000001")!
  static var requests = 0
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.requests += 1
    var data = request.httpBody ?? Data()
    if let stream = request.httpBodyStream {
      stream.open(); defer { stream.close() }
      var buffer = [UInt8](repeating: 0, count: 4096)
      while true { let n = stream.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; data.append(contentsOf: buffer.prefix(n)) }
    }
    let body = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    let path = request.url!.path
    var status = 200
    var result: [String: Any] = [:]
    let id = Self.id.uuidString.lowercased()
    func resource(_ id: String = id, kind: String = "reply", content: [String: Any] = ["prompt":"请礼貌回复"]) -> [String: Any] {
      ["id":id,"kind":kind,"name":"合成模板","description":"合成内容","author":"测试作者","content":content,
       "revision":1,"saves":1,"saved":true,"owned":false,"rating_count":1,"rating_average":5,"my_rating":5]
    }
    if request.httpMethod == "GET", path == "/v1/community/resources" {
      let query = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
      if query.contains(where: { $0.name == "q" && $0.value == "large" }) {
        let entries = (0..<128).map { ["kind":"quick", "code":"key\($0)", "word":String(repeating:"🌱", count:128), "weight":100000] as [String:Any] }
        result = ["items": (0..<20).map { resource(String(format:"10000000-0000-0000-0000-%012d",$0), kind:"dictionary", content:["entries":entries]) }, "has_more":false]
      } else {
        // In form-style query parsing an unescaped plus would become a space.
        if !request.url!.absoluteString.contains("C%2B%2B") { status = 400 }
        result = ["items":[resource()],"has_more":false]
      }
    } else if request.httpMethod == "GET", path.hasSuffix(id) { result = resource() }
    else if request.httpMethod == "POST", path.hasSuffix("/apply") {
      if body["resource_revision"] as? Int != 1 || body["dictionary_revision"] as? Int != 7 { status = 409 }
      result = ["revision":9,"imported":2,"resource_revision":1]
    }
    else if request.httpMethod == "POST" {
      if body["revision"] as? Int != 0 { status = 409 }
      if body["id"] as? String != id || body["kind"] as? String != "reply" { status = 400 }
      result = ["id":id,"revision":1]
    } else if request.httpMethod == "PUT", path.hasSuffix("/save") { result = ["saved":body["saved"] ?? false] }
    else if request.httpMethod == "PUT", path.hasSuffix("/rating") { result = ["stars":body["stars"] ?? 0] }
    else if request.httpMethod == "DELETE", path.hasSuffix(id) { result = ["deleted":true] }
    else { status = 404 }
    let encoded = try! JSONSerialization.data(withJSONObject: result)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url:request.url!,statusCode:status,httpVersion:nil,
      headerFields:["Content-Type":"application/json","Content-Length":String(encoded.count)])!, cacheStoragePolicy:.notAllowed)
    client?.urlProtocol(self, didLoad:encoded); client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class InvalidResourceProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let resource: [String: Any] = [
      "id": ResourceProtocol.id.uuidString.lowercased(), "kind": "reply", "name": "合成模板",
      "description": "", "author": "测试作者", "content": ["prompt": String(repeating: "字", count: 2_001)],
      "revision": 1, "saves": 0, "saved": false, "owned": false,
      "rating_count": 0, "rating_average": 0, "my_rating": 0
    ]
    let object: [String: Any] = request.url?.path == "/v1/community/resources"
      ? ["items": [resource], "has_more": false] : resource
    let data = try! JSONSerialization.data(withJSONObject: object)
    client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200,
      httpVersion: nil, headerFields: ["Content-Type": "application/json"])!,
      cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: data)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendCommunityResourceTests: XCTestCase {
  private func client() -> BackendAccountClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [ResourceProtocol.self]
    return BackendAccountClient(configuration: configuration)
  }
  func testVersionedPublicationAndPersonalActions() async throws {
    let api = client(), id = ResourceProtocol.id
    let page = try await api.communityResources(.reply, search:"C++ 模板")
    XCTAssertEqual(page.items.first?.content.prompt, "请礼貌回复")
    let detail = try await api.communityResource(id)
    XCTAssertEqual(detail.my_rating, 5)
    let created = try await api.publishResource(id:id, kind:.reply, name:"合成模板", description:"",
      content:.init(prompt:"请礼貌回复"), revision:0, token:"session")
    XCTAssertEqual(created.revision, 1)
    do {
      _ = try await api.publishResource(id:id, kind:.reply, name:"更新模板", description:"",
        content:.init(prompt:"新的内容"), revision:9, token:"session")
      XCTFail("Stale revision was accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status,409) }
    try await api.saveResource(id,saved:true,token:"session")
    try await api.rateResource(id,stars:5,token:"session")
    try await api.saveResource(id,saved:false,token:"session")
    try await api.deleteResource(id,token:"session")
  }
  func testApplyResourceSendsBothVersionsAndPropagatesConflict() async throws {
    let api = client()
    let result = try await api.applyResource(ResourceProtocol.id, resourceRevision: 1, dictionaryRevision: 7, token: "session")
    XCTAssertEqual(result.imported, 2)
    XCTAssertEqual(result.revision, 9)
    do {
      _ = try await api.applyResource(ResourceProtocol.id, resourceRevision: 1, dictionaryRevision: 6, token: "session")
      XCTFail("Stale dictionary was accepted")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 409) }
  }
  func testLargeWordPackPageDoesNotRelaxOtherEndpointLimits() async throws {
    let api = client()
    do {
      _ = try await api.request("GET", "/v1/community/resources?kind=dictionary&q=large")
      XCTFail("Ordinary response limit was relaxed")
    } catch is BackendAccountClient.Failure { }
    let page = try await api.communityResources(.dictionary, search:"large")
    XCTAssertEqual(page.items.count,20)
    XCTAssertEqual(page.items.first?.content.entries?.count,128)
  }
  func testInvalidPublicationAndAnonymousPrivateScopeAreRejected() async throws {
    let api = client()
    do {
      _ = try await api.communityResources(.reply, scope:.mine)
      XCTFail("Anonymous private scope was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status,400) }
    do {
      _ = try await api.publishResource(id:ResourceProtocol.id,kind:.dictionary,name:"词包",description:"",
        content:.init(entries:[]),revision:0,token:"session")
      XCTFail("Empty word pack was published")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status,400) }
    ResourceProtocol.requests = 0
    let invalid = BackendAccountClient.ResourceContent(entries: [
      .init(kind: .quick, code: "", word: "合成", weight: -1),
    ])
    do {
      _ = try await api.publishResource(id: ResourceProtocol.id, kind: .dictionary,
        name: "词包", description: "", content: invalid, revision: 0, token: "session")
      XCTFail("Invalid dictionary entry was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    XCTAssertEqual(ResourceProtocol.requests, 0)
  }
  func testCommunityMutationsRejectNilIDsAndUnrepresentableRevisionBeforeSending() async throws {
    let api = client(), nilID = UUID(uuidString: "00000000-0000-0000-0000-000000000000")!
    ResourceProtocol.requests = 0
    do {
      _ = try await api.communityResource(nilID)
      XCTFail("nil resource ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      _ = try await api.publishResource(id: ResourceProtocol.id, kind: .reply, name: "模板", description: "",
                                        content: .init(prompt: "回复"), revision: 50_001, token: "session")
      XCTFail("unrepresentable publication revision was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      _ = try await api.publishResource(id: nilID, kind: .reply, name: "模板", description: "",
                                        content: .init(prompt: "回复"), revision: 0, token: "session")
      XCTFail("nil publication ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      _ = try await api.applyResource(nilID, resourceRevision: 1, dictionaryRevision: 0, token: "session")
      XCTFail("nil apply ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      try await api.saveResource(nilID, saved: true, token: "session")
      XCTFail("nil save ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      try await api.rateResource(nilID, stars: 1, token: "session")
      XCTFail("nil rating ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    do {
      try await api.deleteResource(nilID, token: "session")
      XCTFail("nil delete ID was sent")
    } catch let failure as BackendAccountClient.Failure { XCTAssertEqual(failure.status, 400) }
    XCTAssertEqual(ResourceProtocol.requests, 0)
  }

  func testMalformedResourceResponsesAreRejected() async throws {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [InvalidResourceProtocol.self]
    let api = BackendAccountClient(configuration: configuration)
    do {
      _ = try await api.communityResources(.reply)
      XCTFail("malformed resource page was accepted")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 502)
    }
    do {
      _ = try await api.communityResource(ResourceProtocol.id)
      XCTFail("malformed resource detail was accepted")
    } catch let failure as BackendAccountClient.Failure {
      XCTAssertEqual(failure.status, 502)
    }
  }
}
