import Foundation

/// 「云端候选与排序」页（`shared/backend-ui/candidate/CloudCandidatesView.swift`）读的形状，请求经 Rust 的 `msime_client_cloud_dictionary`（#8201 迁移计划 4c）。
extension BackendAccountClient {
  struct CandidateQuery: Encodable, Sendable {
    let text: String
    let kind: String
    let scheme: String
    let profile: String
    let limit: Int
  }
  struct PersonalCandidate: Decodable, Identifiable, Sendable {
    let code: String
    let word: String
    let weight: Int64
    let canonical_pinyin: String?
    var mutationCode: String { canonical_pinyin.flatMap { $0.isEmpty ? nil : $0 } ?? code }
    var id: String { "\(mutationCode.utf8.count):\(mutationCode)\(word)" }
  }
  struct PersonalCandidates: Decodable, Sendable {
    let candidates: [PersonalCandidate]
    let context: String
    let revision: Int64
  }
  enum RankingMode: String, CaseIterable, Identifiable, Sendable {
    case disabled, pin, halve, linear, promote
    var id: String { rawValue }
    var title: String {
      switch self { case .disabled: return "不调频"; case .pin: return "置顶调频"; case .halve: return "折半调频"; case .linear: return "线性调频"; case .promote: return "一次置前" }
    }
  }
  struct FixedPosition: Decodable, Identifiable, Sendable {
    let context: String
    let code: String
    let word: String
    let position: Int
    var id: String { "\(context.utf8.count):\(context)\(code.utf8.count):\(code)\(word)" }
  }
  struct FixedPositions: Decodable, Sendable {
    let positions: [FixedPosition]
    let offset: Int
    let has_more: Bool
  }
}
