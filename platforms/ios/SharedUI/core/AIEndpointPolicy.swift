import Darwin
import Foundation

/// AI 辅助接口地址的传输策略，与 `crates/client-core/src/ai/endpoint.rs` 逐条一致：https 不限主机；http 只能指向本机或局域网（回环、RFC 1918、100.64.0.0/10、链路本地、IPv6 唯一本地地址、`.local`），因为 API Token 不能明文经过互联网。检查时不做 DNS 解析：IP 只认严格的字面量，除 `localhost` 和 `.local` 以外的域名一律按公网处理。两边共用 `shared/contracts/ai-endpoint/cases.json` 里的用例。
///
/// 只用于 AI 辅助（候选栏 AI 候选、键盘 AI、润色）这条路径；云候选、翻译、语音识别仍然只走 https。`crates/tauri-mobile-platform` 的插件编不进这个文件，那里另有一份同样的判断。
enum AIEndpointPolicy {
  enum Problem: Error, Equatable {
    /// 不是带主机的完整 `http(s)://` 地址，或者带了用户名、密码、`#` 片段、控制字符。
    case invalid
    /// `http://` 地址指向的不是本机或局域网主机。
    case cleartextPublicHost
  }

  static let maximumEndpointBytes = 2_048

  /// 公网 http 被拒绝时给用户看的说明。
  static let cleartextMessage = "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网。"

  /// 地址的问题；可用时为 nil。调用方负责去掉首尾空白。
  static func problem(_ endpoint: String) -> Problem? {
    switch check(endpoint) {
    case .success: nil
    case .failure(let problem): problem
    }
  }

  /// 通过检查时返回地址。
  static func validatedURL(_ endpoint: String) -> URL? {
    if case .success(let url) = check(endpoint) { return url }
    return nil
  }

  /// 保存 Token 用的来源键 `scheme://host:port`：主机小写，端口总是写出来（https 默认 443，http 默认 80），IPv6 保留方括号。地址不合规时为 nil。
  static func origin(_ endpoint: String) -> String? {
    guard case .success = check(endpoint), let components = URLComponents(string: endpoint),
          let scheme = components.scheme?.lowercased(), let host = components.host?.lowercased()
    else { return nil }
    return "\(scheme)://\(host):\(components.port ?? (scheme == "https" ? 443 : 80))"
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

  private static func check(_ endpoint: String) -> Result<URL, Problem> {
    guard !endpoint.isEmpty, endpoint.utf8.count <= maximumEndpointBytes,
          !endpoint.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          // `https:///v1` 会被解析成没有主机的地址；要求 `://` 后面紧跟主机。
          let separator = endpoint.range(of: "://"), let first = endpoint[separator.upperBound...].first, first != "/",
          let components = URLComponents(string: endpoint),
          let scheme = components.scheme?.lowercased(), scheme == "https" || scheme == "http",
          let host = components.host, !host.isEmpty,
          components.user == nil, components.password == nil,
          // `#` 后面哪怕是空的也算片段。
          components.fragment == nil,
          (components.port ?? 0) >= 0, (components.port ?? 0) <= 65_535,
          let url = URL(string: endpoint)
    else { return .failure(.invalid) }
    if scheme == "http" && !isLocalNetworkHost(host) { return .failure(.cleartextPublicHost) }
    return .success(url)
  }

  /// `host` 取自 `URLComponents.host`，IPv6 带方括号。
  static func isLocalNetworkHost(_ host: String) -> Bool {
    if host.hasPrefix("[") && host.hasSuffix("]") {
      guard let bytes = ipv6(String(host.dropFirst().dropLast())) else { return false }
      let loopback = bytes[0..<15].allSatisfy { $0 == 0 } && bytes[15] == 1
      // fe80::/10 链路本地；fc00::/7 唯一本地地址。
      return loopback || (bytes[0] == 0xfe && bytes[1] & 0xc0 == 0x80) || bytes[0] & 0xfe == 0xfc
    }
    if let octets = ipv4(host) {
      let (a, b) = (octets[0], octets[1])
      return a == 127 || a == 10 || (a == 172 && (16...31).contains(b)) || (a == 192 && b == 168)
        // 100.64.0.0/10：运营商级 NAT 与 Tailscale。
        || (a == 100 && (64...127).contains(b)) || (a == 169 && b == 254)
    }
    let domain = host.lowercased()
    if domain == "localhost" { return true }
    guard domain.hasSuffix(".local") else { return false }
    let label = domain.dropLast(".local".count)
    return !label.isEmpty && !label.hasSuffix(".")
  }

  /// 严格的点分十进制：四段、每段 0～255、不带前导零。`010.0.0.1` 这类写法在系统解析里按八进制处理，会连到另一个地址，所以不当作 IP，按域名归入公网。
  private static func ipv4(_ text: String) -> [Int]? {
    let parts = text.split(separator: ".", omittingEmptySubsequences: false)
    guard parts.count == 4 else { return nil }
    var octets: [Int] = []
    for part in parts {
      guard (1...3).contains(part.count), part.allSatisfy({ $0.isASCII && $0.isNumber }),
            part.count == 1 || part.first != "0", let value = Int(part), value <= 255 else { return nil }
      octets.append(value)
    }
    return octets
  }

  /// IPv6 字面量的 16 个字节；`inet_pton` 只解析文本，不做任何查询。
  private static func ipv6(_ text: String) -> [UInt8]? {
    var address = in6_addr()
    guard inet_pton(AF_INET6, text, &address) == 1 else { return nil }
    return withUnsafeBytes(of: &address) { Array($0) }
  }
}
