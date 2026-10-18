import Foundation
import CMSIMEClient

/// 经宿主交来的函数表（`shared/backend/rust/MSIMERustClient.h` 的 `msime_rust_client`）调用 Rust 的账号与云端入口。macOS 的输入法可执行文件在 `dlopen` 后端 dylib 以后经 `MSIMEInstallRustClient` 交来，iOS App 启动时用链进来的 C 函数填好；没有交来时每个入口都报 `rust_client_unavailable`。
///
/// 这些入口会阻塞网络、读写文件、等跨进程锁（最长 20 秒加一次 30 秒的 HTTP，排在别的线程后面时还会累加），所以异步调用都放在一条专用线程上，不占 Swift 的协作线程池，也不在主线程上。
struct BackendRustClient: @unchecked Sendable {
  /// Rust 入口给出的失败。
  struct Failure: Error, Sendable, Equatable {
    /// 错误码，例如 `account_unauthorized`。宿主没交函数表、表里没有这个入口时是 `rust_client_unavailable`；回答不是 `{ok,value|error}` 信封时是 `rust_client_invalid_response`；请求不能编成 JSON 时是 `rust_client_invalid_request`。
    let code: String
    /// 失败回答里的 `retry_after_seconds`（限流、服务繁忙），没有时为 nil。
    let retryAfterSeconds: Int?
    /// 整个失败回答的原文，调用方要别的字段（例如 `status`）时自己解；本地造出的失败是空的。
    let response: Data

    init(code: String, retryAfterSeconds: Int? = nil, response: Data = Data()) {
      self.code = code; self.retryAfterSeconds = retryAfterSeconds; self.response = response
    }
    static let unavailable = Failure(code: "rust_client_unavailable")
    static let invalidResponse = Failure(code: "rust_client_invalid_response")
    static let invalidRequest = Failure(code: "rust_client_invalid_request")
  }

  /// 只收一段 JSON、回一段 JSON 的入口。
  enum Entry: Sendable {
    case clientAgent, account, accountChatCancel, accountGloss, accountCloudClipboard, cloudClipboardE2E
    case community, cloudDictionary, aiSkinCancel, aiSkinJob, cloudSync
    case accountSettingsExport, accountSettingsApply, accountIOSSettingsExport, accountIOSSettingsApply
  }

  let table: msime_rust_client

  /// 版本、大小不对，或者缺了必需的三个函数（`string_free`、`session_store_register`、`session_store_unregister`）时为 nil。
  init?(table: msime_rust_client) {
    // 宿主填的是 C 的 sizeof，含结尾的填充，对应 Swift 的 stride；size 不含结尾填充，结构体末尾一旦有填充两边就对不上。
    guard table.version == MSIME_RUST_CLIENT_VERSION, Int(table.size) == MemoryLayout<msime_rust_client>.stride,
          table.string_free != nil, table.session_store_register != nil, table.session_store_unregister != nil else { return nil }
    self.table = table
  }

  private final class Installed: @unchecked Sendable {
    let lock = NSLock()
    var client: BackendRustClient?
  }
  private static let installed = Installed()

  /// 本进程宿主交来的函数表；还没交来时为 nil。
  static var shared: BackendRustClient? {
    installed.lock.lock(); defer { installed.lock.unlock() }
    return installed.client
  }

  /// 收下宿主的函数表，之后的 `shared` 都用它。表不合格时不收、返回 false，已经收下的那张不变。
  @discardableResult static func install(_ table: msime_rust_client) -> Bool {
    guard let client = BackendRustClient(table: table) else { return false }
    installed.lock.lock(); defer { installed.lock.unlock() }
    installed.client = client
    return true
  }

  private func function(_ entry: Entry) -> msime_rust_client_json_entry? {
    switch entry {
    case .clientAgent: return table.client_agent
    case .account: return table.account
    case .accountChatCancel: return table.account_chat_cancel
    case .accountGloss: return table.account_gloss
    case .accountCloudClipboard: return table.account_cloud_clipboard
    case .cloudClipboardE2E: return table.cloud_clipboard_e2e
    case .community: return table.community
    case .cloudDictionary: return table.cloud_dictionary
    case .aiSkinCancel: return table.ai_skin_cancel
    case .aiSkinJob: return table.ai_skin_job
    case .cloudSync: return table.cloud_sync
    case .accountSettingsExport: return table.account_settings_export
    case .accountSettingsApply: return table.account_settings_apply
    case .accountIOSSettingsExport: return table.account_ios_settings_export
    case .accountIOSSettingsApply: return table.account_ios_settings_apply
    }
  }

