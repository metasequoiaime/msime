import XCTest
@testable import MSIMEBackend

final class BackendSyncPolicyTests: XCTestCase {
  private let conflict = BackendAccountClient.Failure(status: 409, code: "revision_conflict")

  func testFirstRunUploadsWhenTheCloudIsEmptyAndAsksOtherwise() {
    XCTAssertEqual(BackendSyncPolicy.firstRun(cloudHasData: false, choice: nil), .upload)
    XCTAssertEqual(BackendSyncPolicy.firstRun(cloudHasData: false, choice: .useCloud), .upload)
    XCTAssertNil(BackendSyncPolicy.firstRun(cloudHasData: true, choice: nil))
    XCTAssertEqual(BackendSyncPolicy.firstRun(cloudHasData: true, choice: .merge), .merge)
    XCTAssertEqual(BackendSyncPolicy.firstRun(cloudHasData: true, choice: .useCloud), .download)
  }

  func testIncrementalModeTable() {
    XCTAssertEqual(BackendSyncPolicy.incremental(cloudChanged: false, localDirty: false), .none)
    XCTAssertEqual(BackendSyncPolicy.incremental(cloudChanged: false, localDirty: true), .upload)
    XCTAssertEqual(BackendSyncPolicy.incremental(cloudChanged: true, localDirty: false), .download)
    XCTAssertEqual(BackendSyncPolicy.incremental(cloudChanged: true, localDirty: true), .merge)
  }

  func testCloudChangedComparesTheCursorWithTheRevision() {
    XCTAssertTrue(BackendSyncPolicy.cloudChanged(cursor: "", revision: 0))
    XCTAssertFalse(BackendSyncPolicy.cloudChanged(cursor: "7", revision: 7))
    XCTAssertTrue(BackendSyncPolicy.cloudChanged(cursor: "7", revision: 8))
  }

  func testDueThrottlesToFiveMinutesAndThirtySecondsWhenDirty() {
    let start = Date(timeIntervalSince1970: 1_000_000)
    XCTAssertTrue(BackendSyncPolicy.due(now: start, lastAttempt: nil, anyDirty: false))
    XCTAssertFalse(BackendSyncPolicy.due(now: start.addingTimeInterval(29), lastAttempt: start, anyDirty: true))
    XCTAssertTrue(BackendSyncPolicy.due(now: start.addingTimeInterval(30), lastAttempt: start, anyDirty: true))
    XCTAssertFalse(BackendSyncPolicy.due(now: start.addingTimeInterval(299), lastAttempt: start, anyDirty: false))
    XCTAssertTrue(BackendSyncPolicy.due(now: start.addingTimeInterval(300), lastAttempt: start, anyDirty: false))
    // A clock moved backwards does not lock sync out.
    XCTAssertTrue(BackendSyncPolicy.due(now: start.addingTimeInterval(-60), lastAttempt: start, anyDirty: false))
  }

  func testPromptIsRepeatedOnlyAfterTheThrottleUnlessForced() {
    let start = Date(timeIntervalSince1970: 1_000_000)
    XCTAssertTrue(BackendSyncPolicy.promptDue(now: start, promptedAt: nil, force: false))
    XCTAssertFalse(BackendSyncPolicy.promptDue(now: start.addingTimeInterval(60), promptedAt: start, force: false))
    XCTAssertTrue(BackendSyncPolicy.promptDue(now: start.addingTimeInterval(60), promptedAt: start, force: true))
    XCTAssertTrue(BackendSyncPolicy.promptDue(now: start.addingTimeInterval(300), promptedAt: start, force: false))
  }

  func testLocalOnlySettingsAreDropped() {
    let settings: [String: Int] = ["input.schema": 1, "platform.ios.incognito": 2, "developer_options.x": 3,
                                   "diagnostic_log": 4, "voice.contribute_audio": 5]
    XCTAssertEqual(BackendSyncPolicy.withoutLocalOnly(settings), ["input.schema": 1])
  }

  func testConflictRefetchesAndMergesUpToTwoRetries() async throws {
    var puts: [(Int, BackendSyncPolicy.Mode)] = []
    var refetches = 0
    let result = try await BackendSyncPolicy.resolvingConflicts(1, mode: .upload, put: { cloud, mode -> String in
      puts.append((cloud, mode))
      if puts.count < 3 { throw self.conflict }
      return "saved \(cloud)"
    }, refetch: {
      refetches += 1
      return 1 + refetches
    })
    XCTAssertEqual(result, "saved 3")
    XCTAssertEqual(puts.map(\.0), [1, 2, 3])
    XCTAssertEqual(puts.map(\.1), [.upload, .merge, .merge])
    XCTAssertEqual(refetches, 2)
  }

