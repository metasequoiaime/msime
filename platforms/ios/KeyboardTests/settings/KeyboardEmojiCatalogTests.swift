import XCTest
import UIKit

@MainActor
final class KeyboardEmojiCatalogTests: XCTestCase {
  private var savedRecents: [String: Any] = [:]
  private let recentKeys = [KeyboardEmojiRecents.key, KeyboardSymbolRecents.key, KeyboardKaomojiRecents.key]

  override func setUp() {
    super.setUp()
    for key in recentKeys {
      savedRecents[key] = KeyboardFeedbackPreference.defaults.object(forKey: key)
      KeyboardFeedbackPreference.defaults.removeObject(forKey: key)
    }
  }

  override func tearDown() {
    for key in recentKeys {
      if let saved = savedRecents[key] {
        KeyboardFeedbackPreference.defaults.set(saved, forKey: key)
      } else {
        KeyboardFeedbackPreference.defaults.removeObject(forKey: key)
      }
    }
    super.tearDown()
  }

  func testCategoryOrderPageValidationAndBounds() throws {
    XCTAssertEqual(KeyboardEmojiCatalog.categories.map(\.title),
                   ["笑脸", "人物", "动物", "食物", "旅行", "活动", "物品", "符号", "旗帜"])
    XCTAssertEqual(KeyboardEmojiCatalog.columns, 8)
    let category = try XCTUnwrap(KeyboardEmojiCatalog.categories.first)
    let value: [String: Any] = [
      "items": [["text": "😀", "annotation": "fixture", "group": category.group]],
      "next_offset": 64,
      "complete": false,
    ]
    XCTAssertEqual(
      try KeyboardEmojiCatalog.decodePage(value, category: category, requestedOffset: 0),
      KeyboardEmojiCatalog.Page(items: [
        .init(text: "😀", annotation: "fixture", group: category.group),
      ], nextOffset: 64, complete: false))

    var invalid = value
    invalid["next_offset"] = 0
    XCTAssertThrowsError(
      try KeyboardEmojiCatalog.decodePage(invalid, category: category, requestedOffset: 0))
    invalid = value
    invalid["items"] = [["text": "😀", "annotation": "fixture", "group": "wrong"]]
    XCTAssertThrowsError(
      try KeyboardEmojiCatalog.decodePage(invalid, category: category, requestedOffset: 0))
    invalid = value
    invalid["items"] = [["text": "", "annotation": "fixture", "group": category.group]]
    XCTAssertThrowsError(
      try KeyboardEmojiCatalog.decodePage(invalid, category: category, requestedOffset: 0))
    invalid = value
    invalid["next_offset"] = true
    XCTAssertThrowsError(
      try KeyboardEmojiCatalog.decodePage(invalid, category: category, requestedOffset: 0))
  }

  func testRecentsAreDeduplicatedValidatedAndBounded() {
    let oversized = String(repeating: "x", count: 33)
    let values = ["😀", "😀", "", oversized, "🌲"]
      + (0..<30).map { "fixture-\($0)" }
    let normalized = KeyboardEmojiRecents.normalize(values)
    XCTAssertEqual(Array(normalized.prefix(2)), ["😀", "🌲"])
    XCTAssertEqual(normalized.count, KeyboardEmojiCatalog.recentLimit)

    for value in normalized { KeyboardEmojiRecents.record(value) }
    KeyboardEmojiRecents.record("😀")
    XCTAssertEqual(KeyboardEmojiRecents.stored.first, "😀")
    XCTAssertEqual(KeyboardEmojiRecents.stored.count, KeyboardEmojiCatalog.recentLimit)
  }

