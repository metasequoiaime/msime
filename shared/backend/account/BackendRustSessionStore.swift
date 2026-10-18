import Foundation
import Security
#if canImport(Darwin)
import Darwin
#endif
import CMSIMEClient

/// 注入存储里一项的存放位置。
enum BackendRustStoreLocation: Sendable {
  /// 钥匙串的通用密码项。iOS 的登录会话和云剪贴板密钥串在这里，访问组是 App Group，App 与键盘扩展都能读到（与 `BackendKeychain`、`BackendKeychainClipboardKeystore` 是同一项）。
  case keychain(service: String, account: String, accessGroup: String?)
  /// App Group 里由 `BackendLocalStore` 管的文件，读写都持同目录的 `backend-local-store.lock`（匿名会话和身份，与 `BackendAnonymousAccount` 是同一份）。`directory` 为 nil 时是本进程的 App Group 容器。
  case localStore(directory: URL?, fileName: String)
  #if os(macOS)
  /// 仅本人可读写、原子替换、不跟随符号链接的文件，读写规则与 `BackendDesktopSessionFile` 相同（macOS 的 `account-session.json` 和 `cloud-clipboard-keystore`，设置应用的 `FileAccountSessionStorage` 读写同一份）。nil 是没有状态目录，读写都失败。
  case ownerOnlyFile(URL?)
  #endif
}

/// 一个注入存储的四项各放在哪、两把刷新锁各是哪个文件。锁文件必须与尚未迁移的 Swift 用的是同一个，否则两边互相排斥不了，会拿同一枚刷新令牌各刷新一次，服务端随即吊销整个会话。
struct BackendRustStoreLayout: Sendable {
  var account: BackendRustStoreLocation
  var clipboardKeystore: BackendRustStoreLocation
  var anonymous: BackendRustStoreLocation
  var anonymousIdentity: BackendRustStoreLocation
  var accountLock: URL?
  var anonymousLock: URL?

  /// 本平台现有 Swift 实现用的那几个位置。
  static var standard: BackendRustStoreLayout {
    let anonymous = BackendRustStoreLocation.localStore(directory: nil, fileName: BackendAnonymousAccount.sessionFileName)
    let identity = BackendRustStoreLocation.localStore(directory: nil, fileName: BackendAnonymousAccount.identityFileName)
    // 匿名会话两个平台都用 App Group 的 `backend-account-refresh.lock`（`BackendAnonymousAccount.session`）。
    let appGroupLock = BackendFileRefreshLock.appGroup.url
    #if os(macOS)
    // 登录会话是输入法状态目录里的 `account-session.json`，刷新锁是它旁边的 `account-refresh.lock`，与设置应用（Tauri）的 `FileAccountSessionStorage` 相同。
    let directory = BackendDesktopSessionFile.standardDirectory
    return BackendRustStoreLayout(
      account: .ownerOnlyFile(directory?.appendingPathComponent(BackendDesktopSessionFile.fileName, isDirectory: false)),
      clipboardKeystore: .ownerOnlyFile(directory?.appendingPathComponent(BackendDesktopClipboardKeystoreFile.fileName, isDirectory: false)),
      anonymous: anonymous, anonymousIdentity: identity,
      accountLock: BackendDesktopSessionFile.refreshLock.url, anonymousLock: appGroupLock)
    #else
    // iOS 的登录会话在钥匙串里，两个 item 共用 App Group 的同一把锁（`BackendAccountSession.defaultRefreshLock`）：同一进程的两个线程会互相等待，最多 20 秒，与现有 Swift 相同。
    let group = BackendKeychain.defaultAccessGroup
    return BackendRustStoreLayout(
      account: .keychain(service: BackendKeychain.defaultService, account: BackendKeychain.sessionAccount, accessGroup: group),
      clipboardKeystore: .keychain(service: BackendKeychain.defaultService, account: BackendKeychainClipboardKeystore.account,
                                   accessGroup: group),
      anonymous: anonymous, anonymousIdentity: identity,
      accountLock: appGroupLock, anonymousLock: appGroupLock)
    #endif
  }
}

/// 钥匙串的四个调用。测试换成内存里的替身，生产用 Security 框架。
struct BackendRustKeychain: Sendable {
  var copyMatching: @Sendable (_ query: [String: Any]) -> (OSStatus, Data?)
  var update: @Sendable (_ query: [String: Any], _ attributes: [String: Any]) -> OSStatus
  var add: @Sendable (_ attributes: [String: Any]) -> OSStatus
  var delete: @Sendable (_ query: [String: Any]) -> OSStatus

