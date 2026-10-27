import CryptoKit
import Foundation

/// 云剪贴板改走自建中继（`docs/cloud-clipboard-relay.md`）或 WebDAV 网盘（`docs/cloud-clipboard-webdav.md`）时 iOS 这一侧的纯逻辑：设置页当场提示用的地址和凭据规则、App 与键盘用来认出「中继或网盘换过了」的绑定，以及给人看的说法。不碰文件和网络，KeyboardTests 直接测它。
///
/// 地址和凭据的规则与 Rust 的 `valid_relay_url`、`valid_relay_token`、`valid_webdav_username`、`valid_webdav_password`（`crates/client-core/src/preferences.rs`）、共享设置页的 `packages/ui/src/settings/cloud-clipboard-relay.ts` 和 Android 的 `CloudClipboardRelayPolicy` 相同，说法取自共享设置页。这里只为了在输入框下当场说明哪里不行；保存时 Rust 还会校验整份偏好，不合规的值存不进去。
enum IOSCloudClipboardRelayPolicy {
  /// 偏好里中继的键：`{url, token}`，没配置时不写进文档。
  static let relayPreference = "cloud_clipboard_relay"
  /// 偏好里 WebDAV 网盘的键：`{url, username, password}`，与中继只能二选一，没配置时不写进文档。
  static let webdavPreference = "cloud_clipboard_webdav"
  static let maxURLBytes = 2048
  static let minTokenCharacters = 16
  static let maxTokenCharacters = 256
  /// 网盘账号和应用密码各自的上限，与 Rust 的 `MAX_WEBDAV_CREDENTIAL_BYTES` 相同。
  static let maxWebDAVCredentialBytes = 256
  private static let loopbackHosts = ["localhost", "127.0.0.1", "[::1]"]
  /// 绑定取 SHA-256 的前 16 字节，与 Rust 给令牌空间编号的长度相同。
  private static let bindingBytes = 16

  /// 中继以 401 拒绝了令牌，或网盘拒绝了账号、应用密码。不是水杉账号未登录，不能提示去登录。
  static let unauthorized = "relay_unauthorized"
  /// 宿主自己的码：这一页或这次读取开始之后，偏好里的中继或网盘换过了。
  static let changed = "relay_changed"
  static let notConfigured = "relay_not_configured"
  static let unavailable = "relay_unavailable"
  static let storage = "relay_storage"

  /// 同步方式里网盘那一组下面的说明，与共享设置页的 `CLOUD_CLIPBOARD_WEBDAV_NOTE` 相同。
  static let webdavNote = "内容在本机加密，网盘只保存密文；打开面板或复制时同步，没有实时推送；坚果云免费账号每 30 分钟 600 次请求与其他应用共用；建议使用专用账号或专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。"

  // MARK: - 当场提示

  /// 地址校验提示里随用途变化的说法：中继地址，或 WebDAV 的服务器地址。
  private struct Words {
    let name: String
    let server: String
    let credentials: String
  }
  private static let relayWords = Words(name: "中继地址", server: "中继", credentials: "令牌")
  private static let webdavWords = Words(name: "服务器地址", server: "WebDAV 服务器", credentials: "账号和应用密码")

  /// 中继地址的问题，没有问题时为 nil：https（http 只放行本机回环地址）、有主机名、可以带路径前缀，不带查询串、片段和用户信息，不超过 2048 字节。
  static func relayURLIssue(_ url: String) -> String? { endpointIssue(url, relayWords) }

  /// WebDAV 服务器地址（用户选的文件夹）的问题，没有问题时为 nil。规则与中继地址相同（Rust 的网盘偏好也用 `valid_relay_url`）。
  static func webdavURLIssue(_ url: String) -> String? { endpointIssue(url, webdavWords) }

  private static func endpointIssue(_ url: String, _ words: Words) -> String? {
    let full = "请填写以 https:// 开头的完整\(words.name)。"
    if url.isEmpty { return "请填写\(words.name)。" }
    if url.utf8.count > maxURLBytes { return "\(words.name)过长。" }
    if url.unicodeScalars.contains(where: { $0.value < 0x20 || $0.value == 0x7f }) { return "\(words.name)不能包含控制字符。" }
    guard let separator = url.range(of: "://"), separator.lowerBound > url.startIndex else { return full }
    let scheme = url[..<separator.lowerBound].lowercased()
    let rest = url[separator.upperBound...]
    let authority = rest.prefix { !["/", "\\", "?", "#"].contains($0) }
    if authority.isEmpty { return full }
    if authority.contains("@") { return "\(words.name)不能包含用户名或密码，\(words.credentials)请填在下面。" }
    if url.contains("?") || url.contains("#") { return "\(words.name)不能带查询串或片段。" }
    guard let host = host(String(authority)) else { return full }
    if scheme == "https" { return nil }
    if scheme == "http" {
      return loopbackHosts.contains(host.lowercased()) ? nil : "\(words.server)必须使用 HTTPS；HTTP 只支持本机回环地址（127.0.0.1、localhost）。"
    }
    return full
  }