  /// 把字典编成请求。`JSONSerialization` 遇到编不了的对象会抛 Objective-C 异常，所以先查。
  static func request(_ object: [String: Any]) throws -> Data {
    guard JSONSerialization.isValidJSONObject(object),
          let data = try? JSONSerialization.data(withJSONObject: object) else { throw Failure.invalidRequest }
    return data
  }

  /// 在当前线程上同步调用，成功时是 `value` 的 JSON（可能是 `null`、数字这类片段）。会阻塞，只给已经在工作线程上的调用方用。
  func callBlocking(_ entry: Entry, _ request: Data) -> Result<Data, Failure> {
    guard let function = function(entry) else { return .failure(.unavailable) }
    return take(request.withUnsafeBytes { bytes in
      function(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
    })
  }

  /// 放在专用线程上调用，成功时是 `value` 的 JSON。
  func call(_ entry: Entry, _ request: Data) async throws -> Data {
    try await Self.onDedicatedThread { callBlocking(entry, request) }.get()
  }

  func call(_ entry: Entry, _ object: [String: Any]) async throws -> Data {
    try await call(entry, Self.request(object))
  }

  /// 收下 Rust 给的字符串、恰好释放一次，再拆开信封。
  func take(_ raw: UnsafeMutablePointer<CChar>?) -> Result<Data, Failure> {
    guard let raw else { return .failure(.invalidResponse) }
    let data = Data(bytes: raw, count: strlen(raw))
    table.string_free?(raw)
    return Self.envelope(data)
  }

  /// `{"ok":true,"value":...}` 的 `value`，或 `{"ok":false,"error":"..."}` 的失败。
  static func envelope(_ data: Data) -> Result<Data, Failure> {
    guard let object = (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) as? [String: Any],
          let ok = object["ok"] as? NSNumber, CFGetTypeID(ok) == CFBooleanGetTypeID() else { return .failure(.invalidResponse) }
    guard ok.boolValue else {
      guard let code = object["error"] as? String, !code.isEmpty else { return .failure(.invalidResponse) }
      let retry = (object["retry_after_seconds"] as? NSNumber).map(\.intValue)
      return .failure(Failure(code: code, retryAfterSeconds: retry, response: data))
    }
    guard let value = try? JSONSerialization.data(withJSONObject: object["value"] ?? NSNull(), options: [.fragmentsAllowed]) else {
      return .failure(.invalidResponse)
    }
    return .success(value)
  }

  /// 在一条新起的线程上跑 `body`。Rust 入口会阻塞几十秒，放进协作线程池会饿死别的任务；栈给 2 MiB，与 Rust 自己起的线程相同。
  static func onDedicatedThread<T: Sendable>(_ body: @escaping @Sendable () -> T) async -> T {
    await withCheckedContinuation { continuation in
      let thread = Thread { continuation.resume(returning: body()) }
      thread.name = "app.msime.rust-client"
      thread.stackSize = 2 << 20
      thread.start()
    }
  }

  // MARK: - 流式对话与 AI 皮肤

  private final class ChatSink: @unchecked Sendable {
    let continuation: AsyncThrowingStream<String, Error>.Continuation
    init(_ continuation: AsyncThrowingStream<String, Error>.Continuation) { self.continuation = continuation }
  }

  /// 流式对话（`msime_client_account_chat_stream`）：每到一段增量就交出一段，全部连起来就是整段回复；失败时以 `Failure` 结束，已经交出的增量不收回，`account_cancelled` 表示回答期间退出或换了号，调用方要丢掉已经交出的内容。`request` 里的 `request_id` 要与 `requestID` 相同；消费方不再读（任务取消）时调 `msime_client_account_chat_cancel`。
  func chatStream(requestID: String, request: Data) -> AsyncThrowingStream<String, Error> {
    AsyncThrowingStream { continuation in
      guard let stream = table.account_chat_stream else {
        continuation.finish(throwing: Failure.unavailable)
        return
      }
      let sink = Unmanaged.passRetained(ChatSink(continuation)).toOpaque()
      continuation.onTermination = { termination in
        if case .cancelled = termination { cancel(.accountChatCancel, requestID: requestID) }
      }
      let thread = Thread {
        let raw = request.withUnsafeBytes { bytes in
          stream(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, { context, text, length in
            guard let context, let text else { return }
            let sink = Unmanaged<ChatSink>.fromOpaque(context).takeUnretainedValue()
            sink.continuation.yield(String(decoding: UnsafeBufferPointer(start: text, count: length), as: UTF8.self))
          }, sink)
        }
        // 本函数返回以后 Rust 不再调用 on_delta，这时才放掉 sink。
        Unmanaged<ChatSink>.fromOpaque(sink).release()
        switch take(raw) {
        case .success: continuation.finish()
        case .failure(let failure): continuation.finish(throwing: failure)
        }
      }
      thread.name = "app.msime.rust-client.chat"
      thread.stackSize = 2 << 20
      thread.start()
    }
  }

  private final class ProgressSink: @unchecked Sendable {
    let progress: @Sendable (UInt32) -> Void
    init(_ progress: @escaping @Sendable (UInt32) -> Void) { self.progress = progress }
  }

  /// 标记一次生成是否已被取消，让还没开始的生成直接不发。
  private final class CancelFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    var isSet: Bool { lock.lock(); defer { lock.unlock() }; return value }
    func set() { lock.lock(); value = true; lock.unlock() }
  }

