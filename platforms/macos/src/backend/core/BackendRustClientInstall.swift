import CMSIMEClient
import Darwin
import Foundation

/// 由 `input_method_main.mm` 在 `dlopen` 后用 `dlsym` 找到并调用，所以必须是导出的全局符号。输入法可执行文件链着 Rust 宿主库，把 `msime_rust_client_linked()` 填好的表交过来；表的版本或大小不对时不收、返回 false，之后每个 Rust 入口都报 `rust_client_unavailable`。
///
/// 收下表以后接着注册本进程的会话存储（#8201 迁移计划 4b）：输入法进程和独立设置窗口进程都经这里，各注册一次，Objective-C++ 不注册。注册失败只记一条日志，账号页随后报 `rust_client_unavailable`。
@_cdecl("MSIMEInstallRustClient")
public func msimeInstallRustClient(_ table: UnsafeRawPointer?) -> Bool {
  // 先只读开头的 version 和 size：调用方的表可能来自别的版本、比本版本的结构体短，按本版本的大小整个读出来会越界。size 是 C 的 sizeof，与 Swift 的 stride 比（见 `BackendRustClient.init`）。
  guard let table, table.load(as: UInt32.self) == MSIME_RUST_CLIENT_VERSION,
        Int(table.load(fromByteOffset: MemoryLayout<UInt32>.size, as: UInt32.self)) == MemoryLayout<msime_rust_client>.stride else {
    return false
  }
  guard BackendRustClient.install(table.load(as: msime_rust_client.self)), let client = BackendRustClient.shared else { return false }
  // macOS 不会像 iOS 那样把持着文件锁的进程挂起后杀掉，不需要后台执行。
  switch BackendRustAccount.registerShared(client: client, agent: MacRustAgent.current, backgroundActivity: nil) {
  case .success(let account):
    // 输入法同步读的登录状态缓存（`BackendAccountState`）先填一次，不等第一页释义。
    Task.detached { try? await BackendAccountState.refresh(account) }
  case .failure(let failure):
    NSLog("MSIME backend could not register the account session store with Rust: %@", failure.code)
  }
  return true
}

/// 这台 Mac 自报的设备信息：版本号是输入法 bundle 的版本，设备是 `hw.model`（例如 `Mac14,2`），系统是「macOS」加 `ProductVersion` 的数字，版本 ID 是 Info.plist 的 `MSIMEEdition`（没有时是 full）。清洗和拼装在 Rust。
enum MacRustAgent {
  static var current: BackendRustAccount.Agent {
    let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    let edition = (Bundle.main.object(forInfoDictionaryKey: "MSIMEEdition") as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "full"
    let os = ProcessInfo.processInfo.operatingSystemVersion
    let system = "macOS \(os.majorVersion).\(os.minorVersion)" + (os.patchVersion > 0 ? ".\(os.patchVersion)" : "")
    return BackendRustAccount.Agent(platform: "macos", version: version, device: hardwareModel, system: system, edition: edition)
  }

  /// `sysctlbyname("hw.model")`，读不到时为 nil。
  static var hardwareModel: String? {
    var size = 0
    guard sysctlbyname("hw.model", nil, &size, nil, 0) == 0, size > 1 else { return nil }
    var bytes = [CChar](repeating: 0, count: size)
    guard sysctlbyname("hw.model", &bytes, &size, nil, 0) == 0 else { return nil }
    let model = String(cString: bytes)
    return model.isEmpty ? nil : model
  }
}