  /// 主机名（IPv6 连同方括号），端口不对或主机名为空时为 nil。
  private static func host(_ authority: String) -> String? {
    let host: Substring
    let port: Substring
    if authority.hasPrefix("[") {
      guard let close = authority.firstIndex(of: "]") else { return nil }
      host = authority[...close]
      let tail = authority[authority.index(after: close)...]
      if !tail.isEmpty && !tail.hasPrefix(":") { return nil }
      port = tail.isEmpty ? "" : tail.dropFirst()
    } else if let colon = authority.lastIndex(of: ":") {
      host = authority[..<colon]
      port = authority[authority.index(after: colon)...]
    } else {
      host = authority[...]
      port = ""
    }
    if host.isEmpty || host == "[]" || host.contains(where: \.isWhitespace) { return nil }
    if port.count > 5 || !port.allSatisfy({ ("0"..."9").contains($0) }) { return nil }
    if let number = Int(port), number > 65535 { return nil }
    return String(host)
  }

  /// 中继令牌的问题，没有问题时为 nil：16 到 256 个可打印 ASCII 字符，不含空格。
  static func tokenIssue(_ token: String) -> String? {
    if token.isEmpty { return "请填写中继令牌。" }
    if !token.unicodeScalars.allSatisfy({ (0x21...0x7e).contains($0.value) }) { return "中继令牌只能包含可打印的 ASCII 字符，不能有空格。" }
    if token.unicodeScalars.count < minTokenCharacters || token.unicodeScalars.count > maxTokenCharacters {
      return "中继令牌应为 \(minTokenCharacters) 到 \(maxTokenCharacters) 个字符。"
    }
    return nil
  }

  /// WebDAV 账号的问题，没有问题时为 nil：1 到 256 字节，不含控制字符，也不含冒号（HTTP Basic 鉴权用冒号分隔账号和密码）。
  static func usernameIssue(_ username: String) -> String? {
    if username.isEmpty { return "请填写网盘账号。" }
    if username.utf8.count > maxWebDAVCredentialBytes { return "网盘账号过长。" }
    if hasControl(username) { return "网盘账号不能包含控制字符。" }
    if username.contains(":") { return "网盘账号不能包含冒号。" }
    return nil
  }

  /// WebDAV 应用密码的问题，没有问题时为 nil：1 到 256 字节，不含控制字符。应用密码原样使用，不去掉首尾空白。
  static func passwordIssue(_ password: String) -> String? {
    if password.isEmpty { return "请填写应用密码。" }
    if password.utf8.count > maxWebDAVCredentialBytes { return "应用密码过长。" }
    if hasControl(password) { return "应用密码不能包含控制字符。" }
    return nil
  }

  /// 与 Rust 的 `char::is_control` 相同：Unicode 的 Cc 类，即 U+0000–U+001F 和 U+007F–U+009F。
  private static func hasControl(_ text: String) -> Bool {
    text.unicodeScalars.contains { $0.value < 0x20 || (0x7f...0x9f).contains($0.value) }
  }

  // MARK: - 绑定

  /// 偏好里这一份中继配置的绑定：没配置时为空串，否则是 `relay:` 加地址和令牌摘要的十六进制，与 Android 的 `CloudClipboardRelayPolicy.binding` 相同。令牌本身不留在绑定里。
  static func relayBinding(url: String, token: String) -> String { binding("relay:", url, token) }

  /// 偏好里 WebDAV 网盘的绑定：`webdav:` 加地址和账号摘要的十六进制。不含应用密码：换应用密码（移除设备之后应该换）不换设备身份，与 Rust 给密钥串选空间的规则相同。
  static func webdavBinding(url: String, username: String) -> String { binding("webdav:", url, username) }

