import Foundation
import Security
#if canImport(Darwin)
import Darwin
#endif

@_silgen_name("msime_client_cloud_clipboard_source")
private func msimeRelaySource(_ directory: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_relay_cloud_clipboard")
private func msimeRelayAction(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_relay_cloud_clipboard_exchange")
private func msimeRelayExchange(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_cloud_clipboard_webdav_probe")
private func msimeWebDAVProbe(_ request: UnsafePointer<UInt8>?, _ length: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeRelayStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// 经自建中继或 WebDAV 网盘失败。`code` 是共享层给的码（`relay_unauthorized`、`webdav_rollback` 等，见 `msime_client.h`），或宿主自己的 `relay_changed`；说法见 `IOSCloudClipboardRelayPolicy.failureMessage`。
struct IOSCloudClipboardRelayFailure: Error, LocalizedError, Equatable, Sendable {
  let code: String
  var errorDescription: String? { IOSCloudClipboardRelayPolicy.failureMessage(code) }
}

/// 经用户自建的中继（`docs/cloud-clipboard-relay.md`）或 WebDAV 网盘（`docs/cloud-clipboard-webdav.md`）读写云剪贴板，App 的云剪贴板页和键盘扩展共用。做法与 Android 的 `CloudClipboardRelay` 相同：端到端加密的操作（列表、上传、设备）由 Swift 跑与账号路径同一个驱动循环（`BackendAccountClient.clipboardE2E(_:e2e:send:)`），密钥串存在 App Group 访问组的钥匙串里（`IOSCloudClipboardRelayKeystore`），每个请求经 Rust 的 `msime_client_relay_cloud_clipboard_exchange` 发出（Rust 校验地址、限定路径、带上令牌或网盘凭据、限制响应；网盘时在本机扮演中继）。不涉及内容、不碰密钥串的动作（开关、置顶、保留、删除、清空）交给 `msime_client_relay_cloud_clipboard`。不经后者跑端到端加密的操作：那条路由 Rust 把密钥串写成只靠文件权限保护的明文文件。
///
/// 配置是 `open` 那一刻偏好里的那一份，这一页（或键盘这一次读取）之后的请求都发往它；偏好里配置了中继或网盘就只走它（以 `msime_client_cloud_clipboard_source` 为准），出错也不退回水杉账号。每个动作都阻塞在网络上，C 调用放在专用线程上。
struct IOSCloudClipboardRelay: Sendable {
  enum Backend: String, Sendable { case relay, webdav }

  /// 中继密钥串之外放在本机的东西（网盘日志的缓存 `cloud-clipboard-webdav-state` 和锁文件）所在的目录名，在 App Group 容器里，App 和键盘用同一个。
  static let directoryName = "cloud-clipboard-relay"

  let backend: Backend
  let url: String
  let username: String
  let binding: String
  let preferencesDirectory: String
  let directory: URL
  let deviceName: String
  /// 令牌或应用密码；是凭据，不写日志。
  private let secret: String
  private let keystore: IOSCloudClipboardRelayKeystore
  private let drive: @Sendable (Data) -> Data?

  var isWebDAV: Bool { backend == .webdav }

  /// 共享偏好目录：App Group 里的共享状态目录，键盘的 `preferences_directory` 就是它。
  static var preferencesDirectory: String { MetasequoiaInputSessionBridge.sharedStateDirectory }

  /// App Group 容器里的 `cloud-clipboard-relay/`。只放密文和公开数据，不进设备备份（与 Android 的 `noBackupFilesDir` 同样考虑）。
  static var directory: URL {
    IOSSharedPaths.sharedContainerOrApplicationSupport(for: MSIMEAppEdition.appGroupIdentifier)
      .appendingPathComponent(directoryName, isDirectory: true)
  }

  // MARK: - 选路

  /// 偏好文档里中继或网盘的配置：网盘优先，与共享层选路相同（两者同时配置的文档共享层不认，`selected` 会报错）。都没配置时为 nil。不读文件。
  struct Configuration: Equatable, Sendable {
    let backend: Backend
    let url: String
    let username: String
    /// 是否已经存着令牌或应用密码；设置页只看有没有，不回显。
    let hasSecret: Bool
    let binding: String
    fileprivate let secret: String

    init?(preferences: [String: Any]?) {
      if let webdav = preferences?[IOSCloudClipboardRelayPolicy.webdavPreference] as? [String: Any],
         let url = webdav["url"] as? String, !url.isEmpty {
        let username = webdav["username"] as? String ?? ""
        let password = webdav["password"] as? String ?? ""
        self.init(backend: .webdav, url: url, username: username, secret: password,
                  binding: IOSCloudClipboardRelayPolicy.webdavBinding(url: url, username: username))
      } else if let relay = preferences?[IOSCloudClipboardRelayPolicy.relayPreference] as? [String: Any],
                let url = relay["url"] as? String, !url.isEmpty {
        let token = relay["token"] as? String ?? ""
        self.init(backend: .relay, url: url, username: "", secret: token,
                  binding: IOSCloudClipboardRelayPolicy.relayBinding(url: url, token: token))
      } else {
        return nil
      }
    }

    private init(backend: Backend, url: String, username: String, secret: String, binding: String) {
      self.backend = backend; self.url = url; self.username = username; self.secret = secret
      self.binding = binding; hasSecret = !secret.isEmpty
    }

    /// 已存的应用密码；设置页「留空沿用」时拿它去测试连接和保存，不显示。
    var savedPassword: String { backend == .webdav ? secret : "" }
    /// 已存的中继令牌，用途同上。
    var savedToken: String { backend == .relay ? secret : "" }
  }

  /// 偏好文件里现在的配置；没配置时为 nil，偏好读不了时抛出 `storage`。读文件，不要在主线程上调用。
  static func currentConfiguration(preferencesDirectory: String = IOSCloudClipboardRelay.preferencesDirectory) throws -> Configuration? {
    guard !preferencesDirectory.isEmpty,
          let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: URL(fileURLWithPath: preferencesDirectory, isDirectory: true))
    else { throw IOSCloudClipboardRelayFailure(code: "storage") }
    return Configuration(preferences: preferences)
  }

  /// 云剪贴板这时该走中继（或网盘）还是水杉账号，以共享层的 `msime_client_cloud_clipboard_source` 为准。读不了偏好（`storage`）或配置不合规（`relay_not_configured`）时抛出，调用方按失败处理，不改走账号。只读文件、不联网。
  static func selected(preferencesDirectory: String = IOSCloudClipboardRelay.preferencesDirectory) async throws -> Bool {
    let data = Data(preferencesDirectory.utf8)
    guard !data.isEmpty, data.count <= 16_384 else { throw IOSCloudClipboardRelayFailure(code: "storage") }
    let value = try await call(data) { msimeRelaySource($0, $1) }
    guard let object = (try? JSONSerialization.jsonObject(with: value)) as? [String: Any],
          let source = object["source"] as? String else { throw IOSCloudClipboardRelayFailure(code: "invalid_response") }
    switch source {
    case "relay": return true
    case "account": return false
    default: throw IOSCloudClipboardRelayFailure(code: "invalid_response")
    }
  }

  /// 按偏好里现在的配置打开。没有配置时抛出 `relay_not_configured`，偏好读不了时抛出 `storage`。读文件，不要在主线程上调用。
  static func open(preferencesDirectory: String = IOSCloudClipboardRelay.preferencesDirectory, directory: URL = IOSCloudClipboardRelay.directory,
                   deviceName: String, keychain: IOSCloudClipboardRelayKeystore.Keychain = .system,
                   drive: @escaping @Sendable (Data) -> Data? = IOSCloudClipboardDriver.e2e.drive) throws -> IOSCloudClipboardRelay {
    guard let configuration = try currentConfiguration(preferencesDirectory: preferencesDirectory) else {
      throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.notConfigured)
    }
    return IOSCloudClipboardRelay(configuration: configuration, preferencesDirectory: preferencesDirectory,
                                  directory: directory, deviceName: deviceName, keychain: keychain, drive: drive)
  }

  private init(configuration: Configuration, preferencesDirectory: String, directory: URL, deviceName: String,
               keychain: IOSCloudClipboardRelayKeystore.Keychain, drive: @escaping @Sendable (Data) -> Data?) {
    backend = configuration.backend
    url = configuration.url
    username = configuration.username
    secret = configuration.secret
    binding = configuration.binding
    self.preferencesDirectory = preferencesDirectory
    self.directory = directory
    self.deviceName = deviceName
    self.drive = drive
    let space = configuration.binding
    keystore = IOSCloudClipboardRelayKeystore(
      binding: space, lockURL: directory.appendingPathComponent(IOSCloudClipboardRelayKeystore.lockName, isDirectory: false),
      keychain: keychain,
      // 写之前确认偏好里还是这个中继：换了中继，旧中继上的身份不该写进新中继的那一项。
      ensureCurrent: {
        guard (try? IOSCloudClipboardRelay.currentConfiguration(preferencesDirectory: preferencesDirectory))?.binding == space else {
          throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed)
        }
      })
  }

  /// 偏好里的中继还是打开时那一个；换了地址或令牌（网盘是地址或账号）、改回了水杉账号都抛出 `relay_changed`。读文件。
  func requireCurrent() throws {
    guard try Self.currentConfiguration(preferencesDirectory: preferencesDirectory)?.binding == binding else {
      throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.changed)
    }
  }

  // MARK: - 动作

  /// 设置页的一个动作，形状与 `msime_client_relay_cloud_clipboard` 的 action 相同，结果的形状也相同。端到端加密的动作经驱动循环，其余直接交给共享层。
  func request(_ action: [String: Any]) async throws -> Data {
    switch action["operation"] as? String {
    case "list": return try await driven(.list(search: action["search"] as? String ?? ""))
    case "add":
      guard let text = action["text"] as? String else { throw IOSCloudClipboardRelayFailure(code: "invalid_cloud_clipboard") }
      return try await driven(.add(text: text))
    case "devices": return try await driven(.status)
    case "approve", "reject", "remove":
      guard let id = action["device_id"] as? String else { throw IOSCloudClipboardRelayFailure(code: "invalid_cloud_clipboard") }
      switch action["operation"] as? String {
      case "approve": return try await driven(.approve(deviceID: id))
      case "reject": return try await driven(.reject(deviceID: id))
      default: return try await driven(.remove(deviceID: id))
      }
    case "reset": return try await driven(.reset)
    case "set_enabled", "pin", "set_retention", "delete", "clear": return try await direct(action)
    default: throw IOSCloudClipboardRelayFailure(code: "invalid_cloud_clipboard")
    }
  }

  /// 列表，带本机解密后的搜索；与账号路径同样校验。
  func page(search: String = "") async throws -> BackendAccountClient.ClipboardPage {
    try BackendAccountClient.clipboardPage(try await driven(.list(search: search)))
  }

  /// 加密后上传一段文字。本机等待批准时以 `clipboard_device_pending` 失败。
  func add(_ text: String) async throws -> BackendAccountClient.ClipboardItem {
    try BackendAccountClient.validatedClipboardItem(try await driven(.add(text: text)))
  }

  /// 跑一个端到端加密的操作。驱动报的错按 `IOSCloudClipboardRelayPolicy.driverFailureCode` 归类，与 Rust 那条路给出的码相同。
  private func driven(_ operation: BackendAccountClient.ClipboardE2EOperation) async throws -> Data {
    let name = deviceName
    let e2e = BackendClipboardE2E(drive: drive, keystore: keystore, deviceName: { name })
    do {
      return try await BackendAccountClient.clipboardE2E(operation, e2e: e2e) { method, path, body in
        try await exchange(method, path, body)
      }
    } catch let failure as BackendAccountClient.Failure {
      throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.driverFailureCode(status: failure.status, code: failure.code))
    }
  }

  /// 把驱动的一个请求发给打开时的那个中继或网盘，状态码和正文原样交回驱动。请求里有令牌或应用密码，不写日志。网盘时 Rust 在 `directory` 里放日志的本机缓存和锁文件。
  private func exchange(_ method: String, _ path: String, _ body: Data?) async throws -> (status: Int, body: Data) {
    var request: [String: Any] = ["url": url, "method": method, "path": path]
    switch backend {
    case .relay: request["token"] = secret
    case .webdav:
      try Self.prepare(directory)
      request["backend"] = "webdav"
      request["username"] = username
      request["password"] = secret
      request["directory"] = directory.path
    }
    if let body {
      guard let text = String(data: body, encoding: .utf8) else { throw IOSCloudClipboardRelayFailure(code: "invalid_cloud_clipboard") }
      request["body"] = text
    }
    let value = try await Self.call(try Self.encode(request)) { msimeRelayExchange($0, $1) }
    guard let object = (try? JSONSerialization.jsonObject(with: value)) as? [String: Any],
          let status = object["status"] as? NSNumber, CFGetTypeID(status) != CFBooleanGetTypeID(),
          (100...599).contains(status.intValue), let text = object["body"] as? String
    else { throw IOSCloudClipboardRelayFailure(code: "invalid_response") }
    return (status.intValue, Data(text.utf8))
  }

  /// 不涉及内容、不碰密钥串的动作。请求里没有凭据（Rust 从偏好里读）；发之前确认偏好里还是打开时那个中继。
  private func direct(_ action: [String: Any]) async throws -> Data {
    try requireCurrent()
    try Self.prepare(directory)
    let request: [String: Any] = ["preferences_directory": preferencesDirectory, "directory": directory.path,
                                  "device_name": deviceName, "action": action]
    return try await Self.call(try Self.encode(request)) { msimeRelayAction($0, $1) }
  }

  /// WebDAV 设置的「测试连接」：用设置页里还没保存的草稿在那个文件夹里试一遍网盘行为（`msime_client_cloud_clipboard_webdav_probe`）。带上本机目录，通过时 Rust 顺带删掉那里的日志缓存，防回退从网盘现在的样子重新开始。不通过时以共享层的错误码抛出，说法见 `IOSCloudClipboardRelayPolicy.probeMessage`。请求里有应用密码，不写日志。大约二十个请求。
  static func probeWebDAV(url: String, username: String, password: String, directory: URL = IOSCloudClipboardRelay.directory) async throws {
    try prepare(directory)
    let request: [String: Any] = ["url": url, "username": username, "password": password, "directory": directory.path]
    _ = try await call(try encode(request)) { msimeWebDAVProbe($0, $1) }
  }

  // MARK: - 调用

  /// 建好本机目录：只有本应用能进，不进设备备份。
  private static func prepare(_ directory: URL) throws {
    do {
      try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
                                              attributes: [.posixPermissions: 0o700])
    } catch { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
    IOSSharedPaths.excludeFromBackup(directory)
  }

  private static func encode(_ object: [String: Any]) throws -> Data {
    guard JSONSerialization.isValidJSONObject(object), let data = try? JSONSerialization.data(withJSONObject: object),
          data.count <= 131_072 else { throw IOSCloudClipboardRelayFailure(code: "invalid_cloud_clipboard") }
    return data
  }

  /// 在专用线程上调用一个 C 入口，拆开 `{ok,value|error}` 信封；失败以共享层的码抛出。回答只释放一次，原文不带进错误（里面可能有剪贴板内容）。Rust 入口会阻塞几十秒，不放进协作线程池；栈给 2 MiB，与 `BackendRustClient.onDedicatedThread` 相同（键盘扩展不编译那个文件）。
  private static func call(_ request: Data,
                           _ function: @escaping @Sendable (UnsafePointer<UInt8>?, Int) -> UnsafeMutablePointer<CChar>?) async throws -> Data {
    let reply: Data? = await withCheckedContinuation { continuation in
      let thread = Thread {
        let raw = request.withUnsafeBytes { bytes in function(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count) }
        guard let raw else { return continuation.resume(returning: nil) }
        let data = Data(bytes: raw, count: strlen(raw))
        msimeRelayStringFree(raw)
        continuation.resume(returning: data)
      }
      thread.name = "app.msime.cloud-clipboard-relay"
      thread.stackSize = 2 << 20
      thread.start()
    }
    guard let reply, let envelope = (try? JSONSerialization.jsonObject(with: reply)) as? [String: Any],
          let ok = envelope["ok"] as? NSNumber, CFGetTypeID(ok) == CFBooleanGetTypeID() else {
      throw IOSCloudClipboardRelayFailure(code: "invalid_response")
    }
    guard ok.boolValue else {
      let code = envelope["error"] as? String ?? ""
      throw IOSCloudClipboardRelayFailure(code: code.isEmpty ? "invalid_response" : code)
    }
    guard let value = try? JSONSerialization.data(withJSONObject: envelope["value"] ?? NSNull(), options: [.fragmentsAllowed]) else {
      throw IOSCloudClipboardRelayFailure(code: "invalid_response")
    }
    return value
  }
}

