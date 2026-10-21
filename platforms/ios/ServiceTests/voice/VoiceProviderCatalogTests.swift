import Foundation
import XCTest

final class VoiceProviderCatalogTests: XCTestCase {
  private func fixture() throws -> Data {
    try JSONSerialization.data(withJSONObject: [
      "ok": true,
      "value": [
        "asr": [
          ["id": "synthetic", "endpoint": "https://example.invalid/unrelated"],
          ["id": "doubao", "endpoint": "wss://example.invalid/second"],
        ],
        "doubao_stream_endpoints": [
          ["id": "third", "endpoint": "wss://example.invalid/third", "title": "第三种接口"],
          ["id": "first", "endpoint": "wss://example.invalid/first", "title": "第一种接口"],
          ["id": "second", "endpoint": "wss://example.invalid/second", "title": "第二种接口"],
        ],
      ],
    ])
  }

  func testSyntheticCatalogOwnsOrderTitlesAndDefaultEndpoint() throws {
    let catalog = try VoiceProviderCatalog(data: fixture())
    XCTAssertEqual(catalog.doubaoDefaultEndpoint, "wss://example.invalid/second")
    XCTAssertEqual(catalog.doubaoStreamEndpoints.map(\.id), ["third", "first", "second"])
    XCTAssertEqual(catalog.doubaoStreamEndpoints.map(\.title), ["第三种接口", "第一种接口", "第二种接口"])
    XCTAssertEqual(catalog.doubaoStreamEndpoints[1].endpoint, "wss://example.invalid/first")
  }

  func testDefaultEndpointComesFromTheMatchingProvider() throws {
    var envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: fixture()) as? [String: Any])
    var value = try XCTUnwrap(envelope["value"] as? [String: Any])
    value["asr"] = [["id": "doubao", "endpoint": "wss://example.invalid/third"]]
    envelope["value"] = value
    let catalog = try VoiceProviderCatalog(data: JSONSerialization.data(withJSONObject: envelope))
    XCTAssertEqual(catalog.doubaoDefaultEndpoint, "wss://example.invalid/third")
    XCTAssertEqual(catalog.doubaoStreamEndpoints.count, 3)
  }

  func testUnavailableAndMalformedCatalogsAreRejected() throws {
    for body in [
      "{}", "{\"ok\":false}",
      "{\"ok\":true,\"value\":{\"asr\":[],\"doubao_stream_endpoints\":[]}}",
      "{\"ok\":true,\"value\":{\"asr\":[{\"id\":\"doubao\",\"endpoint\":false}],\"doubao_stream_endpoints\":[]}}",
      "{\"ok\":true,\"value\":{\"asr\":[{\"id\":\"doubao\",\"endpoint\":\"wss://example.invalid\"}],\"doubao_stream_endpoints\":[{\"id\":\"test\",\"endpoint\":\"wss://example.invalid\",\"title\":null}]}}",
    ] {
      XCTAssertThrowsError(try VoiceProviderCatalog(data: Data(body.utf8)))
    }
  }

  func testSharedCatalogReadsTheRealCABIAndRetainsItsSnapshot() throws {
    let catalog = try XCTUnwrap(VoiceProviderCatalog.shared)
    XCTAssertFalse(catalog.doubaoDefaultEndpoint.isEmpty)
    XCTAssertTrue(catalog.doubaoStreamEndpoints.contains { $0.endpoint == catalog.doubaoDefaultEndpoint })
    XCTAssertTrue(catalog.doubaoStreamEndpoints.allSatisfy { !$0.id.isEmpty && !$0.title.isEmpty })
    let again = try XCTUnwrap(VoiceProviderCatalog.load())
    XCTAssertEqual(catalog.doubaoStreamEndpoints, again.doubaoStreamEndpoints)
    XCTAssertEqual(catalog.doubaoDefaultEndpoint, again.doubaoDefaultEndpoint)
  }
}