  static let system = BackendRustKeychain(
    copyMatching: { query in
      var result: CFTypeRef?
      let status = SecItemCopyMatching(query as CFDictionary, &result)
      return (status, result as? Data)
    },
    update: { query, attributes in SecItemUpdate(query as CFDictionary, attributes as CFDictionary) },
    add: { attributes in SecItemAdd(attributes as CFDictionary, nil) },
    delete: { query in SecItemDelete(query as CFDictionary) })
}

/// 交给 Rust 的会话存储（`msime_client_session_store`，#8201 迁移计划 4a）：钥匙串、App Group 文件和 flock 由这里提供，编解码、布局、合法性和加锁顺序都在 Rust。注册时布局是 1（Swift 的 `BackendSavedSession`），置 SHARED：App 与键盘扩展、输入法与设置应用都读写同一份，Rust 每次都重读，刷新时持这里的锁。
///
/// 回调可能在任意线程上并发调用，包括 Rust 自己的线程；它们都是同步的，不碰主线程，也不抛错。失败一律返回负数，不当作不存在：钥匙串锁着（`errSecInteractionNotAllowed`）、未签名的构建（-34018）读不了时，Rust 报 `account_storage`，而不是把用户当作已退出。
final class BackendRustSessionStore: @unchecked Sendable {
  /// 宿主在持锁期间保持后台执行的办法：给一个原因，返回结束它的闭包。iOS 上挂起时还持有 App Group 文件锁的进程会被系统杀掉（0xdead10cc），App 里用 `beginBackgroundTask`。
  typealias BackgroundActivity = @Sendable (_ reason: String) -> (@Sendable () -> Void)

  let layout: BackendRustStoreLayout
  private let keychain: BackendRustKeychain
  private let backgroundActivity: BackgroundActivity?
  private let held = NSLock()
  private var nextToken: UInt64 = 0
  private var locks: [UInt64: (descriptor: Int32, end: (@Sendable () -> Void)?)] = [:]

  /// `backgroundActivity` 没有默认值，原因见 `BackendRustAccount.registerShared`。
  init(layout: BackendRustStoreLayout = .standard, keychain: BackendRustKeychain = .system,
       backgroundActivity: BackgroundActivity?) {
    self.layout = layout
    self.keychain = keychain
    self.backgroundActivity = backgroundActivity
  }

  private func location(_ item: UInt32) -> BackendRustStoreLocation? {
    switch item {
    case MSIME_CLIENT_SESSION_ITEM_ACCOUNT.rawValue: return layout.account
    case MSIME_CLIENT_SESSION_ITEM_CLIPBOARD_KEYSTORE.rawValue: return layout.clipboardKeystore
    case MSIME_CLIENT_SESSION_ITEM_ANONYMOUS.rawValue: return layout.anonymous
    case MSIME_CLIENT_SESSION_ITEM_ANONYMOUS_IDENTITY.rawValue: return layout.anonymousIdentity
    default: return nil
    }
  }

  // MARK: - 回调的实现

  /// `load`：>0 是写进 `buffer` 的字节数，0 是不存在，-1 是失败（包括超过 `buffer` 的容量）。
  func load(_ item: UInt32, into buffer: UnsafeMutableRawBufferPointer) -> Int {
    guard let location = location(item), let data = read(location, item: item, capacity: buffer.count) else { return -1 }
    guard let bytes = data else { return 0 }
    // 空的一项不是任何一方写下的会话；说它不存在会让 Rust 当作没登录。
    guard !bytes.isEmpty, bytes.count <= buffer.count else { return -1 }
    bytes.copyBytes(to: buffer)
    return bytes.count
  }

  /// 外层 nil 是失败，内层 nil 是不存在。
  private func read(_ location: BackendRustStoreLocation, item: UInt32, capacity: Int) -> Data?? {
    switch location {
    case let .keychain(service, account, accessGroup):
      var query = Self.query(service: service, account: account, accessGroup: accessGroup)
      query[kSecReturnData as String] = true
      query[kSecMatchLimit as String] = kSecMatchLimitOne
      let (status, data) = keychain.copyMatching(query)
      if status == errSecItemNotFound { return .some(nil) }
      guard status == errSecSuccess, let data else { return nil }
      return .some(Self.trimmed(data, item: item))
    case let .localStore(directory, fileName):
      // 不用 `try?`：它会把「不存在」和「读不了」压成同一个 nil。
      do { return .some(try BackendLocalStore(fileName: fileName, directory: directory).loadData(maximumBytes: capacity)
        .map { Self.trimmed($0, item: item) }) } catch { return nil }
    #if os(macOS)
    case let .ownerOnlyFile(url):
      guard let url else { return nil }
      do { return .some(try BackendDesktopSessionFile.readOwnerOnly(url, maximumBytes: capacity).map { Self.trimmed($0, item: item) }) }
      catch { return nil }
    #endif
    }
  }