  func testSharedBridgeReadsVerifiedPackagedCatalog() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-emoji-catalog-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let resources = try XCTUnwrap(bridge.candidateGlossResources())
    let category = try XCTUnwrap(KeyboardEmojiCatalog.categories.first)
    let page = try KeyboardEmojiCatalog.loadPage(
      resources: resources, category: category, offset: 0)
    XCTAssertFalse(page.items.isEmpty)
    XCTAssertTrue(page.items.allSatisfy { $0.group == category.group })
    XCTAssertGreaterThan(page.nextOffset, 0)
  }

  func testSymbolParentsAndPagesAreValidatedAndDeduplicated() throws {
    XCTAssertEqual(try KeyboardEmojiCatalog.decodeSymbolParents(["symbol_groups": [
      ["parent": "Math", "title": "Operators"],
      ["parent": "Math", "title": "Fractions"],
      ["parent": "", "title": "Empty"],
      ["parent": "Hearts", "title": "Hearts"],
    ]]), ["Math", "Hearts"])
    XCTAssertThrowsError(try KeyboardEmojiCatalog.decodeSymbolParents(["items": []]))

    var offsets: [Int] = []
    let symbols = try KeyboardEmojiCatalog.collectSymbols { offset in
      offsets.append(offset)
      return offset == 0
        ? ["items": [["text": "+"], ["text": "−"]], "next_offset": 2, "complete": false]
        : ["items": [["text": "−"], ["text": "×"]], "next_offset": 4, "complete": true]
    }
    XCTAssertEqual(offsets, [0, 2])
    XCTAssertEqual(symbols, ["+", "−", "×"], "一个符号挂在两个子组下只出现一次")
    XCTAssertThrowsError(try KeyboardEmojiCatalog.collectSymbols { _ in
      ["items": [["text": "+"]], "next_offset": 0, "complete": false]
    }, "游标不前进时不能死循环")
    XCTAssertThrowsError(try KeyboardEmojiCatalog.collectSymbols { _ in
      ["items": [["text": ""]], "next_offset": 1, "complete": true]
    })
    XCTAssertThrowsError(try KeyboardEmojiCatalog.collectSymbols { _ in
      ["items": [["text": "+"]], "next_offset": true, "complete": true]
    })
    XCTAssertThrowsError(try KeyboardEmojiCatalog.collectSymbols { _ in
      ["items": [["text": "+"]], "next_offset": 1.5, "complete": true]
    })
  }

  func testSymbolPagesRejectACompletePageThatExceedsTheCumulativeLimit() {
    let pageSize = 255
    XCTAssertThrowsError(try KeyboardEmojiCatalog.collectSymbols { offset in
      let page = offset / pageSize
      let items = (0..<pageSize).map { ["text": "fixture-symbol-\(page * pageSize + $0)"] }
      return [
        "items": items,
        "next_offset": offset + pageSize,
        "complete": page == 8,
      ]
    })
  }

  func testSharedBridgeReadsPackagedSymbolCatalog() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-symbol-catalog-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let resources = try XCTUnwrap(bridge.candidateGlossResources())
    let parents = try KeyboardEmojiCatalog.symbolParents(resources: resources)
    XCTAssertTrue(parents.contains("Punctuation"))
    XCTAssertTrue(parents.contains("Letters"))
    for parent in parents {
      XCTAssertNotNil(KeyboardEmojiCatalog.symbolParentTitles[parent], "\(parent) 要有中文标题")
    }
    let letters = try KeyboardEmojiCatalog.loadSymbols(resources: resources, parent: "Letters")
    XCTAssertGreaterThan(letters.count, 255, "最大的一类要翻过不止一页")
    XCTAssertEqual(Set(letters).count, letters.count)
  }

  func testSymbolSearchFindsSymbolsByEnglishPinyinAndInitials() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-symbol-search-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let resources = try XCTUnwrap(bridge.candidateGlossResources())
    for query in ["arrow", "jiantou", "JT"] {
      let found = try XCTUnwrap(KeyboardEmojiCatalog.searchSymbols(resources: resources, query: query), query)
      XCTAssertTrue(found.contains("→"), query)
      XCTAssertEqual(Set(found).count, found.count)
    }
    XCTAssertTrue(try XCTUnwrap(KeyboardEmojiCatalog.searchSymbols(resources: resources, query: "huobi")).contains("€"))
    XCTAssertEqual(try KeyboardEmojiCatalog.searchSymbols(resources: resources, query: "qqqqzzzz"), [])
    XCTAssertNil(try KeyboardEmojiCatalog.searchSymbols(resources: resources, query: "→ 1"), "nothing left to search for")
    let wide = try XCTUnwrap(KeyboardEmojiCatalog.searchSymbols(resources: resources, query: "a"))
    XCTAssertLessThanOrEqual(wide.count, KeyboardEmojiCatalog.maximumSymbolMatches)
  }

  func testSymbolPanelSearchSwapsCategoriesForALetterPadAndBackReturns() async throws {
    var closed = false
    var inserted: [String] = []
    let panel = KeyboardSymbolPanelView(
      catalog: .init(parents: { [] }, symbols: { _ in [] }, search: { query in
        XCTAssertFalse(Thread.isMainThread)
        return query == "jt" ? ["→", "←"] : []
      }),
      onInsert: { inserted.append($0) }, onDelete: {}, onClose: { closed = true })
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    panel.layoutIfNeeded()
    try button("symbolSearchKey", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(panel.searchQuery, "")
    XCTAssertEqual(try label("symbolSearchStatus", in: panel).isHidden, false)
    try button("symbolSearchKey-j", in: panel).sendActions(for: .primaryActionTriggered)
    try button("symbolSearchKey-t", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(panel.searchQuery, "jt")
    for _ in 0..<100 where panel.shownSymbols.isEmpty { try await Task.sleep(nanoseconds: 20_000_000) }
    XCTAssertEqual(panel.shownSymbols, ["→", "←"])
    try button("symbolKey_→", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(inserted, ["→"])
    XCTAssertTrue(closed, "a found symbol goes in and closes an unlocked panel, as a browsed one does")
    closed = false

    try button("symbolSearchDelete", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(panel.searchQuery, "j")
    let status = try label("symbolSearchStatus", in: panel)
    for _ in 0..<100 where status.text != "没有找到相关符号" { try await Task.sleep(nanoseconds: 20_000_000) }
    XCTAssertFalse(status.isHidden)
    XCTAssertEqual(status.text, "没有找到相关符号")

    try button("closeSymbolPanel", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(panel.searchQuery)
    XCTAssertFalse(closed, "back leaves the search before it leaves the panel")
    XCTAssertEqual(panel.shownSymbols, KeyboardSymbolPanelView.categories[0].symbols)
    try button("closeSymbolPanel", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(closed)

    let offline = KeyboardSymbolPanelView(onInsert: { _ in }, onDelete: {}, onClose: {})
    XCTAssertNil(descendants(offline).first { $0.accessibilityIdentifier == "symbolSearchKey" },
                 "without a catalog there is nothing to search")
  }

  func testSymbolRecentsStayApartFromEmojiAndLeadThePanel() throws {
    KeyboardSymbolRecents.record("，")
    KeyboardSymbolRecents.record("@gmail.com")
    KeyboardSymbolRecents.record("，")
    XCTAssertEqual(KeyboardSymbolRecents.stored, ["，", "@gmail.com"])
    XCTAssertEqual(KeyboardEmojiRecents.stored, [], "符号不挤进表情的最近")
    for index in 0..<40 { KeyboardSymbolRecents.record("s\(index)") }
    XCTAssertEqual(KeyboardSymbolRecents.stored.count, KeyboardSymbolRecents.limit)
    XCTAssertEqual(KeyboardSymbolRecents.stored.first, "s39")

    var inserted: [String] = []
    let panel = KeyboardSymbolPanelView(recents: ["→", "㎡"], onInsert: { inserted.append($0) },
                                        onDelete: {}, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    panel.layoutIfNeeded()
    XCTAssertEqual(panel.categoryCount, KeyboardSymbolPanelView.categories.count + 1)
    XCTAssertEqual(try button("symbolCategory_0", in: panel).title(for: .normal), "最近")
    XCTAssertEqual(try button("symbolCategory_1", in: panel).title(for: .normal), "常用")
    try button("symbolKey_㎡", in: panel).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(inserted, ["㎡"])

    let fresh = KeyboardSymbolPanelView(onInsert: { _ in }, onDelete: {}, onClose: {})
    XCTAssertEqual(fresh.categoryCount, KeyboardSymbolPanelView.categories.count, "没用过符号时不显示空的最近")
  }

  func testSymbolPanelAppendsCatalogCategoriesAndLoadsThemOffTheMainThread() async throws {
    let panel = KeyboardSymbolPanelView(
      catalog: .init(parents: { ["Math", "Unlisted"] }, symbols: { parent in
        XCTAssertFalse(Thread.isMainThread)
        return parent == "Math" ? ["∑", "∞"] : []
      }),
      onInsert: { _ in }, onDelete: {}, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    panel.layoutIfNeeded()
    let fixed = KeyboardSymbolPanelView.categories.count
    XCTAssertEqual(panel.categoryCount, fixed + 2)
    XCTAssertEqual(try button("symbolCategory_\(fixed)", in: panel).title(for: .normal), "数学")
    XCTAssertEqual(try button("symbolCategory_\(fixed + 1)", in: panel).title(for: .normal), "Unlisted",
                   "目录以后新增的类别按原名显示")

    try button("symbolCategory_\(fixed)", in: panel).sendActions(for: .primaryActionTriggered)
    for _ in 0..<100 where descendants(panel).first(where: { $0.accessibilityIdentifier == "symbolKey_∑" }) == nil {
      try await Task.sleep(nanoseconds: 20_000_000)
    }
    XCTAssertNotNil(descendants(panel).first { $0.accessibilityIdentifier == "symbolKey_∞" })

    let unreadable = KeyboardSymbolPanelView(
      catalog: .init(parents: { throw KeyboardEmojiCatalogError.invalidPage }, symbols: { _ in [] }),
      onInsert: { _ in }, onDelete: {}, onClose: {})
    XCTAssertEqual(unreadable.categoryCount, fixed, "目录读不出来时手机常用的几类照常可用")
  }

  func testPickerLoadsInBackgroundAndRoutesInsertDeleteAndClose() async throws {
    let category = try XCTUnwrap(KeyboardEmojiCatalog.categories.first)
    let changed = expectation(description: "initial state and loaded page")
    changed.expectedFulfillmentCount = 2
    var inserted: String?
    var deleted = false
    var closed = false
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { requested, offset in
        XCTAssertEqual(requested, category)
        XCTAssertEqual(offset, 0)
        return KeyboardEmojiCatalog.Page(items: [
          .init(text: "😀", annotation: "fixture", group: category.group),
          .init(text: "🌲", annotation: "fixture", group: category.group),
        ], nextOffset: 2, complete: true)
      },
      onInsert: { inserted = $0 },
      onDelete: { deleted = true },
      onClose: { closed = true },
      onCatalogChange: { changed.fulfill() })
    picker.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    picker.layoutIfNeeded()
    await fulfillment(of: [changed], timeout: 2)

    let grid = try XCTUnwrap(descendants(picker).first {
      $0.accessibilityIdentifier == "emojiGrid"
    } as? UICollectionView)
    XCTAssertEqual(grid.numberOfItems(inSection: 0), 2)
    picker.collectionView(grid, didSelectItemAt: IndexPath(item: 0, section: 0))
    XCTAssertEqual(inserted, "😀")
    XCTAssertEqual(KeyboardEmojiRecents.stored.first, "😀")
    try button("emojiDeleteKey", in: picker).sendActions(for: .primaryActionTriggered)
    try button("closeEmojiPicker", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(deleted)
    XCTAssertTrue(closed)
  }

  func testSearchKeepsOnlyLettersAndSpansEveryGroup() throws {
    let search = try XCTUnwrap(KeyboardEmojiCatalog.search("Xiao 笑1!"))
    XCTAssertEqual(search.search, "xiao")
    XCTAssertEqual(search.group, "")
    XCTAssertFalse(search.isKaomoji)
    XCTAssertNil(KeyboardEmojiCatalog.search("1 笑!"))
    XCTAssertEqual(KeyboardEmojiCatalog.search(String(repeating: "a", count: 40))?.search.count,
                   KeyboardEmojiCatalog.maximumSearchLength)
    let value: [String: Any] = [
      "items": [
        ["text": "😀", "annotation": "fixture", "group": "Smileys and emotion"],
        ["text": "🌲", "annotation": "fixture", "group": "Animals and nature"],
      ],
      "next_offset": 2,
      "complete": true,
    ]
    XCTAssertEqual(
      try KeyboardEmojiCatalog.decodePage(value, category: search, requestedOffset: 0).items.map(\.text),
      ["😀", "🌲"])
  }

  func testSharedBridgeSearchesPackagedCatalogByPinyinAndEnglish() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-emoji-search-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let resources = try XCTUnwrap(bridge.candidateGlossResources())
    for query in ["xiao", "smile"] {
      let page = try KeyboardEmojiCatalog.loadPage(
        resources: resources, category: try XCTUnwrap(KeyboardEmojiCatalog.search(query)), offset: 0)
      XCTAssertFalse(page.items.isEmpty, query)
    }
    let groups = Set(try KeyboardEmojiCatalog.loadPage(
      resources: resources, category: try XCTUnwrap(KeyboardEmojiCatalog.search("xiao")), offset: 0).items.map(\.group))
    XCTAssertGreaterThan(groups.count, 1, "a search is not limited to one category")
  }

  func testPickerSearchTypesOnItsOwnPadAndReturnsToTheCategories() async throws {
    let category = try XCTUnwrap(KeyboardEmojiCatalog.categories.first)
    final class Queries: @unchecked Sendable {
      private let lock = NSLock()
      private var values: [String] = []
      func append(_ value: String) { lock.lock(); values.append(value); lock.unlock() }
      var all: [String] { lock.lock(); defer { lock.unlock() }; return values }
    }
    let searches = Queries()
    let searched = expectation(description: "results for xi")
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { requested, _ in
        if !requested.search.isEmpty {
          searches.append(requested.search)
          if requested.search == "xi" { searched.fulfill() }
          return KeyboardEmojiCatalog.Page(items: [
            .init(text: "😄", annotation: "fixture", group: "Smileys and emotion"),
          ], nextOffset: 1, complete: true)
        }
        return KeyboardEmojiCatalog.Page(items: [
          .init(text: "😀", annotation: "fixture", group: requested.group),
        ], nextOffset: 1, complete: true)
      },
      onInsert: { _ in },
      onDelete: {},
      onClose: { XCTFail("back from a search returns to the categories, not the keyboard") },
      onCatalogChange: nil)
    picker.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    picker.layoutIfNeeded()
    let pad = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiSearchPad" })
    XCTAssertTrue(pad.isHidden)

    try button("emojiSearchButton", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(picker.searchQuery, "")
    XCTAssertFalse(pad.isHidden)
    let status = try XCTUnwrap(descendants(picker).first {
      $0.accessibilityIdentifier == "emojiCatalogStatus"
    } as? UILabel)
    XCTAssertEqual(status.text, "输入拼音或英文搜索表情")

    try button("emojiSearchKey-x", in: picker).sendActions(for: .primaryActionTriggered)
    try button("emojiSearchKey-i", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(picker.searchQuery, "xi")
    await fulfillment(of: [searched], timeout: 2)
    try await Task.sleep(nanoseconds: 100_000_000)
    let grid = try XCTUnwrap(descendants(picker).first {
      $0.accessibilityIdentifier == "emojiGrid"
    } as? UICollectionView)
    XCTAssertEqual(grid.numberOfItems(inSection: 0), 1)
    let title = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiTitle" } as? UILabel)
    XCTAssertEqual(title.text, "xi")

    try button("emojiSearchDelete", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(picker.searchQuery, "x")
    try button("closeEmojiPicker", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(picker.searchQuery)
    XCTAssertTrue(pad.isHidden)
    XCTAssertEqual(title.text, "表情")
    let typed = searches.all
    XCTAssertTrue(typed.allSatisfy { $0 == "x" || $0 == "xi" }, "only typed queries reach the catalog: \(typed)")
  }

  /// 按键区里的面板在手机横屏只有约 168pt 高：搜索时字母键盘让出高度，结果条仍放得下一整行（颜文字行高 44pt）。竖屏默认高度下字母键盘保持设计稿的 106pt。
  func testKeyAreaSearchKeepsAFullRowOfResultsInALandscapeKeyArea() throws {
    let cases: [(height: CGFloat, padHeight: CGFloat?)] = [(168, nil), (201, 3 * 32 + 2 * 5)]
    for (height, padHeight) in cases {
      let picker = KeyboardEmojiPickerView(
        resources: "/fixture",
        loader: { requested, _ in
          KeyboardEmojiCatalog.Page(items: [.init(text: "😀", annotation: "fixture", group: requested.group)], nextOffset: 1, complete: true)
        },
        showsHeader: false, onInsert: { _ in }, onDelete: {}, onClose: {})
      picker.frame = CGRect(x: 0, y: 0, width: 844, height: height)
      picker.layoutIfNeeded()
      try button("emojiSearchButton", in: picker).sendActions(for: .primaryActionTriggered)
      // 不在窗口里的视图换了约束不会自己排版，要先标记再排。
      picker.setNeedsLayout()
      picker.layoutIfNeeded()
      let grid = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiGrid" })
      let pad = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiSearchPad" })
      XCTAssertFalse(pad.isHidden)
      XCTAssertGreaterThanOrEqual(grid.bounds.height, 44 - 0.5, "height \(height)")
      XCTAssertLessThanOrEqual(grid.frame.maxY, pad.frame.minY + 0.5, "height \(height)")
      XCTAssertGreaterThan(pad.bounds.height, 0, "height \(height)")
      if let padHeight { XCTAssertEqual(pad.bounds.height, padHeight, accuracy: 0.5, "the pad keeps its design height when it fits") }
    }
  }

  func testSearchCanLookThroughTheKaomojiCatalog() throws {
    let search = try XCTUnwrap(KeyboardEmojiCatalog.search("Kiss", kaomoji: true))
    XCTAssertTrue(search.isKaomoji)
    XCTAssertEqual(search.group, KeyboardEmojiCatalog.kaomoji.group)
    XCTAssertEqual(search.search, "kiss")
    XCTAssertNil(KeyboardEmojiCatalog.search("笑", kaomoji: true))

    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-kaomoji-search-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let resources = try XCTUnwrap(MetasequoiaInputSessionBridge(stateRoot: state).candidateGlossResources())
    for query in ["kiss", "qian"] {
      let page = try KeyboardEmojiCatalog.loadPage(
        resources: resources, category: try XCTUnwrap(KeyboardEmojiCatalog.search(query, kaomoji: true)), offset: 0)
      XCTAssertFalse(page.items.isEmpty, query)
    }
  }

  func testPickerSearchScopeFollowsTheKaomojiTabAndCanBeSwitched() async throws {
    final class Requests: @unchecked Sendable {
      private let lock = NSLock()
      private var values: [(search: String, kaomoji: Bool)] = []
      func append(_ value: (String, Bool)) { lock.lock(); values.append(value); lock.unlock() }
      var all: [(search: String, kaomoji: Bool)] { lock.lock(); defer { lock.unlock() }; return values }
    }
    let requests = Requests()
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { requested, _ in
        if !requested.search.isEmpty { requests.append((requested.search, requested.isKaomoji)) }
        return KeyboardEmojiCatalog.Page(items: [
          .init(text: requested.isKaomoji ? "(^_^)" : "😀", annotation: "fixture", group: requested.group),
        ], nextOffset: 1, complete: true)
      },
      onInsert: { _ in },
      onDelete: {},
      onClose: {},
      onCatalogChange: nil)
    picker.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    picker.layoutIfNeeded()
    let tabs = descendants(picker).compactMap { $0 as? UIButton }.filter { $0.accessibilityIdentifier?.hasPrefix("emojiCategory-") == true }
    let kaomojiTab = try XCTUnwrap(tabs.first { $0.accessibilityLabel == "颜文字" })
    let scope = try button("emojiSearchScopeEmoji", in: picker).superview
    XCTAssertEqual(scope?.isHidden, true)

    kaomojiTab.sendActions(for: .primaryActionTriggered)
    try button("emojiSearchButton", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(picker.searchesKaomoji, "a search from the kaomoji tab looks through kaomoji")
    XCTAssertEqual(scope?.isHidden, false)
    let title = try XCTUnwrap(descendants(picker).first { $0.accessibilityIdentifier == "emojiTitle" } as? UILabel)
    XCTAssertEqual(title.text, "搜索颜文字")
    try button("emojiSearchKey-k", in: picker).sendActions(for: .primaryActionTriggered)
    try button("emojiSearchScopeEmoji", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(picker.searchesKaomoji)
    XCTAssertEqual(picker.searchQuery, "k", "switching scope keeps the letters")
    try await Task.sleep(nanoseconds: 300_000_000)
    let seen = requests.all
    XCTAssertTrue(seen.contains { $0.search == "k" && $0.kaomoji }, "\(seen)")
    XCTAssertTrue(seen.contains { $0.search == "k" && !$0.kaomoji }, "\(seen)")

    try button("closeEmojiPicker", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(scope?.isHidden, true)
    tabs[0].sendActions(for: .primaryActionTriggered)
    try button("emojiSearchButton", in: picker).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(picker.searchesKaomoji, "a search from an Emoji tab looks through Emoji")
  }

  func testKaomojiIsTheLastTabAndAcceptsLongerLines() throws {
    let kaomoji = KeyboardEmojiCatalog.kaomoji
    XCTAssertEqual(kaomoji.title, "颜文字")
    XCTAssertTrue(kaomoji.isKaomoji)
    XCTAssertFalse(KeyboardEmojiCatalog.categories.contains { $0.isKaomoji })
    let line = String(repeating: "ヽ", count: 59)
    let value: [String: Any] = [
      "items": [["text": line, "annotation": "fixture", "group": kaomoji.group]],
      "next_offset": 1,
      "complete": true,
    ]
    XCTAssertEqual(
      try KeyboardEmojiCatalog.decodePage(value, category: kaomoji, requestedOffset: 0).items.first?.text,
      line)
    let emoji = try XCTUnwrap(KeyboardEmojiCatalog.categories.first)
    var asEmoji = value
    asEmoji["items"] = [["text": line, "annotation": "fixture", "group": emoji.group]]
    XCTAssertThrowsError(
      try KeyboardEmojiCatalog.decodePage(asEmoji, category: emoji, requestedOffset: 0),
      "the Emoji limit still applies outside the kaomoji tab")
    XCTAssertEqual(KeyboardEmojiPickerView.kaomojiColumns(width: 378), 2)
    XCTAssertEqual(KeyboardEmojiPickerView.kaomojiColumns(width: 200), 2)
    XCTAssertEqual(KeyboardEmojiPickerView.kaomojiColumns(width: 1012), 5)
  }

  func testEmojiColumnsStayAtEightOnAPhoneAndGrowOnAnIPad() {
    XCTAssertEqual(KeyboardEmojiPickerView.emojiColumns(width: 378), 8)
    XCTAssertEqual(KeyboardEmojiPickerView.emojiColumns(width: 430), 8)
    XCTAssertEqual(KeyboardEmojiPickerView.emojiColumns(width: 1012), 18)
  }

  func testSharedBridgeReadsPackagedKaomoji() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-kaomoji-catalog-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let resources = try XCTUnwrap(bridge.candidateGlossResources())
    let page = try KeyboardEmojiCatalog.loadPage(
      resources: resources, category: KeyboardEmojiCatalog.kaomoji, offset: 0)
    XCTAssertFalse(page.items.isEmpty)
    XCTAssertFalse(page.complete)
    XCTAssertTrue(page.items.contains { $0.text.unicodeScalars.count > 1 })
  }

  func testPickerKaomojiGoesToItsOwnRecentsTab() async throws {
    let kaomoji = KeyboardEmojiCatalog.kaomoji
    let loaded = expectation(description: "kaomoji page")
    var inserted: String?
    let picker = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { category, _ in
        if category == kaomoji { loaded.fulfill() }
        return KeyboardEmojiCatalog.Page(items: [
          .init(text: category == kaomoji ? "(*^▽^*)" : "😀", annotation: "fixture", group: category.group),
        ], nextOffset: 1, complete: true)
      },
      onInsert: { inserted = $0 },
      onDelete: {},
      onClose: {},
      onCatalogChange: nil)
    picker.frame = CGRect(x: 0, y: 0, width: 390, height: 260)
    picker.layoutIfNeeded()
    let tabs = descendants(picker).compactMap { $0 as? UIButton }
      .filter { $0.accessibilityIdentifier?.hasPrefix("emojiCategory-") == true }
    let last = try XCTUnwrap(tabs.last)
    XCTAssertEqual(last.accessibilityLabel, "颜文字")
    last.sendActions(for: .primaryActionTriggered)
    await fulfillment(of: [loaded], timeout: 2)
    try await Task.sleep(nanoseconds: 100_000_000)

    let grid = try XCTUnwrap(descendants(picker).first {
      $0.accessibilityIdentifier == "emojiGrid"
    } as? UICollectionView)
    XCTAssertEqual(grid.numberOfItems(inSection: 0), 1)
    picker.collectionView(grid, didSelectItemAt: IndexPath(item: 0, section: 0))
    XCTAssertEqual(inserted, "(*^▽^*)")
    XCTAssertTrue(KeyboardEmojiRecents.stored.isEmpty, "kaomoji stay out of the Emoji recents")
    XCTAssertEqual(KeyboardKaomojiRecents.stored, ["(*^▽^*)"])
    let status = try XCTUnwrap(descendants(picker).first {
      $0.accessibilityIdentifier == "emojiCatalogStatus"
    } as? UILabel)
    XCTAssertEqual(status.text, "1 个颜文字")
    XCTAssertFalse(tabs.contains { $0.accessibilityLabel == "最近颜文字" }, "no tab until a kaomoji is used")

    let reopened = KeyboardEmojiPickerView(
      resources: "/fixture",
      loader: { _, _ in .init(items: [], nextOffset: 0, complete: true) },
      onInsert: { inserted = $0 }, onDelete: {}, onClose: {}, onCatalogChange: nil)
    reopened.frame = picker.frame
    reopened.layoutIfNeeded()
    let reopenedTabs = descendants(reopened).compactMap { $0 as? UIButton }
      .filter { $0.accessibilityIdentifier?.hasPrefix("emojiCategory-") == true }
    XCTAssertEqual(reopenedTabs.suffix(2).map(\.accessibilityLabel), ["最近颜文字", "颜文字"])
    XCTAssertFalse(reopenedTabs.contains { $0.accessibilityLabel == "最近" }, "the Emoji recents are still empty")
    try XCTUnwrap(reopenedTabs.first { $0.accessibilityLabel == "最近颜文字" }).sendActions(for: .primaryActionTriggered)
    let reopenedGrid = try XCTUnwrap(descendants(reopened).first {
      $0.accessibilityIdentifier == "emojiGrid"
    } as? UICollectionView)
    XCTAssertEqual(reopenedGrid.numberOfItems(inSection: 0), 1)
    inserted = nil
    reopened.collectionView(reopenedGrid, didSelectItemAt: IndexPath(item: 0, section: 0))
    XCTAssertEqual(inserted, "(*^▽^*)")
    XCTAssertEqual(KeyboardKaomojiRecents.stored, ["(*^▽^*)"])
    XCTAssertTrue(KeyboardEmojiRecents.stored.isEmpty)
  }

  func testKaomojiRecentsKeepLongLinesAndStayBounded() {
    let long = String(repeating: "(^_^)", count: 12)
    XCTAssertFalse(KeyboardEmojiCatalog.validRecent(long), "too long for the Emoji recents")
    KeyboardKaomojiRecents.record(long)
    XCTAssertEqual(KeyboardKaomojiRecents.stored, [long])
    for index in 0..<30 { KeyboardKaomojiRecents.record("(\(index))") }
    XCTAssertEqual(KeyboardKaomojiRecents.stored.count, KeyboardKaomojiRecents.limit)
    XCTAssertEqual(KeyboardKaomojiRecents.stored.first, "(29)")
    XCTAssertEqual(KeyboardKaomojiRecents.normalize(["a", "a", "", "b"]), ["a", "b"])
  }

  func testControllerExposesToolbarAndMoreMenuEntries() throws {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    controller.view.layoutIfNeeded()

    let hasPicker = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardEmojiPicker" } }
    let emoji = try XCTUnwrap(try button("emojiShortcut", in: controller.view) as? KeyboardToolbarButton)
    emoji.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(hasPicker())
    XCTAssertTrue(emoji.isActive, "the toolbar icon of the open panel is highlighted")
    // 面板自己没有返回按钮：再点同一个工具栏图标就关闭它。
    emoji.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasPicker())
    XCTAssertFalse(emoji.isActive)

    // 表情也是功能菜单里的一格；点它会用表情选择器替换菜单。
    try button("moreShortcut", in: controller.view).sendActions(for: .primaryActionTriggered)
    let tile = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "moreCard-表情" } as? UIControl)
    tile.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(hasPicker())
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
  }

  private func descendants(_ root: UIView) -> [UIView] {
    root.subviews + root.subviews.flatMap(descendants)
  }

  private func label(_ identifier: String, in root: UIView) throws -> UILabel {
    try XCTUnwrap(descendants(root).first { $0.accessibilityIdentifier == identifier } as? UILabel)
  }

  private func button(_ identifier: String, in root: UIView) throws -> UIButton {
    try XCTUnwrap(descendants(root).first {
      $0.accessibilityIdentifier == identifier
    } as? UIButton)
  }
}
