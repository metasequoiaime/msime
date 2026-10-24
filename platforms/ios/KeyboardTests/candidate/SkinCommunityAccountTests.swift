import XCTest

/// 记下按账号取消快照的调用。
private final class CancelRecorder: @unchecked Sendable {
  private let lock = NSLock()
  private var recorded: [String] = []
  func record(_ accountID: String) { lock.lock(); recorded.append(accountID); lock.unlock() }
  var accounts: [String] { lock.lock(); defer { lock.unlock() }; return recorded }
}

/// 一个会变的登录状态：替身 Rust 的 `status` 按它回答，登录、退出类操作改它。
private final class SignedIn: @unchecked Sendable {
  private let lock = NSLock()
  private var value: String?
  init(_ value: String?) { self.value = value }
  var user: String? {
    get { lock.lock(); defer { lock.unlock() }; return value }
    set { lock.lock(); value = newValue; lock.unlock() }
  }
}

/// 「我的」页的登录、资料、退出和注销经 Rust（#8201 迁移计划 4b）：请求的 action、回答的解码，以及 Rust 的登录和退出没有清理钩子以后由 Swift 补上的「取消旧账号排队中的词库快照」。
final class SkinCommunityAccountTests: XCTestCase {
  private func api(_ recorder: CancelRecorder, failing: Bool = false) -> SkinCommunityAPI {
    SkinCommunityAPI(rust: { FakeRust.account }, cancelSnapshot: { accountID in
      recorder.record(accountID)
      if failing { throw DictionarySnapshotQueue.Failure.busy }
    })
  }

  /// 按 `state` 回答 status；登录类操作把状态换成 `loginUser`，退出类操作清掉它，`failures` 里的操作以那个码失败（`signsOut` 时失败前也清掉，像 Rust 断网时先清本机会话那样）。
  private func answer(_ state: SignedIn, loginUser: String = "new-user", failures: [String: String] = [:], signsOut: Bool = false) {
    FakeRust.answer { call in
      let operation = call.operation ?? ""
      switch operation {
      case "status": return FakeRust.ok(["user": state.user.map { FakeRust.user($0) as Any } ?? NSNull()])
      case "apple_challenge":
        return FakeRust.ok(["challengeId": "apple-challenge", "expiresIn": 300, "nonce": "server-nonce"])
      case "apple_login", "login", "google_token_login":
        state.user = loginUser
        return FakeRust.ok(["user": FakeRust.user(loginUser)])
      case "logout", "account_delete", "forget":
        if let code = failures[operation] {
          if signsOut { state.user = nil }
          return FakeRust.failure(code)
        }
        state.user = nil
        return FakeRust.ok(operation == "account_delete" ? ["voice_samples": ["ok": true]] as Any : NSNull())
      case "profile":
        return FakeRust.ok(["user": FakeRust.user("user-1", "旧昵称", email: "u@example.com"), "providers": ["google", "email"]])
      case "rename":
        let name = call.action["display_name"] as? String ?? ""
        return FakeRust.ok(["user": FakeRust.user("user-1", name), "providers": ["apple"]])
      default: return FakeRust.failure("account_invalid")
      }
    }
  }

  func testStartupPreparesAnonymousAccountWhenSignedOut() async {
    FakeRust.replies(["account.ensure_anonymous": FakeRust.ok(NSNull())], signedIn: nil)
    await IOSRustAccount.ensureAnonymous(FakeRust.account)
    XCTAssertEqual(FakeRust.calls.map(\.operation), ["status", "ensure_anonymous"])
  }

  func testStartupLeavesSignedInAccountAlone() async {
    FakeRust.replies([:], signedIn: "synthetic-signed-in-user")
    await IOSRustAccount.ensureAnonymous(FakeRust.account)
    XCTAssertEqual(FakeRust.calls.map(\.operation), ["status"])
  }

  func testStartupDoesNotTreatUnreadableAccountAsSignedOut() async {
    for code in ["account_storage", "rust_client_invalid_response"] {
      FakeRust.replies(["account.status": FakeRust.failure(code)])
      await IOSRustAccount.ensureAnonymous(FakeRust.account)
      XCTAssertEqual(FakeRust.calls.map(\.operation), ["status"], "读取失败不能触发匿名注册：\(code)")
    }
  }