/// 自建中继和 WebDAV 网盘的本机密钥串（设备私钥和当前的剪贴板密钥）：钥匙串通用密码项，服务名与账号会话相同（`app.msime.backend.account`），账号名 `cloud-clipboard-relay-keystore`，访问组是 App Group（`BackendKeychain.defaultAccessGroup`），所以 App 和键盘扩展读写同一份，同一台设备在一个中继里只登记一次。可读性是 `AfterFirstUnlockThisDeviceOnly`：不随备份或迁移去别的设备。退出水杉账号不删除它（`BackendKeychain.clear` 只删账号那两项）。
///
/// 内容是 `{binding, keystore}`：`binding` 是这份身份属于哪个中继或网盘（`IOSCloudClipboardRelayPolicy` 的绑定），与打开时不同就读作没有，于是换了中继地址、令牌或网盘账号就生成新的设备身份，写入时直接覆盖。同一个中继里已经存着另一个身份时写入以 `BackendClipboardKeystoreConflict` 拒绝，驱动循环改用存着的那个重来：App 和键盘第一次同时用时只会登记一台设备。比较和写入在 `lockURL` 的 flock 里做，两个进程互相排斥。
///
/// 读不出来（设备锁着、缺少权限）抛出 `relay_storage`，不当作没有，否则会生成新的设备身份，这台设备就得重新批准；内容读得出但解不开（不是这个格式）时身份本来就找不回来了，读作没有。
struct IOSCloudClipboardRelayKeystore: BackendClipboardKeystoreStorage {
  static let account = "cloud-clipboard-relay-keystore"
  static let lockName = "cloud-clipboard-relay-keystore.lock"
  let binding: String
  let lockURL: URL
  var keychain: Keychain = .system
  var service = BackendKeychain.defaultService
  var accessGroup = BackendKeychain.defaultAccessGroup
  /// 写入之前在锁里调用，抛出就不写。
  var ensureCurrent: @Sendable () throws -> Void = {}

