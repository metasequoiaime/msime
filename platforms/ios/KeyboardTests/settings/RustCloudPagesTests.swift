import XCTest

/// 「我的」页下面经 Rust 的几页（#8201 迁移计划 4c）：云剪贴板、云词库、打字统计上传、QQ 群助手和 AI 对话发出的 action、按 user.id 的绑定和失败时的说法。
final class RustCloudPagesTests: XCTestCase {
  /// 按 `user` 回答 status，其余入口按 `table` 的「入口.操作」回答；`user` 可以在请求之间改。
  private final class Script: @unchecked Sendable {
    private let lock = NSLock()
    private var current: String?
    init(_ user: String?) { current = user }
    var user: String? {
      get { lock.lock(); defer { lock.unlock() }; return current }
      set { lock.lock(); current = newValue; lock.unlock() }
    }
  }

  private func answer(_ script: Script, _ table: [String: (FakeRust.Call) -> String]) {
    FakeRust.answer { call in
      let key = "\(call.entry).\(call.operation ?? "")"
      if key == "account.status" { return FakeRust.ok(["user": script.user.map { FakeRust.user($0) as Any } ?? NSNull()]) }
      return table[key]?(call) ?? FakeRust.failure("rust_client_unavailable")
    }
  }

  private let page: [String: Any] = ["enabled": true, "retention_days": 7, "items": [["id": "a", "text": "合成", "updated_at": "2026-10-18T00:00:00Z"]],
                                     "e2e": ["status": "ready", "code": "1234", "pending_devices": 0, "undecryptable": 0]]
  private let devices: [String: Any] = ["enabled": true, "status": "ready", "device_id": "d", "code": "1234",
                                        "devices": [["device_id": "d", "name": "iPhone", "created_at": "", "approved": true, "current": true, "code": "1234"]]]

  // MARK: - 云剪贴板

  func testClipboardRefreshSendsTheDeviceNameAndReadsDevicesOnlyWhenNeeded() async throws {
    let script = Script("user-1")
    answer(script, ["cloud_clipboard.list": { _ in FakeRust.ok(self.page) },
                    "cloud_clipboard.devices": { _ in FakeRust.ok(self.devices) },
                    "cloud_clipboard.clear": { _ in FakeRust.ok([String: Any]()) },
                    "cloud_clipboard.approve": { _ in FakeRust.ok(self.devices) }])
    let clipboard = IOSCloudClipboard(account: FakeRust.account, deviceName: "小明的 iPhone")
    let loaded = try await clipboard.refresh(boundTo: nil, search: "合成") { send in
      _ = try await send(["operation": "clear"]); return nil
    }
    XCTAssertEqual(loaded.accountID, "user-1")
    XCTAssertEqual(loaded.page.items.count, 1)
    XCTAssertEqual(loaded.devices?.devices.count, 1)
    let calls = FakeRust.calls.filter { $0.entry == "cloud_clipboard" }
    XCTAssertEqual(calls.map(\.operation), ["clear", "list", "devices"])
    XCTAssertTrue(calls.allSatisfy { $0.request["device_name"] as? String == "小明的 iPhone" && $0.request["session_store"] as? Int == 7 })
    XCTAssertEqual(calls[1].action["search"] as? String, "合成")

    // 操作自己带回设备状态时不再读一次。
    answer(script, ["cloud_clipboard.list": { _ in FakeRust.ok(self.page) },
                    "cloud_clipboard.approve": { _ in FakeRust.ok(self.devices) }])
    _ = try await clipboard.refresh(boundTo: "user-1", search: "") { send in
      try BackendRustAccount.decode(BackendAccountClient.ClipboardDevices.self,
                                    try await send(try XCTUnwrap(IOSCloudClipboard.action(.approve(deviceID: "e")))))
    }
    XCTAssertEqual(FakeRust.calls.filter { $0.entry == "cloud_clipboard" }.map(\.operation), ["approve", "list"])
    XCTAssertEqual(FakeRust.calls("approve").first?.action["device_id"] as? String, "e")
  }

