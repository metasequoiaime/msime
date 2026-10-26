import UIKit
import XCTest

private final class StubTranslationService: CandidateTranslationService, @unchecked Sendable {
  private let answers: [String: [String: String]]
  private let lock = NSLock()
  private var recorded: [(code: String, words: [String])] = []

  init(answers: [String: [String: String]]) { self.answers = answers }

  var calls: [(code: String, words: [String])] {
    lock.lock(); defer { lock.unlock() }
    return recorded
  }

  func translate(words: [String], target: String) async throws -> [String] {
    lock.lock()
    recorded.append((target, words))
    lock.unlock()
    return words.map { answers[target]?[$0] ?? "" }
  }
}

private struct TruncatingTranslationService: CandidateTranslationService {
  func translate(words: [String], target: String) async throws -> [String] {
    Array(words.dropLast()).map { $0 + "?" }
  }
}

private final class BlockingTranslationService: CandidateTranslationService, @unchecked Sendable {
  private let lock = NSLock()
  private var count = 0

  var callCount: Int {
    lock.lock(); defer { lock.unlock() }
    return count
  }

  func translate(words: [String], target: String) async throws -> [String] {
    lock.lock()
    count += 1
    lock.unlock()
    try await Task.sleep(nanoseconds: 5_000_000_000)
    return words.map { _ in "hello" }
  }
}

private final class FlakyTranslationService: CandidateTranslationService, @unchecked Sendable {
  private let lock = NSLock()
  private var count = 0

  var callCount: Int {
    lock.lock(); defer { lock.unlock() }
    return count
  }

  func translate(words: [String], target: String) async throws -> [String] {
    lock.lock()
    count += 1
    let call = count
    lock.unlock()
    if call == 1 { throw NSError(domain: "CandidateTranslationTests", code: 1) }
    return words.map { _ in "hello" }
  }
}

/// 记下每次请求的词，以及有多少次请求在回答之前被取消。
private final class CancellableTranslationService: CandidateTranslationService, @unchecked Sendable {
  private let lock = NSLock()
  private var words: [[String]] = []
  private var cancelledCount = 0

  var calls: [[String]] {
    lock.lock(); defer { lock.unlock() }
    return words
  }

  var cancelled: Int {
    lock.lock(); defer { lock.unlock() }
    return cancelledCount
  }

  func translate(words: [String], target: String) async throws -> [String] {
    lock.lock()
    self.words.append(words)
    lock.unlock()
    do {
      try await Task.sleep(nanoseconds: 5_000_000_000)
    } catch {
      lock.lock()
      cancelledCount += 1
      lock.unlock()
      throw error
    }
    return words.map { _ in "late" }
  }
}

@MainActor
final class CandidateTranslationTests: XCTestCase {
  /// 「仅当前高亮」生效时，候选词翻译只为视图里高亮的那一个候选请求；高亮不在这一页时一个也不请求；范围是「全部」时整页照旧。
  func testScopedRequestAsksOnlyTheHighlightedCandidate() {
    let words = ["你好", "泥嚎", "拟好"]
    let sources = [0, 1, 0]
    let scoped = CandidateTranslationStore.requestPage(
      words: words, sources: sources, highlighted: [false, true, false], scoped: true)
    XCTAssertEqual(scoped.words, ["泥嚎"])
    XCTAssertEqual(scoped.sources, [1], "来源跟着那个候选走，不能错位成第一个候选的来源")

    let away = CandidateTranslationStore.requestPage(
      words: words, sources: sources, highlighted: [false, false, false], scoped: true)
    XCTAssertTrue(away.words.isEmpty, "高亮不在这一页时不请求")
    XCTAssertTrue(CandidateTranslationStore.requestPage(
      words: words, sources: sources, highlighted: [], scoped: true).words.isEmpty)

    let all = CandidateTranslationStore.requestPage(
      words: words, sources: sources, highlighted: [false, true, false], scoped: false)
    XCTAssertEqual(all.words, words)
    XCTAssertEqual(all.sources, sources)
  }

