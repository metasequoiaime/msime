import XCTest
import UIKit

/// 社区页经 Rust 的 `msime_client_community`（#8201 迁移计划 4c）：发出的 action、回答的解码与校验、失败换成的提示。HTTP、令牌和匿名账号的选择在 Rust 里，由 host-api 和 client-services 的测试覆盖。
final class SkinCommunityTests: XCTestCase {
  private let skinID = "a1234567-1234-1234-1234-123456789abc"
  private let resourceID = "10000000-0000-4000-8000-000000000001"
  private var api: SkinCommunityAPI { SkinCommunityAPI(rust: { FakeRust.account }) }

  private func design(background: Int = 15266027) -> [String: Any] {
    ["background": background, "keyBackground": 16777215, "keyForeground": 1516829, "accent": 1596487,
     "actionBackground": 1596487, "cornerRadius": 8, "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false,
     "keyShape": NSNull(), "photo": NSNull()]
  }

  private func skin(_ id: String? = nil, name: String = "纸感", category: String? = "acg", owned: Bool = false) -> [String: Any] {
    var value: [String: Any] = ["id": id ?? skinID, "name": name, "description": "", "author": "作者", "design": design(),
                                "downloads": 2, "rating_count": 1, "rating_average": 4, "owned": owned, "my_rating": 0]
    if let category { value["category"] = category }
    return value
  }

  private func resource(_ id: String? = nil, kind: String = "dictionary", saved: Bool = false) -> [String: Any] {
    var content: [String: Any] = [:]
    if kind == "dictionary" { content["entries"] = [["kind": "pinyin", "code": "a", "word": "啊", "weight": 1]] }
    else { content["prompt"] = "请礼貌地回复。" }
    return ["id": id ?? resourceID, "kind": kind, "name": "词包", "description": "", "author": "作者", "content": content,
            "revision": 3, "saves": 1, "saved": saved, "owned": false, "rating_count": 0, "rating_average": 0, "my_rating": 0]
  }

  private func answerCommunity(_ table: [String: String]) {
    FakeRust.answer { call in
      guard call.entry == "community" else { return FakeRust.ok(["user": FakeRust.user("user-1")]) }
      return table[call.operation ?? ""] ?? FakeRust.failure("community_invalid")
    }
  }

  func testKeyboardSkinRequestsGoThroughRust() async throws {
    answerCommunity([
      "keyboard_skin_list": FakeRust.ok(["skins": [skin()], "has_more": false]),
      "keyboard_skin_detail": FakeRust.ok(skin(category: "food", owned: true)),
      "keyboard_skin_publish": FakeRust.ok(NSNull()),
      "keyboard_skin_set_category": FakeRust.ok(skin(category: "minimal", owned: true)),
      "keyboard_skin_download": FakeRust.ok(design()),
      "keyboard_skin_rate": FakeRust.ok(NSNull()),
      "keyboard_skin_unpublish": FakeRust.ok(NSNull()),
    ])
    let page = try await api.list(search: "纸感 & 星光")
    XCTAssertEqual(page.skins.first?.downloads, 2)
    XCTAssertEqual(page.skins.first?.category, .acg)
    XCTAssertNil(page.skins.first?.design.photo)
    _ = try await api.list(offset: 20, mine: true, category: .tech)
    let detail = try await api.detail(skinID)
    XCTAssertEqual(detail.category, .food)
    try await api.publish(id: skinID, name: "测试", description: "说明", design: CustomKeyboardSkin(), category: .cute)
    try await api.publish(id: skinID, name: "测试", description: "", design: CustomKeyboardSkin())
    let changed = try await api.setCategory(skinID, category: .minimal)
    XCTAssertEqual(changed.category, .minimal)
    let downloaded = try await api.download(skinID)
    XCTAssertEqual(downloaded.background, 15266027)
    try await api.rate(skinID, stars: 5)
    try await api.unpublish(skinID)

    let calls = FakeRust.calls.filter { $0.entry == "community" }
    XCTAssertTrue(calls.allSatisfy { $0.request["session_store"] as? Int == 7 && $0.request["directory"] == nil })
    let lists = FakeRust.calls("keyboard_skin_list")
    XCTAssertEqual(lists[0].action["search"] as? String, "纸感 & 星光")
    XCTAssertEqual(lists[0].action["mine"] as? Bool, false)
    XCTAssertNil(lists[0].action["category"])
    XCTAssertEqual(lists[1].action["offset"] as? Int, 20)
    XCTAssertEqual(lists[1].action["mine"] as? Bool, true)
    XCTAssertEqual(lists[1].action["category"] as? String, "tech")
    let publishes = FakeRust.calls("keyboard_skin_publish")
    XCTAssertEqual(publishes.map { $0.action["category"] as? String }, ["cute", "other"])
    XCTAssertEqual(publishes[0].action["description"] as? String, "说明")
    XCTAssertNotNil(publishes[0].action["design"] as? [String: Any], "设计以 JSON 对象交给 Rust")
    XCTAssertEqual(FakeRust.calls("keyboard_skin_set_category").first?.action["category"] as? String, "minimal")
    XCTAssertEqual(FakeRust.calls("keyboard_skin_rate").first?.action["stars"] as? Int, 5)
    XCTAssertEqual(FakeRust.calls("keyboard_skin_download").first?.action["id"] as? String, skinID)
  }