  func testClipboardDropsResultsWhenTheAccountChanges() async throws {
    let script = Script("user-2")
    answer(script, ["cloud_clipboard.list": { _ in FakeRust.ok(self.page) }])
    let clipboard = IOSCloudClipboard(account: FakeRust.account, deviceName: "iPhone")
    do {
      _ = try await clipboard.refresh(boundTo: "user-1", search: "") { _ in nil }
      XCTFail("bound to another account")
    } catch { XCTAssertTrue(error is CancellationError, "\(error)") }
    XCTAssertTrue(FakeRust.calls("list").isEmpty, "换了号就不再发请求")

    // 请求期间退出：结果丢掉。
    answer(script, ["cloud_clipboard.list": { _ in script.user = nil; return FakeRust.ok(self.page) },
                    "cloud_clipboard.devices": { _ in FakeRust.ok(self.devices) }])
    do {
      _ = try await clipboard.refresh(boundTo: nil, search: "") { _ in nil }
      XCTFail("signed out during the request")
    } catch { XCTAssertTrue(error is CancellationError, "\(error)") }
  }

  func testClipboardDeviceActions() {
    XCTAssertEqual(IOSCloudClipboard.action(.reject(deviceID: "x"))?["operation"] as? String, "reject")
    XCTAssertEqual(IOSCloudClipboard.action(.remove(deviceID: "x"))?["device_id"] as? String, "x")
    XCTAssertEqual(IOSCloudClipboard.action(.reset)?["operation"] as? String, "reset")
    XCTAssertEqual(IOSCloudClipboard.action(.status)?["operation"] as? String, "devices")
    XCTAssertNil(IOSCloudClipboard.action(.list(search: "")))
  }

  // MARK: - 云词库

  func testCloudDictionaryRequestsCarryTheTransferDirectoryAndCheckTheAccount() async throws {
    let script = Script("user-1")
    answer(script, ["cloud_dictionary.list": { _ in FakeRust.ok(["entries": [], "has_more": false, "offset": 0]) }])
    let dictionary = IOSCloudDictionary(accountID: "user-1", rust: { FakeRust.account })
    _ = try await dictionary.request(["operation": "list", "kind": "pinyin", "offset": 0, "search": ""])
    let call = try XCTUnwrap(FakeRust.calls("list").first)
    XCTAssertEqual(call.request["snapshot_directory"] as? String, try IOSCloudDictionary.transferDirectory().path)
    XCTAssertEqual(call.request["session_store"] as? Int, 7)

    script.user = "user-2"
    do { _ = try await dictionary.request(["operation": "list"]); XCTFail("another account") } catch is CancellationError { }
    XCTAssertEqual(FakeRust.calls("list").count, 1)
  }

  func testSnapshotMetadataNeedsTheFieldsThePagesShow() throws {
    let snapshot = try IOSCloudDictionary.Snapshot(["cloudRevision": 7, "records": 9, "entries": 3, "overlays": 2, "positions": 1,
                                                    "selections": 3, "sha256": "abc", "bytes": 10])
    XCTAssertEqual(snapshot.cloudRevision, 7)
    XCTAssertEqual(snapshot.records, 9)
    XCTAssertThrowsError(try IOSCloudDictionary.Snapshot(["cloudRevision": 7, "sha256": "abc"]))
    XCTAssertThrowsError(try IOSCloudDictionary.Snapshot(["cloudRevision": true, "records": 1, "entries": 1, "overlays": 1,
                                                          "positions": 1, "sha256": "abc"]))
  }

  func testPrivateFilesAreOwnerOnly() throws {
    let file = try IOSCloudDictionary.privateFile("dictionary-pinyin.tsv")
    defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
    let permissions = try FileManager.default.attributesOfItem(atPath: file.deletingLastPathComponent().path)[.posixPermissions] as? NSNumber
    XCTAssertEqual(permissions?.intValue, 0o700)
    XCTAssertEqual(file.lastPathComponent, "dictionary-pinyin.tsv")
  }

  // MARK: - 打字统计上传

  private var statistics: TypingStatisticsStore {
    TypingStatisticsStore(directory: FileManager.default.temporaryDirectory.appendingPathComponent("typing-upload-tests", isDirectory: true))
  }

  private func uploadStatus(_ enabled: Bool) -> String {
    FakeRust.ok(["enabled": enabled, "last_upload_ms": 1_700_000_000_000, "signed_in": true])
  }

