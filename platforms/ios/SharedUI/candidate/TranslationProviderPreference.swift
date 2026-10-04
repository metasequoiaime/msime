import Foundation
import CoreFoundation

/// Where candidate glosses come from: nowhere online (the default), a service the user signed up for with their own credentials, or the 水杉 account's translation API. The three provider objects live in the shared preference document (`niutrans`, `custom_translation`, `tencent_tmt`) so the choice follows the same precedence as the other hosts: NiuTrans first, then the custom endpoint, then Tencent TMT when both of its secrets are usable. The account is used only when the user explicitly chose it (`translation_account`) and none of their own services applies; it is never a fallback. A chosen provider that is not usable yields no online glosses at all rather than sending the words somewhere the user did not pick.
enum TranslationProvider: String, CaseIterable, Sendable {
  case off, account, niutrans, tencent, custom
  var title: String {
    switch self {
    case .off: "不使用在线翻译"
    case .account: "水杉账号（发送到 api.msime.app）"
    case .niutrans: "小牛翻译"
    case .tencent: "腾讯云机器翻译"
    case .custom: "自定义接口（DeepLX 兼容）"
    }
  }
}

/// The resolved route for one keyboard session; carries credentials, so it is never logged.
enum TranslationRoute: Equatable, Sendable {
  case account
  case niutrans(appID: String, apiKey: String)
  case tencent(secretID: String, secretKey: String, region: String)
  case custom(endpoint: String, apiKey: String)
  /// Nothing was chosen, or the chosen provider is not usable: no online glosses.
  case none

  var provider: TranslationProvider? {
    switch self {
    case .account: .account
    case .niutrans: .niutrans
    case .tencent: .tencent
    case .custom: .custom
    case .none: nil
    }
  }

  /// Glosses cached under one route must not show up after the user switches service or credentials.
  var cacheScope: String {
    switch self {
    case .account: "account"
    case let .niutrans(appID, apiKey): "niutrans|\(appID)|\(apiKey.hashValue)"
    case let .tencent(secretID, secretKey, region): "tencent|\(secretID)|\(secretKey.hashValue)|\(region)"
    case let .custom(endpoint, apiKey): "custom|\(endpoint)|\(apiKey.hashValue)"
    case .none: "none"
    }
  }
}

enum TranslationProviderPreference {
  static let niutransKey = "niutrans"
  static let customKey = "custom_translation"
  static let tencentKey = "tencent_tmt"
  /// True only when the user explicitly picked the 水杉 account in 「翻译服务」; absent means no.
  static let accountKey = "translation_account"
  static let defaultTencentRegion = "ap-guangzhou"

  /// The same check as client-core's `usable_credential`: empty strings, `<placeholders>` and `FAKESECRET_` samples are not credentials.
  static func usable(_ value: String) -> Bool {
    let trimmed = trimmed(value)
    return !trimmed.isEmpty && !(trimmed.hasPrefix("<") && trimmed.hasSuffix(">")) && !trimmed.hasPrefix("FAKESECRET_")
  }

  static func route(in preferences: [String: Any]?) -> TranslationRoute {
    let niutrans = preferences?[niutransKey] as? [String: Any] ?? [:]
    let custom = preferences?[customKey] as? [String: Any] ?? [:]
    let tencent = preferences?[tencentKey] as? [String: Any] ?? [:]
    if niutrans["enabled"] as? Bool == true {
      let appID = trimmed(niutrans["app_id"] as? String ?? "")
      let apiKey = trimmed(niutrans["apikey"] as? String ?? "")
      return usable(appID) && usable(apiKey) ? .niutrans(appID: appID, apiKey: apiKey) : .none
    }
    if custom["enabled"] as? Bool == true {
      let endpoint = trimmed(custom["endpoint"] as? String ?? "")
      return endpoint.isEmpty ? .none : .custom(endpoint: endpoint, apiKey: trimmed(custom["api_key"] as? String ?? ""))
    }
    // Tencent defaults to enabled, so only usable secrets make it the route.
    let secretID = trimmed(tencent["secret_id"] as? String ?? "")
    let secretKey = trimmed(tencent["secret_key"] as? String ?? "")
    if tencent["enabled"] as? Bool ?? true, usable(secretID), usable(secretKey) {
      let region = trimmed(tencent["region"] as? String ?? "")
      return .tencent(secretID: secretID, secretKey: secretKey, region: region.isEmpty ? defaultTencentRegion : region)
    }
    return preferences?[accountKey] as? Bool == true ? .account : .none
  }

  /// The provider the settings page shows as selected, which may be one whose credentials are still incomplete.
  static func selected(in preferences: [String: Any]?) -> TranslationProvider {
    if (preferences?[niutransKey] as? [String: Any])?["enabled"] as? Bool == true { return .niutrans }
    if (preferences?[customKey] as? [String: Any])?["enabled"] as? Bool == true { return .custom }
    if case .tencent = route(in: preferences) { return .tencent }
    return preferences?[accountKey] as? Bool == true ? .account : .off
  }

