import CMSIMEClient
import Foundation
import Security
import XCTest
@testable import MSIMEBackend

/// 只模拟注册响应和回调所有权，不调用网络或系统钥匙串。
private enum LearningRegistry {
  static var replies: [String] = []
  static var descriptors: [msime_client_session_store] = []
  static var agents: [String?] = []

  static func table() -> msime_rust_client {
    var table = msime_rust_client()
    table.version = MSIME_RUST_CLIENT_VERSION
    table.size = UInt32(MemoryLayout<msime_rust_client>.stride)
    table.string_free = { free($0) }
    table.session_store_register = { pointer in
      guard let store = pointer?.pointee else { return nil }
      LearningRegistry.descriptors.append(store)
      LearningRegistry.agents.append(store.client_agent.map { String(cString: $0) })
      return strdup(LearningRegistry.replies.removeFirst())
    }
    table.session_store_unregister = { _ in strdup(#"{"ok":true,"value":true}"#) }
    return table
  }
}

final class BackendRustLearningStoreTests: XCTestCase {
  private let success = #"{"ok":true,"value":{"session_store":9}}"#
  private let rejected = #"{"ok":false,"error":"invalid_session_store"}"#
  private var root: URL!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-learning-store-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    LearningRegistry.replies = []
    LearningRegistry.descriptors = []
    LearningRegistry.agents = []
  }

  override func tearDownWithError() throws { try FileManager.default.removeItem(at: root) }

  private func layout(learning: Bool = true) -> BackendRustStoreLayout {
    BackendRustStoreLayout(
      account: .ownerOnlyFile(root.appendingPathComponent("account-session.json")),
      clipboardKeystore: .ownerOnlyFile(root.appendingPathComponent("cloud-clipboard-keystore")),
      anonymous: .localStore(directory: root, fileName: "anonymous-session.json"),
      anonymousIdentity: .localStore(directory: root, fileName: "anonymous-account.json"),
      accountLock: root.appendingPathComponent("account-refresh.lock"), anonymousLock: root.appendingPathComponent("anonymous.lock"),
      learningKeystore: learning ? .ownerOnlyFile(root.appendingPathComponent(BackendLearningKeystore.name)) : nil)
  }

  private func register(_ object: BackendRustSessionStore) -> Result<UInt64, BackendRustClient.Failure> {
    object.register(with: BackendRustClient(table: LearningRegistry.table())!, clientAgent: "msime-macos/1.0.0")
  }

  func testStandardMacLocationAndExplicitCapabilityKeepLegacyLayoutsUnchanged() throws {
    guard case let .ownerOnlyFile(url)? = BackendRustStoreLayout.standard.learningKeystore else { return XCTFail("缺少原生学习密钥位置") }
    XCTAssertEqual(url, BackendDesktopSessionFile.standardDirectory?.appendingPathComponent("learning-keystore", isDirectory: false))
    for enabled in [false, true] {
      LearningRegistry.replies = [success]
      let object = BackendRustSessionStore(layout: layout(learning: enabled), backgroundActivity: nil)
      XCTAssertEqual(register(object), .success(9))
      let descriptor = LearningRegistry.descriptors.last!
      XCTAssertEqual(descriptor.flags, enabled ? 3 : 1)
      descriptor.release?(descriptor.context)
    }
  }

  func testOldRuntimeRetriesWithTheSameContextAndAgentAndReleasesExactlyOnce() throws {
    LearningRegistry.replies = [rejected, success]
    weak var weakStore: BackendRustSessionStore?
    var descriptor: msime_client_session_store!
    autoreleasepool {
      let object = BackendRustSessionStore(layout: layout(), backgroundActivity: nil)
      weakStore = object
      XCTAssertEqual(register(object), .success(9))
      XCTAssertEqual(LearningRegistry.descriptors.map(\.flags), [3, 1])
      XCTAssertEqual(LearningRegistry.agents, ["msime-macos/1.0.0", "msime-macos/1.0.0"])
      XCTAssertEqual(LearningRegistry.descriptors[0].context, LearningRegistry.descriptors[1].context)
      descriptor = LearningRegistry.descriptors.last!
    }
    XCTAssertNotNil(weakStore)
    descriptor.release?(descriptor.context)
    XCTAssertNil(weakStore)
  }

  func testDefiniteRegistrationFailuresDoNotLeakTheRetainOrRetryOtherErrors() {
    for replies in [[rejected, rejected], [#"{"ok":false,"error":"account_unavailable"}"#]] {
      LearningRegistry.replies = replies
      LearningRegistry.descriptors = []
      weak var weakStore: BackendRustSessionStore?
      autoreleasepool {
        let object = BackendRustSessionStore(layout: layout(), backgroundActivity: nil)
        weakStore = object
        guard case .failure = register(object) else { return XCTFail("注册错误被当成成功") }
      }
      XCTAssertNil(weakStore)
      XCTAssertEqual(LearningRegistry.descriptors.count, replies.count)
    }
  }

  func testUnknownRegistrationOutcomeNeverRetriesOrReleasesPossiblyOwnedContext() {
    for replies in [["invalid"], [rejected, "invalid"]] {
      LearningRegistry.replies = replies
      LearningRegistry.descriptors = []
      weak var weakStore: BackendRustSessionStore?
      autoreleasepool {
        let object = BackendRustSessionStore(layout: layout(), backgroundActivity: nil)
        weakStore = object
        XCTAssertEqual(register(object), .failure(.invalidResponse))
      }
      XCTAssertNotNil(weakStore)
      XCTAssertEqual(LearningRegistry.descriptors.count, replies.count)
      // 模拟可能已经接管 context 的 Rust 最终释放，测试不留下实际泄漏。
      let descriptor = LearningRegistry.descriptors.last!
      descriptor.release?(descriptor.context)
      XCTAssertNil(weakStore)
    }
  }

  func testLegacyRemoveAccountAlsoClearsLearningButAnonymousAndClipboardDoNot() throws {
    LearningRegistry.replies = [rejected, success]
    let object = BackendRustSessionStore(layout: layout(), backgroundActivity: nil)
    XCTAssertEqual(register(object), .success(9))
    let descriptor = LearningRegistry.descriptors.last!
    defer { descriptor.release?(descriptor.context) }
    let key = MSIME_CLIENT_SESSION_ITEM_LEARNING_KEYSTORE.rawValue
    XCTAssertEqual(object.save(key, Data("synthetic-learning".utf8)), 0)
    for item in [MSIME_CLIENT_SESSION_ITEM_ANONYMOUS.rawValue, MSIME_CLIENT_SESSION_ITEM_CLIPBOARD_KEYSTORE.rawValue] {
      XCTAssertEqual(descriptor.remove?(descriptor.context, item), 0)
      XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("learning-keystore").path))
    }
    XCTAssertEqual(descriptor.remove?(descriptor.context, MSIME_CLIENT_SESSION_ITEM_ACCOUNT.rawValue), 0)
    XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("learning-keystore").path))
  }

  func testLegacyRemoveTriesBothItemsAndReportsEitherFailure() throws {
    for failed in ["account-session.json", "learning-keystore"] {
      LearningRegistry.replies = [rejected, success]
      let object = BackendRustSessionStore(layout: layout(), backgroundActivity: nil)
      XCTAssertEqual(register(object), .success(9))
      let descriptor = LearningRegistry.descriptors.last!
      let blocked = root.appendingPathComponent(failed)
      try FileManager.default.createDirectory(at: blocked, withIntermediateDirectories: false)
      let other = root.appendingPathComponent(failed == "learning-keystore" ? "account-session.json" : "learning-keystore")
      try BackendDesktopSessionFile.writeOwnerOnly(Data("synthetic".utf8), named: other.lastPathComponent, in: root)
      XCTAssertEqual(descriptor.remove?(descriptor.context, MSIME_CLIENT_SESSION_ITEM_ACCOUNT.rawValue), -1)
      XCTAssertFalse(FileManager.default.fileExists(atPath: other.path))
      XCTAssertTrue(FileManager.default.fileExists(atPath: blocked.path))
      descriptor.release?(descriptor.context)
      try FileManager.default.removeItem(at: blocked)
    }
  }

  func testSwiftLegacyFileLogoutRemovesLearningAndKeepsSessionWhenItsRemovalFails() throws {
    let store = BackendDesktopSessionFile(directory: root)
    for name in ["account-session.json", "cloud-clipboard-keystore", "learning-keystore"] {
      try BackendDesktopSessionFile.writeOwnerOnly(Data("synthetic".utf8), named: name, in: root)
    }
    try store.clear()
    for name in ["account-session.json", "cloud-clipboard-keystore", "learning-keystore"] {
      XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent(name).path))
    }
    try BackendDesktopSessionFile.writeOwnerOnly(Data("synthetic".utf8), named: "account-session.json", in: root)
    try FileManager.default.createDirectory(at: root.appendingPathComponent("learning-keystore"), withIntermediateDirectories: false)
    XCTAssertThrowsError(try store.clear())
    XCTAssertTrue(FileManager.default.fileExists(atPath: root.appendingPathComponent("account-session.json").path))
  }

  func testKeychainClearUsesIndependentNameAndPreservesFailure() throws {
    for status in [errSecSuccess, errSecItemNotFound, errSecInteractionNotAllowed] {
      var calls = 0
      let clear = {
        try BackendLearningKeystore.clearKeychain(service: "synthetic.service", delete: { query in
          calls += 1
          XCTAssertEqual(query[kSecAttrService as String] as? String, "synthetic.service")
          XCTAssertEqual(query[kSecAttrAccount as String] as? String, "learning-keystore")
          XCTAssertNil(query[kSecAttrAccessGroup as String])
          return status
        })
      }
      if status == errSecInteractionNotAllowed { XCTAssertThrowsError(try clear()) }
      else { XCTAssertNoThrow(try clear()) }
      XCTAssertEqual(calls, 1)
    }
  }
}