  /// AI 键盘皮肤（`msime_client_ai_skin_generate`），成功时是三套方案的 JSON。`progress` 在生成线程上按 1 到 3 依次调用，不能重入 `msime_client_*`；任务取消时调 `msime_client_ai_skin_cancel`，生成随后以 `ai_skin_cancelled` 结束。
  func aiSkinGenerate(requestID: String, request: Data,
                      progress: @escaping @Sendable (UInt32) -> Void = { _ in }) async throws -> Data {
    guard let generate = table.ai_skin_generate else { throw Failure.unavailable }
    let cancelled = CancelFlag()
    return try await withTaskCancellationHandler {
      try await Self.onDedicatedThread { () -> Result<Data, Failure> in
        // 已经取消的不必再调 Rust。在这之后、Rust 登记 id 之前到达的取消由 Rust 记下，登记时直接按已取消开始。
        if cancelled.isSet { return .failure(Failure(code: "ai_skin_cancelled")) }
        let sink = Unmanaged.passRetained(ProgressSink(progress)).toOpaque()
        defer { Unmanaged<ProgressSink>.fromOpaque(sink).release() }
        return take(request.withUnsafeBytes { bytes in
          generate(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, { context, completed in
            guard let context else { return }
            Unmanaged<ProgressSink>.fromOpaque(context).takeUnretainedValue().progress(completed)
          }, sink)
        })
      }.get()
    } onCancel: {
      cancelled.set()
      cancel(.aiSkinCancel, requestID: requestID)
    }
  }

  /// 取消一次流式对话或 AI 皮肤生成。两个取消入口都在任意线程上立即返回。
  private func cancel(_ entry: Entry, requestID: String) {
    guard let request = try? Self.request(["request_id": requestID]) else { return }
    _ = callBlocking(entry, request)
  }

  // MARK: - 会话存储

  /// `msime_client_session_store_register`：成功时是存储的 id。失败时 Rust 没有接管 `store.context`，调用方要自己放掉它。
  func register(_ store: UnsafePointer<msime_client_session_store>) -> Result<UInt64, Failure> {
    guard let register = table.session_store_register else { return .failure(.unavailable) }
    return take(register(store)).flatMap { value in
      guard let object = (try? JSONSerialization.jsonObject(with: value)) as? [String: Any],
            let id = object["session_store"] as? NSNumber, id.int64Value > 0 else { return .failure(.invalidResponse) }
      return .success(id.uint64Value)
    }
  }

  /// `msime_client_session_store_unregister`：成功时是这个 id 是否曾经注册。
  func unregister(_ id: UInt64) -> Result<Bool, Failure> {
    guard let unregister = table.session_store_unregister else { return .failure(.unavailable) }
    return take(unregister(id)).flatMap { value in
      guard let registered = (try? JSONSerialization.jsonObject(with: value, options: [.fragmentsAllowed])) as? Bool else {
        return .failure(.invalidResponse)
      }
      return .success(registered)
    }
  }
}