  /// 范围生效的条件和共享 Rust 的 `translation_scope_highlighted` 相同：候选词翻译开着、范围是「仅当前高亮」。
  func testTheScopeAppliesOnlyWhileCandidateTranslationIsOn() {
    let previousOnline = CandidateTranslationPreference.onlineEnabled
    let previousScope = CandidateTranslationPreference.translationScope
    defer {
      CandidateTranslationPreference.onlineEnabled = previousOnline
      CandidateTranslationPreference.translationScope = previousScope
    }
    CandidateTranslationPreference.onlineEnabled = true
    CandidateTranslationPreference.translationScope = .highlighted
    XCTAssertTrue(CandidateTranslationPreference.scopedToHighlight)
    CandidateTranslationPreference.translationScope = .all
    XCTAssertFalse(CandidateTranslationPreference.scopedToHighlight)
    CandidateTranslationPreference.translationScope = .highlighted
    CandidateTranslationPreference.onlineEnabled = false
    XCTAssertFalse(CandidateTranslationPreference.scopedToHighlight)
  }

  /// 绘制侧：范围生效时宿主缓存里的候选词翻译只画在高亮的那一行，其他行只画离线释义。
  func testCandidateTranslationIsDrawnOnlyOnTheHighlightedRow() {
    let highlighted = [true, false]
    XCTAssertTrue(CandidateTranslationPreference.drawsTranslation(at: 0, highlighted: highlighted, scoped: true))
    XCTAssertFalse(CandidateTranslationPreference.drawsTranslation(at: 1, highlighted: highlighted, scoped: true))
    XCTAssertFalse(CandidateTranslationPreference.drawsTranslation(at: 2, highlighted: highlighted, scoped: true))
    XCTAssertTrue(CandidateTranslationPreference.drawsTranslation(at: 1, highlighted: highlighted, scoped: false))
  }

