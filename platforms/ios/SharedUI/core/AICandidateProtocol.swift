import Foundation

@_silgen_name("msime_client_ai_candidate_protocol")
private func msimeClientAICandidateProtocol(
  _ request: UnsafePointer<UInt8>?, _ length: UInt
) -> UnsafeMutablePointer<CChar>?

/// 候选回复保留切分元信息直到 Rust 校验，不能先取 text 再交给引擎。
enum AICandidateProtocol {
  static func parse(
    _ body: Data, query: Data, limit: Int,
    decode: (UnsafeMutablePointer<CChar>?) throws -> Any
  ) -> [String] {
    guard !body.isEmpty, body.count <= 1_048_576, query.count <= 131_072,
          (1...10).contains(limit), let text = String(data: body, encoding: .utf8),
          let document = try? JSONSerialization.jsonObject(with: query),
          let data = try? JSONSerialization.data(withJSONObject: [
            "op": "parse", "query": document, "body": text, "limit": limit
          ]), data.count <= 8 * 1024 * 1024 else { return [] }
    return data.withUnsafeBytes { bytes in
      (try? decode(msimeClientAICandidateProtocol(
        bytes.bindMemory(to: UInt8.self).baseAddress, UInt(data.count)))) as? [String] ?? []
    }
  }
}