  func testCurrentUserReadsRustStatusAndUnreadableStateIsAnError() async throws {
    let state = SignedIn("user-1")
    answer(state)
    let api = api(CancelRecorder())
    let user = try await api.currentUser()
    XCTAssertEqual(user?.id, "user-1")
    let signedIn = try await api.signedIn()
    XCTAssertTrue(signedIn)
    state.user = nil
    let signedOut = try await api.signedIn()
    XCTAssertFalse(signedOut)

    // 钥匙串锁着、未签名的构建：读不了不能当成已经退出（迁移差异 M41）。
    FakeRust.answer { _ in FakeRust.failure("account_storage") }
    do { _ = try await api.currentUser(); XCTFail("an unreadable session must not read as signed out") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "account_storage") }
  }

  func testAppleChallengeAndLoginGoThroughRust() async throws {
    let state = SignedIn(nil)
    answer(state, loginUser: "apple-user")
    let api = api(CancelRecorder())
    let challenge = try await api.challenge()
    XCTAssertEqual(challenge.challenge_id, "apple-challenge")
    XCTAssertEqual(challenge.nonce, "server-nonce")
    try await api.login(challenge: challenge.challenge_id, identityToken: "identity-token")
    let login = try XCTUnwrap(FakeRust.calls("apple_login").first)
    XCTAssertEqual(login.entry, "account")
    XCTAssertEqual(login.request["session_store"] as? Int, 7)
    XCTAssertEqual(login.action["challenge_id"] as? String, "apple-challenge")
    XCTAssertEqual(login.action["identity_token"] as? String, "identity-token")
    let user = try await api.currentUser()
    XCTAssertEqual(user?.id, "apple-user")
  }

  func testLoginCancelsThePreviousAccountsSnapshotOnlyWhenTheAccountChanges() async throws {
    let recorder = CancelRecorder()
    let state = SignedIn("previous-user")
    answer(state, loginUser: "new-user")
    try await api(recorder).login(challenge: "c", identityToken: "t")
    XCTAssertEqual(recorder.accounts, ["previous-user"])

    // 同一个账号重新登录：排队中的快照还是它的，不取消。
    answer(state, loginUser: "new-user")
    try await api(recorder).login(challenge: "c", identityToken: "t")
    XCTAssertEqual(recorder.accounts, ["previous-user"])

    // 原来没登录：没有要取消的。
    let fresh = CancelRecorder()
    answer(SignedIn(nil))
    try await api(fresh).login(challenge: "c", identityToken: "t")
    XCTAssertEqual(fresh.accounts, [])
  }

  func testFailedSnapshotCancelDoesNotFailTheLogin() async throws {
    let recorder = CancelRecorder()
    let state = SignedIn("previous-user")
    answer(state, loginUser: "new-user")
    try await api(recorder, failing: true).login(challenge: "c", identityToken: "t")
    XCTAssertEqual(recorder.accounts, ["previous-user"])
    XCTAssertEqual(state.user, "new-user")
  }

  func testCodeAndGoogleLoginsShareTheSameCleanup() async throws {
    let recorder = CancelRecorder()
    let state = SignedIn("previous-user")
    answer(state, loginUser: "code-user")
    let cancel: @Sendable (String) throws -> Void = { recorder.record($0) }
    let user = try await SkinCommunityAPI.signIn(FakeRust.account, cancelSnapshot: cancel) { account in
      try await account.account(["operation": "login", "challenge_id": "c", "code": "123456"])
    }
    XCTAssertEqual(user.id, "code-user")
    XCTAssertEqual(recorder.accounts, ["previous-user"])
  }

  func testSignOutDeletionAndExpiredClearCancelTheOldAccountsSnapshot() async throws {
    let operations: [(String, [String: Any], @Sendable (SkinCommunityAPI) async throws -> Void)] = [
      ("logout", ["all": false], { try await $0.logout() }),
      ("logout", ["all": true], { try await $0.logout(all: true) }),
      ("account_delete", [:], { try await $0.logout(deleteAccount: true) }),
      ("forget", [:], { try await $0.clearExpiredLogin() }),
    ]
    for (operation, fields, run) in operations {
      let recorder = CancelRecorder()
      let state = SignedIn("user-1")
      answer(state)
      try await run(api(recorder, failing: true))
      let call = try XCTUnwrap(FakeRust.calls(operation).first, operation)
      for (key, value) in fields { XCTAssertEqual(call.action[key] as? Bool, value as? Bool, operation) }
      XCTAssertNil(state.user, operation)
      XCTAssertEqual(recorder.accounts, ["user-1"], "\(operation) 取消快照失败也照样退出")
    }
  }

  func testOfflineLogoutStillCancelsTheSnapshotAndReportsTheFailure() async throws {
    let recorder = CancelRecorder()
    let state = SignedIn("user-1")
    answer(state, failures: ["logout": "account_unavailable"], signsOut: true)
    do { try await api(recorder).logout(); XCTFail("the server failure must reach the page") }
    catch let failure as BackendRustClient.Failure { XCTAssertEqual(failure.code, "account_unavailable") }
    XCTAssertEqual(recorder.accounts, ["user-1"])
  }

  func testDeletionThatNeedsARecentLoginKeepsTheSessionAndTheSnapshot() async throws {
    let recorder = CancelRecorder()
    let state = SignedIn("user-1")
    answer(state, failures: ["account_delete": "account_recent_login_required"])
    do { try await api(recorder).logout(deleteAccount: true); XCTFail("deletion must fail") }
    catch let failure as BackendRustClient.Failure {
      XCTAssertEqual(failure.code, "account_recent_login_required")
      XCTAssertEqual(failure.localizedDescription, "需要重新登录后再注销：请先退出登录，重新登录后再试。")
    }
    XCTAssertEqual(state.user, "user-1")
    XCTAssertEqual(recorder.accounts, [])
  }

  func testProfileAndRename() async throws {
    answer(SignedIn("user-1"))
    let api = api(CancelRecorder())
    let profile = try await api.profile()
    XCTAssertEqual(profile.user.display_name, "旧昵称")
    XCTAssertEqual(profile.user.email, "u@example.com")
    XCTAssertEqual(profile.providers, ["google", "email"])

    for invalid in ["  ", String(repeating: "字", count: 65), "名字\n换行"] {
      do { _ = try await api.updateProfile(name: invalid); XCTFail("invalid name accepted") }
      catch { XCTAssertTrue(error.localizedDescription.contains("1–64")) }
    }
    XCTAssertTrue(FakeRust.calls("rename").isEmpty, "不合格的昵称不发给 Rust")
    let renamed = try await api.updateProfile(name: " 新昵称 ")
    XCTAssertEqual(renamed.user.display_name, "新昵称")
    XCTAssertEqual(FakeRust.calls("rename").first?.action["display_name"] as? String, "新昵称")
  }

  func testNicknamePresentationUsesRealSharedUnicodeRules() throws {
    let name = try CommunityProfilePolicy.present("\u{feff}\u{3000}合成👩‍💻\u{a0}\u{feff}")
    XCTAssertEqual(name.normalized, "合成👩‍💻")
    XCTAssertTrue(name.valid)
    XCTAssertEqual(name.maximumCharacters, 64)
    for invalid in ["", " \u{feff} ", "合成\n昵称", "合成\u{7f}昵称", "合成\u{85}昵称"] {
      XCTAssertFalse(try CommunityProfilePolicy.present(invalid).valid)
    }
  }

  func testNicknamePresentationCountsScalarsInsteadOfUTF16OrGraphemes() throws {
    for scalar in ["字", "𠮷", "🦌"] {
      XCTAssertTrue(try CommunityProfilePolicy.present(String(repeating: scalar, count: 64)).valid)
      XCTAssertFalse(try CommunityProfilePolicy.present(String(repeating: scalar, count: 65)).valid)
    }
    XCTAssertTrue(try CommunityProfilePolicy.present(String(repeating: "e\u{301}", count: 32)).valid)
    XCTAssertFalse(try CommunityProfilePolicy.present(String(repeating: "e\u{301}", count: 33)).valid)
  }

  func testRenameSubmitsSharedNormalizationAndRejectsOversizedRequests() async throws {
    answer(SignedIn("user-1"))
    let api = api(CancelRecorder())
    let profile = try await api.updateProfile(name: "\u{feff} 合成👩‍💻 \u{feff}")
    XCTAssertEqual(profile.user.display_name, "合成👩‍💻")
    XCTAssertEqual(FakeRust.calls("rename").first?.action["display_name"] as? String, "合成👩‍💻")
    do {
      _ = try await api.updateProfile(name: String(repeating: "字", count: 6_000))
      XCTFail("超出共享接口上限的昵称不能提交")
    } catch {
      XCTAssertTrue(error.localizedDescription.contains("缩短"))
    }
    XCTAssertEqual(FakeRust.calls("rename").count, 1)
  }
}
