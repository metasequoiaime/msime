import CMSIMEClient
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
  /// "approved", "pending" or "removed" on the user's own skins when the request asked for fields=moderation.
  var moderation: String? = nil
  /// Post-moderation: only a removal is shown to the author, never a pending state or a reason.
  var removed: Bool { owned && moderation == "removed" }
  /// 发布分类。每个返回皮肤条目的请求都带 `include=category`；早于分类功能的服务端不返回该字段，此时为 `nil`。
  var category: CommunitySkinCategory? = nil
}

/// 社区键盘皮肤的发布分类，与候选窗口皮肤共用同一组固定 id。分类只是发布元数据，不计入任何请求摘要。
enum CommunitySkinCategory: String, CaseIterable, Identifiable, Codable, Sendable {
  case nature, guofeng, acg, cute, food, tech, minimal, other

  var id: String { rawValue }

  /// 界面上显示的分类名称，`allCases` 的顺序即筛选按钮的顺序。
  var label: String {
    switch self {
    case .nature: "自然"
    case .guofeng: "国风"
    case .acg: "二次元"
    case .cute: "可爱"
    case .food: "美食"
    case .tech: "科技夜色"
    case .minimal: "简约"
    case .other: "其他"
    }
  }

  /// 服务端将来新增的分类 id 一律读作 `other`，旧客户端不会因此丢掉整个条目。
  init(from decoder: Decoder) throws {
    let value = try decoder.singleValueContainer().decode(String.self)
    self = Self(rawValue: value) ?? .other
  }
}

enum CommunitySkinCategorySelectionPolicy {
  static func afterFailedLoad(previous: CommunitySkinCategory?) -> CommunitySkinCategory? {
    previous
  }
}

struct CommunityPage: Decodable, Sendable { let skins: [CommunitySkin]; let has_more: Bool }
struct CommunityChallenge: Decodable, Sendable { let challenge_id: String; let nonce: String }
typealias CommunityUser = BackendAccountClient.User
typealias CommunityProfile = BackendRustAccount.Profile
struct CommunityFailure: LocalizedError {
  let message: String
  var errorDescription: String? { message }
}

enum CommunityResponseValidation {
  static func validSkin(_ skin: CommunitySkin) -> Bool {
    CommunityValidation.validID(skin.id)
      && CommunityValidation.validText(skin.name, minimum: 1, maximum: 32, multiline: false, trimmed: true)
      && CommunityValidation.validText(skin.description, minimum: 0, maximum: 280, multiline: true)
      && CommunityValidation.validText(skin.author, minimum: 1, maximum: 128, multiline: false, trimmed: true)
      && skin.downloads >= 0 && skin.downloads <= CommunityValidation.maximumJavaScriptInteger
      && CommunityValidation.validRating(count: skin.rating_count, average: skin.rating_average, mine: skin.my_rating)
  }

  static func validResource(_ item: CommunityResource, expectedKind: CommunityResourceKind) -> Bool {
    guard item.kind == expectedKind, CommunityValidation.validID(item.id), item.revision > 0,
          CommunityValidation.validText(item.name, minimum: 1, maximum: 32, multiline: false, trimmed: true),
          CommunityValidation.validText(item.description, minimum: 0, maximum: 280, multiline: true),
          CommunityValidation.validText(item.author, minimum: 1, maximum: 128, multiline: false, trimmed: true),
          item.saves >= 0, item.saves <= CommunityValidation.maximumJavaScriptInteger,
          CommunityValidation.validRating(count: item.rating_count, average: item.rating_average, mine: item.my_rating)
    else { return false }
    switch item.kind {
    case .reply:
      return (item.content.entries ?? []).isEmpty
        && item.content.prompt.map { CommunityValidation.validText($0, minimum: 1, maximum: 2_000, multiline: true) } == true
    case .dictionary:
      guard item.content.prompt == nil, let entries = item.content.entries,
            (1...128).contains(entries.count) else { return false }
      var seen = Set<String>()
      return entries.allSatisfy { entry in
        ["pinyin", "wubi", "quick", "english"].contains(entry.kind)
          && CommunityValidation.validText(entry.code, minimum: 1, maximum: 256, multiline: false)
          && CommunityValidation.validText(entry.word, minimum: 1, maximum: 1_024, multiline: false)
          && entry.weight >= 0
          && seen.insert("\(entry.kind)|\(entry.code)|\(entry.word)").inserted
      }
    }
  }
}

