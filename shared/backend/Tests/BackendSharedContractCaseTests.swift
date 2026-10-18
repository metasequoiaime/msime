import Foundation
import XCTest
@testable import MSIMEBackend

/// 读 `shared/contracts/` 下各平台共用的用例文件（#8201 迁移计划第 6 步）。测试从源码树里读，与 Rust 的 `include_str!` 读的是同一份。
enum SharedContract {
  static func cases(_ name: String, file: StaticString = #filePath) throws -> [[String: Any]] {
    // Tests → backend → shared，再进 contracts。
    let shared = URL(fileURLWithPath: "\(file)").deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: shared.appendingPathComponent("contracts/\(name)/cases.json"))
    let contract = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    return try XCTUnwrap(contract["cases"] as? [[String: Any]])
  }
}

/// `shared/contracts/account-session-layouts/cases.json` 里 Apple 布局的两份真实字节（Swift 自己写的、Rust 按 layout 1 写的），Swift 的 `BackendSavedSession` 都读得出，读出的与 Rust 读出的是同一个会话。Native、鸿蒙扁平和 Android v2 不经 Swift。
final class BackendSessionLayoutCaseTests: XCTestCase {
  private static let referenceDateUnixMilliseconds = 978_307_200_000.0

  func testEveryAppleLayoutCaseDecodesToTheSessionRustReads() throws {
    let cases = try SharedContract.cases("account-session-layouts").filter { $0["layout"] as? String == "apple" }
    XCTAssertEqual(Set(cases.compactMap { $0["name"] as? String }), ["apple_swift", "apple_rust"])
    for item in cases {
      let name = try XCTUnwrap(item["name"] as? String)
      let document = try XCTUnwrap(item["document"] as? String)
      let expected = try XCTUnwrap(item["session"] as? [String: Any])
      let tokens = try XCTUnwrap(expected["tokens"] as? [String: Any])
      let user = try XCTUnwrap(tokens["user"] as? [String: Any])
      let saved = try JSONDecoder().decode(BackendSavedSession.self, from: Data(document.utf8))
      // 与 BackendKeychain、BackendDesktopSessionFile 读的时候一样过一遍校验；用例的过期时间都在过去，不会因为太远被拒。
      _ = try BackendSavedSession.validated(saved)
      XCTAssertEqual(saved.tokens.access_token, tokens["access_token"] as? String, name)
      XCTAssertEqual(saved.tokens.refresh_token, tokens["refresh_token"] as? String, name)
      XCTAssertEqual(saved.tokens.token_type, tokens["token_type"] as? String, name)
      XCTAssertEqual(saved.tokens.expires_in, tokens["expires_in"] as? Int, name)
      XCTAssertEqual(saved.tokens.user, BackendAccountClient.User(
        id: try XCTUnwrap(user["id"] as? String), display_name: try XCTUnwrap(user["display_name"] as? String),
        created_at: try XCTUnwrap(user["created_at"] as? String), email: user["email"] as? String,
        avatar_url: user["avatar_url"] as? String), name)
      // Rust 把 expiresAt 换成 Unix 毫秒时四舍五入。
      let milliseconds = (saved.expiresAt.timeIntervalSinceReferenceDate * 1000 + Self.referenceDateUnixMilliseconds).rounded()
      XCTAssertEqual(Int64(milliseconds), (expected["expires_at_unix_ms"] as? NSNumber)?.int64Value, name)
      XCTAssertEqual(saved.sessionID?.uuidString.lowercased(), expected["session_id"] as? String, name)
    }
  }
}

/// `shared/contracts/account-errors/cases.json`：Rust 交来的每个账号错误码（带不带 Retry-After），iOS 和 macOS 的账号页说成用例里 `apple` 的那一句。
final class BackendAccountErrorCaseTests: XCTestCase {
  func testEveryAccountErrorCaseReadsAsTheAppleSentence() throws {
    let cases = try SharedContract.cases("account-errors")
    XCTAssertGreaterThan(cases.count, 10)
    for item in cases {
      let code = try XCTUnwrap(item["code"] as? String)
      let failure = BackendRustClient.Failure(code: code, retryAfterSeconds: item["retry_after_seconds"] as? Int)
      XCTAssertEqual(failure.errorDescription, item["apple"] as? String, code)
    }
  }
}
