import Foundation

/// 云剪贴板端到端加密驱动（`msime_client_cloud_clipboard_e2e`）和它需要的本机状态，由宿主注入。`shared/backend` 不链接 Rust 库，所以驱动以闭包的形式传进来：iOS 的 App 和键盘扩展直接绑定 C 函数，macOS 由输入法进程在加载后端 dylib 后把函数指针交给它。协议见 `docs/cloud-clipboard-e2e.md` 的「客户端驱动」。
struct BackendClipboardE2E: Sendable {
  /// 调用一次驱动：输入一份 JSON，返回 ABI 外层 `{"ok":…}` 的 JSON；拿不到回复时为 nil。
  let drive: @Sendable (Data) -> Data?
  /// 本机的密钥串，驱动每一步交回的新值都在发下一个请求之前写进去。
  let keystore: any BackendClipboardKeystoreStorage
  /// 登记本机时给其他设备看的名字，例如「MacBook Pro」「iPhone」。
  let deviceName: @Sendable () async -> String

  init(drive: @escaping @Sendable (Data) -> Data?, keystore: any BackendClipboardKeystoreStorage,
       deviceName: @escaping @Sendable () async -> String) {
    self.drive = drive; self.keystore = keystore; self.deviceName = deviceName
  }

  private struct SessionBinding: Sendable {
    let session: BackendAccountSession
    let userID: String
    let sessionID: UUID
  }
  private var binding: SessionBinding?

  /// 每次操作固定发起时的登录身份；令牌刷新保留身份，重新登录使旧操作失效。
  func bound(to session: BackendAccountSession, userID: String, sessionID: UUID) -> Self {
    var copy = self
    copy.binding = SessionBinding(session: session, userID: userID, sessionID: sessionID)
    return copy
  }

  func requireSession() async throws {
    try Task.checkCancellation()
    if let binding {
      try await binding.session.requireSession(matchingUserID: binding.userID, matchingSessionID: binding.sessionID)
    }
  }

  func loadKeystore() async throws -> String? {
    guard let binding else { return try keystore.load() }
    return try await binding.session.withSessionStorage(matchingUserID: binding.userID,
                                                        matchingSessionID: binding.sessionID) { try keystore.load() }
  }

  func saveKeystore(_ value: String) async throws {
    guard let binding else { return try keystore.save(value) }
    try await binding.session.withSessionStorage(matchingUserID: binding.userID,
                                                 matchingSessionID: binding.sessionID) { try keystore.save(value) }
  }

  /// 把 C ABI 的驱动函数和释放函数包成 `drive`。回复按 `msime_client_string_free` 的约定释放且只释放一次。
  static func callingCABI(_ function: @escaping (UnsafePointer<UInt8>?, Int) -> UnsafeMutablePointer<CChar>?,
                          free: @escaping (UnsafeMutablePointer<CChar>?) -> Void) -> @Sendable (Data) -> Data? {
    { request in
      let raw = request.withUnsafeBytes { bytes in
        function(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
      }
      guard let raw else { return nil }
      defer { free(raw) }
      return Data(bytes: raw, count: strlen(raw))
    }
  }
}

extension BackendAccountClient {
  /// 一条已解密的条目。`pinned` 和 `device` 由驱动给出；手工构造（测试、旧代码）时两者默认都为 nil。
  struct ClipboardItem: Decodable, Identifiable, Sendable {
    let id: String
    let text: String
    let updated_at: String
    var pinned: Bool? = nil
    /// 写入该条目的设备名；未知时为 nil。
    var device: String? = nil
  }
  /// 端到端加密的状态：`status` 是 `ready`、`pending`（本机等待批准，条目为空）或 `disabled`；`code` 是本机的 16 位核对码；`pending_devices` 是等待本机批准的设备数；`undecryptable` 是解不开而被略去的条目数。
  struct ClipboardE2EStatus: Decodable, Equatable, Sendable {
    let status: String
    let code: String
    let pending_devices: Int
    let undecryptable: Int
  }
  /// `retention_days` 是服务端保留条目的天数（1、7 或 30；0 表示保留到删除为止）。`e2e` 只有驱动给出的列表才有。
  struct ClipboardPage: Decodable, Sendable {
    let enabled: Bool
    let items: [ClipboardItem]
    var retention_days: Int? = nil
    var e2e: ClipboardE2EStatus? = nil
    /// 本机还在等待其他设备批准，读不到也不能上传。
    var isPending: Bool { e2e?.status == "pending" }
  }
  /// 账号下登记的一台设备。`approved` 表示它持有当前的剪贴板密钥，`current` 标出本机，`code` 是它的核对码。
  struct ClipboardDevice: Decodable, Identifiable, Equatable, Sendable {
    let device_id: String
    let name: String
    let created_at: String
    let approved: Bool
    let current: Bool
    let code: String
    var id: String { device_id }
  }
  /// 驱动的 `status`、`approve`、`reject`、`remove`、`reset` 的结果。
  struct ClipboardDevices: Decodable, Sendable {
    let enabled: Bool
    let status: String
    let device_id: String
    let code: String
    let devices: [ClipboardDevice]
    var isPending: Bool { status == "pending" }
    /// 等待本机批准的其他设备；本机自己还没被批准时它批准不了别人，列表为空。
    var awaitingApproval: [ClipboardDevice] {
      status == "ready" ? devices.filter { !$0.current && !$0.approved } : []
    }
  }
  /// 驱动的操作。
  enum ClipboardE2EOperation: Equatable, Sendable {
    case list(search: String)
    case add(text: String)
    case status
    case approve(deviceID: String)
    case reject(deviceID: String)
    case remove(deviceID: String)
    case reset