  /// 头文件要求密钥串不带换行；Tauri 设置页等别的写入方可能在末尾留一个，去掉它（现有 Swift 读取时同样容忍）。
  private static func trimmed(_ data: Data, item: UInt32) -> Data {
    guard item == MSIME_CLIENT_SESSION_ITEM_CLIPBOARD_KEYSTORE.rawValue else { return data }
    var end = data.endIndex
    while end > data.startIndex, data[data.index(before: end)] == 0x0A || data[data.index(before: end)] == 0x0D {
      end = data.index(before: end)
    }
    return data[data.startIndex..<end]
  }

  /// `save`：0 是已写入；身份已经存在时不覆盖、返回 1；-1 是失败。
  func save(_ item: UInt32, _ bytes: Data) -> Int32 {
    guard let location = location(item) else { return -1 }
    let createOnly = item == MSIME_CLIENT_SESSION_ITEM_ANONYMOUS_IDENTITY.rawValue
    switch location {
    case let .keychain(service, account, accessGroup):
      let query = Self.query(service: service, account: account, accessGroup: accessGroup)
      let attributes: [String: Any] = [kSecValueData as String: bytes,
                                       kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
      let added = keychain.add(query.merging(attributes) { _, new in new })
      if added == errSecSuccess { return 0 }
      guard added == errSecDuplicateItem else { return -1 }
      if createOnly { return 1 }
      return keychain.update(query, attributes) == errSecSuccess ? 0 : -1
    case let .localStore(directory, fileName):
      let store = BackendLocalStore(fileName: fileName, directory: directory)
      do {
        if createOnly { return try store.createIfAbsent(bytes) ? 0 : 1 }
        try store.saveData(bytes)
        return 0
      } catch { return -1 }
    #if os(macOS)
    case let .ownerOnlyFile(url):
      // 这种文件只放登录会话和密钥串，没有「不存在才写」的写法。
      guard let url, !createOnly else { return -1 }
      do {
        try BackendDesktopSessionFile.writeOwnerOnly(bytes, named: url.lastPathComponent, in: url.deletingLastPathComponent())
        return 0
      } catch { return -1 }
    #endif
    }
  }

  /// `remove`：0 是成功，不存在也算成功；-1 是失败。只删这一项：Rust 清登录会话时会分别删 ACCOUNT 和 CLIPBOARD_KEYSTORE。
  func remove(_ item: UInt32) -> Int32 {
    guard let location = location(item) else { return -1 }
    switch location {
    case let .keychain(service, account, _):
      // 与 `BackendKeychain.clear` 相同，匹配本进程持有的每个访问组。
      let status = keychain.delete(Self.query(service: service, account: account, accessGroup: nil))
      return status == errSecSuccess || status == errSecItemNotFound ? 0 : -1
    case let .localStore(directory, fileName):
      do { try BackendLocalStore(fileName: fileName, directory: directory).clear(); return 0 } catch { return -1 }
    #if os(macOS)
    case let .ownerOnlyFile(url):
      guard let url else { return -1 }
      do {
        try BackendDesktopSessionFile.removeOwnerOnly(named: url.lastPathComponent, in: url.deletingLastPathComponent())
        return 0
      } catch { return -1 }
    #endif
    }
  }

  /// `lock`：0 是已持有（令牌写进 `token`），1 是等到 `timeout` 还没拿到，-1 是失败。每次新开一个描述符做 `flock(LOCK_EX)`，所以同一进程里仍用 `BackendFileRefreshLock` 的 Swift 代码也被排斥。轮询等待，不在 `flock` 里无限阻塞。
  func lock(_ item: UInt32, timeout: TimeInterval, token: inout UInt64) -> Int32 {
    let url: URL?
    switch item {
    case MSIME_CLIENT_SESSION_ITEM_ACCOUNT.rawValue: url = layout.accountLock
    case MSIME_CLIENT_SESSION_ITEM_ANONYMOUS.rawValue: url = layout.anonymousLock
    default: return -1
    }
    // 在开始等锁之前就申请后台执行，拿到锁和申请之间没有空档。
    let end = backgroundActivity?("app.msime.account-refresh-lock")
    guard let descriptor = try? BackendFileRefreshLock(url: url).openDescriptor() else { end?(); return -1 }
    let deadline = Date().addingTimeInterval(timeout)
    while flock(descriptor, LOCK_EX | LOCK_NB) != 0 {
      let failure = errno
      guard failure == EWOULDBLOCK || failure == EINTR else { close(descriptor); end?(); return -1 }
      guard Date() < deadline else { close(descriptor); end?(); return 1 }
      usleep(50_000)
    }
    held.lock()
    nextToken += 1
    let issued = nextToken
    locks[issued] = (descriptor, end)
    held.unlock()
    token = issued
    return 0
  }

  /// `unlock`：放开 `lock` 给的那一把。认不出的令牌什么也不做。
  func unlock(_ item: UInt32, token: UInt64) {
    held.lock()
    let entry = locks.removeValue(forKey: token)
    held.unlock()
    guard let entry else { return }
    flock(entry.descriptor, LOCK_UN)
    close(entry.descriptor)
    entry.end?()
  }

  private static func query(service: String, account: String, accessGroup: String?) -> [String: Any] {
    var query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                kSecAttrService as String: service,
                                kSecAttrAccount as String: account]
    if let accessGroup { query[kSecAttrAccessGroup as String] = accessGroup }
    return query
  }

