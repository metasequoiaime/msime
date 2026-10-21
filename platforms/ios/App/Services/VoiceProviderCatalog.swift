import Foundation

@_silgen_name("msime_client_voice_provider_catalog")
private func msimeClientVoiceProviderCatalog() -> UnsafeMutablePointer<CChar>?
@_silgen_name("msime_client_string_free")
private func msimeClientVoiceCatalogStringFree(_ value: UnsafeMutablePointer<CChar>?)

/// Rust 语音服务目录的持有快照。只读取公开预设，不访问偏好、文件、网络或设备。
struct VoiceProviderCatalog: Sendable {
  struct StreamEndpoint: Decodable, Equatable, Sendable {
    let id: String
    let endpoint: String
    let title: String
  }

  private struct Provider: Decodable {
    let id: String
    let endpoint: String
  }

  private struct Value: Decodable {
    let asr: [Provider]
    let doubaoStreamEndpoints: [StreamEndpoint]

    enum CodingKeys: String, CodingKey {
      case asr
      case doubaoStreamEndpoints = "doubao_stream_endpoints"
    }
  }

  private struct Envelope: Decodable {
    let ok: Bool
    let value: Value
  }

  private enum Failure: Error { case unavailable }

  let doubaoDefaultEndpoint: String
  let doubaoStreamEndpoints: [StreamEndpoint]

  /// 缓存静态目录；读取失败时不复制宿主默认表兜底，已保存的地址仍由原配置持有。
  static let shared = load()

  init(data: Data) throws {
    let envelope = try JSONDecoder().decode(Envelope.self, from: data)
    guard envelope.ok, let doubao = envelope.value.asr.first(where: { $0.id == "doubao" }) else {
      throw Failure.unavailable
    }
    doubaoDefaultEndpoint = doubao.endpoint
    doubaoStreamEndpoints = envelope.value.doubaoStreamEndpoints
  }

  static func load() -> Self? {
    guard let raw = msimeClientVoiceProviderCatalog() else { return nil }
    defer { msimeClientVoiceCatalogStringFree(raw) }
    return try? Self(data: Data(bytes: raw, count: strlen(raw)))
  }
}