    var json: [String: Any] {
      switch self {
      case .list(let search): return ["op": "list", "search": search]
      case .add(let text): return ["op": "add", "text": text]
      case .status: return ["op": "status"]
      case .approve(let id): return ["op": "approve", "device_id": id]
      case .reject(let id): return ["op": "reject", "device_id": id]
      case .remove(let id): return ["op": "remove", "device_id": id]
      case .reset: return ["op": "reset"]
      }
    }
    fileprivate var isValid: Bool {
      switch self {
      case .list(let search): return BackendAccountClient.validClipboardSearch(search)
      case .add(let text): return BackendAccountClient.validClipboardText(text)
      case .status, .reset: return true
      case .approve(let id), .reject(let id), .remove(let id): return BackendAccountClient.validClipboardDeviceID(id)
      }
    }
  }
  /// 服务端接受的保留天数选项；0 表示保留到删除为止。
  static let clipboardRetentionDays = [1, 7, 30, 0]
  /// 驱动请求的响应上限。列表最多 50 条、每条密文不超过 16384 个字符，约 850 KiB；交回驱动时的输入上限是 2 MiB。
  static let clipboardResponseLimit = 1536 * 1024
  /// 一次操作最多经过的驱动步数。驱动自己限定了换钥的轮数，正常情况下不超过十步。
  static let clipboardDriverSteps = 32

  /// 用驱动完成一次操作，返回驱动 `done` 的 JSON。驱动要求的每个请求都带 `token` 发出，不论状态码都把状态码和正文交回驱动；传输失败照常抛错，不交给驱动。驱动给出的密钥串一变就先保存，再发下一个请求。驱动的错误抛成 `Failure`，保留它的 `status` 和 `code`：401 因此会触发调用方原有的刷新令牌后重试整个操作。
  func clipboardE2E(_ operation: ClipboardE2EOperation, token: String, e2e: BackendClipboardE2E?) async throws -> Data {
    guard operation.isValid else {
      if case .add = operation { throw Failure(status: 400, code: "invalid_clipboard_text") }
      throw Failure(status: 400)
    }
    guard let e2e else { throw Failure(status: 0) }
    var saved = try await e2e.loadKeystore()
    var input: [String: Any] = ["start": operation.json, "keystore": saved ?? NSNull(),
                                "device_name": await e2e.deviceName()]
    for _ in 0..<Self.clipboardDriverSteps {
      try await e2e.requireSession()
      let step = try Self.clipboardDriverStep(e2e, input)
      if let value = step["keystore"] {
        guard let keystore = value as? String else { throw Failure(status: 0) }
        if keystore != saved {
          try await e2e.saveKeystore(keystore)
          saved = keystore
        }
      }
      if let request = step["request"] {
        guard let request = request as? [String: Any], let state = step["state"],
              let method = request["method"] as? String, ["GET", "PUT", "POST", "DELETE"].contains(method),
              let path = request["path"] as? String, Self.isClipboardPath(path) else { throw Failure(status: 0) }
        let body: Data?
        switch request["body"] {
        case nil, is NSNull: body = nil
        case let text as String: body = Data(text.utf8)
        default: throw Failure(status: 0)
        }
        try await e2e.requireSession()
        let response = try await exchange(method, path, token: token, body: body,
                                          maximumResponseBytes: Self.clipboardResponseLimit)
        let text: String
        if let decoded = String(data: response.body, encoding: .utf8) { text = decoded }
        else if (200..<300).contains(response.status) { throw Failure(status: 0) }
        else { text = "" }
        input = ["resume": state, "response": ["status": response.status, "body": text]]
        continue
      }
      if let done = step["done"] {
        guard JSONSerialization.isValidJSONObject(done) else { throw Failure(status: 0) }
        return try JSONSerialization.data(withJSONObject: done)
      }
      if let error = step["error"] as? [String: Any] {
        let status = (error["status"] as? NSNumber)?.intValue ?? 0
        let code = (error["code"] as? String).flatMap { Self.validErrorCode($0) ? $0 : nil }
        throw Failure(status: (0...599).contains(status) ? status : 0, code: code)
      }
      throw Failure(status: 0)
    }
    throw Failure(status: 0)
  }