  func testConflictBeyondTheRetriesAndOtherFailuresAreThrown() async {
    var puts = 0
    do {
      _ = try await BackendSyncPolicy.resolvingConflicts(0, mode: .merge, put: { _, _ -> Int in
        puts += 1
        throw self.conflict
      }, refetch: { 0 })
      XCTFail("expected the conflict to be thrown")
    } catch {
      XCTAssertTrue(BackendSyncPolicy.isConflict(error))
    }
    XCTAssertEqual(puts, 3)

    puts = 0
    do {
      _ = try await BackendSyncPolicy.resolvingConflicts(0, mode: .merge, put: { _, _ -> Int in
        puts += 1
        throw BackendAccountClient.Failure(status: 409, code: "session_changed")
      }, refetch: { 0 })
      XCTFail("expected the failure to be thrown")
    } catch {
      XCTAssertFalse(BackendSyncPolicy.isConflict(error))
    }
    XCTAssertEqual(puts, 1)
  }

  func testFailureMessagesMatchAndroid() {
    typealias Failure = BackendAccountClient.Failure
    func message(_ error: Error) -> String? { BackendSyncPolicy.failureMessage(error, fallback: "本机失败") }
    XCTAssertNil(message(CancellationError()))
    XCTAssertNil(message(URLError(.cancelled)))
    XCTAssertEqual(message(URLError(.notConnectedToInternet)), "网络不可用，稍后自动重试")
    XCTAssertEqual(message(CocoaError(.fileWriteUnknown)), "本机失败")
    XCTAssertEqual(message(Failure(status: 401)), "登录已失效，请重新登录后再同步")
    XCTAssertEqual(message(Failure(status: 503, code: "service_disabled")), "云同步暂时不可用")
    XCTAssertEqual(message(Failure(status: 400, code: "invalid_phrases")),
                   "常用语超出云端限制（最多 500 条，每条最多 2000 字），常用语没有同步")
    XCTAssertEqual(message(Failure(status: 400, code: "invalid_preference_field")),
                   "有设置项云端不认识，设置没有同步（invalid_preference_field）")
    XCTAssertEqual(message(Failure(status: 403, code: "account_banned")), "账号已被停用，无法同步")
    XCTAssertEqual(message(Failure(status: 429)), "云端暂时繁忙，稍后自动重试（HTTP 429）")
    XCTAssertEqual(message(Failure(status: 0)), "云端暂时繁忙，稍后自动重试")
    XCTAssertEqual(message(Failure(status: 404, code: "not_found")), "同步失败（HTTP 404 not_found）")
    XCTAssertEqual(message(Failure(status: 409)), "同步失败（HTTP 409）")
  }

  func testStatusLineMatchesAndroid() {
    let now = Date(timeIntervalSince1970: 1_700_000_000)
    func line(enabled: Bool = true, error: String? = nil, unheld: Int = 0, synced: Date? = nil) -> String {
      BackendSyncPolicy.statusLine(enabled: enabled, error: error, unheldPhrases: unheld, lastSynced: synced, now: now)
    }
    XCTAssertEqual(line(enabled: false, error: "网络不可用，稍后自动重试"), "云同步已关闭")
    XCTAssertEqual(line(), "云同步已开启 · 尚未同步")
    XCTAssertEqual(line(synced: now.addingTimeInterval(-120)), "已同步 · 最近 2 分钟前")
    XCTAssertEqual(line(unheld: 2, synced: now), "云端有 2 条常用语超出本机上限，未下载但已保留")
    XCTAssertEqual(line(error: "网络不可用，稍后自动重试", unheld: 2), "网络不可用，稍后自动重试")
    XCTAssertEqual(line(error: "", synced: now), "已同步 · 最近 刚刚")
  }

  func testRelativeTimeMatchesAndroid() {
    let now = Date(timeIntervalSince1970: 1_000_000)
    XCTAssertEqual(BackendSyncPolicy.ago(now: now, then: now.addingTimeInterval(-59)), "刚刚")
    XCTAssertEqual(BackendSyncPolicy.ago(now: now, then: now.addingTimeInterval(-5 * 60)), "5 分钟前")
    XCTAssertEqual(BackendSyncPolicy.ago(now: now, then: now.addingTimeInterval(-3 * 3600)), "3 小时前")
    XCTAssertEqual(BackendSyncPolicy.ago(now: now, then: now.addingTimeInterval(-50 * 3600)), "2 天前")
    XCTAssertEqual(BackendSyncPolicy.ago(now: now, then: now.addingTimeInterval(60)), "刚刚")
  }
}