  func testResourceAndReportRequestsGoThroughRust() async throws {
    answerCommunity([
      "resource_list": FakeRust.ok(["items": [resource()], "has_more": true]),
      "resource_detail": FakeRust.ok(resource(saved: true)),
      "resource_publish": FakeRust.ok(["id": resourceID, "revision": 4]),
      "resource_save": FakeRust.ok(NSNull()),
      "resource_rate": FakeRust.ok(NSNull()),
      "resource_unpublish": FakeRust.ok(NSNull()),
      "report": FakeRust.ok(NSNull()),
    ])
    let page = try await api.resources(.dictionary, scope: "mine", search: "C++", offset: 20)
    XCTAssertEqual(page.items.first?.content.entries?.count, 1)
    XCTAssertTrue(page.has_more)
    let detail = try await api.resource(resourceID)
    XCTAssertTrue(detail.saved)
    let content = CommunityResourceContent(entries: [CommunityWord(PersonalWord(kind: .pinyin, key: "a", value: "啊", weight: 1))], prompt: nil)
    try await api.publishResource(id: resourceID, kind: .dictionary, name: "词包", description: "", content: content, revision: 3)
    try await api.saveResource(resourceID, saved: true)
    try await api.rateResource(resourceID, stars: 4)
    try await api.unpublishResource(resourceID)
    try await api.report(kind: "dictionaries", itemID: resourceID, reason: "其他", detail: "  合成说明\n")

    let list = try XCTUnwrap(FakeRust.calls("resource_list").first?.action)
    XCTAssertEqual(list["kind"] as? String, "dictionary")
    XCTAssertEqual(list["scope"] as? String, "mine")
    XCTAssertEqual(list["search"] as? String, "C++")
    XCTAssertEqual(list["offset"] as? Int, 20)
    let publish = try XCTUnwrap(FakeRust.calls("resource_publish").first?.action)
    let entries = try XCTUnwrap((publish["content"] as? [String: Any])?["entries"] as? [[String: Any]])
    XCTAssertEqual(entries.first?["word"] as? String, "啊")
    XCTAssertNil((publish["content"] as? [String: Any])?["prompt"], "没有提示词时不带这个键")
    XCTAssertEqual(FakeRust.calls("resource_save").first?.action["saved"] as? Bool, true)
    let report = try XCTUnwrap(FakeRust.calls("report").first?.action)
    XCTAssertEqual(report["kind"] as? String, "dictionaries")
    XCTAssertEqual(report["id"] as? String, resourceID)
    XCTAssertEqual(report["reason"] as? String, "其他")
    XCTAssertEqual(report["detail"] as? String, "合成说明")
  }

  /// 没登录时下载和举报照样交给 Rust，由它用本机匿名账号；这里不再要求先登录。
  func testSignedOutDownloadAndReportStillReachRust() async throws {
    FakeRust.answer { call in
      if call.entry == "account" { return FakeRust.ok(["user": NSNull()]) }
      switch call.operation {
      case "keyboard_skin_download": return FakeRust.ok(self.design())
      case "report": return FakeRust.ok(NSNull())
      default: return FakeRust.failure("community_invalid")
      }
    }
    let signedIn = try await api.signedIn()
    XCTAssertFalse(signedIn)
    _ = try await api.download(skinID)
    try await api.report(kind: "skins", itemID: skinID, reason: "其他", detail: "")
    XCTAssertEqual(FakeRust.calls("keyboard_skin_download").count, 1)
    XCTAssertEqual(FakeRust.calls("report").count, 1)
  }