  /// 钥匙串的四个调用；测试换成内存里的替身。与 `BackendRustKeychain` 同形，键盘扩展不编译那个文件。
  struct Keychain: Sendable {
    var copyMatching: @Sendable (_ query: [String: Any]) -> (OSStatus, Data?)
    var update: @Sendable (_ query: [String: Any], _ attributes: [String: Any]) -> OSStatus
    var add: @Sendable (_ attributes: [String: Any]) -> OSStatus
    var delete: @Sendable (_ query: [String: Any]) -> OSStatus

    static let system = Keychain(
      copyMatching: { query in
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        return (status, result as? Data)
      },
      update: { query, attributes in SecItemUpdate(query as CFDictionary, attributes as CFDictionary) },
      add: { attributes in SecItemAdd(attributes as CFDictionary, nil) },
      delete: { query in SecItemDelete(query as CFDictionary) })
  }

  private struct Stored: Codable {
    let binding: String
    let keystore: String
  }

  private var query: [String: Any] {
    var query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                kSecAttrService as String: service,
                                kSecAttrAccount as String: Self.account]
    if let accessGroup { query[kSecAttrAccessGroup as String] = accessGroup }
    return query
  }

  func load() throws -> String? {
    try withLock { try read() }.flatMap { $0.binding == binding ? $0.keystore : nil }
  }

  func save(_ value: String) throws {
    guard backendClipboardKeystoreValue(Data(value.utf8)) == value,
          let identity = backendClipboardKeystoreIdentity(value) else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
    try withLock {
      let current = try read()
      if let current, current.binding == binding {
        if current.keystore == value { return }
        if let stored = backendClipboardKeystoreIdentity(current.keystore), stored != identity {
          throw BackendClipboardKeystoreConflict(stored: current.keystore)
        }
      }
      try ensureCurrent()
      try write(Stored(binding: binding, keystore: value))
    }
  }

  func clear() throws {
    let status = keychain.delete(query)
    guard status == errSecSuccess || status == errSecItemNotFound else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
  }

  private func read() throws -> Stored? {
    var query = query
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    let (status, data) = keychain.copyMatching(query)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess, let data else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
    guard data.count <= 4096, let stored = try? JSONDecoder().decode(Stored.self, from: data),
          backendClipboardKeystoreValue(Data(stored.keystore.utf8)) == stored.keystore else { return nil }
    return stored
  }

  private func write(_ stored: Stored) throws {
    guard let data = try? JSONEncoder().encode(stored) else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
    let attributes: [String: Any] = [kSecValueData as String: data,
                                     kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
    var status = keychain.update(query, attributes)
    if status == errSecItemNotFound { status = keychain.add(query.merging(attributes) { _, new in new }) }
    guard status == errSecSuccess else { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
  }

  /// 在 `lockURL` 的 flock 里做 `body`，最多等 5 秒。只包住钥匙串的一读一写，持锁时间很短。
  private func withLock<T>(_ body: () throws -> T) throws -> T {
    let descriptor: Int32
    do { descriptor = try BackendFileRefreshLock(url: lockURL).openDescriptor() }
    catch { throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage) }
    defer { close(descriptor) }
    let deadline = Date().addingTimeInterval(5)
    while flock(descriptor, LOCK_EX | LOCK_NB) != 0 {
      guard errno == EWOULDBLOCK || errno == EINTR, Date() < deadline else {
        throw IOSCloudClipboardRelayFailure(code: IOSCloudClipboardRelayPolicy.storage)
      }
      usleep(20_000)
    }
    defer { flock(descriptor, LOCK_UN) }
    return try body()
  }
}