  /// 列表，带客户端搜索。本机等待批准时 `items` 为空、`isPending` 为真。
  func clipboard(token: String, search: String = "", e2e: BackendClipboardE2E?) async throws -> ClipboardPage {
    let data = try await clipboardE2E(.list(search: search), token: token, e2e: e2e)
    return try Self.validatedClipboardPage(data)
  }

  /// 加密后上传一段文字。本机等待批准时失败，错误码 `clipboard_device_pending`。
  func addClipboard(_ text: String, token: String, e2e: BackendClipboardE2E?) async throws -> ClipboardItem {
    let data = try await clipboardE2E(.add(text: text), token: token, e2e: e2e)
    guard let item = try? JSONDecoder().decode(ClipboardItem.self, from: data),
          Self.validClipboardItem(item) else { throw Failure(status: 0) }
    return Self.normalized(item)
  }

  private static func clipboardDriverStep(_ e2e: BackendClipboardE2E, _ input: [String: Any]) throws -> [String: Any] {
    guard JSONSerialization.isValidJSONObject(input),
          let request = try? JSONSerialization.data(withJSONObject: input),
          let reply = e2e.drive(request),
          let root = try? JSONSerialization.jsonObject(with: reply) as? [String: Any],
          root["ok"] as? Bool == true,
          let value = root["value"] as? [String: Any] else { throw Failure(status: 0) }
    return value
  }

  /// 驱动只会要求云剪贴板自己的接口；别的路径一律不发，免得一个出错的驱动拿账号凭据去访问别处。
  private static func isClipboardPath(_ path: String) -> Bool {
    let base = "/v1/users/me/clipboard"
    return path == base || path.hasPrefix(base + "/") || path.hasPrefix(base + "?")
  }

  private static func validErrorCode(_ code: String) -> Bool {
    (1...64).contains(code.utf8.count)
      && code.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || $0 == 95 }
  }

  private static func validatedClipboardPage(_ data: Data) throws -> ClipboardPage {
    guard let page = try? JSONDecoder().decode(ClipboardPage.self, from: data),
          page.items.count <= 50,
          page.items.allSatisfy(validClipboardItem),
          page.e2e.map({ ["ready", "pending", "disabled"].contains($0.status) && validClipboardCode($0.code)
            && $0.pending_devices >= 0 && $0.undecryptable >= 0 }) ?? true else { throw Failure(status: 0) }
    // 较新的可选字段解析不了时降级处理，而不是让整页失败，和 Android 客户端一样：未知的保留天数当作缺省，空的或不可用的设备名直接丢弃。
    let retention = page.retention_days.flatMap { clipboardRetentionDays.contains($0) ? $0 : nil }
    return ClipboardPage(enabled: page.enabled, items: page.items.map(normalized), retention_days: retention, e2e: page.e2e)
  }

  private static func normalized(_ item: ClipboardItem) -> ClipboardItem {
    var item = item
    if let device = item.device, device.isEmpty || !validClipboardDevice(device) { item.device = nil }
    return item
  }

  fileprivate static func validClipboardText(_ text: String) -> Bool {
    !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
      && text.utf16.count <= 4000
      && !text.unicodeScalars.contains { scalar in
        scalar.properties.generalCategory == .control && ![9, 10, 13].contains(scalar.value)
      }
  }

  fileprivate static func validClipboardSearch(_ search: String) -> Bool {
    search.utf8.count <= 1024
      && !search.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }

  private static func validClipboardID(_ id: String) -> Bool {
    id.utf8.count == 64 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }

  /// 设备编号：32 个小写十六进制字符。
  static func validClipboardDeviceID(_ id: String) -> Bool {
    id.utf8.count == 32 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }

  /// 核对码：16 位数字，每 4 位一组用空格分开。
  private static func validClipboardCode(_ code: String) -> Bool {
    let groups = code.split(separator: " ", omittingEmptySubsequences: false)
    return groups.count == 4 && groups.allSatisfy { $0.utf8.count == 4 && $0.utf8.allSatisfy { (48...57).contains($0) } }
  }

  private static func validClipboardDevice(_ device: String) -> Bool {
    device.utf8.count <= 128 && !device.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }

  private static func validClipboardItem(_ item: ClipboardItem) -> Bool {
    validClipboardID(item.id)
      && validClipboardText(item.text)
      && !item.updated_at.isEmpty
      && item.updated_at.utf8.count <= 128
      && !item.updated_at.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
}
