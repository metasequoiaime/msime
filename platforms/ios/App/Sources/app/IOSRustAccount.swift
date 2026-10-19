import CMSIMEClient
import Foundation
import UIKit

/// App 启动时装好 Rust 函数表、注册本进程的会话存储（#8201 迁移计划 4b）。登录会话在 App Group 访问组的钥匙串里，匿名会话和身份在 App Group 的文件里（`BackendRustStoreLayout.standard`），刷新锁两项共用 App Group 的 `backend-account-refresh.lock`：键盘扩展仍用 Swift 的 `BackendAccountSession`（负责人决定：未测，扩展保留 Swift），两边持同一把 flock、每次都在锁里重读，所以可以共存。
///
/// 一个进程只注册一次，之后的页面都经 `BackendRustAccount.shared`。注册不联网，可以在主线程上做；失败时只记一条诊断，账号页随后报 `rust_client_unavailable`。
enum IOSRustAccount {
  /// 装表并注册。要在任何页面用到 `BackendRustAccount.shared` 之前调用。
  @discardableResult
  static func install() -> BackendRustAccount? {
    BackendRustClient.install(msime_rust_client_linked())
    guard let client = BackendRustClient.shared else { return nil }
    switch BackendRustAccount.registerShared(client: client, agent: agent, backgroundActivity: backgroundActivity) {
    case .success(let account): return account
    case .failure(let failure):
      DiagnosticLog.shared.write("rust_session_store_unavailable code=\(failure.code)")
      return nil
    }
  }

  /// 本机匿名账号还没有可用会话时注册它，好让键盘在需要之前就能用上（身份和会话在 App Group 里，键盘复用）。已经登录、或者读不了登录状态（钥匙串锁着、未签名的构建）时不注册：读不了不等于没登录，不该为此多注册一个匿名账号。失败时下次启动再试。
  static func ensureAnonymous(_ account: BackendRustAccount) async {
    let signedIn: BackendAccountClient.User?
    do { signedIn = try await account.status() }
    catch { return }
    guard signedIn == nil else { return }
    _ = try? await account.account(["operation": "ensure_anonymous"])
  }

  /// 这台设备自报的信息：版本号是 App 的版本，设备是机型标识（例如 `iPhone17,1`；模拟器上是它模拟的机型），系统是「iOS」加系统版本，版本 ID 是本版本的 edition。清洗和拼装在 Rust（`msime_client_client_agent`）。
  static var agent: BackendRustAccount.Agent {
    let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    return BackendRustAccount.Agent(platform: "ios", version: version, device: hardwareModel,
                                    system: "iOS " + systemVersion,
                                    edition: MSIMEAppEdition.identifier)
  }

  /// `17.4.1` 这样的数字版本；补丁号为 0 时省掉。
  static var systemVersion: String {
    let version = ProcessInfo.processInfo.operatingSystemVersion
    return "\(version.majorVersion).\(version.minorVersion)" + (version.patchVersion > 0 ? ".\(version.patchVersion)" : "")
  }

  /// `uname` 的机型标识；模拟器上 `uname` 只给出 `arm64`，改读它模拟的机型。读不到时为 nil，Rust 换成 unknown。
  static var hardwareModel: String? {
    if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"], !simulated.isEmpty { return simulated }
    var name = utsname()
    guard uname(&name) == 0 else { return nil }
    let machine = withUnsafeBytes(of: &name.machine) { bytes in
      String(decoding: bytes.prefix(while: { $0 != 0 }), as: UTF8.self)
    }
    return machine.isEmpty ? nil : machine
  }

  /// 持刷新锁期间保持后台执行：挂起时还持有 App Group 文件锁的进程会被系统杀掉（0xdead10cc）。`beginBackgroundTask` 可以在任意线程上调用；系统收回后台时间之前先结束它，锁随后由 Rust 放开。
  static let backgroundActivity: BackendRustSessionStore.BackgroundActivity = { reason in
    let task = BackgroundTask()
    task.begin(reason)
    return { task.end() }
  }

  /// 一次后台任务，结束恰好一次（到期回调和正常结束谁先到都行）。
  private final class BackgroundTask: @unchecked Sendable {
    private let lock = NSLock()
    private var identifier = UIBackgroundTaskIdentifier.invalid

    func begin(_ reason: String) {
      let identifier = UIApplication.shared.beginBackgroundTask(withName: reason) { [weak self] in self?.end() }
      lock.lock()
      self.identifier = identifier
      lock.unlock()
    }

    func end() {
      lock.lock()
      let identifier = self.identifier
      self.identifier = .invalid
      lock.unlock()
      guard identifier != .invalid else { return }
      UIApplication.shared.endBackgroundTask(identifier)
    }
  }
}
