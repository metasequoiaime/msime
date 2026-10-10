import Foundation

@MainActor final class SyntheticClipboardAPI: DesktopCloudClipboardAPI {
  var calls = 0
  var uploaded = ""
  var enabled = true
  func clipboard(token: String, search: String) async throws -> BackendAccountClient.ClipboardPage {
    calls += 1
    return .init(enabled: enabled, items: [.init(id: String(repeating: "a", count: 64), text: "synthetic\n\t合成", updated_at: "synthetic-time")])
  }
  func addClipboard(_ text: String, token: String) async throws -> BackendAccountClient.ClipboardItem {
    calls += 1; uploaded = text
    return .init(id: String(repeating: "a", count: 64), text: text, updated_at: "synthetic-time")
  }
  func deleteClipboard(id: String?, token: String) async throws { calls += 1; assert(id?.count == 64) }
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws { calls += 1; assert(!enabled) }
}

@MainActor final class RetryingClipboardAPI: DesktopCloudClipboardAPI {
  var calls = 0
  var rejectEveryCall = false
  func clipboard(token: String, search: String) async throws -> BackendAccountClient.ClipboardPage {
    calls += 1
    if calls == 1 || rejectEveryCall { throw BackendAccountClient.Failure(status: 401) }
    assert(token == "fresh-token")
    return .init(enabled: true, items: [])
  }
  func addClipboard(_ text: String, token: String) async throws -> BackendAccountClient.ClipboardItem {
    calls += 1; throw BackendAccountClient.Failure(status: 500)
  }
  func deleteClipboard(id: String?, token: String) async throws { calls += 1 }
  func setClipboardEnabled(_ enabled: Bool, token: String) async throws { calls += 1 }
}

@main enum DesktopCloudClipboardProviderTest {
  @MainActor static func main() async throws {
    let api = SyntheticClipboardAPI()
    let provider = BackendCloudClipboardProvider(client: api, credentials: { "synthetic-token" })
    let page = try await provider.execute(["operation":"list", "search":"合成"])
    assert((page["items"] as? [[String: Any]])?.count == 1)
    let text = String(repeating: "界", count: 3997) + "\n\r\t"
    _ = try await provider.execute(["operation":"add", "text":text])
    assert(api.uploaded == text)
    _ = try await provider.execute(["operation":"delete", "id":String(repeating: "a", count: 64)])
    _ = try await provider.execute(["operation":"set_enabled", "enabled":false])
    assert(api.calls == 4)
    _ = try await provider.execute(["operation":"add", "text":"👩‍💻"])
    assert(api.uploaded == "👩‍💻")
    _ = try await provider.execute(["operation":"list", "search":"👩‍💻"])
    for request: NSDictionary in [
      ["operation":"add", "text":String(repeating: "界", count: 4001)],
      ["operation":"add", "text":"synthetic\0"], ["operation":"list", "search":"bad\n"],
      ["operation":"add", "text":"synthetic\u{0085}"],
      ["operation":"delete", "id":"bad/id"], ["operation":"set_enabled", "enabled":1],
      ["operation":"token"],
    ] {
      do { _ = try await provider.execute(request); assertionFailure("invalid action accepted") } catch { }
    }
    assert(api.calls == 6)
    var identities = 0
    let changed = BackendCloudClipboardProvider(client: api, credentials: {
      identities += 1
      if identities > 1 { throw CancellationError() }
      return "synthetic-token"
    })
    do { _ = try await changed.execute(["operation":"list"]); assertionFailure("old account data exposed") } catch { }
    let before = api.calls
    let missing = BackendCloudClipboardProvider(client: api, credentials: { throw CancellationError() })
    do { _ = try await missing.execute(["operation":"add", "text":"synthetic"]); assertionFailure("signed-out mutation") } catch { }
    assert(api.calls == before)
    let response: NSDictionary = await withCheckedContinuation { continuation in
      let progress = provider.request(["operation":"add", "text":"synthetic"], completion: { continuation.resume(returning: $0) })
      progress.cancel()
    }
    assert(response["ok"] as? Bool == false && api.calls == before)
    assert(response["error"] as? String == "unavailable")

    let retryAPI = RetryingClipboardAPI()
    var refreshes = 0
    var currentToken = "stale-token"
    let retrying = BackendCloudClipboardProvider(client: retryAPI, credentials: { currentToken },
                                                  refreshCredentials: { rejected in
      assert(rejected == "stale-token")
      refreshes += 1
      currentToken = "fresh-token"
      return "fresh-token"
    })
    _ = try await retrying.execute(["operation":"list", "search":"合成"])
    assert(retryAPI.calls == 2 && refreshes == 1)

    let persistentAPI = RetryingClipboardAPI()
    persistentAPI.rejectEveryCall = true
    var persistentRefreshes = 0
    let persistent = BackendCloudClipboardProvider(client: persistentAPI, credentials: { "stale-token" },
                                                   refreshCredentials: { _ in
      persistentRefreshes += 1
      return "fresh-token"
    })
    do {
      _ = try await persistent.execute(["operation":"list"])
      assertionFailure("a second 401 must fail")
    } catch let failure as BackendAccountClient.Failure {
      assert(failure.status == 401)
    }
    assert(persistentAPI.calls == 2 && persistentRefreshes == 1)

    let switchedAPI = RetryingClipboardAPI()
    var switchedRefreshes = 0
    let switched = BackendCloudClipboardProvider(client: switchedAPI, credentials: { "stale-token" },
                                                 refreshCredentials: { _ in
      switchedRefreshes += 1
      throw CancellationError()
    })
    do {
      _ = try await switched.execute(["operation":"list"])
      assertionFailure("an account switch must stop the retry")
    } catch is CancellationError { }
    assert(switchedAPI.calls == 1 && switchedRefreshes == 1)

    // Sending a local history entry reads the server flag first and uploads only while it is on.
    let sending = SyntheticClipboardAPI()
    let sender = BackendCloudClipboardProvider(client: sending, credentials: { "synthetic-token" })
    let sent = await sender.send("synthetic\n合成")
    assert(sent == .sent && sending.uploaded == "synthetic\n合成" && sending.calls == 2)
    sending.enabled = false
    sending.uploaded = ""
    let disabled = await sender.send("synthetic")
    assert(disabled == .disabled && sending.uploaded.isEmpty && sending.calls == 3)
    sending.enabled = true
    let rejected = await sender.send("synthetic\0")
    assert(rejected == .failed && sending.uploaded.isEmpty && sending.calls == 4)
    let signedOut = BackendCloudClipboardProvider(client: sending, credentials: { throw CancellationError() })
    let unauthenticated = await signedOut.send("synthetic")
    assert(unauthenticated == .failed && sending.calls == 4)
    assert(BackendCloudClipboardProvider.SendOutcome.signedOut.message == "登录水杉账号后可在设备间同步剪贴板")
    assert(BackendCloudClipboardProvider.SendOutcome.disabled.message == "云剪贴板未开启")
  }
}
