import Foundation

/// 一个登录账号的不透明标记：后端发出的释义通知和输入法的释义缓存都按它分开账号，双方都不必交出账号 id。只按 user.id 算：C 上拿不到会话代次，同一个账号退出又重新登录时标记不变，缓存里的释义照旧可用（内容本来就是这个账号的）。
func backendAccountTranslationMarker(userID: String) -> UInt64 {
  var hash: UInt64 = 14695981039346656037
  for byte in userID.utf8 {
    hash ^= UInt64(byte)
    hash &*= 1099511628211
  }
  return hash
}

/// 登录状态的缓存，给输入法在主线程上同步读（`MSIMEBackendAccountTranslationMarker`、`MSIMEBackendAccountSignedIn`）。主线程上不能等 Rust 读存储（可能要等另一个进程的刷新锁），所以这里只读后台刷新的那一份：注册会话存储时、输入源第一次激活时，以及每一页释义发出前后各用 Rust 的 `status` 刷新一次（#8201 迁移计划 4e）。设置应用里登录或退出以后，到下一页释义发出前，缓存里还是原来的账号。
enum BackendAccountState {
  private final class Cache: @unchecked Sendable {
    let lock = NSLock()
    var marker: UInt64 = 0
    var signedIn = false
  }
  private static let cache = Cache()

  /// 当前登录账号的标记，没有登录时是 0。
  static var marker: UInt64 {
    cache.lock.lock(); defer { cache.lock.unlock() }
    return cache.marker
  }
  /// 是否登录了账号（不含本机匿名账号）。
  static var signedIn: Bool {
    cache.lock.lock(); defer { cache.lock.unlock() }
    return cache.signedIn
  }

  /// 读 Rust 的 `status`、更新缓存，返回新的标记。读不了存储时抛出，缓存不变。
  @discardableResult
  static func refresh(_ rust: BackendRustAccount) async throws -> UInt64 {
    let user = try await rust.status()
    let marker = user.map { backendAccountTranslationMarker(userID: $0.id) } ?? 0
    store(marker: marker, signedIn: user != nil)
    return marker
  }

  private static func store(marker: UInt64, signedIn: Bool) {
    cache.lock.lock(); defer { cache.lock.unlock() }
    cache.marker = marker; cache.signedIn = signedIn
  }
}

/// 只想决定要不要提示登录的原生设置界面用的同步状态，令牌不经过这里。登录账号读缓存，本机匿名账号读 App Group 里的会话文件（「本机账号」留在 Swift）。
@_cdecl("MSIMEBackendAccountSignedIn")
public func msimeBackendAccountSignedIn() -> Bool {
  if BackendAccountState.signedIn { return true }
  return ((try? BackendAnonymousAccount.sessionStorage().load()) ?? nil) != nil
}

/// 账号候选释义的缓存范围。输入法只需要分开不同的账号，拿不到账号 id 本身。
@_cdecl("MSIMEBackendAccountTranslationMarker")
public func msimeBackendAccountTranslationMarker() -> UInt64 {
  BackendAccountState.marker
}

/// 输入源第一次激活后为只用键盘的服务（候选释义等）备好本机匿名账号。尽力而为：打字不等账号，断网也不拦输入。由 Rust 的 `ensure_anonymous` 注册（#8201 迁移计划 4b）：已登录时不调；已有匿名会话时 Rust 只读一次存储、不联网；读不了登录状态时这次不注册，免得把读不了当作没有登录。
private actor AnonymousAccountBootstrap {
  static let shared = AnonymousAccountBootstrap()
  private var attempted = false

  func runOnce() async {
    guard !attempted else { return }
    attempted = true
    guard let rust = BackendRustAccount.shared else { return }
    do { try await BackendAccountState.refresh(rust) } catch { return }
    if BackendAccountState.signedIn { return }
    _ = try? await rust.account(["operation": "ensure_anonymous"])
  }
}

@_cdecl("MSIMEEnsureAnonymousAccount")
public func msimeEnsureAnonymousAccount() {
  Task { await AnonymousAccountBootstrap.shared.runOnce() }
}