enum CommunityProfilePolicy {
  struct NamePresentation: Decodable {
    let normalized: String
    let valid: Bool
    let maximumCharacters: Int

    enum CodingKeys: String, CodingKey {
      case normalized, valid
      case maximumCharacters = "maximum_chars"
    }
  }

  /// 昵称规则由共享 Rust 决定；纯计算不读会话、文件或网络，可以同步更新输入提示。
  static func present(_ value: String) throws -> NamePresentation {
    let request = try BackendRustClient.request(["name": value, "id": ""])
    let raw = request.withUnsafeBytes { bytes in
      msime_client_account_name(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
    }
    guard let raw else { throw BackendRustClient.Failure.invalidResponse }
    defer { msime_client_string_free(raw) }
    let data = Data(bytes: raw, count: strlen(raw))
    return try JSONDecoder().decode(NamePresentation.self, from: BackendRustClient.envelope(data).get())
  }
}

/// 键盘皮肤社区、社区词包与回复模板、举报和账号操作，都经 Rust 的 `msime_client_community` 和 `msime_client_account`（#8201 迁移计划 4b、4c），令牌不出 Rust。键盘皮肤的下载和举报登录账号优先、没有登录时 Rust 用本机匿名账号，其余要登录的操作只认登录账号。请求进行中给出令牌的账号退出或换了号时 Rust 报 `community_cancelled`，这里按取消处理，页面不提示。
actor SkinCommunityAPI {
  static let shared = SkinCommunityAPI()
  private static let maximumPageItems = 20
  /// 按账号取消键盘待处理的词库快照，默认操作 App Group 里的共享队列；测试传入可观察或会失败的实现。
  private let cancelSnapshot: @Sendable (String) throws -> Void
  /// 默认是 App 启动时注册的那一份，测试传入替身函数表上的账号。
  private let rust: @Sendable () throws -> BackendRustAccount
  init(rust: @escaping @Sendable () throws -> BackendRustAccount = { try BackendRustAccount.current() },
       cancelSnapshot: @escaping @Sendable (String) throws -> Void = {
         try DictionarySnapshotQueue().cancelIfPresent(accountID: $0)
       }) {
    self.rust = rust
    self.cancelSnapshot = cancelSnapshot
  }
  /// 登录的账号（Rust 的 `status`，只读存储、不联网）；读不了登录状态时抛 `account_storage`，不当作没登录。
  func currentUser() async throws -> CommunityUser? { try await rust().status() }
  func signedIn() async throws -> Bool { try await currentUser() != nil }

  /// 发一个社区 action，按 `T` 读回答；失败换成社区页面的提示，`community_cancelled` 换成 `CancellationError`。
  private func community<T: Decodable>(_ action: [String: Any], as type: T.Type) async throws -> T {
    let data = try await communityData(action)
    do { return try JSONDecoder().decode(T.self, from: data) }
    catch { throw CommunityFailure(message: "社区暂时不可用，请稍后重试。") }
  }

  /// 回答是 `null` 的社区 action。
  private func community(_ action: [String: Any]) async throws {
    _ = try await communityData(action)
  }

  private func communityData(_ action: [String: Any]) async throws -> Data {
    let data: Data
    do { data = try await rust().community(action) }
    catch let failure as BackendRustClient.Failure { throw try Self.failure(failure) }
    try Task.checkCancellation()
    return data
  }

  /// Rust 的社区错误码换成页面上的一句话。服务端更细的码（例如「下载使用后才能评分」「最多发布 50 款」）Rust 归进 `community_forbidden`、`community_conflict`，这里按归并后的码说。
  static func failure(_ failure: BackendRustClient.Failure) throws -> CommunityFailure {
    let moderation = BackendAccountClient.Failure.moderationMessages
    let message: String
    switch failure.code {
    case "community_cancelled", "account_cancelled": throw CancellationError()
    case "community_blocked_content", "account_blocked_content": message = moderation["blocked_content"]!
    case "community_screening_unavailable", "account_screening_unavailable": message = moderation["screening_unavailable"]!
    case "community_account_banned", "account_banned": message = moderation["account_banned"]!
    case "community_unauthorized", "account_unauthorized": message = "请先登录水杉账号，或登录已过期，请重新登录。"
    case "community_not_found", "account_not_found": message = "作品不存在或已下架。"
    case "community_invalid", "account_invalid": message = "请检查名称、词条或提示词是否符合要求。"
    case "community_forbidden": message = "下载或收藏后才能评分，且不能评价自己的作品。"
    case "community_conflict", "account_conflict": message = "作品已更新或达到发布上限，请刷新后重试。"
    case "community_rate_limited", "account_rate_limited": message = "操作较频繁，请稍后重试。"
    case "account_storage": message = failure.localizedDescription
    default: message = "社区暂时不可用，请稍后重试。"
    }
    return CommunityFailure(message: message)
  }

  /// Apple 原生登录的挑战（`apple_challenge`）：nonce 放进 `ASAuthorizationAppleIDRequest`，challenge id 放进 state。
  func challenge() async throws -> CommunityChallenge {
    let value = try BackendRustAccount.challenge(try await rust().account(["operation": "apple_challenge"]))
    guard let nonce = value.nonce, !nonce.isEmpty else { throw CommunityFailure(message: "Apple 登录暂不可用，请稍后重试。") }
    return CommunityChallenge(challenge_id: value.challenge_id, nonce: nonce)
  }
  /// 用 Apple 的 identity token 登录（`apple_login`）。登录换了账号时取消上一个账号排队中的词库快照：Rust 的登录没有「发布新身份之前」的清理钩子，由这里在登录以后补上。
  func login(challenge: String, identityToken: String) async throws {
    _ = try await Self.signIn(rust(), cancelSnapshot: cancelSnapshot) { account in
      try await account.account(["operation": "apple_login", "challenge_id": challenge, "identity_token": identityToken])
    }
  }
  /// 经 Rust 登录（验证码、Apple、Google 都走这里），返回登录后的账号。登录之前读一次 `status` 记下原来的账号，登录成功而账号不同时取消原来那个账号排队中的词库快照；取消失败只记诊断，不让已经成功的登录报错。原来的账号读不了时（钥匙串锁着）不知道该取消谁，不取消。
  static func signIn(_ account: BackendRustAccount, cancelSnapshot: @escaping @Sendable (String) throws -> Void,
                     _ login: (BackendRustAccount) async throws -> Data) async throws -> CommunityUser {
    let previous = try? await account.status()?.id
    let user = try BackendRustAccount.signedInUser(try await login(account))
    if let previous, previous != user.id { Self.removedAccountCleanup("login", cancel: cancelSnapshot)(previous) }
    return user
  }
  func profile() async throws -> CommunityProfile {
    try BackendRustAccount.profile(try await rust().account(["operation": "profile"]))
  }
  func updateProfile(name: String) async throws -> CommunityProfile {
    guard let presentation = try? CommunityProfilePolicy.present(name) else {
      throw CommunityFailure(message: "昵称无法校验，请缩短后重试。")
    }
    guard presentation.valid else {
      throw CommunityFailure(message: "昵称需为 1–\(presentation.maximumCharacters) 个字符，不能包含换行或控制字符。")
    }
    return try BackendRustAccount.profile(try await rust().account(["operation": "rename", "display_name": presentation.normalized]))
  }
  /// 退出（`logout`，`all` 为 true 时退出所有设备）或注销（`account_delete`）。Rust 退出时总是先清掉本机会话再请服务端撤销，所以断网时本机已经退出、这里仍然抛出那次失败；注销需要最近登录时是 `account_recent_login_required`，本机登录留着。本机会话清掉以后取消原来那个账号排队中的词库快照。
  func logout(deleteAccount: Bool = false, all: Bool = false) async throws {
    let account = try rust()
    let previous = try? await account.status()?.id
    let cancel = cancelSnapshot
    let cleanup = {
      if let previous { Self.removedAccountCleanup(deleteAccount ? "delete_account" : "logout", cancel: cancel)(previous) }
    }
    let action: [String: Any] = deleteAccount ? ["operation": "account_delete"] : ["operation": "logout", "all": all]
    do {
      _ = try await account.account(action)
    } catch {
      // 退出时断网：本机会话已经清掉，快照照样取消。
      if (try? await account.status()) == .some(nil) { cleanup() }
      throw error
    }
    cleanup()
  }
  /// 只清掉本机会话（`forget`），给读不出来或已经失效的登录。
  func clearExpiredLogin() async throws {
    let account = try rust()
    let previous = try? await account.status()?.id
    _ = try await account.account(["operation": "forget"])
    if let previous { Self.removedAccountCleanup("clear_expired_login", cancel: cancelSnapshot)(previous) }
  }
  /// 退出登录、注销账号和清除失效登录时取消旧账号的快照。用户已决定离开、远端账号已删除或登录已失效，取消失败也不能让本地会话停在旧账号上，所以只记一条诊断再继续；诊断只含操作名和失败类别，不含账号、路径或错误原文。
  static func removedAccountCleanup(_ operation: String, cancel: @escaping @Sendable (String) throws -> Void) -> @Sendable (String) -> Void {
    return { accountID in
      do { try cancel(accountID) } catch {
        let reason = (error as? DictionarySnapshotQueue.Failure).map { "\($0)" } ?? "other"
        let log = DiagnosticLog.shared
        log.configure(directory: MetasequoiaInputSessionBridge.sharedStateDirectory,
                      enabled: DiagnosticLog.isEnabled(in: MetasequoiaInputSessionBridge.loadSharedPreferences()))
        log.write("snapshot_cancel_failed operation=\(operation) reason=\(reason)")
      }
    }
  }
  /// `category` 为 `nil` 时不按分类筛选。`mine` 列出自己的作品（要登录），带着审核状态，已下架的能标出来。
  func list(offset: Int = 0, search: String = "", mine: Bool = false,
            category: CommunitySkinCategory? = nil) async throws -> CommunityPage {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled {
      return CommunityPage(skins: CommunityPreviewFixtures.skins.filter { category == nil || $0.category == category }, has_more: false)
    }
    #endif
    var action: [String: Any] = ["operation": "keyboard_skin_list", "offset": offset, "search": search, "mine": mine]
    if let category { action["category"] = category.rawValue }
    let page = try await community(action, as: CommunityPage.self)
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
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let skin = try await community(["operation": "keyboard_skin_detail", "id": id], as: CommunitySkin.self)
    guard CommunityResponseValidation.validSkin(skin),
          CommunityValidation.matchesID(skin.id, requested: id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return skin
  }
  func publish(id: String, name: String, description: String, design: CustomKeyboardSkin,
               category: CommunitySkinCategory = .other) async throws {
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "keyboard_skin_publish", "id": id, "name": name, "description": description,
                         "design": try Self.json(design.normalized), "category": category.rawValue])
  }
  /// 作者修改自己作品的分类，返回更新后的条目。返回的条目不带 `moderation`，调用方要沿用原条目的审核状态。
  func setCategory(_ id: String, category: CommunitySkinCategory) async throws -> CommunitySkin {
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let skin = try await community(["operation": "keyboard_skin_set_category", "id": id, "category": category.rawValue],
                                   as: CommunitySkin.self)
    guard CommunityResponseValidation.validSkin(skin),
          CommunityValidation.matchesID(skin.id, requested: id), skin.category == category else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return skin
  }
  /// 领取并记一次下载。没有登录账号时 Rust 用设备的匿名身份，与 Android 的「获取」一致；评分仍要登录。
  func download(_ id: String) async throws -> CustomKeyboardSkin {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled, let skin = CommunityPreviewFixtures.skins.first(where: { $0.id == id }) { return skin.design }
    #endif
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let design = try await community(["operation": "keyboard_skin_download", "id": id], as: CustomKeyboardSkin.self)
    guard design == design.normalized else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return design.normalized
  }
  func rate(_ id: String, stars: Int) async throws {
    guard CommunityValidation.validID(id), (1...5).contains(stars) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "keyboard_skin_rate", "id": id, "stars": stars])
  }
  func unpublish(_ id: String) async throws {
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "keyboard_skin_unpublish", "id": id])
  }
  struct ResourcePage: Decodable { let items: [CommunityResource]; let has_more: Bool }
  func resources(_ kind: CommunityResourceKind, scope: String = "", search: String = "", offset: Int = 0) async throws -> ResourcePage {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled {
      return ResourcePage(items: CommunityPreviewFixtures.items.filter { $0.kind == kind && (scope != "saved" || $0.saved) && (scope != "mine" || $0.owned) }, has_more: false)
    }
    #endif
    let page = try await community(["operation": "resource_list", "kind": kind.rawValue, "scope": scope,
                                    "search": search, "offset": offset], as: ResourcePage.self)
    guard Self.validPage(page.items, hasMore: page.has_more),
          page.items.allSatisfy({ CommunityResponseValidation.validResource($0, expectedKind: kind) }) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return page
  }

  private static func validPage<T: Identifiable>(_ items: [T], hasMore: Bool) -> Bool where T.ID == String {
    guard items.count <= maximumPageItems, !(hasMore && items.isEmpty) else { return false }
    var ids = Set<String>()
    return items.allSatisfy { ids.insert($0.id.lowercased()).inserted }
  }

  /// 编成 JSON 对象，放进请求的 action 里。
  private static func json<T: Encodable>(_ value: T) throws -> Any {
    guard let object = try? JSONSerialization.jsonObject(with: JSONEncoder().encode(value)) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return object
  }

  func resource(_ id: String) async throws -> CommunityResource {
    #if DEBUG && targetEnvironment(simulator)
    if CommunityPreviewFixtures.enabled, let item = CommunityPreviewFixtures.items.first(where: { $0.id == id }) { return item }
    #endif
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let resource = try await community(["operation": "resource_detail", "id": id], as: CommunityResource.self)
    guard CommunityResponseValidation.validResource(resource, expectedKind: resource.kind),
          CommunityValidation.matchesID(resource.id, requested: id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    return resource
  }
  func publishResource(id: String, kind: CommunityResourceKind, name: String, description: String,
                       content: CommunityResourceContent, revision: Int) async throws {
    struct Result: Decodable { let id: String; let revision: Int }
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    let result = try await community(["operation": "resource_publish", "id": id, "kind": kind.rawValue, "name": name,
                                      "description": description, "content": try Self.json(content), "revision": revision],
                                     as: Result.self)
    guard CommunityValidation.matchesID(result.id, requested: id), result.revision > 0 else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
  }
  func saveResource(_ id: String, saved: Bool) async throws {
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "resource_save", "id": id, "saved": saved])
  }
  func rateResource(_ id: String, stars: Int) async throws {
    guard CommunityValidation.validID(id), (1...5).contains(stars) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "resource_rate", "id": id, "stars": stars])
  }
  /// 举报别人的作品。登录不登录都行：没有登录账号时 Rust 用设备的匿名账号发。
  func report(kind: String, itemID: String, reason: String, detail: String) async throws {
    let detail = detail.trimmingCharacters(in: .whitespacesAndNewlines)
    guard CommunityValidation.validID(itemID), BackendAccountClient.reportReasons.contains(reason),
          CommunityValidation.validText(detail, minimum: 0, maximum: 1_000, multiline: true) else {
      throw CommunityFailure(message: "请选择举报原因，补充说明不超过 1000 字。")
    }
    try await community(["operation": "report", "kind": kind, "id": itemID, "reason": reason, "detail": detail])
  }
  func unpublishResource(_ id: String) async throws {
    guard CommunityValidation.validID(id) else {
      throw CommunityFailure(message: "社区暂时不可用，请稍后重试。")
    }
    try await community(["operation": "resource_unpublish", "id": id])
  }
}
