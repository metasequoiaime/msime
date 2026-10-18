import Foundation

/// 打字统计的「上传到云端」，对应 Android 的 `home/TypingUpload`：经 Rust 的 `msime_client_account`（`typing_upload_status`、`typing_upload_set`、`typing_upload`，#8201 迁移计划 4c）。开关、上次上传的时间、设备标识、请求体和十分钟的节流都在 Rust 里，读写的是键盘也在写的同一个统计目录（App Group 的 `MSIME`）；同一进程里的打开、关闭和上传由 Rust 互相排开，关掉以后的删除不会和还在路上的上传交错。
///
/// 应用回到前台（`MetasequoiaImeApp`）和打开统计页时各试一次，节流由 Rust 判断；只有登录账号上传，匿名账号和没登录时什么都不发。失败静默，下次再试。
actor TypingUpload {
  static let shared = TypingUpload()

  /// 开关、上次上传成功的时间（还没有时为 nil）和这台设备有没有登录账号。
  struct Status: Equatable, Sendable {
    let enabled: Bool
    let lastUpload: Date?
    let signedIn: Bool
  }

  /// 打开或关闭的结果：新的状态，以及要告诉用户的一句话。
  struct Change: Equatable, Sendable {
    let status: Status
    let message: String
  }

  private let directory: URL?
  private let rust: @Sendable () throws -> BackendRustAccount
  private let now: @Sendable () -> Date

  init(store: TypingStatisticsStore = TypingStatisticsStore(),
       rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() },
       now: @escaping @Sendable () -> Date = { Date() }) {
    self.directory = store.directory
    self.rust = rust
    self.now = now
  }

  /// 回到前台时调用：开着上传、登录着时试一次，不到十分钟由 Rust 跳过。
  func onActive() async {
    _ = try? await call(["operation": "typing_upload", "force": false], dated: true)
  }

  /// 读开关状态，顺带补传一次（与共享设置页挂上时调 `sync` 相同）。补传失败不打扰用户，只读状态；读不出状态时为 nil。
  func sync() async -> Status? {
    if let status = try? await call(["operation": "typing_upload", "force": false], dated: true) { return status }
    return try? await call(["operation": "typing_upload_status"], dated: false)
  }

  /// 打开或关闭上传。打开要求已登录，打开后 Rust 立刻上传最近有字数的至多 7 天；关闭先在本机关掉，再删除云端的数据，删除失败时开关仍是关的。读写本机状态失败时为 nil。
  func setEnabled(_ enabled: Bool) async -> Change? {
    do {
      let status = try await call(["operation": "typing_upload_set", "enabled": enabled], dated: true)
      return Change(status: status, message: enabled ? "已打开上传，最近 7 天的每日计数已传到你的水杉账号。" : "已关闭上传，云端的打字统计已删除。")
    } catch {
      guard let status = try? await call(["operation": "typing_upload_status"], dated: false) else { return nil }
      return Change(status: status, message: Self.message(error, enabling: enabled))
    }
  }

  /// 失败时的说法，与共享设置页的 `uploadErrorMessage` 相同；`enabling` 是这次在打开还是关闭。
  static func message(_ error: Error, enabling: Bool) -> String {
    let code = (error as? BackendRustClient.Failure)?.code
    if code == "account_unauthorized" {
      return enabling ? "先在「我的」页登录水杉账号，再打开上传。" : "已关闭上传。没有登录，云端的数据没有删掉，登录后打开再关闭一次即可删除。"
    }
    if code == "account_rate_limited" && enabling { return "上传过于频繁，过一会儿会自动再传。" }
    return enabling ? "已打开上传，但这次没有传上去，稍后会自动再试。" : "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。"
  }

  /// 发一个上传操作，读回 `{enabled, last_upload_ms, signed_in}`。`dated` 的操作带本机日期（YYYY-MM-DD）和平台。
  private func call(_ action: [String: Any], dated: Bool) async throws -> Status {
    guard let directory else { throw BackendRustClient.Failure(code: "account_storage") }
    var action = action
    action["statistics_directory"] = directory.path
    if dated {
      action["day"] = Self.day(now())
      action["platform"] = "ios"
    }
    let value = try BackendRustAccount.object(try await rust().account(action))
    guard let enabled = value["enabled"] as? Bool, let signedIn = value["signed_in"] as? Bool else {
      throw BackendRustClient.Failure.invalidResponse
    }
    let last = (value["last_upload_ms"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue / 1000) }
    return Status(enabled: enabled, lastUpload: last, signedIn: signedIn)
  }

  /// 本机日历上的日期。
  static func day(_ date: Date, calendar: Calendar = .current) -> String {
    let parts = calendar.dateComponents([.year, .month, .day], from: date)
    return String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
  }
}
