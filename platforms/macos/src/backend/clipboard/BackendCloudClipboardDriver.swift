import Foundation

/// 云剪贴板登记本机时给其他设备看的名字。端到端加密驱动和密钥串都在 Rust 里（`msime_client_account_cloud_clipboard`，#8201），宿主只交这个名字。
enum MacCloudClipboardDriver {
  /// 「系统设置 → 通用 → 关于本机」里的电脑名称，取不到时用「Mac」。只在登记本机时发给服务端，其他设备在设备列表里看到它。
  static var deviceName: String {
    let name = Host.current().localizedName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return name.isEmpty ? "Mac" : name
  }
}
