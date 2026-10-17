import Foundation

@_silgen_name("msime_client_ai_endpoint_check")
private func msimeClientAIEndpointCheck(_ endpoint: UnsafePointer<UInt8>?, _ length: UInt) -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientAIEndpointStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// AI 辅助接口地址的传输策略：https 不限主机；http 只能指向本机或局域网，因为 API Token 不能明文经过互联网。规则只在 Rust（`crates/client-providers/src/ai/endpoint.rs`），这里经 `msime_client_ai_endpoint_check` 询问，不再自己判断；`AIEndpointPolicyTests` 经 C 接口逐条跑 `shared/contracts/ai-endpoint/cases.json`。
///
/// 只用于 AI 辅助（候选栏 AI 候选、键盘 AI、润色）这条路径；云候选、翻译、语音识别仍然只走 https。`crates/tauri-mobile-platform` 的插件编不进这个文件，那里另有一份同样的判断。
enum AIEndpointPolicy {
  enum Problem: Error, Equatable {
    /// 不是带主机的完整 `http(s)://` 地址，或者带了用户名、密码、`#` 片段、控制字符、反斜杠，或者 `http://` 的主机不是规范写法。
    case invalid
    /// `http://` 地址指向的不是本机或局域网主机。
    case cleartextPublicHost
  }

  /// 公网 http 被拒绝时给用户看的说明。
  static let cleartextMessage = "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网。"

  /// 地址的问题；可用时为 nil。调用方负责去掉首尾空白。
  static func problem(_ endpoint: String) -> Problem? {
    guard let answer = answer(endpoint) else { return .invalid }
    switch answer["problem"] {
    case is NSNull: return nil
    case let problem as String where problem == "cleartext_public": return .cleartextPublicHost
    default: return .invalid
    }
  }

  /// 通过检查时返回地址。Rust 放行、系统却解析不出带主机的 `URL` 时也是 nil，不发请求。
  static func validatedURL(_ endpoint: String) -> URL? {
    guard problem(endpoint) == nil, let url = URL(string: endpoint), url.host?.isEmpty == false else { return nil }
    return url
  }

  /// 保存 Token 用的来源键 `scheme://host:port`，由 Rust 算出，与设置页、Android 和桌面端相同。地址不合规时为 nil。
  static func origin(_ endpoint: String) -> String? {
    answer(endpoint)?["origin"] as? String
  }

  /// 已经通过检查的地址对应的来源键。
  static func origin(of url: URL) -> String? { origin(url.absoluteString) }

  /// 发往 `url` 的会话配置。明文 http 只会发往本机或局域网，要直连、不经过系统代理：走代理的话，Token 会明文交给代理，代理还可能在公网上。https 原样返回，仍按系统设置。
  static func sessionConfiguration(_ configuration: URLSessionConfiguration, for url: URL?) -> URLSessionConfiguration {
    guard url?.scheme?.lowercased() == "http",
          let direct = configuration.copy() as? URLSessionConfiguration else { return configuration }
    // 空字典表示不用任何代理（包括 PAC），nil 才是跟随系统。
    direct.connectionProxyDictionary = [:]
    return direct
  }

  /// Rust 的回答 `{origin, problem, hint}`；C 接口拒绝请求或回复读不懂时为 nil。不做 I/O。
  private static func answer(_ endpoint: String) -> [String: Any]? {
    let bytes = Array(endpoint.utf8)
    guard let raw = bytes.withUnsafeBufferPointer({ msimeClientAIEndpointCheck($0.baseAddress, UInt($0.count)) })
    else { return nil }
    defer { msimeClientAIEndpointStringFree(raw) }
    guard let envelope = try? JSONSerialization.jsonObject(with: Data(bytes: raw, count: strlen(raw))) as? [String: Any],
          envelope["ok"] as? Bool == true else { return nil }
    return envelope["value"] as? [String: Any]
  }
}