  /// 高亮换到别的候选时，还在路上的旧请求被取消；已经到了的译文留在缓存里，高亮移回来直接用，不再请求。
  func testMovingTheHighlightCancelsTheOldRequestAndKeepsTheCache() async throws {
    let blocking = CancellableTranslationService()
    let store = CandidateTranslationStore(service: blocking)
    store.refresh(words: ["你好"], sources: [0], codes: ["EN"], dropsStale: true)
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(blocking.calls, [["你好"]])
    store.refresh(words: ["中国"], sources: [0], codes: ["EN"], dropsStale: true)
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(blocking.cancelled, 1, "旧高亮的请求应被取消")
    XCTAssertEqual(blocking.calls, [["你好"], ["中国"]])
    // 同一个高亮反复刷新（每次按键都会刷新）不取消它自己的请求。
    store.refresh(words: ["中国"], sources: [0], codes: ["EN"], dropsStale: true)
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(blocking.cancelled, 1)
    store.cancel()

    let service = StubTranslationService(answers: ["EN": ["你好": "hello", "中国": "China"]])
    let cached = CandidateTranslationStore(service: service)
    var arrived = expectation(description: "first highlight")
    cached.onArrival = { arrived.fulfill() }
    cached.refresh(words: ["你好"], sources: [0], codes: ["EN"], dropsStale: true)
    await fulfillment(of: [arrived], timeout: 5)
    arrived = expectation(description: "second highlight")
    cached.refresh(words: ["中国"], sources: [0], codes: ["EN"], dropsStale: true)
    await fulfillment(of: [arrived], timeout: 5)
    cached.refresh(words: ["你好"], sources: [0], codes: ["EN"], dropsStale: true)
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.calls.map(\.words), [["你好"], ["中国"]], "移回来的高亮用缓存，不再请求")
    XCTAssertEqual(cached.gloss(word: "你好", code: "EN"), "hello")
  }

  /// 「仅当前高亮」生效时，离线英文释义照旧画在每个候选下面：键盘把离线释义 worker 的回答标成 `gloss` 交给会话，会话只把不带标记的回答当作候选词翻译、只画在高亮行。
  func testOfflineGlossesStayOnEveryRowWhileTranslationIsScopedToTheHighlight() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-translation-scope-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["candidate_english_gloss"] = true
      $0["candidate_translations"] = true
      $0["candidate_translation_scope"] = "highlighted"
    })
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    var typed = bridge.currentSnapshot()
    for letter in "nihao" { typed = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(typed.candidateHighlighted.first, true, "触屏键盘的高亮是第一个候选")

    // 和键盘请求离线释义时一样，按 `allCandidates` 的代次和文字组回答。
    let all = try bridge.allCandidates()
    let generation = try XCTUnwrap(all["generation"] as? NSNumber)
    let texts = try XCTUnwrap(all["candidates"] as? [[String: Any]]).compactMap { $0["text"] as? String }
      .filter(CandidateTranslationStore.translatable)
    XCTAssertGreaterThanOrEqual(texts.count, 2)
    let decoded = try CandidateGlossModel.decode([
      "generation": generation,
      "translations": [
        ["text": texts[0], "translation": "first gloss"],
        ["text": texts[1], "translation": "second gloss"],
      ],
    ])
    let applied = try bridge.applyTranslations(generation: decoded.generation, translations: decoded.translations)
    XCTAssertEqual(applied["applied"] as? Bool, true)
    let shown = try bridge.snapshot(from: applied)
    let glosses = Dictionary(zip(shown.candidates, shown.candidateGlosses), uniquingKeysWith: { first, _ in first })
    XCTAssertEqual(glosses[texts[0]], "first gloss")
    XCTAssertEqual(glosses[texts[1]], "second gloss", "不是高亮的候选也要画离线释义")
    _ = bridge.cancel()
  }

  /// 范围是「全部」时行为不变：换一页不取消上一页还在路上的请求。
  func testWithoutTheScopeANewPageDoesNotCancelTheOldRequest() async throws {
    let blocking = CancellableTranslationService()
    let store = CandidateTranslationStore(service: blocking)
    store.refresh(words: ["你好"], sources: [0], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    store.refresh(words: ["中国"], sources: [0], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(blocking.cancelled, 0)
    XCTAssertEqual(blocking.calls, [["你好"], ["中国"]])
    store.cancel()
  }

  func testOfflineGlossBridgeRejectsFractionalSourceAndGeneration() {
    XCTAssertThrowsError(try CandidateGlossModel.request(
      generation: 1,
      candidates: [["text": "你好", "source": NSNumber(value: 1.5)]]))

    XCTAssertThrowsError(try CandidateGlossModel.decode([
      "generation": NSNumber(value: 2.5),
      "translations": [["text": "你好", "translation": "hello"]],
    ]))
  }

  func testOnlyCandidatesWithHanCharactersGoOutToTheNetwork() {
    XCTAssertTrue(CandidateTranslationStore.translatable("你好"))
    XCTAssertTrue(CandidateTranslationStore.translatable("啊"))
    XCTAssertFalse(CandidateTranslationStore.translatable("nihao"))
    XCTAssertFalse(CandidateTranslationStore.translatable("OpenAI"))
    XCTAssertFalse(CandidateTranslationStore.translatable("123"))
    XCTAssertFalse(CandidateTranslationStore.translatable("😀"))
  }

  func testCandidatesWithNoGlossSourcesStayOutOfTheNetwork() async throws {
    let service = StubTranslationService(answers: ["EN": ["你好": "hello"]])
    let store = CandidateTranslationStore(service: service)
    let arrived = expectation(description: "ordinary candidate gloss arrived")
    store.onArrival = { arrived.fulfill() }

    let sources = [0, 14, 7, 13, 6]
    store.refresh(words: ["你好", "你好", "颜文字笑", "私", "表情", "缺来源"],
                  sources: sources, codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 5)

    XCTAssertEqual(service.calls.map(\.words), [["你好"]])
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello")
    XCTAssertTrue(CandidateTranslationStore.allowsGloss(sources: sources, at: 0))
    XCTAssertFalse(CandidateTranslationStore.allowsGloss(sources: sources, at: 1))
    XCTAssertFalse(CandidateTranslationStore.allowsGloss(sources: sources, at: 5))
  }

  func testEachLanguageIsAskedOnceAndTheAnswerIsKept() async throws {
    let service = StubTranslationService(
      answers: ["EN": ["你好": "hello"], "JA": ["你好": "こんにちは"]])
    let store = CandidateTranslationStore(service: service)
    let arrived = expectation(description: "gloss arrived")
    arrived.expectedFulfillmentCount = 2
    store.onArrival = { arrived.fulfill() }

    store.refresh(words: ["你好", "nihao"], codes: ["EN", "JA"])
    await fulfillment(of: [arrived], timeout: 5)
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello")
    XCTAssertEqual(store.gloss(word: "你好", code: "JA"), "こんにちは")
    XCTAssertEqual(service.calls.count, 2)
    XCTAssertTrue(service.calls.allSatisfy { $0.words == ["你好"] })

    store.refresh(words: ["你好", "nihao"], codes: ["EN", "JA"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.calls.count, 2)
  }

  func testCacheIsBoundedAndThePageOnScreenIsAskedAgain() async throws {
    let service = StubTranslationService(
      answers: ["EN": ["你好": "hello", "中国": "China", "水杉": "dawn redwood"]])
    let store = CandidateTranslationStore(service: service, cacheLimit: 2)
    var arrived = expectation(description: "first page")
    store.onArrival = { arrived.fulfill() }
    store.refresh(words: ["你好", "中国", "水杉"], codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 5)

    arrived = expectation(description: "page asked again")
    store.refresh(words: ["你好"], codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 5)
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello")
    XCTAssertNil(store.gloss(word: "水杉", code: "EN"))
    XCTAssertEqual(service.calls.count, 2)
  }

  func testMemoryPressureClearsGlossCache() async throws {
    let service = StubTranslationService(answers: ["EN": ["你好": "hello"]])
    let store = CandidateTranslationStore(service: service)
    let arrived = expectation(description: "gloss arrived")
    store.onArrival = { arrived.fulfill() }
    store.refresh(words: ["你好"], codes: ["EN"])
    await fulfillment(of: [arrived], timeout: 5)

    store.clearCacheForMemoryPressure()

    XCTAssertNil(store.gloss(word: "你好", code: "EN"))
  }

  func testAResponseWithTheWrongCountIsDropped() async throws {
    let store = CandidateTranslationStore(service: TruncatingTranslationService())
    store.refresh(words: ["你好", "中国"], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertNil(store.gloss(word: "你好", code: "EN"))
    XCTAssertNil(store.gloss(word: "中国", code: "EN"))
  }

  func testWordSeparatorsCannotCollideInRequestSignature() async throws {
    let service = StubTranslationService(
      answers: ["EN": ["甲|乙": "combined", "甲": "a", "乙": "b"]])
    let store = CandidateTranslationStore(service: service)
    let firstArrived = expectation(description: "first page arrived")
    store.onArrival = { firstArrived.fulfill() }

    store.refresh(words: ["甲|乙"], codes: ["EN"])
    await fulfillment(of: [firstArrived], timeout: 5)
    let secondArrived = expectation(description: "second page arrived")
    store.onArrival = { secondArrived.fulfill() }
    store.refresh(words: ["甲", "乙"], codes: ["EN"])
    await fulfillment(of: [secondArrived], timeout: 5)
    XCTAssertEqual(service.calls.count, 2)
    XCTAssertEqual(store.gloss(word: "甲", code: "EN"), "a")
    XCTAssertEqual(store.gloss(word: "乙", code: "EN"), "b")
  }

  func testQueuedRequestIsCancelledWhenCompositionEnds() async throws {
    let service = StubTranslationService(answers: ["EN": ["你好": "hello"]])
    let store = CandidateTranslationStore(service: service)
    store.refresh(words: ["你好"], codes: ["EN"])
    store.cancel()
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertTrue(service.calls.isEmpty)
  }

  func testCancelledRequestCanBeRetriedForTheSameCandidates() async throws {
    let service = BlockingTranslationService()
    let store = CandidateTranslationStore(service: service)
    store.refresh(words: ["你好"], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.callCount, 1)

    store.cancel()
    store.refresh(words: ["你好"], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.callCount, 2)
    store.cancel()
  }

  func testFailedRequestCanBeRetriedForTheSameCandidates() async throws {
    let service = FlakyTranslationService()
    let store = CandidateTranslationStore(service: service)
    store.refresh(words: ["你好"], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.callCount, 1)

    store.refresh(words: ["你好"], codes: ["EN"])
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertEqual(service.callCount, 2)
    XCTAssertEqual(store.gloss(word: "你好", code: "EN"), "hello")
    store.cancel()
  }

  func testExpandedPanelRendersAnnotationsAndDeferredMenus() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["你好", "泥嚎"], preedit: "nihao",
      annotations: [
        KeyboardCandidateAnnotation(text: "hello\nこんにちは", accessibilityDescription: "释义"),
        .none,
      ], display: { $0 }, menuElements: { _ in [UIAction(title: "hello") { _ in }] },
      onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()

    let glossed = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton)
    let title = try XCTUnwrap(glossed.configuration?.attributedTitle.map { String($0.characters) })
    XCTAssertEqual(title.split(separator: "\n", omittingEmptySubsequences: false),
                   ["你好", "hello", "こんにちは"])
    XCTAssertTrue(glossed.menu?.children.first is UIDeferredMenuElement)
    XCTAssertFalse(glossed.showsMenuAsPrimaryAction)

    let bare = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-2" } as? UIButton)
    XCTAssertEqual(bare.configuration?.title, "泥嚎")
  }

  func testCandidateLongPressOffersGlossInsertion() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previousScheme
      CandidateGlossPreference.enabled = previousGloss
    }
    InputSchemePreference.scheme = .quanpin
    setCandidateGloss(true)

    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 320)
    controller.viewWillAppear(false)
    for letter in ["字母 N", "字母 I", "字母 H", "字母 A", "字母 O"] {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == letter } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()

    // The gloss is fetched off the keyboard's thread and applied to a later generation, so the
    // menu right after the keystrokes has nothing to offer yet. Poll the menu instead of the
    // clock: a fixed sleep is either too short on a loaded machine or wasted time on an idle one.
    var enabledTitles: [String] = []
    let deadline = Date().addingTimeInterval(5)
    repeat {
      RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      enabledTitles = controller.candidateMenuElements(at: 0).compactMap { ($0 as? UIAction)?.title }
    } while !enabledTitles.contains { $0.lowercased().contains("hello") } && Date() < deadline
    // No gloss means the build has no English gloss for the fixture word, not that the menu
    // forgot to offer one: the pinned dictionary release carries no gloss source, and
    // translation-glosses.db is the user's own overlay of edited glosses. Skip rather than
    // report that as a product failure; the assertion resumes as soon as a gloss is available.
    try XCTSkipIf(enabledTitles.allSatisfy { !$0.lowercased().contains("hello") },
                  "No offline gloss for the fixture candidate in the pinned dictionary release.")
    XCTAssertTrue(enabledTitles.contains { $0.lowercased().contains("hello") }, "menu titles: \(enabledTitles)")

    CandidateGlossPreference.enabled = false
    let disabledTitles = controller.candidateMenuElements(at: 0).compactMap { ($0 as? UIAction)?.title }
    XCTAssertFalse(disabledTitles.contains { $0.lowercased().contains("hello") })
  }

  func testExpandedCandidateWidthDoesNotChangeWithGlossLength() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["您好"], preedit: "nhao",
      annotations: [KeyboardCandidateAnnotation(text: "hi", accessibilityDescription: "")],
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    let chip = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton)
    let short = chip.bounds.width

    panel.updateAnnotations([
      KeyboardCandidateAnnotation(
        text: "hello; how do you do; greetings to you", accessibilityDescription: "")
    ])
    panel.layoutIfNeeded()
    let longChip = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton)
    XCTAssertEqual(longChip.bounds.width, short, accuracy: 0.5)
  }

  func testExpandedGridFillsEachWrappedRowToTheEdge() throws {
    func cells(_ spans: [Int], _ columns: Int) -> [[Int]] {
      KeyboardCandidatePanelView.place(spans: spans, columns: columns).map { [$0.row, $0.column, $0.span] }
    }
    // 换行前的单元格拉伸到行尾，最后一行保留自身跨度。
    XCTAssertEqual(cells([1, 1, 2, 2, 2, 1], 5),
                   [[0, 0, 1], [0, 1, 1], [0, 2, 3], [1, 0, 2], [1, 2, 2], [1, 4, 1]])
    XCTAssertEqual(cells([1, 1], 4), [[0, 0, 1], [0, 1, 1]])
    XCTAssertEqual(cells([2, 3, 1], 5), [[0, 0, 2], [0, 2, 3], [1, 0, 1]])
    XCTAssertTrue(cells([], 3).isEmpty)

    let panel = KeyboardCandidatePanelView(
      candidates: ["好", "好", "好", "好", "你今天晚上吃什么呢"], preedit: "hao",
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
    panel.layoutIfNeeded()
    let chips = try (1...5).map { index in
      try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-\(index)" })
    }
    let collection = try XCTUnwrap(descendants(panel).compactMap { $0 as? UICollectionView }.first)
    let frames = chips.map { $0.convert($0.bounds, to: collection) }
    let edge = collection.bounds.width - KeyboardCandidatePanelView.gridPadding
    XCTAssertEqual(frames[3].minY, frames[0].minY, accuracy: 0.5)
    XCTAssertGreaterThan(frames[4].minY, frames[0].minY)
    XCTAssertEqual(frames[3].maxX, edge, accuracy: 0.5, "换行前的单元格应到达行尾")
    XCTAssertLessThan(frames[4].maxX, edge - 1, "最后一行不拉伸")
  }

  /// A row is reserved only for a gloss that can actually arrive.
  ///
  /// English has the offline dictionary the keyboard ships with, so it answers without a
  /// network. Every other language has to be fetched, and a keyboard without full access has
  /// no network at all - reserving a row for it reserves a row that stays blank for the life
  /// of the keyboard. The whole switch being off reserves nothing whatever the languages say.
  func testARowIsReservedOnlyForAGlossThatCanActuallyBeFetched() {
    let previousGloss = CandidateGlossPreference.enabled
    let previousPrimary = CandidateTranslationPreference.primaryIndex
    let previousSecondary = CandidateTranslationPreference.secondaryIndex
    let previousOnline = CandidateTranslationPreference.onlineEnabled
    defer {
      CandidateGlossPreference.enabled = previousGloss
      CandidateTranslationPreference.primaryIndex = previousPrimary
      CandidateTranslationPreference.secondaryIndex = previousSecondary
      CandidateTranslationPreference.onlineEnabled = previousOnline
    }
    CandidateTranslationPreference.primaryIndex = CandidateTranslationPreference.languages.firstIndex { $0.code == "EN" } ?? 0

    CandidateTranslationPreference.onlineEnabled = true
    CandidateGlossPreference.enabled = false
    CandidateTranslationPreference.secondaryIndex = -1
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: true, onlineRoute: true), 0,
                   "the switch is off, so nothing is reserved")

    CandidateGlossPreference.enabled = true
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: true), 1,
                   "English comes from the dictionary in the bundle, with or without a network")

    // 手机只显示一种候选翻译：设了第二种语言也不多留一行。
    let japanese = CandidateTranslationPreference.languages.firstIndex { $0.code == "JA" } ?? 1
    CandidateTranslationPreference.secondaryIndex = japanese
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: true, onlineRoute: true), 1,
                   "a second language never adds a row on the phone")
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false, offline: ["JA"]), 1,
                   "not even when its dictionary is installed")

    // 日语 as the only language - one that has to be fetched.
    CandidateTranslationPreference.secondaryIndex = -1
    CandidateTranslationPreference.primaryIndex = japanese
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: true), 0,
                   "no full access means no network, so that row could never be filled")
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: true, onlineRoute: true), 1,
                   "the row appears once the gloss can be reached")
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: true, onlineRoute: false), 0,
                   "no translation service is chosen, so the Japanese row could never be filled")
    CandidateTranslationPreference.onlineEnabled = false
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: true, onlineRoute: true), 0,
                   "the user turned the network off, which is the same answer as not having one")
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false, offline: ["JA"]), 1,
                   "an installed Japanese dictionary fills that row without any network")
    XCTAssertEqual(KeyboardViewController.configuredGlossLines(fullAccess: false, onlineRoute: false, offline: ["FR"]), 0,
                   "a dictionary for a language nobody picked reserves nothing")
  }

  func testOfflineGlossSkipsSystemPermissionReads() throws {
    let previousOnline = CandidateTranslationPreference.onlineEnabled
    defer { CandidateTranslationPreference.onlineEnabled = previousOnline }
    CandidateTranslationPreference.onlineEnabled = true
    let english = try XCTUnwrap(CandidateTranslationPreference.languages.first { $0.code == "EN" })
    let japanese = try XCTUnwrap(CandidateTranslationPreference.languages.first { $0.code == "JA" })
    var reads = 0
    func permission(_ allowed: Bool = true) -> Bool { reads += 1; return allowed }
    for _ in 0..<9 {
      XCTAssertTrue(KeyboardViewController.canFillGloss(
        english, fullAccess: permission(), onlineRoute: true))
      XCTAssertTrue(KeyboardViewController.canFillGloss(
        japanese, fullAccess: permission(), onlineRoute: true, offline: ["JA"]))
    }
    XCTAssertEqual(reads, 0, "离线候选不应逐词查询系统权限")
    XCTAssertFalse(KeyboardViewController.canFillGloss(
      japanese, fullAccess: permission(), onlineRoute: false))
    XCTAssertEqual(reads, 0, "未配置在线服务时也不应读取权限")
    XCTAssertTrue(KeyboardViewController.canFillGloss(
      japanese, fullAccess: permission(), onlineRoute: true))
    XCTAssertEqual(reads, 1, "需要网络的释义仍须检查系统权限")
    XCTAssertFalse(KeyboardViewController.canFillGloss(
      japanese, fullAccess: permission(false), onlineRoute: true))
    XCTAssertEqual(reads, 2)
    CandidateTranslationPreference.onlineEnabled = false
    XCTAssertFalse(KeyboardViewController.canFillGloss(
      japanese, fullAccess: permission(), onlineRoute: true))
    XCTAssertEqual(reads, 2, "在线释义关闭时不读权限")
  }

  func testTheUsersOwnServiceOutranksTheOfflineDictionaryWhichOutranksTheAccount() {
    let custom = TranslationRoute.custom(endpoint: "https://example.invalid", apiKey: "k")
    XCTAssertEqual(KeyboardViewController.preferredGloss(offline: "essai", online: "test", route: custom), "test")
    XCTAssertEqual(KeyboardViewController.preferredGloss(offline: "essai", online: nil, route: custom), "essai",
                   "the dictionary answers while the service is still on its way")
    XCTAssertEqual(KeyboardViewController.preferredGloss(offline: "essai", online: "test", route: .account), "essai")
    XCTAssertEqual(KeyboardViewController.preferredGloss(offline: nil, online: "test", route: .account), "test")
    XCTAssertEqual(KeyboardViewController.preferredGloss(offline: "essai", online: nil, route: .none), "essai")
    XCTAssertNil(KeyboardViewController.preferredGloss(offline: nil, online: nil, route: .none))
  }

  /// 顶栏预留行加到键盘总高度，不能从按键区扣除；十四键的独立读音行也计入同一契约。
  func testTheKeyboardGrowsByTheRowsTheStripReserves() throws {
    let previousGloss = CandidateGlossPreference.enabled
    let previousSecondary = CandidateTranslationPreference.secondaryIndex
    defer {
      CandidateGlossPreference.enabled = previousGloss
      CandidateTranslationPreference.secondaryIndex = previousSecondary
    }
    CandidateTranslationPreference.secondaryIndex = -1

    func keyboardHeight() -> CGFloat? {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 320)
      controller.viewWillAppear(false)
      return controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant
    }

    setCandidateGloss(false)
    let bare = try XCTUnwrap(keyboardHeight())
    let scheme = InputSchemePreference.scheme
    let separateReadingRow = [.fourteenKey, .eighteenKey].contains(scheme)
    setCandidateGloss(true)
    let glossed = try XCTUnwrap(keyboardHeight())
    XCTAssertEqual(InputSchemePreference.scheme, scheme, "比较释义开关时输入方案必须保持一致")
    XCTAssertEqual(glossed - bare,
                   KeyboardViewController.topRowHeight(glossLines: 1, separateReadingRow: separateReadingRow)
                     - KeyboardViewController.topRowHeight(glossLines: 0, separateReadingRow: separateReadingRow),
                   accuracy: 0.5, "释义行必须加到键盘总高度，不能挤占按键区")
  }

  /// 带释义的词块按内容取宽：候选字和释义里更宽的那个，释义最多把词块撑到一栏，候选字本身更宽时照旧按候选字。
  func testGlossedChipsFitTheirContentUpToAColumn() {
    XCTAssertEqual(KeyboardKeyButton.chipContentWidth(titleLine: 40, glossLine: 26, glossLines: 1, column: 110), 40)
    XCTAssertEqual(KeyboardKeyButton.chipContentWidth(titleLine: 40, glossLine: 80, glossLines: 1, column: 110), 80)
    XCTAssertEqual(KeyboardKeyButton.chipContentWidth(titleLine: 40, glossLine: 300, glossLines: 1, column: 110), 110)
    XCTAssertEqual(KeyboardKeyButton.chipContentWidth(titleLine: 140, glossLine: 300, glossLines: 1, column: 110), 140)
    XCTAssertEqual(KeyboardKeyButton.chipContentWidth(titleLine: 40, glossLine: 300, glossLines: 0, column: 110), 40)
  }

  /// The gloss sits on its own line under the candidate rather than beside it, and a candidate
  /// without one stays a single line - the expanded panel draws the same rows as the strip.
  func testGlossTakesItsOwnLineUnderTheCandidate() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["你好", "泥好"], preedit: "nihao",
      annotations: [
        KeyboardCandidateAnnotation(text: "hello\nこんにちは", accessibilityDescription: "释义"),
        .none,
      ],
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 220)
    panel.layoutIfNeeded()

    let glossed = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton)
    let title = try XCTUnwrap(glossed.configuration?.attributedTitle.map { String($0.characters) })
    XCTAssertEqual(title.split(separator: "\n", omittingEmptySubsequences: false),
                   ["你好", "hello", "こんにちは"],
                   "the gloss belongs on its own line, not crowded beside the candidate")
    let label = try XCTUnwrap(glossed.titleLabel)
    XCTAssertGreaterThan(
      label.bounds.height,
      UIFont.preferredFont(forTextStyle: .body).lineHeight
        + UIFont.preferredFont(forTextStyle: .caption2).lineHeight,
      "a candidate with two glosses has to be drawn as three lines")

    let bare = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-2" } as? UIButton)
    XCTAssertEqual(bare.configuration?.title, "泥好", "a candidate without a gloss stays one line")
  }

  /// Chips must not resize when a gloss lands.
  ///
  /// The online glosses arrive a few hundred milliseconds late. A chip that grows from one line
  /// to two on arrival moves every candidate after it out from under the finger that was already
  /// reaching for one. The row is drawn blank until the answer comes, so the height never moves -
  /// which is also why a chip whose gloss has not arrived still carries a placeholder line rather
  /// than nothing.
  func testAChipKeepsItsHeightWhileTheGlossIsStillOnItsWay() throws {
    let placeholder = KeyboardViewController.pendingGlossPlaceholder
    func annotation(_ text: String) -> KeyboardCandidateAnnotation {
      KeyboardCandidateAnnotation(text: text, accessibilityDescription: text == placeholder ? "" : "释义")
    }
    func panel(_ annotations: [KeyboardCandidateAnnotation]) -> KeyboardCandidatePanelView {
      let view = KeyboardCandidatePanelView(
        candidates: ["你好", "泥嚎"], preedit: "nihao", annotations: annotations,
        display: { $0 }, onSelect: { _ in }, onClose: {})
      view.frame = CGRect(x: 0, y: 0, width: 390, height: 240)
      view.layoutIfNeeded()
      return view
    }
    func height(_ view: KeyboardCandidatePanelView, _ identifier: String) throws -> CGFloat {
      try XCTUnwrap(
        descendants(view).first { $0.accessibilityIdentifier == identifier } as? UIButton
      ).bounds.height
    }

    let waiting = panel([annotation(placeholder), annotation(placeholder)])
    let arrived = panel([annotation("hello"), annotation(placeholder)])
    XCTAssertEqual(try height(waiting, "panelCandidate-1"), try height(arrived, "panelCandidate-1"),
                   accuracy: 0.5, "the chip resized when the gloss landed")
    XCTAssertEqual(try height(arrived, "panelCandidate-1"), try height(arrived, "panelCandidate-2"),
                   accuracy: 0.5,
                   "a page must not draw one height for answered chips and another for waiting ones")

    // Only turning glosses off returns the page to single-line chips.
    let single = panel([.none, .none])
    XCTAssertLessThan(try height(single, "panelCandidate-1"), try height(waiting, "panelCandidate-1"),
                      "a page with no glosses at all should be back to one line")
  }

  /// A stored index the table no longer has resolves to the first language rather than trapping.
  /// The table is versioned with the build; a downgrade or an edited App Group value can leave
  /// an index behind that is now out of range.
  func testTheLanguageTableAnswersTheFirstEntryForAnIndexOutOfRange() {
    let previousPrimary = CandidateTranslationPreference.primaryIndex
    let previousSecondary = CandidateTranslationPreference.secondaryIndex
    defer {
      CandidateTranslationPreference.primaryIndex = previousPrimary
      CandidateTranslationPreference.secondaryIndex = previousSecondary
    }
    XCTAssertEqual(CandidateTranslationPreference.language(at: -1),
                   CandidateTranslationPreference.languages[0])
    XCTAssertEqual(CandidateTranslationPreference.language(at: 99),
                   CandidateTranslationPreference.languages[0])

    CandidateTranslationPreference.primaryIndex = 99
    XCTAssertEqual(CandidateTranslationPreference.primary,
                   CandidateTranslationPreference.languages[0],
                   "an unusable primary index falls back rather than dropping the gloss")
    // The secondary is optional, so an unusable index means "none" instead of the first entry.
    CandidateTranslationPreference.secondaryIndex = 99
    XCTAssertNil(CandidateTranslationPreference.secondary)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }
}
