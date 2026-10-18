import CMSIMEClient
import Foundation
import Security
import XCTest
#if canImport(Darwin)
import Darwin
#endif
@testable import MSIMEBackend

/// 替身 Rust 的注册入口：把宿主交来的结构体原样留下，测试再经其中的 C 回调读写，与 Rust 调用的方式相同。
private enum FakeRegistry {
  static var store: msime_client_session_store?
  static var clientAgent: String?
  static var reply = #"{"ok":true,"value":{"session_store":3}}"#

  static func table() -> msime_rust_client {
    var table = msime_rust_client()
    table.version = MSIME_RUST_CLIENT_VERSION
    table.size = UInt32(MemoryLayout<msime_rust_client>.stride)
    table.string_free = { free($0) }
    table.session_store_register = { store in
      guard let store else { return strdup(#"{"ok":false,"error":"invalid_session_store"}"#) }
      FakeRegistry.store = store.pointee
      // Rust 在注册时复制 client_agent，之后不再读这个指针。
      FakeRegistry.clientAgent = store.pointee.client_agent.map { String(cString: $0) }
      return strdup(FakeRegistry.reply)
    }
    table.session_store_unregister = { _ in strdup(#"{"ok":true,"value":true}"#) }
    return table
  }
}

/// 内存里的钥匙串，可以指定下一次读写返回的状态码。
private final class MemoryKeychain: @unchecked Sendable {
  let lock = NSLock()
  var items: [String: Data] = [:]
  var copyStatus: OSStatus?
  var deleteStatus: OSStatus?
  var deletedGroups: [String?] = []

  private static func key(_ query: [String: Any]) -> String {
    "\(query[kSecAttrService as String] as? String ?? "")|\(query[kSecAttrAccount as String] as? String ?? "")"
  }

  var calls: BackendRustKeychain {
    BackendRustKeychain(
      copyMatching: { [self] query in
        lock.lock(); defer { lock.unlock() }
        if let copyStatus { return (copyStatus, nil) }
        guard let data = items[Self.key(query)] else { return (errSecItemNotFound, nil) }
        return (errSecSuccess, data)
      },
      update: { [self] query, attributes in
        lock.lock(); defer { lock.unlock() }
        guard items[Self.key(query)] != nil else { return errSecItemNotFound }
        items[Self.key(query)] = attributes[kSecValueData as String] as? Data
        return errSecSuccess
      },
      add: { [self] attributes in
        lock.lock(); defer { lock.unlock() }
        guard items[Self.key(attributes)] == nil else { return errSecDuplicateItem }
        XCTAssertEqual(attributes[kSecAttrAccessible as String] as? String, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String)
        items[Self.key(attributes)] = attributes[kSecValueData as String] as? Data
        return errSecSuccess
      },
      delete: { [self] query in
        lock.lock(); defer { lock.unlock() }
        deletedGroups.append(query[kSecAttrAccessGroup as String] as? String)
        if let deleteStatus { return deleteStatus }
        return items.removeValue(forKey: Self.key(query)) == nil ? errSecItemNotFound : errSecSuccess
      })
  }
}

private let account = MSIME_CLIENT_SESSION_ITEM_ACCOUNT.rawValue
private let keystore = MSIME_CLIENT_SESSION_ITEM_CLIPBOARD_KEYSTORE.rawValue
private let anonymous = MSIME_CLIENT_SESSION_ITEM_ANONYMOUS.rawValue
private let identity = MSIME_CLIENT_SESSION_ITEM_ANONYMOUS_IDENTITY.rawValue

final class BackendRustSessionStoreTests: XCTestCase {
  private var root: URL!
  private var state: URL { root.appendingPathComponent("state", isDirectory: true) }
  private var group: URL { root.appendingPathComponent("group", isDirectory: true) }

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-rust-store-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    FakeRegistry.store = nil
    FakeRegistry.clientAgent = nil
    FakeRegistry.reply = #"{"ok":true,"value":{"session_store":3}}"#
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: root)
  }

  private func fileLayout() -> BackendRustStoreLayout {
    BackendRustStoreLayout(
      account: .ownerOnlyFile(state.appendingPathComponent("account-session.json")),
      clipboardKeystore: .ownerOnlyFile(state.appendingPathComponent("cloud-clipboard-keystore")),
      anonymous: .localStore(directory: group, fileName: "anonymous-session.json"),
      anonymousIdentity: .localStore(directory: group, fileName: "anonymous-account.json"),
      accountLock: state.appendingPathComponent("account-refresh.lock"),
      anonymousLock: group.appendingPathComponent("backend-account-refresh.lock"))
  }

  private func keychainLayout() -> BackendRustStoreLayout {
    var layout = fileLayout()
    layout.account = .keychain(service: BackendKeychain.defaultService, account: BackendKeychain.sessionAccount,
                               accessGroup: "group.synthetic")
    layout.clipboardKeystore = .keychain(service: BackendKeychain.defaultService, account: BackendKeychainClipboardKeystore.account,
                                         accessGroup: "group.synthetic")
    layout.anonymousIdentity = .keychain(service: "synthetic.identity", account: "identity", accessGroup: nil)
    return layout
  }

  /// 注册一个存储，返回替身 Rust 收下的结构体。
  private func registered(_ store: BackendRustSessionStore, agent: String? = "msime-macos/0.52.0 (Mac; macOS 26; edition=full)",
                          file: StaticString = #filePath, line: UInt = #line) throws -> msime_client_session_store {
    let client = try XCTUnwrap(BackendRustClient(table: FakeRegistry.table()))
    XCTAssertEqual(store.register(with: client, clientAgent: agent), .success(3), file: file, line: line)
    return try XCTUnwrap(FakeRegistry.store, file: file, line: line)
  }

  private func load(_ store: msime_client_session_store, _ item: UInt32, capacity: Int = 65536) -> (Int, Data) {
    var buffer = [UInt8](repeating: 0, count: capacity)
    let count = buffer.withUnsafeMutableBufferPointer { store.load!(store.context, item, $0.baseAddress, capacity) }
    return (count, count > 0 ? Data(buffer[..<count]) : Data())
  }

  private func save(_ store: msime_client_session_store, _ item: UInt32, _ data: Data) -> Int32 {
    data.withUnsafeBytes { store.save!(store.context, item, $0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
  }

  private func syntheticSession(sessionID: UUID? = UUID()) -> BackendSavedSession {
    let user = BackendAccountClient.User(id: "synthetic-user", display_name: "合成用户", created_at: "2026-10-18")
    let tokens = BackendAccountClient.Tokens(access_token: String(repeating: "a", count: 64),
      refresh_token: String(repeating: "b", count: 64), token_type: "Bearer", expires_in: 900, user: user)
    return BackendSavedSession(tokens: tokens, expiresAt: Date().addingTimeInterval(600), sessionID: sessionID)
  }

  /// 锁文件和存放位置必须与还没迁移的 Swift 会话是同一个，否则两边互相排斥不了（#8201 迁移计划 4a）。
  func testStandardLayoutUsesTheFilesAndLocksOfTheSwiftSessions() throws {
    let layout = BackendRustStoreLayout.standard
    #if os(macOS)
    guard case .ownerOnlyFile(let session) = layout.account, case .ownerOnlyFile(let keystore) = layout.clipboardKeystore else {
      return XCTFail("macOS keeps the account in files")
    }
    let directory = try XCTUnwrap(BackendDesktopSessionFile.standardDirectory)
    XCTAssertEqual(session, directory.appendingPathComponent("account-session.json", isDirectory: false))
    XCTAssertEqual(keystore, directory.appendingPathComponent("cloud-clipboard-keystore", isDirectory: false))
    // 登录会话的锁在会话文件旁边，与 `BackendAccountSession()` 和设置应用的 `FileAccountSessionStorage` 相同。
    XCTAssertEqual(layout.accountLock, directory.appendingPathComponent("account-refresh.lock", isDirectory: false))
    XCTAssertEqual(layout.accountLock, BackendDesktopSessionFile.refreshLock.url)
    #endif
    // 匿名会话两个平台都在 App Group，锁是 `BackendAnonymousAccount.session` 用的那一把。
    XCTAssertEqual(layout.anonymousLock, BackendFileRefreshLock.appGroup.url)
    XCTAssertEqual(layout.anonymousLock?.lastPathComponent, "backend-account-refresh.lock")
    guard case .localStore(nil, let anonymous) = layout.anonymous, case .localStore(nil, let identity) = layout.anonymousIdentity else {
      return XCTFail("anonymous items live in the App Group local store")
    }
    XCTAssertEqual(anonymous, "anonymous-session.json")
    XCTAssertEqual(identity, "anonymous-account.json")
  }

  func testRegistrationDescribesASharedAppleLayoutStoreAndCopiesTheAgent() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    XCTAssertEqual(store.version, UInt32(MSIME_CLIENT_SESSION_STORE_VERSION))
    XCTAssertEqual(store.layout, 1)
    XCTAssertEqual(store.flags, MSIME_CLIENT_SESSION_STORE_SHARED)
    XCTAssertNotNil(store.load); XCTAssertNotNil(store.save); XCTAssertNotNil(store.remove)
    XCTAssertNotNil(store.lock); XCTAssertNotNil(store.unlock); XCTAssertNotNil(store.release)
    XCTAssertEqual(FakeRegistry.clientAgent, "msime-macos/0.52.0 (Mac; macOS 26; edition=full)")
    store.release?(store.context)
  }

  func testReleaseAfterRegistrationAndFailedRegistrationEachDropTheStoreExactlyOnce() throws {
    weak var released: BackendRustSessionStore?
    try autoreleasepool {
      let object = BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil)
      released = object
      let store = try registered(object, agent: nil)
      XCTAssertNil(FakeRegistry.clientAgent)
      store.release?(store.context)
    }
    XCTAssertNil(released, "release did not drop the retain taken at registration")

    // 注册被拒时 Rust 没有接管 context，也不会调用 release，存储自己放掉那次 retain。
    FakeRegistry.reply = #"{"ok":false,"error":"invalid_session_store"}"#
    weak var rejected: BackendRustSessionStore?
    autoreleasepool {
      let object = BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil)
      rejected = object
      let client = BackendRustClient(table: FakeRegistry.table())!
      XCTAssertEqual(object.register(with: client, clientAgent: nil),
                     .failure(.init(code: "invalid_session_store", response: Data(FakeRegistry.reply.utf8))))
    }
    XCTAssertNil(rejected)
  }

  func testEveryItemIsAbsentAtFirstAndRemovingAbsentItemsSucceeds() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    for item in [account, keystore, anonymous, identity] {
      XCTAssertEqual(load(store, item).0, 0, "item \(item)")
      XCTAssertEqual(store.remove!(store.context, item), 0, "item \(item)")
    }
    XCTAssertEqual(load(store, 99).0, -1)
  }