  func testRustFailuresBecomeCommunityMessages() async throws {
    let cases: [(String, String)] = [
      ("community_unauthorized", "请先登录水杉账号，或登录已过期，请重新登录。"),
      ("community_not_found", "作品不存在或已下架。"),
      ("community_invalid", "请检查名称、词条或提示词是否符合要求。"),
      ("community_forbidden", "下载或收藏后才能评分，且不能评价自己的作品。"),
      ("community_conflict", "作品已更新或达到发布上限，请刷新后重试。"),
      ("community_rate_limited", "操作较频繁，请稍后重试。"),
      ("community_blocked_content", "内容包含不允许发布的词语，请修改后再提交"),
      ("community_screening_unavailable", "审核服务暂时不可用，请稍后重试"),
      ("community_account_banned", "该账号已被封禁，暂时无法使用账号相关功能"),
      ("community_unavailable", "社区暂时不可用，请稍后重试。"),
      ("rust_client_unavailable", "社区暂时不可用，请稍后重试。"),
    ]
    for (code, message) in cases {
      FakeRust.answer { _ in FakeRust.failure(code) }
      do { try await api.rate(skinID, stars: 3); XCTFail(code) }
      catch let failure as CommunityFailure { XCTAssertEqual(failure.message, message, code) }
    }
    // 请求进行中给出令牌的账号退出或换了号：作废，页面不提示。
    FakeRust.answer { _ in FakeRust.failure("community_cancelled") }
    do { _ = try await api.list(); XCTFail("cancelled") } catch is CancellationError { }
  }

  func testMalformedAnswersAreRejected() async throws {
    let rejected = "社区暂时不可用，请稍后重试。"
    func expectRejected(_ table: [String: String], _ run: () async throws -> Void, _ label: String) async {
      answerCommunity(table)
      do { try await run(); XCTFail(label) }
      catch let failure as CommunityFailure { XCTAssertEqual(failure.message, rejected, label) }
      catch { XCTFail("\(label): \(error)") }
    }
    await expectRejected(["keyboard_skin_list": FakeRust.ok(["skins": [skin(), skin()], "has_more": false])],
                         { _ = try await self.api.list() }, "duplicate skins")
    await expectRejected(["keyboard_skin_list": FakeRust.ok(["skins": Array(repeating: skin(), count: 21).enumerated().map {
      self.skin(String(format: "a1234567-1234-1234-1234-%012d", $0.offset + 1)) }, "has_more": false])],
                         { _ = try await self.api.list() }, "oversized page")
    await expectRejected(["keyboard_skin_list": FakeRust.ok(["skins": [skin(name: "")], "has_more": false])],
                         { _ = try await self.api.list() }, "empty name")
    await expectRejected(["keyboard_skin_list": FakeRust.ok(["skins": [], "has_more": true])],
                         { _ = try await self.api.list() }, "empty page with more")
    await expectRejected(["keyboard_skin_detail": FakeRust.ok(skin("b7654321-4321-4321-4321-cba987654321"))],
                         { _ = try await self.api.detail(self.skinID) }, "another skin's detail")
    await expectRejected(["keyboard_skin_detail": FakeRust.ok(["id": skinID])],
                         { _ = try await self.api.detail(self.skinID) }, "undecodable detail")
    await expectRejected(["keyboard_skin_set_category": FakeRust.ok(skin(category: "food"))],
                         { _ = try await self.api.setCategory(self.skinID, category: .minimal) }, "category not changed")
    await expectRejected(["keyboard_skin_download": FakeRust.ok(design(background: 0x1_000_000))],
                         { _ = try await self.api.download(self.skinID) }, "design out of range")
    await expectRejected(["resource_list": FakeRust.ok(["items": [resource(kind: "reply")], "has_more": false])],
                         { _ = try await self.api.resources(.dictionary) }, "wrong resource kind")
    await expectRejected(["resource_publish": FakeRust.ok(["id": "b7654321-4321-4321-4321-cba987654321", "revision": 4])],
                         { try await self.api.publishResource(id: self.resourceID, kind: .reply, name: "回复", description: "",
                                                              content: CommunityResourceContent(entries: nil, prompt: "提示"), revision: 0) },
                         "another publication")
  }

