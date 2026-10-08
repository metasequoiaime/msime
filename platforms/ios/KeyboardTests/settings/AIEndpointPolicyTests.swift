import XCTest

/// 与 `crates/client-core/src/ai/endpoint.rs` 跑同一组用例（`shared/contracts/ai-endpoint/cases.json`）。两边结论不一致时，设置页会放行键盘拒绝的地址，或者把 Token 存在键盘找不到的来源键下。
final class AIEndpointPolicyTests: XCTestCase {
  func testSharedContractCases() throws {
    let file = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "cases", withExtension: "json"))
    let root = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    let cases = try XCTUnwrap(root["cases"] as? [[String: Any]])
    XCTAssertGreaterThan(cases.count, 40)
    for item in cases {
      let endpoint = try XCTUnwrap(item["endpoint"] as? String)
      let result: String
      switch AIEndpointPolicy.problem(endpoint) {
      case nil: result = "allowed"
      case .invalid: result = "invalid"
      case .cleartextPublicHost: result = "cleartext_public"
      }
      XCTAssertEqual(result, item["result"] as? String, endpoint)
      XCTAssertEqual(AIEndpointPolicy.origin(endpoint), item["origin"] as? String, endpoint)
    }
  }

  /// 带前导零的点分写法在系统解析里按八进制处理（`010.0.0.1` 是 8.0.0.1），不能当成 10/8 放行。
  func testLeadingZeroIPv4IsNotTreatedAsPrivate() {
    XCTAssertEqual(AIEndpointPolicy.problem("http://010.0.0.1/v1"), .cleartextPublicHost)
    XCTAssertEqual(AIEndpointPolicy.problem("http://0x7f000001/v1"), .cleartextPublicHost)
  }

  /// 明文地址的主机不收百分号编码和空方括号：`URLComponents.host` 是解码后的，`evil.com%00.local` 解码后以 `.local` 结尾。
  func testCleartextHostRejectsPercentEncodingAndEmptyBrackets() {
    for endpoint in ["http://evil.com%00.local/v1", "http://[fe80::1%25en0]/v1", "http://%31%32%37.0.0.1/v1", "http://[]/v1"] {
      XCTAssertEqual(AIEndpointPolicy.problem(endpoint), .invalid, endpoint)
    }
  }

  /// 键盘读 Token 用的钥匙串账户就是来源键；https 的键与以前完全相同，已存的 Token 不会丢。
  func testKeyboardKeychainAccountIsTheOrigin() throws {
    let https = try XCTUnwrap(URL(string: "https://API.Example.com/v1/chat/completions"))
    XCTAssertEqual(KeyboardAIService.query(url: https)[kSecAttrAccount as String] as? String, "https://api.example.com:443")
    let local = try XCTUnwrap(URL(string: "http://192.168.1.20:1234/v1/chat/completions"))
    XCTAssertEqual(KeyboardAIService.query(url: local)[kSecAttrAccount as String] as? String, "http://192.168.1.20:1234")
  }

  /// 明文请求直连，不经过系统代理；https 仍按系统设置，传入的配置不被改动。
  func testCleartextRequestsBypassTheSystemProxy() throws {
    let base = URLSessionConfiguration.ephemeral
    let local = AIEndpointPolicy.sessionConfiguration(base, for: URL(string: "http://192.168.1.20:1234/v1"))
    XCTAssertEqual(local.connectionProxyDictionary?.isEmpty, true)
    XCTAssertNil(base.connectionProxyDictionary)
    let secure = AIEndpointPolicy.sessionConfiguration(base, for: URL(string: "https://api.example.com/v1"))
    XCTAssertTrue(secure === base)
  }

  func testKeyboardConfigurationExplainsPublicHTTP() {
    var configuration = CustomServiceConfiguration()
    configuration.endpoint = "http://api.example.com/v1/chat/completions"
    configuration.model = "synthetic-model"
    XCTAssertThrowsError(try configuration.validatedURL()) { error in
      XCTAssertEqual(error.localizedDescription, AIEndpointPolicy.cleartextMessage)
    }
    configuration.endpoint = "http://192.168.1.20:1234/v1/chat/completions"
    XCTAssertEqual(try configuration.validatedURL().absoluteString, "http://192.168.1.20:1234/v1/chat/completions")
  }
}