  // MARK: - 注册

  /// 注册到 Rust，成功时是之后请求里 `session_store` 的值。`clientAgent` 是 `msime_client_client_agent` 拼出的 User-Agent，nil 时与不带 client_agent 的目录模式相同。同一份存储在一个进程里只能注册一次：每次注册各有一套进程内的锁和会话缓存。
  func register(with client: BackendRustClient, clientAgent: String?) -> Result<UInt64, BackendRustClient.Failure> {
    let context = Unmanaged.passRetained(self).toOpaque()
    var store = msime_client_session_store()
    store.version = UInt32(MSIME_CLIENT_SESSION_STORE_VERSION)
    store.layout = 1
    store.flags = MSIME_CLIENT_SESSION_STORE_SHARED
    store.context = context
    store.load = { context, item, buffer, capacity in
      guard let context, let buffer else { return -1 }
      return Unmanaged<BackendRustSessionStore>.fromOpaque(context).takeUnretainedValue()
        .load(item, into: UnsafeMutableRawBufferPointer(start: buffer, count: capacity))
    }
    store.save = { context, item, bytes, length in
      guard let context, let bytes else { return -1 }
      return Unmanaged<BackendRustSessionStore>.fromOpaque(context).takeUnretainedValue()
        .save(item, Data(bytes: bytes, count: length))
    }
    store.remove = { context, item in
      guard let context else { return -1 }
      return Unmanaged<BackendRustSessionStore>.fromOpaque(context).takeUnretainedValue().remove(item)
    }
    store.lock = { context, item, timeout, token in
      guard let context, let token else { return -1 }
      return Unmanaged<BackendRustSessionStore>.fromOpaque(context).takeUnretainedValue()
        .lock(item, timeout: TimeInterval(timeout) / 1000, token: &token.pointee)
    }
    store.unlock = { context, item, token in
      guard let context else { return }
      Unmanaged<BackendRustSessionStore>.fromOpaque(context).takeUnretainedValue().unlock(item, token: token)
    }
    // 注册成功时 Rust 接管 context，最后一次使用结束时恰好调用一次 release。
    store.release = { context in
      guard let context else { return }
      Unmanaged<BackendRustSessionStore>.fromOpaque(context).release()
    }
    let result: Result<UInt64, BackendRustClient.Failure>
    if let clientAgent {
      result = clientAgent.withCString { agent in
        store.client_agent = agent
        return client.register(&store)
      }
    } else {
      result = client.register(&store)
    }
    // 注册失败时 Rust 没有接管 context，也不会调用 release；回答读不懂时不知道它接管了没有，宁可漏掉这一次 retain，也不在它可能还在用的时候放掉。
    if case .failure(let failure) = result, failure != .invalidResponse {
      Unmanaged<BackendRustSessionStore>.fromOpaque(context).release()
    }
    return result
  }
}
