import XCTest

/// 云同步和「设置同步」页经 Rust（#8201 迁移计划 4d）：`msime_client_cloud_sync` 的请求、要写本机时先写 App Group 再提交、提交失败时恢复，以及手动上传、应用的两步。云同步的规则本身由 host-api 的测试覆盖。
@MainActor
final class RustCloudSyncTests: XCTestCase {
  private var root: URL!
  private var feedback: IOSCloudSettings.Feedback!

  override func setUpWithError() throws {
    root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-rust-cloud-sync-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    feedback = IOSCloudSettings.Feedback.current
  }

  override func tearDownWithError() throws {
    if let feedback { IOSCloudSettings.restore(feedback) }
    if let root { try? FileManager.default.removeItem(at: root) }
  }

  private func status(enabled: Bool = true, line: String = "上次同步：刚刚") -> [String: Any] {
    ["enabled": enabled, "status_line": line, "account_id": "user-1", "disabled_on_cloud_cleared": false,
     "sections": ["settings": ["dirty": false], "phrases": ["dirty": true]]]
  }

  private func sync() -> CloudSync { CloudSync(state: CloudSyncState(directory: root), rust: { FakeRust.account }) }

  /// 等 `request` 起的那一轮跑完（它在主线程上的一个任务里）。
  private func settle(_ until: () -> Bool) async throws {
    for _ in 0..<200 where !until() { try await Task.sleep(nanoseconds: 10_000_000) }
  }

  func testARunSendsTheKeyboardSettingsAndCommitsWhatItWrote() async throws {
    let requested = try IOSCloudSettings.native()
    var changed = requested
    changed["soundEnabled"] = !(requested["soundEnabled"] as? Bool ?? true)
    changed["hapticStrength"] = "strong"
    FakeRust.answer { call in
      switch call.operation {
      case "run": return FakeRust.ok(["outcome": "pending_commit", "run_id": "run-1", "host_writes": ["ios_native": changed],
                                      "status": self.status()])
      case "commit": return FakeRust.ok(self.status(line: "已同步"))
      default: return FakeRust.failure("cloud_sync_invalid")
      }
    }
    let outcome = await sync().perform(trigger: "manual", choice: nil)
    XCTAssertEqual(outcome, .finished)
    let run = try XCTUnwrap(FakeRust.calls("run").first)
    XCTAssertEqual(run.entry, "cloud_sync")
    XCTAssertEqual(run.request["session_store"] as? Int, 7)
    XCTAssertEqual(run.request["state_directory"] as? String, root.path)
    XCTAssertEqual(run.request["preferences_directory"] as? String, MetasequoiaInputSessionBridge.sharedStateDirectory)
    XCTAssertEqual(run.action["trigger"] as? String, "manual")
    XCTAssertEqual(run.action["capabilities"] as? [String: Bool], ["settings": true, "skins_library": false, "phrases": true, "dictionary": false])
    let inputs = try XCTUnwrap(run.action["host_inputs"] as? [String: Any])
    XCTAssertEqual((inputs["ios_native"] as? [String: Any])?["inputScheme"] as? String, requested["inputScheme"] as? String)
    XCTAssertNotNil(inputs["settings_locked"] as? Bool)
    // 先写 App Group，再把实际保存下来的值交给 commit。
    XCTAssertEqual(KeyboardFeedbackPreference.soundEnabled, changed["soundEnabled"] as? Bool)
    XCTAssertEqual(KeyboardFeedbackPreference.hapticStrength.rawValue, "strong")
    let commit = try XCTUnwrap(FakeRust.calls("commit").first)
    XCTAssertEqual(commit.action["run_id"] as? String, "run-1")
    let saved = try XCTUnwrap((commit.action["host_results"] as? [String: Any])?["saved_native"] as? [String: Any])
    XCTAssertEqual(saved["hapticStrength"] as? String, "strong")
  }

  func testAFailedCommitPutsTheAppGroupBack() async throws {
    let before = IOSCloudSettings.Feedback.current
    var changed = try IOSCloudSettings.native()
    changed["hapticsEnabled"] = !before.haptics
    FakeRust.answer { call in
      switch call.operation {
      case "run": return FakeRust.ok(["outcome": "pending_commit", "run_id": "run-2", "host_writes": ["ios_native": changed],
                                      "status": self.status()])
      case "commit": return FakeRust.failure("conflict")
      default: return FakeRust.failure("cloud_sync_invalid")
      }
    }
    _ = await sync().perform(trigger: "active", choice: nil)
    XCTAssertEqual(FakeRust.calls("commit").count, 1)
    XCTAssertEqual(IOSCloudSettings.Feedback.current, before, "提交失败时 App Group 回到写之前的值")
  }

  func testACloudClearedPromptOffersToStopSyncing() async throws {
    FakeRust.answer { call in
      switch call.operation {
      case "run" where call.action["choice"] == nil:
        return FakeRust.ok(["outcome": "needs_choice", "reason": "cloud_cleared", "status": self.status()])
      case "run": return FakeRust.ok(["outcome": "disabled", "status": self.status(enabled: false, line: "云端数据已被清空，已关闭同步，本机数据没有改动")])
      default: return FakeRust.failure("cloud_sync_invalid")
      }
    }
    let sync = sync()
    sync.onActive()
    try await settle { sync.asksFirstRun }
    XCTAssertTrue(sync.asksFirstRun)
    XCTAssertTrue(sync.cloudCleared)
    sync.asksFirstRun = false
    sync.choose("disable")
    try await settle { FakeRust.calls("run").count == 2 && !sync.status.enabled }
    let choice = try XCTUnwrap(FakeRust.calls("run").last?.action)
    XCTAssertEqual(choice["trigger"] as? String, "choice")
    XCTAssertEqual(choice["choice"] as? String, "disable")
    XCTAssertEqual(sync.status.statusLine, "云端数据已被清空，已关闭同步，本机数据没有改动")
  }