  func testInvalidLocalInputIsNotSent() async throws {
    FakeRust.answer { _ in FakeRust.ok(NSNull()) }
    for run in [{ try await self.api.rate("not-a-uuid", stars: 3) }, { try await self.api.rate(self.skinID, stars: 6) },
                { try await self.api.unpublish("") }, { try await self.api.saveResource("00000000-0000-0000-0000-000000000000", saved: true) }]
      as [() async throws -> Void] {
      do { try await run(); XCTFail("invalid input accepted") } catch is CommunityFailure { }
    }
    do { try await api.report(kind: "skins", itemID: skinID, reason: "不在列表里", detail: ""); XCTFail("bad reason") }
    catch let failure as CommunityFailure { XCTAssertEqual(failure.message, "请选择举报原因，补充说明不超过 1000 字。") }
    XCTAssertTrue(FakeRust.calls.isEmpty)
  }

  @MainActor func testCommunityPreviewDoesNotChangeActiveDesign() throws {
    let previous = CustomKeyboardSkinStore.current
    let backdrop = KeyboardSkinBackgroundView(frame: CGRect(x: 0, y: 0, width: 320, height: 240))
    var design = CustomKeyboardSkin.templates[2].1
    design.pattern = 2
    backdrop.skin = .designed(design)
    let before = UIGraphicsImageRenderer(bounds: backdrop.bounds).image { backdrop.layer.render(in: $0.cgContext) }.pngData()
    design.background = 0xEEFFFF
    design.gradientEnd = nil
    backdrop.skin = .designed(design)
    let after = UIGraphicsImageRenderer(bounds: backdrop.bounds).image { backdrop.layer.render(in: $0.cgContext) }.pngData()
    XCTAssertNotEqual(before, after)
    XCTAssertEqual(CustomKeyboardSkinStore.current, previous)
    XCTAssertEqual(CustomKeyboardSkin.rgb(try XCTUnwrap(backdrop.backgroundColor)), design.background)
  }
}

final class SkinCommunityCategoryTests: XCTestCase {
  private func skinJSON(category: String?) -> Data {
    var value: [String: Any] = [
      "id": "a1234567-1234-1234-1234-123456789abc", "name": "测试", "description": "", "author": "作者",
      "design": ["background": 15266027, "keyBackground": 16777215, "keyForeground": 1516829,
                 "accent": 1596487, "actionBackground": 1596487, "cornerRadius": 8,
                 "borderWidth": 0, "shadow": 0, "pattern": 0, "monospaced": false],
      "downloads": 0, "rating_count": 0, "rating_average": 0, "owned": false, "my_rating": 0
    ]
    if let category { value["category"] = category }
    return try! JSONSerialization.data(withJSONObject: value)
  }

  func testCategoryDecoding() throws {
    let decoder = JSONDecoder()
    XCTAssertEqual(try decoder.decode(CommunitySkin.self, from: skinJSON(category: "guofeng")).category, .guofeng)
    XCTAssertNil(try decoder.decode(CommunitySkin.self, from: skinJSON(category: nil)).category)
    // 服务端将来新增的分类读作 other，条目本身不能因此读取失败。
    XCTAssertEqual(try decoder.decode(CommunitySkin.self, from: skinJSON(category: "future-category")).category, .other)
    XCTAssertEqual(CommunitySkinCategory.allCases.map(\.rawValue),
                   ["nature", "guofeng", "acg", "cute", "food", "tech", "minimal", "other"])
    XCTAssertEqual(CommunitySkinCategory.allCases.map(\.label),
                   ["自然", "国风", "二次元", "可爱", "美食", "科技夜色", "简约", "其他"])
  }

  func testFailedCategoryLoadRestoresDisplayedSelection() {
    XCTAssertEqual(CommunitySkinCategorySelectionPolicy.afterFailedLoad(previous: .nature), .nature)
    XCTAssertNil(CommunitySkinCategorySelectionPolicy.afterFailedLoad(previous: nil))
  }
}
