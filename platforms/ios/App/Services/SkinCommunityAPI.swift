import Foundation

struct CommunitySkin: Codable, Identifiable, Sendable {
  let id: String
  let name: String
  let description: String
  let author: String
  let design: CustomKeyboardSkin
  let downloads: Int
  let rating_count: Int
  let rating_average: Double
  let owned: Bool
  let my_rating: Int
}
struct CommunityPage: Decodable, Sendable { let skins: [CommunitySkin]; let has_more: Bool }
struct CommunityChallenge: Decodable, Sendable { let challenge_id: String; let nonce: String }
typealias CommunityUser = BackendAccountClient.User
typealias CommunityProfile = BackendAccountClient.Profile
struct CommunityFailure: LocalizedError {
  let message: String
  var errorDescription: String? { message }
}

enum CommunityResponseValidation {
  private static let maximumJavaScriptInteger = 9_007_199_254_740_991

  static func validID(_ value: String) -> Bool {
    guard let id = UUID(uuidString: value) else { return false }
    return id.uuidString != "00000000-0000-0000-0000-000000000000"
  }

  static func matchesID(_ value: String, requested: String) -> Bool {
    guard validID(value), validID(requested),
          let valueID = UUID(uuidString: value), let requestedID = UUID(uuidString: requested)
    else { return false }
    return valueID == requestedID
  }

  static func validText(_ value: String, minimum: Int, maximum: Int,
                        multiline: Bool, trimmed: Bool = false) -> Bool {
    let scalars = value.unicodeScalars
    guard (minimum...maximum).contains(scalars.count),
          (!trimmed || value == value.trimmingCharacters(in: .whitespacesAndNewlines)) else {
      return false
    }
    return !scalars.contains { scalar in
      CharacterSet.controlCharacters.contains(scalar)
        && !(multiline && (scalar == "\n" || scalar == "\t"))
    }
  }

  static func validRating(count: Int, average: Double, mine: Int) -> Bool {
    count >= 0 && count <= maximumJavaScriptInteger && (0...5).contains(mine)
      && average.isFinite && (0...5).contains(average)
      && (count != 0 || average == 0)
  }

  static func validSkin(_ skin: CommunitySkin) -> Bool {
    validID(skin.id)
      && validText(skin.name, minimum: 1, maximum: 32, multiline: false, trimmed: true)
      && validText(skin.description, minimum: 0, maximum: 280, multiline: true)
      && validText(skin.author, minimum: 1, maximum: 128, multiline: false, trimmed: true)
      && skin.downloads >= 0 && skin.downloads <= maximumJavaScriptInteger
      && validRating(count: skin.rating_count, average: skin.rating_average, mine: skin.my_rating)
  }

  static func validResource(_ item: CommunityResource, expectedKind: CommunityResourceKind) -> Bool {
    guard item.kind == expectedKind, validID(item.id), item.revision > 0,
          validText(item.name, minimum: 1, maximum: 32, multiline: false, trimmed: true),
          validText(item.description, minimum: 0, maximum: 280, multiline: true),
          validText(item.author, minimum: 1, maximum: 128, multiline: false, trimmed: true),
          item.saves >= 0, item.saves <= maximumJavaScriptInteger,
          validRating(count: item.rating_count, average: item.rating_average, mine: item.my_rating)
    else { return false }
    switch item.kind {
    case .reply:
      return (item.content.entries ?? []).isEmpty
        && item.content.prompt.map { validText($0, minimum: 1, maximum: 2_000, multiline: true) } == true
    case .dictionary:
      guard item.content.prompt == nil, let entries = item.content.entries,
            (1...128).contains(entries.count) else { return false }
      var seen = Set<String>()
      return entries.allSatisfy { entry in
        ["pinyin", "wubi", "quick", "english"].contains(entry.kind)
          && validText(entry.code, minimum: 1, maximum: 256, multiline: false)
          && validText(entry.word, minimum: 1, maximum: 1_024, multiline: false)
          && entry.weight >= 0
          && seen.insert("\(entry.kind)|\(entry.code)|\(entry.word)").inserted
      }
    }
  }
}

enum CommunityProfilePolicy {
  static let maximumNameScalars = 64

  static func normalizedName(_ value: String) -> String {
    value.trimmingCharacters(in: .whitespacesAndNewlines)
  }

  static func validName(_ value: String) -> Bool {
    let name = normalizedName(value)
    return !name.isEmpty && name.unicodeScalars.count <= maximumNameScalars
      && !name.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
  }
}