  func testEnablingNeedsAnAccountAndSigningOutClearsTheProgress() async throws {
    FakeRust.answer { call in
      switch call.operation {
      case "set_enabled": return FakeRust.failure("cloud_sync_signed_out")
      case "sign_out": return FakeRust.ok(self.status(enabled: false, line: "未开启"))
      case "status": return FakeRust.ok(self.status())
      default: return FakeRust.failure("cloud_sync_invalid")
      }
    }
    let sync = sync()
    do { try await sync.setEnabled(true); XCTFail("signed out") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "cloud_sync_signed_out") }
    XCTAssertTrue(FakeRust.calls("run").isEmpty)
    await sync.refresh()
    XCTAssertEqual(sync.status.statusLine, "上次同步：刚刚")
    XCTAssertEqual(sync.status.dirty, [.phrases])
    await sync.signOut()
    XCTAssertFalse(sync.status.enabled)
    XCTAssertEqual(FakeRust.calls("sign_out").first?.request["session_store"] as? Int, 7)
  }

  // MARK: - 设置同步页

  func testManualUploadMergesInRustAndPutsWithTheRevisionAndAccount() async throws {
    let cloud: [String: Any] = ["revision": 5, "settings": ["platform.android.sound": true]]
    let schema = RustSettingsExport.schema
    FakeRust.answer { call in
      switch (call.entry, call.operation) {
      case ("ios_settings_export", _):
        return FakeRust.ok(["settings": ["input.schema": "quanpin"],
                            "merged": ["revision": 5, "settings": ["input.schema": "quanpin", "platform.android.sound": true]]])
      case ("account", "preferences_put"): return FakeRust.ok(["revision": 6, "settings": ["input.schema": "quanpin"]])
      default: return FakeRust.failure("account_invalid")
      }
    }
    let updated = try await SettingsSyncApply.upload(cloud: cloud, schema: schema, userID: "user-1", account: FakeRust.account)
    XCTAssertEqual(updated["revision"] as? Int, 6)
    let export = try XCTUnwrap(FakeRust.calls.first { $0.entry == "ios_settings_export" }?.request)
    XCTAssertEqual((export["cloud"] as? [String: Any])?["revision"] as? Int, 5)
    XCTAssertNotNil(export["ios_native"] as? [String: Any])
    XCTAssertNotNil(export["schema"] as? [String: Any])
    let put = try XCTUnwrap(FakeRust.calls("preferences_put").first?.action)
    XCTAssertEqual(put["revision"] as? Int, 5)
    XCTAssertEqual(put["user_id"] as? String, "user-1")
    XCTAssertEqual((put["document"] as? [String: Any])?["platform.android.sound"] as? Bool, true)
  }

  func testManualApplyWritesTheAppGroupBetweenPlanAndCommitAndRestoresOnFailure() async throws {
    let before = IOSCloudSettings.Feedback.current
    var requested = try IOSCloudSettings.native()
    requested["soundEnabled"] = !before.sound
    var commits = 0
    FakeRust.answer { call in
      guard call.entry == "ios_settings_apply" else { return FakeRust.failure("account_invalid") }
      if call.request["step"] as? String == "plan" { return FakeRust.ok(["ios_native": requested]) }
      commits += 1
      return commits == 1 ? FakeRust.ok(["preferences": [String: Any]()]) : FakeRust.failure("conflict")
    }
    let cloud: [String: Any] = ["revision": 2, "settings": ["platform.ios.sound_enabled": !before.sound]]
    try await SettingsSyncApply.apply(cloud, client: FakeRust.account.client)
    XCTAssertEqual(KeyboardFeedbackPreference.soundEnabled, !before.sound)
    let commit = try XCTUnwrap(FakeRust.calls.last { $0.request["step"] as? String == "commit" }?.request)
    XCTAssertEqual((commit["saved_native"] as? [String: Any])?["soundEnabled"] as? Bool, !before.sound)
    XCTAssertNil(commit["ios_native"], "commit 只带实际保存下来的值")
    XCTAssertNil(commit["schema"], "应用不带字段表，多出的字段 Rust 按请求不合法处理")

    IOSCloudSettings.restore(before)
    do { try await SettingsSyncApply.apply(cloud, client: FakeRust.account.client); XCTFail("conflict") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "conflict") }
    XCTAssertEqual(IOSCloudSettings.Feedback.current, before, "提交失败时 App Group 回到写之前的值")
  }

  func testNativeKeyboardSettingsComeFromTheDocument() throws {
    let document: [String: Any] = ["traditional_chinese_output": true, "global_theme": "night"]
    let native = try IOSCloudSettings.native(document: document)
    XCTAssertEqual(native["traditionalChineseOutput"] as? Bool, true)
    XCTAssertEqual(native["globalTheme"] as? String, "night")
    XCTAssertEqual(native["inlinePreedit"] as? Bool, false)
    XCTAssertEqual(native["hapticsAvailable"] as? Bool, KeyboardFeedbackPreference.hapticsAvailable)
    XCTAssertTrue(native["customKeyboardSkin"] is NSNull, "文档里没有自定义键盘设计时不上传")
  }
}