  func testTypingUploadSendsTheDirectoryDayAndPlatform() async throws {
    answer(Script("user-1"), ["account.typing_upload": { _ in self.uploadStatus(true) },
                              "account.typing_upload_set": { _ in self.uploadStatus(false) }])
    let calendar = Calendar(identifier: .gregorian)
    let day = try XCTUnwrap(calendar.date(from: DateComponents(year: 2026, month: 3, day: 7, hour: 12)))
    let upload = TypingUpload(store: statistics, rust: { FakeRust.account }, now: { day })
    let status = await upload.sync()
    XCTAssertEqual(status?.enabled, true)
    XCTAssertEqual(status?.lastUpload, Date(timeIntervalSince1970: 1_700_000_000))
    let call = try XCTUnwrap(FakeRust.calls("typing_upload").first?.action)
    XCTAssertEqual(call["statistics_directory"] as? String, statistics.directory?.path)
    XCTAssertEqual(call["day"] as? String, TypingUpload.day(day))
    XCTAssertEqual(call["platform"] as? String, "ios")
    XCTAssertEqual(call["force"] as? Bool, false)

    let change = await upload.setEnabled(false)
    XCTAssertEqual(change?.message, "已关闭上传，云端的打字统计已删除。")
    XCTAssertEqual(FakeRust.calls("typing_upload_set").first?.action["enabled"] as? Bool, false)
  }

  func testTypingUploadFailuresKeepTheSwitchAndExplain() async throws {
    answer(Script(nil), ["account.typing_upload_set": { _ in FakeRust.failure("account_unauthorized") },
                         "account.typing_upload_status": { _ in self.uploadStatus(false) }])
    let upload = TypingUpload(store: statistics, rust: { FakeRust.account })
    let enabling = await upload.setEnabled(true)
    XCTAssertEqual(enabling?.message, "先在「我的」页登录水杉账号，再打开上传。")
    XCTAssertEqual(enabling?.status.enabled, false)
    let disabling = await upload.setEnabled(false)
    XCTAssertEqual(disabling?.message, "已关闭上传。没有登录，云端的数据没有删掉，登录后打开再关闭一次即可删除。")
    XCTAssertEqual(TypingUpload.message(BackendRustClient.Failure(code: "account_rate_limited"), enabling: true), "上传过于频繁，过一会儿会自动再传。")
    XCTAssertEqual(TypingUpload.message(BackendRustClient.Failure(code: "account_network"), enabling: false),
                   "已关闭上传，但云端的数据没有删掉，请联网后打开再关闭一次。")
  }

  func testTypingUploadDayFollowsTheLocalCalendar() throws {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = try XCTUnwrap(TimeZone(identifier: "Asia/Tokyo"))
    let date = Date(timeIntervalSince1970: 1_773_673_200) // 2026-03-16T15:00:00Z
    XCTAssertEqual(TypingUpload.day(date, calendar: calendar), "2026-03-17")
  }

  // MARK: - QQ 群助手

  func testQQLinksNeedASignedInAccountAndGoThroughRust() async throws {
    let script = Script(nil)
    answer(script, ["account.qq_links": { _ in FakeRust.ok(["links": [["id": "12", "created_at": "2026-10-17T03:04:05Z"], ["id": 5]]]) },
                    "account.qq_link_redeem": { _ in FakeRust.ok(["id": "13", "created_at": ""]) },
                    "account.qq_link_delete": { _ in FakeRust.ok(NSNull()) }])
    do { _ = try await QQLinksService.links { FakeRust.account }; XCTFail("signed out") } catch QQLinksService.Failure.signedOut { }
    XCTAssertTrue(FakeRust.calls("qq_links").isEmpty)
    script.user = "user-1"
    let links = try await QQLinksService.links { FakeRust.account }
    XCTAssertEqual(links, [QQLinksService.Link(id: "12", createdAt: "2026-10-17T03:04:05Z")], "格式不对的行跳过")
    try await QQLinksService.redeem(" 1234-5678 ") { FakeRust.account }
    XCTAssertEqual(FakeRust.calls("qq_link_redeem").first?.action["code"] as? String, " 1234-5678 ", "清洗交给 Rust")
    try await QQLinksService.delete("12") { FakeRust.account }
    XCTAssertEqual(FakeRust.calls("qq_link_delete").first?.action["id"] as? String, "12")
  }