actor SkinCommunityAPI {
  static let shared = SkinCommunityAPI()
  private static let maximumPageItems = 20
  private let client: BackendAccountClient
  private let account: BackendAccountSession
  init(client: BackendAccountClient = BackendAccountClient(), account: BackendAccountSession = .shared) {
    self.client = client; self.account = account
  }
  func currentUser() async throws -> CommunityUser? { try await account.user() }
  func signedIn() async throws -> Bool { try await account.user() != nil }
  private func request<T: Decodable>(_ path: String, method: String = "GET", body: Data? = nil, authenticated: Bool = false, maximumResponseBytes: Int = 1024 * 1024) async throws -> T {
    // Reading the session throws the same type a request does, so it has to sit inside the same
    // conversion. Outside it, an unreadable keychain surfaced BackendAccountClient's status-0
    // fallback — 请求未完成 — in place of anything this screen could say about the community.
    var identity: (userID: String, token: String)?
    let data: Data
    do {
      if try await account.user() != nil { identity = try await account.credentials() }
      let token = identity?.token
      if authenticated && token == nil { throw CommunityFailure(message: "请先使用 Apple 登录。") }
      do { data = try await client.request(method, path, token: token, body: body, maximumResponseBytes: maximumResponseBytes) }
      catch let error as BackendAccountClient.Failure where error.status == 401 && token != nil {
        guard let identity else { throw CancellationError() }
        let fresh = try await account.credentials(retrying: token, matchingUserID: identity.userID)
        data = try await client.request(method, path, token: fresh.token, body: body, maximumResponseBytes: maximumResponseBytes)
      }
      guard try await account.user()?.id == identity?.userID else { throw CancellationError() }
    } catch let error as BackendAccountClient.Failure { throw Self.failure(Data(), status: error.status) }
    try Task.checkCancellation()
    return try JSONDecoder().decode(T.self, from: data.isEmpty ? Data("{}".utf8) : data)
  }
  private static func failure(_ data: Data, status: Int) -> CommunityFailure {
    let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    let code = (root?["error"] as? [String: String])?["code"] ?? ""
    let message: String
    switch code {
    case "provider_disabled", "user_auth_disabled": message = "社区登录尚未启用，请稍后重试。"
    case "download_before_rating_or_own_skin": message = "下载使用后才能评分，且不能评价自己的作品。"
    case "skin_publish_limit": message = "最多发布 50 款皮肤，请先下架部分作品。"
    case "recent_login_required": message = "请退出并重新使用 Apple 登录后，再注销账号。"
    case "invalid_skin_design", "invalid_skin_metadata": message = "皮肤内容或名称不符合发布要求。"
    default:
      switch status {
      case 401: message = "登录已过期，请重新使用 Apple 登录。"
      case 404: message = "作品不存在或已下架。"
      case 400: message = "请检查名称、词条或提示词是否符合要求。"
      case 403: message = "收藏后才能评分，且不能评价自己的作品。"
      case 409: message = "作品已更新或达到发布上限，请刷新后重试。"
      case 429: message = "操作较频繁，请稍后重试。"
      default: message = "社区暂时不可用，请稍后重试。"
      }
    }
    return CommunityFailure(message: message)
  }
  func challenge() async throws -> CommunityChallenge {
    let value = try await client.challenge(provider: "apple")
    guard let nonce = value.nonce else { throw CommunityFailure(message: "Apple 登录暂不可用，请稍后重试。") }
    return CommunityChallenge(challenge_id: value.challenge_id, nonce: nonce)
  }
  func login(challenge: String, identityToken: String) async throws {
    try await account.signIn(challenge: challenge, credential: identityToken)
  }
  func profile() async throws -> CommunityProfile {
    let token = try await account.accessToken()
    let profile = try await client.profile(token: token)
    try await account.updateUser(profile.user, matching: token)
    return profile
  }
  func updateProfile(name: String) async throws -> CommunityProfile {
    let name = CommunityProfilePolicy.normalizedName(name)
    guard CommunityProfilePolicy.validName(name) else {
      throw CommunityFailure(message: "昵称需为 1–64 个字符，不能包含换行或控制字符。")
    }
    let token = try await account.accessToken()
    try await client.rename(name, token: token)
    let profile = try await client.profile(token: token)
    try await account.updateUser(profile.user, matching: token)
    return profile
  }
  func logout(deleteAccount: Bool = false, all: Bool = false) async throws {
    if deleteAccount {
      let token = try await account.accessToken()
      try await client.deleteAccount(token: token)
      try await account.forget()
    } else { try await account.logout(all: all) }
  }
  func clearExpiredLogin() async throws { try await account.forget() }
  func list(offset: Int = 0, search: String = "") async throws -> CommunityPage {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled { return CommunityPage(skins: CommunityPreviewFixtures.skins, has_more: false) }
    #endif
    var parts = URLComponents()
    parts.path = "/v1/community/skins"
    parts.queryItems = [.init(name: "offset", value: String(offset)), .init(name: "q", value: search)]
    parts.percentEncodedQuery = parts.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
    let page: CommunityPage = try await request(parts.string!)
    guard Self.validPage(page.skins, hasMore: page.has_more),
          page.skins.allSatisfy(CommunityResponseValidation.validSkin) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return page
  }
  func detail(_ id: String) async throws -> CommunitySkin {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled, let skin = CommunityPreviewFixtures.skins.first(where: { $0.id == id }) { return skin }
    #endif
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let skin: CommunitySkin = try await request("/v1/community/skins/\(id)")
    guard CommunityResponseValidation.validSkin(skin),
          CommunityResponseValidation.matchesID(skin.id, requested: id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return skin
  }
  func publish(id: String, name: String, description: String, design: CustomKeyboardSkin) async throws {
    struct Payload: Encodable { let id: String; let name: String; let description: String; let design: CustomKeyboardSkin }
    struct Result: Decodable { let id: String }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/skins", method: "POST", body: JSONEncoder().encode(Payload(id: id, name: name, description: description, design: design.normalized)), authenticated: true)
    guard CommunityResponseValidation.matchesID(result.id, requested: id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func download(_ id: String) async throws -> CustomKeyboardSkin {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled, let skin = CommunityPreviewFixtures.skins.first(where: { $0.id == id }) { return skin.design }
    #endif
    struct Result: Decodable, Sendable { let design: CustomKeyboardSkin }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/skins/\(id)/download", method: "POST", body: Data("{}".utf8), authenticated: true)
    guard result.design == result.design.normalized else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return result.design.normalized
  }
  func rate(_ id: String, stars: Int) async throws {
    struct Result: Decodable { let stars: Int }
    guard CommunityResponseValidation.validID(id), (1...5).contains(stars) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/skins/\(id)/rating", method: "PUT", body: JSONSerialization.data(withJSONObject: ["stars": stars]), authenticated: true)
    guard result.stars == stars else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func unpublish(_ id: String) async throws {
    struct Result: Decodable { let deleted: Bool }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/skins/\(id)", method: "DELETE", authenticated: true)
    guard result.deleted else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  struct ResourcePage: Decodable { let items: [CommunityResource]; let has_more: Bool }
  func resources(_ kind: CommunityResourceKind, scope: String = "", search: String = "", offset: Int = 0) async throws -> ResourcePage {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled {
      return ResourcePage(items: CommunityPreviewFixtures.items.filter { $0.kind == kind && (scope != "saved" || $0.saved) && (scope != "mine" || $0.owned) }, has_more: false)
    }
    #endif
    var parts = URLComponents(); parts.path = "/v1/community/resources"
    parts.queryItems = [.init(name: "kind", value: kind.rawValue), .init(name: "scope", value: scope),
      .init(name: "q", value: search), .init(name: "offset", value: String(offset))]
    parts.percentEncodedQuery = parts.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
    let page: ResourcePage = try await request(parts.string!, maximumResponseBytes: 48 * 1024 * 1024)
    guard Self.validPage(page.items, hasMore: page.has_more),
          page.items.allSatisfy({ CommunityResponseValidation.validResource($0, expectedKind: kind) }) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return page
  }

  private static func validPage<T: Identifiable>(_ items: [T], hasMore: Bool) -> Bool {
    guard items.count <= maximumPageItems, !(hasMore && items.isEmpty) else { return false }
    var ids = Set<AnyHashable>()
    return items.allSatisfy { ids.insert(AnyHashable($0.id)).inserted }
  }

  func resource(_ id: String) async throws -> CommunityResource {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled, let item = CommunityPreviewFixtures.items.first(where: { $0.id == id }) { return item }
    #endif
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let resource: CommunityResource = try await request("/v1/community/resources/\(id)", maximumResponseBytes: 3 * 1024 * 1024)
    guard CommunityResponseValidation.validResource(resource, expectedKind: resource.kind),
          CommunityResponseValidation.matchesID(resource.id, requested: id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return resource
  }
  func publishResource(id: String, kind: CommunityResourceKind, name: String, description: String,
                       content: CommunityResourceContent, revision: Int) async throws {
    struct Payload: Encodable {
      let id: String; let kind: CommunityResourceKind; let name: String; let description: String
      let content: CommunityResourceContent; let revision: Int
    }
    struct Result: Decodable { let id: String; let revision: Int }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/resources", method: "POST", body: JSONEncoder().encode(
      Payload(id: id, kind: kind, name: name, description: description, content: content, revision: revision)), authenticated: true)
    guard CommunityResponseValidation.matchesID(result.id, requested: id), result.revision > 0 else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func saveResource(_ id: String, saved: Bool) async throws {
    struct Result: Decodable { let saved: Bool }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/resources/\(id)/save", method: "PUT",
      body: JSONSerialization.data(withJSONObject: ["saved": saved]), authenticated: true)
    guard result.saved == saved else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func rateResource(_ id: String, stars: Int) async throws {
    struct Result: Decodable { let stars: Int }
    guard CommunityResponseValidation.validID(id), (1...5).contains(stars) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/resources/\(id)/rating", method: "PUT",
      body: JSONSerialization.data(withJSONObject: ["stars": stars]), authenticated: true)
    guard result.stars == stars else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func unpublishResource(_ id: String) async throws {
    struct Result: Decodable { let deleted: Bool }
    guard CommunityResponseValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result: Result = try await request("/v1/community/resources/\(id)", method: "DELETE", authenticated: true)
    guard result.deleted else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }

}
