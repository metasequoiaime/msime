import Foundation

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
  struct RankingResult: Decodable, Sendable {
    struct Selection: Decodable, Sendable { let count: Int }
    let revision: Int64
    let changed: Bool
    let selection: Selection
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
  struct DictionaryRevision: Decodable, Sendable { let revision: Int64 }

  func personalCandidates(_ query: CandidateQuery, token: String) async throws -> PersonalCandidates {
    let page: PersonalCandidates = try await json("POST", "/v1/users/me/dictionary/candidates", token: token, body: JSONEncoder().encode(query))
    guard page.candidates.count <= query.limit,
          page.revision >= 0,
          Self.validCandidateText(page.context, maximum: 1024, empty: true),
          page.candidates.allSatisfy(Self.validPersonalCandidate) else { throw Failure(status: 0) }
    return page
  }
  func rankCandidate(_ candidate: PersonalCandidate, query: CandidateQuery, revision: Int64, mode: RankingMode, step: Int = 1, trigger: Int = 1, forceTop: Bool = false, token: String) async throws -> RankingResult {
    guard revision >= 0, (1...100).contains(step), (1...10).contains(trigger), query.kind != "quick" else { throw Failure(status: 400) }
    struct Action: Encodable { let code: String; let word: String; let mode: String; let linear_step: Int; let trigger_count: Int; let force_top: Bool }
    struct Body: Encodable { let revision: Int64; let query: CandidateQuery; let action: Action }
    return try await json("POST", "/v1/users/me/dictionary/ranking", token: token,
      body: JSONEncoder().encode(Body(revision: revision, query: query, action: .init(code: candidate.mutationCode, word: candidate.word, mode: mode.rawValue, linear_step: step, trigger_count: trigger, force_top: forceTop))))
  }
  func removeCandidate(_ candidate: PersonalCandidate, query: CandidateQuery, revision: Int64, token: String) async throws -> DictionaryChange {
    guard revision >= 0, query.kind != "quick" else { throw Failure(status: 400) }
    struct Body: Encodable { let revision: Int64; let query: CandidateQuery; let code: String; let word: String }
    return try await json("DELETE", "/v1/users/me/dictionary/candidates", token: token,
      body: JSONEncoder().encode(Body(revision: revision, query: query, code: candidate.mutationCode, word: candidate.word)))
  }
  func fixedPositions(context: String = "", offset: Int = 0, token: String) async throws -> FixedPositions {
    guard (0...1_000_000).contains(offset) else { throw Failure(status: 400) }
    var components = URLComponents()
    components.path = "/v1/users/me/dictionary/positions"
    components.queryItems = [.init(name: "context", value: context), .init(name: "offset", value: String(offset)), .init(name: "limit", value: "100")]
    guard let path = Self.encodedPath(components) else { throw Failure(status: 400) }
    let page: FixedPositions = try await json("GET", path, token: token)
    guard page.positions.count <= 100, page.offset == offset,
          page.positions.allSatisfy({ position in
            (context.isEmpty || position.context == context)
              && Self.validCandidateText(position.context, maximum: 1024)
              && Self.validCandidateText(position.code, maximum: 256)
              && Self.validCandidateText(position.word, maximum: 1024)
              && (1...5).contains(position.position)
          }) else { throw Failure(status: 0) }
    return page
  }
  func setFixedPosition(context: String, code: String, word: String, position: Int?, revision: Int64, token: String) async throws -> DictionaryRevision {
    guard revision >= 0, position == nil || (1...5).contains(position!) else { throw Failure(status: 400) }
    struct Body: Encodable { let context: String; let code: String; let word: String; let position: Int?; let revision: Int64 }
    return try await json(position == nil ? "DELETE" : "PUT", "/v1/users/me/dictionary/positions", token: token,
      body: JSONEncoder().encode(Body(context: context, code: code, word: word, position: position, revision: revision)))
  }

  private static func validPersonalCandidate(_ candidate: PersonalCandidate) -> Bool {
    validCandidateText(candidate.code, maximum: 256)
      && validCandidateText(candidate.word, maximum: 1024)
      && candidate.weight >= 0
      && candidate.canonical_pinyin.map { validCandidateText($0, maximum: 256, empty: true) } ?? true
  }

  private static func validCandidateText(_ value: String, maximum: Int, empty: Bool = false) -> Bool {
    (empty || !value.isEmpty)
      && value.utf8.count <= maximum
      && !value.unicodeScalars.contains { $0.properties.generalCategory == .control }
  }
}
