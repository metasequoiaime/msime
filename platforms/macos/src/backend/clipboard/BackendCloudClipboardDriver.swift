import Foundation

/// 云剪贴板的端到端加密驱动。dylib 不链接 Rust 库，`msime_client_cloud_clipboard_e2e` 和 `msime_client_string_free` 链接在输入法可执行文件里，由 `input_method_main.mm` 在加载 dylib 之后经 `MSIMEInstallRustClient` 连同整张 Rust 函数表交过来（`BackendRustClient.shared`），输入法和独立设置窗口两种启动方式都会走到这一步。
enum MacCloudClipboardDriver {
  /// 本进程的驱动、密钥串（与 `account-session.json` 同目录的 `cloud-clipboard-keystore`）和设备名；输入法没有交来函数表时为 nil，此时云剪贴板的读写都会失败，而不是退回明文。
  static var e2e: BackendClipboardE2E? {
    guard let table = BackendRustClient.shared?.table, let drive = table.cloud_clipboard_e2e, let free = table.string_free else { return nil }
    return BackendClipboardE2E(drive: BackendClipboardE2E.callingCABI({ drive($0, $1) }, free: { free($0) }),
                               keystore: BackendDesktopClipboardKeystoreFile(), deviceName: { deviceName })
  }

  /// 「系统设置 → 通用 → 关于本机」里的电脑名称，取不到时用「Mac」。只在登记本机时发给服务端，其他设备在设备列表里看到它。
  static var deviceName: String {
    let name = Host.current().localizedName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return name.isEmpty ? "Mac" : name
  }
}