  /// Write one provider as the only enabled one. Credentials of the others are kept so switching back does not lose them; Tencent is written disabled explicitly because its default is enabled.
  static func select(_ provider: TranslationProvider, niutrans: (appID: String, apiKey: String),
                     tencent: (secretID: String, secretKey: String, region: String),
                     custom: (endpoint: String, apiKey: String), in document: inout [String: Any]) {
    let region = trimmed(tencent.region)
    document[niutransKey] = ["enabled": provider == .niutrans, "app_id": trimmed(niutrans.appID), "apikey": trimmed(niutrans.apiKey)]
    document[customKey] = ["enabled": provider == .custom, "endpoint": trimmed(custom.endpoint), "api_key": trimmed(custom.apiKey)]
    document[tencentKey] = ["enabled": provider == .tencent,
                            "secret_id": trimmed(tencent.secretID), "secret_key": trimmed(tencent.secretKey),
                            "region": region.isEmpty ? defaultTencentRegion : region]
    // Present only when chosen, as the shared core writes it, so saving an unchanged choice leaves the document equal.
    if provider == .account { document[accountKey] = true } else { document.removeValue(forKey: accountKey) }
  }

  private static func trimmed(_ value: String) -> String { value.trimmingCharacters(in: .whitespacesAndNewlines) }
}

/// Translates candidate words through a user-chosen provider. The shared host builds and signs every request and parses every reply; this type only moves bytes over the transport. Returns one gloss or nil per word, in order.
struct TranslationProviderClient: Sendable {
  static let tencentBatch = 9
  static let maxTextLength = 40
  var transport: any OnlineCandidateTransport = URLSessionOnlineCandidateTransport()
  var now: @Sendable () -> Date = { Date() }

  func translate(words: [String], target: String, route: TranslationRoute) async -> [String?] {
    let target = target.lowercased()
    var results = [String?](repeating: nil, count: words.count)
    let eligible = words.indices.filter { !words[$0].isEmpty && words[$0].count <= Self.maxTextLength }
    switch route {
    case let .tencent(secretID, secretKey, region):
      for start in stride(from: 0, to: eligible.count, by: Self.tencentBatch) {
        let batch = Array(eligible[start..<min(eligible.count, start + Self.tencentBatch)])
        let request: [String: Any] = [
          "config": ["enabled": true, "secret_id": secretID, "secret_key": secretKey, "region": region],
          "texts": batch.map { words[$0] }, "source_language": "zh", "target_language": target,
          "timestamp": Int(now().timeIntervalSince1970),
        ]
        guard let glosses = await exchange(provider: "tencent", request: request, expected: batch.count) else { continue }
        for (index, gloss) in zip(batch, glosses) { results[index] = gloss }
      }
    case let .niutrans(appID, apiKey):
      for index in eligible {
        let request: [String: Any] = [
          "config": ["enabled": true, "app_id": appID, "apikey": apiKey], "text": words[index],
          "source_language": "zh", "target_language": target,
          "timestamp": String(Int64(now().timeIntervalSince1970 * 1000)),
        ]
        results[index] = await exchange(provider: "niutrans", request: request, expected: 1)?.first ?? nil
      }
    case let .custom(endpoint, apiKey):
      for index in eligible {
        let request: [String: Any] = [
          "config": ["enabled": true, "endpoint": endpoint, "api_key": apiKey], "text": words[index],
          "source_language": "zh", "target_language": target,
        ]
        results[index] = await exchange(provider: "custom", request: request, expected: 1)?.first ?? nil
      }
    case .account, .none:
      break
    }
    return results
  }

  private func exchange(provider: String, request: [String: Any], expected: Int) async -> [String?]? {
    guard let descriptor = MetasequoiaInputSessionBridge.translationRequest(provider: provider, request),
          let urlRequest = Self.urlRequest(descriptor),
          let body = await transport.fetch(urlRequest) else { return nil }
    return MetasequoiaInputSessionBridge.parseTranslationResponse(provider: provider, body: body, expected: expected)
  }

  /// The HTTPS POST a descriptor describes. `body_utf8` is sent byte for byte because Tencent signed exactly those bytes; `body` is a JSON object the custom endpoint expects.
  static func urlRequest(_ descriptor: [String: Any]) -> OnlineCandidateRequest? {
    guard let text = descriptor["url"] as? String, let url = URL(string: text), url.scheme == "https",
          (descriptor["method"] as? String ?? "POST") == "POST" else { return nil }
    let payload: Data
    if let utf8 = descriptor["body_utf8"] as? String {
      payload = Data(utf8.utf8)
    } else if let body = descriptor["body"], JSONSerialization.isValidJSONObject(body),
              let data = try? JSONSerialization.data(withJSONObject: body) {
      payload = data
    } else {
      return nil
    }
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.httpBody = payload
    for (name, value) in descriptor["headers"] as? [String: String] ?? [:] {
      request.setValue(value, forHTTPHeaderField: name)
    }
    let milliseconds: Int
    if descriptor["timeout_ms"] == nil {
      milliseconds = 2500
    } else {
      guard let value = integer(descriptor["timeout_ms"]), value > 0 else { return nil }
      milliseconds = value
    }
    let timeout = TimeInterval(min(10_000, max(1_000, milliseconds))) / 1000
    let maxBytes: Int
    if descriptor["max_response_bytes"] == nil {
      maxBytes = 1_048_576
    } else {
      guard let value = integer(descriptor["max_response_bytes"]), value > 0 else { return nil }
      maxBytes = min(1_048_576, value)
    }
    return OnlineCandidateRequest(urlRequest: request, connectTimeout: timeout, timeout: timeout, maxBytes: maxBytes)
  }

  private static func integer(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          let integer = Int(number.stringValue),
          integer >= 0,
          NSNumber(value: integer).compare(number) == .orderedSame else { return nil }
    return integer
  }
}