  func testAccountBytesAreTheSwiftSessionFileAndRustBytesReadBackInSwift() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    // Swift 写的会话，Rust 读到的是原样的字节。
    let swift = syntheticSession()
    try BackendDesktopSessionFile(directory: state).save(swift)
    let (count, bytes) = load(store, account)
    XCTAssertGreaterThan(count, 0)
    XCTAssertEqual(bytes, try Data(contentsOf: state.appendingPathComponent("account-session.json")))

    // Rust 按布局 1 写的会话（共享用例 `apple_rust`：sessionID 小写，expiresAt 是 2001 年起的秒数带 .0），经回调存下以后 Swift 照常读出。
    let rust = try XCTUnwrap(SharedContract.cases("account-session-layouts").first { $0["name"] as? String == "apple_rust" })
    let expected = try XCTUnwrap(rust["session"] as? [String: Any])
    XCTAssertEqual(save(store, account, Data(try XCTUnwrap(rust["document"] as? String).utf8)), 0)
    let read = try XCTUnwrap(BackendDesktopSessionFile(directory: state).load())
    XCTAssertEqual(read.sessionID?.uuidString.lowercased(), expected["session_id"] as? String)
    XCTAssertEqual(read.tokens.access_token, (expected["tokens"] as? [String: Any])?["access_token"] as? String)
    XCTAssertEqual(read.expiresAt.timeIntervalSinceReferenceDate * 1000 + 978_307_200_000,
                   (expected["expires_at_unix_ms"] as? NSNumber)?.doubleValue)
    let mode = try FileManager.default.attributesOfItem(atPath: state.appendingPathComponent("account-session.json").path)[.posixPermissions] as? NSNumber
    XCTAssertEqual(mode?.intValue, 0o600)
  }

  func testLoadRefusesBuffersTooSmallAndFilesItCannotRead() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    XCTAssertEqual(save(store, account, Data(repeating: 0x41, count: 100)), 0)
    XCTAssertEqual(load(store, account, capacity: 99).0, -1)
    XCTAssertEqual(load(store, account, capacity: 100).0, 100)
    // 别人可读的会话文件不是任何一方写下的，是失败，不是不存在。
    chmod(state.appendingPathComponent("account-session.json").path, 0o644)
    XCTAssertEqual(load(store, account).0, -1)
    // 读不了的匿名会话（这里是一个同名目录）同样是失败。
    try FileManager.default.createDirectory(at: group.appendingPathComponent("anonymous-session.json"), withIntermediateDirectories: true)
    XCTAssertEqual(load(store, anonymous).0, -1)
  }

  func testKeystoreLosesTrailingNewlinesAndIsRemovedOnItsOwn() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    try BackendDesktopSessionFile.writeOwnerOnly(Data("synthetic-keystore\r\n".utf8), named: "cloud-clipboard-keystore", in: state)
    XCTAssertEqual(load(store, keystore).1, Data("synthetic-keystore".utf8))
    try BackendDesktopSessionFile(directory: state).save(syntheticSession())
    // 删除登录会话不连带密钥串：Rust 会再单独删 CLIPBOARD_KEYSTORE。
    XCTAssertEqual(store.remove!(store.context, account), 0)
    XCTAssertEqual(load(store, account).0, 0)
    XCTAssertGreaterThan(load(store, keystore).0, 0)
    XCTAssertEqual(store.remove!(store.context, keystore), 0)
    XCTAssertEqual(load(store, keystore).0, 0)
  }

  func testAnonymousSessionIsTheLocalStoreFileAndIdentityIsWrittenOnlyOnce() throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    let swift = syntheticSession()
    try BackendLocalStore(fileName: "anonymous-session.json", directory: group).save(swift)
    XCTAssertEqual(load(store, anonymous).1, try Data(contentsOf: group.appendingPathComponent("anonymous-session.json")))
    XCTAssertEqual(store.remove!(store.context, anonymous), 0)
    XCTAssertNil(try BackendLocalStore(fileName: "anonymous-session.json", directory: group).load())

    let first = Data(#"{"subject":"msime-syntheticaaaaaaaa","secret":"first"}"#.utf8)
    XCTAssertEqual(save(store, identity, first), 0)
    XCTAssertEqual(save(store, identity, Data(#"{"subject":"msime-syntheticbbbbbbbb","secret":"second"}"#.utf8)), 1)
    XCTAssertEqual(load(store, identity).1, first)
    // Swift 的匿名账号读到的也是第一份。
    let stored = try XCTUnwrap(BackendLocalStore.read("anonymous-account.json", directory: group))
    XCTAssertEqual(stored, first)
  }

  func testKeychainAbsentIsZeroButUnreadableKeychainIsAFailure() throws {
    let keychain = MemoryKeychain()
    let store = try registered(BackendRustSessionStore(layout: keychainLayout(), keychain: keychain.calls, backgroundActivity: nil))
    XCTAssertEqual(load(store, account).0, 0)
    // 未签名的构建（-34018）和锁屏时的钥匙串都不是「已退出」。
    for status in [OSStatus(-34018), errSecInteractionNotAllowed] {
      keychain.copyStatus = status
      XCTAssertEqual(load(store, account).0, -1, "status \(status)")
    }
    keychain.copyStatus = nil

    let swift = try JSONEncoder().encode(syntheticSession())
    XCTAssertEqual(save(store, account, swift), 0)
    XCTAssertEqual(save(store, account, swift + Data(" ".utf8)), 0, "an existing item is updated in place")
    XCTAssertEqual(load(store, account).1, swift + Data(" ".utf8))
    XCTAssertEqual(save(store, keystore, Data("synthetic-keystore".utf8)), 0)
    XCTAssertEqual(save(store, identity, Data("first".utf8)), 0)
    XCTAssertEqual(save(store, identity, Data("second".utf8)), 1)
    XCTAssertEqual(load(store, identity).1, Data("first".utf8))

    XCTAssertEqual(store.remove!(store.context, account), 0)
    XCTAssertEqual(load(store, account).0, 0)
    XCTAssertGreaterThan(load(store, keystore).0, 0)
    // 删除不限定访问组，与 `BackendKeychain.clear` 相同。
    XCTAssertEqual(keychain.deletedGroups, [nil])
    XCTAssertEqual(store.remove!(store.context, account), 0, "absent is success")
    keychain.deleteStatus = errSecInteractionNotAllowed
    XCTAssertEqual(store.remove!(store.context, keystore), -1)
  }

  func testLockExcludesSwiftRefreshLockOnTheSameFileAndAnotherDescriptor() async throws {
    let store = try registered(BackendRustSessionStore(layout: fileLayout(), backgroundActivity: nil))
    var token: UInt64 = 0
    XCTAssertEqual(store.lock!(store.context, account, 20_000, &token), 0)
    XCTAssertNotEqual(token, 0)
    // 同一进程里另一个描述符上的 flock 拿不到：还没迁移的 Swift 代码与 Rust 互相排斥。
    let swiftLock = BackendFileRefreshLock(url: state.appendingPathComponent("account-refresh.lock"), timeout: 0.2)
    do { _ = try await swiftLock.run { true }; XCTFail("Swift took the lock Rust holds") } catch {}
    var second: UInt64 = 0
    XCTAssertEqual(store.lock!(store.context, account, 100, &second), 1, "a second lock times out")
    // 匿名会话是另一个文件，不受影响。
    var other: UInt64 = 0
    XCTAssertEqual(store.lock!(store.context, anonymous, 100, &other), 0)
    store.unlock!(store.context, anonymous, other)
    store.unlock!(store.context, account, token)
    let reacquired = try await swiftLock.run { true }
    XCTAssertTrue(reacquired)

    // 反过来，Swift 持锁期间 Rust 的回调等到超时。
    let holding = Task { try await BackendFileRefreshLock(url: state.appendingPathComponent("account-refresh.lock")).run {
      try await Task.sleep(nanoseconds: 600_000_000); return true
    } }
    try await Task.sleep(nanoseconds: 150_000_000)
    XCTAssertEqual(store.lock!(store.context, account, 100, &token), 1)
    let held = try await holding.value
    XCTAssertTrue(held)
    XCTAssertEqual(store.lock!(store.context, account, 1000, &token), 0)
    store.unlock!(store.context, account, token)
    XCTAssertEqual(store.lock!(store.context, keystore, 100, &token), -1, "only the two session items are locks")
  }

  func testBackgroundActivityCoversEveryHeldLockAndEndsOnTimeout() throws {
    final class Activities: @unchecked Sendable { let lock = NSLock(); var begun = 0; var ended = 0 }
    let activities = Activities()
    let object = BackendRustSessionStore(layout: fileLayout()) { _ in
      activities.lock.lock(); activities.begun += 1; activities.lock.unlock()
      return { activities.lock.lock(); activities.ended += 1; activities.lock.unlock() }
    }
    let store = try registered(object)
    var token: UInt64 = 0
    XCTAssertEqual(store.lock!(store.context, anonymous, 1000, &token), 0)
    XCTAssertEqual(activities.begun, 1); XCTAssertEqual(activities.ended, 0)
    var second: UInt64 = 0
    XCTAssertEqual(store.lock!(store.context, anonymous, 100, &second), 1)
    XCTAssertEqual(activities.begun, 2); XCTAssertEqual(activities.ended, 1)
    store.unlock!(store.context, anonymous, token)
    XCTAssertEqual(activities.ended, 2)
  }
}