  private static func binding(_ prefix: String, _ url: String, _ second: String) -> String {
    guard !url.isEmpty else { return "" }
    var data = Data(url.utf8)
    data.append(0)
    data.append(contentsOf: Array(second.utf8))
    return prefix + SHA256.hash(data: data).prefix(bindingBytes).map { String(format: "%02x", $0) }.joined()
  }

  // MARK: - 说法

  /// 驱动和中继给出的已知错误码，与 Rust 的 `CLIPBOARD_ERROR_CODES` 和 Android 的 `CloudClipboardRelayPolicy.KNOWN_CODES` 相同。iOS 自己跑中继的驱动，驱动报的错要按 Rust 那条路（`RelayError::code`）同样归类，界面上的说法才一致。
  private static let knownCodes: Set<String> = [
    "clipboard_device_pending", "clipboard_sync_disabled", "clipboard_key_conflict", "clipboard_device_limit",
    "clipboard_e2e_required", "invalid_clipboard_device", "invalid_clipboard_key", "invalid_clipboard_ciphertext",
    "invalid_clipboard_text", "invalid_device_id", "invalid_search", "invalid_response", "invalid_state",
    "not_found", "account_invalid", "account_unauthorized", "account_rate_limited", "account_storage",
    "account_cancelled", "account_conflict", "account_unavailable", "account_banned", "clipboard_full",
    unauthorized, notConfigured, unavailable, storage, "clipboard_keystore_conflict",
  ]

  /// 经中继跑驱动时，驱动报的错交给界面的码：已知的码原样保留，其余按状态码归类，400 是请求不合规，404 是不存在，别的都算中继不可用。
  static func driverFailureCode(status: Int, code: String?) -> String {
    if let code, knownCodes.contains(code) { return code }
    switch status {
    case 400: return "invalid_cloud_clipboard"
    case 404: return "not_found"
    default: return unavailable
    }
  }

  /// 经中继或网盘失败时给人看的一句话，与共享设置页的 `cloudClipboardErrorMessage` 相同，只把「设置的『剪贴板』页」换成 iOS 上的「我的 → 云剪贴板」；驱动的码沿用 iOS 账号路径的说法。
  static func failureMessage(_ code: String) -> String {
    if let message = BackendAccountClient.Failure.clipboardMessages[code] { return message }
    switch code {
    case unauthorized: return "自建中继拒绝了令牌（或 WebDAV 网盘拒绝了账号、应用密码），请在「我的 → 云剪贴板」检查"
    case notConfigured: return "自建中继或 WebDAV 网盘的设置无效，请在「我的 → 云剪贴板」重新填写"
    case unavailable: return "连不上自建中继或 WebDAV 网盘，请确认地址正确、网络可用，稍后重试"
    case "webdav_rate_limited": return "WebDAV 网盘暂时限制了请求次数（坚果云免费账号每 30 分钟 600 次，与其他应用共用），请稍后再试"
    case "webdav_rollback": return "网盘上的云剪贴板记录比这台设备读到过的旧，或者被改动过，已停止同步。若是你自己清理过网盘文件夹，请在「我的 → 云剪贴板」重新测试连接并保存"
    case "webdav_unsupported": return "这个网盘不支持云剪贴板需要的文件操作，请在「我的 → 云剪贴板」换一个 WebDAV 服务"
    case "clipboard_full": return "云剪贴板的 50 条都已置顶，取消置顶或删除几条后再发送"
    case storage: return "无法读写本机保存的中继密钥或网盘缓存，请稍后重试"
    case changed, "clipboard_source_changed": return "云剪贴板同步来源已更改，请关闭后重新打开。"
    case "not_found": return "这台设备已不在列表里，请刷新后再试"
    case "storage": return "无法读取本机的设置，请稍后重试"
    default: return "云剪贴板服务暂不可用，请稍后重试"
    }
  }

  /// 「测试连接」失败时给人看的一句话，与共享设置页的 `cloudClipboardWebDavProbeMessage` 相同。
  static func probeMessage(_ code: String) -> String {
    switch code {
    case notConfigured: return "地址、账号或应用密码不完整。"
    case "webdav_unsupported": return "这个网盘不支持云剪贴板需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。"
    case unauthorized: return "网盘拒绝了这个账号或应用密码。"
    case unavailable: return "连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。"
    case "webdav_rate_limited": return "网盘暂时限制了请求次数（坚果云免费账号每 30 分钟 600 次，与其他应用共用），请稍后再试。"
    default: return "测试连接失败，请稍后再试。"
    }
  }
}