  func testQQLinkMessages() {
    XCTAssertEqual(QQLinksService.normalizedCode(" 1234-5678 "), "12345678")
    XCTAssertNil(QQLinksService.normalizedCode("1234567"))
    XCTAssertEqual(QQLinksService.message(BackendRustClient.Failure(code: "account_invalid"), redeeming: true), "绑定码是 8 位数字，请检查后重试。")
    XCTAssertEqual(QQLinksService.message(BackendRustClient.Failure(code: "account_qq_link_limit"), redeeming: true),
                   "这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。")
    XCTAssertEqual(QQLinksService.message(BackendRustClient.Failure(code: "account_rate_limited"), redeeming: true), "尝试次数太多，请一小时后再试。")
    XCTAssertEqual(QQLinksService.message(BackendRustClient.Failure(code: "account_network")), "连不上服务器，请检查网络后再试。")
  }

  // MARK: - AI 对话

  @MainActor
  func testChatStreamsTheReplyWithTheDefaultModel() async throws {
    let script = Script("user-1")
    answer(script, ["account.chat_models": { _ in FakeRust.ok(["data": [["id": "m1"]], "defaultModel": "m1"]) }])
    FakeRust.streaming { _ in FakeRust.Stream(deltas: ["你", "好"], result: FakeRust.ok(["content": "你好"])) }
    let chat = KeyboardChatModel(rust: { FakeRust.account })
    await chat.loadAccount()
    XCTAssertEqual(chat.accountID, "user-1")
    chat.send("在吗")
    for _ in 0..<200 where chat.sending { try await Task.sleep(nanoseconds: 10_000_000) }
    XCTAssertEqual(chat.messages.map(\.role), ["user", "assistant"])
    XCTAssertEqual(chat.messages.last?.text, "你好")
    let call = try XCTUnwrap(FakeRust.calls.first { $0.entry == "chat_stream" })
    XCTAssertEqual(call.request["model"] as? String, "m1")
    XCTAssertEqual(call.request["session_store"] as? Int, 7)
    XCTAssertEqual((call.request["messages"] as? [[String: String]])?.last?["content"], "在吗")
    XCTAssertFalse((call.request["request_id"] as? String ?? "").isEmpty)
    XCTAssertNil(chat.error)
  }

  @MainActor
  func testChatDropsTheReplyWhenTheAccountChanged() async throws {
    let script = Script("user-1")
    answer(script, ["account.chat_models": { _ in FakeRust.ok(["data": [["id": "m1"]], "defaultModel": "m1"]) }])
    FakeRust.streaming { _ in FakeRust.Stream(deltas: ["别人的"], result: FakeRust.failure("account_cancelled")) }
    let chat = KeyboardChatModel(rust: { FakeRust.account })
    await chat.loadAccount()
    chat.send("在吗")
    for _ in 0..<200 where chat.sending { try await Task.sleep(nanoseconds: 10_000_000) }
    XCTAssertEqual(chat.messages.map(\.role), ["user"], "account_cancelled 时已经交出的增量要丢掉")
    XCTAssertEqual(chat.error, "账号已变化，这次回复没有保留。")
  }

  @MainActor
  func testChatRateLimitSaysHowLongToWait() async throws {
    let script = Script("user-1")
    answer(script, ["account.chat_models": { _ in FakeRust.ok(["data": [["id": "m1"]], "defaultModel": "m1"]) }])
    FakeRust.streaming { _ in FakeRust.Stream(deltas: [], result: FakeRust.failure("account_rate_limited", retryAfter: 60)) }
    let chat = KeyboardChatModel(rust: { FakeRust.account })
    await chat.loadAccount()
    chat.send("在吗")
    for _ in 0..<200 where chat.sending { try await Task.sleep(nanoseconds: 10_000_000) }
    XCTAssertEqual(chat.error, "操作过于频繁，请约 1 分钟后再试。")
  }

  @MainActor
  func testSignedOutChatAsksToSignIn() async throws {
    answer(Script(nil), [:])
    let chat = KeyboardChatModel(rust: { FakeRust.account })
    await chat.loadAccount()
    XCTAssertTrue(chat.loginNeeded)
    XCTAssertNil(chat.accountID)
    XCTAssertTrue(FakeRust.calls("chat_models").isEmpty)
  }
}
