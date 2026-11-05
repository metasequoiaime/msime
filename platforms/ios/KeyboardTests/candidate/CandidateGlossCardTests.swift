import ObjectiveC
import UIKit
import XCTest

/// 长按候选打开的释义卡片（#10915）：内容怎么排、两个按钮、怎么关，以及原来长按菜单里的操作仍然做得到。
///
/// 控制器用例和邮箱后缀的用例一样把 `textDocumentProxy` 换成内存里的文档，看得到上屏了什么。长按手势本身由 UIKit 识别，用例走同一个入口的读屏操作「查看释义」。
@MainActor
final class CandidateGlossCardTests: XCTestCase {
  /// `text` 只放已上屏的文字；标记文本另记在 `marked`，取消标记时并进 `text`，插入文字时替换掉它。和真实文档一样，整段上屏走「改标记再取消标记」（#10946）时也能看到上屏了什么。
  private final class Document: NSObject, UITextDocumentProxy {
    var text = ""
    var marked = ""
    var documentContextBeforeInput: String? { text }
    var documentContextAfterInput: String? { nil }
    var selectedText: String? { nil }
    var documentInputMode: UITextInputMode? { nil }
    var documentIdentifier = UUID()
    var hasText: Bool { !text.isEmpty }
    func insertText(_ inserted: String) { marked = ""; text += inserted }
    func deleteBackward() { if !text.isEmpty { text.removeLast() } }
    func adjustTextPosition(byCharacterOffset offset: Int) {}
    func setMarkedText(_ markedText: String, selectedRange: NSRange) { marked = markedText }
    func unmarkText() {
      text += marked
      marked = ""
    }
  }

  // MARK: - 内容

  func testOfflineSensesAreListedOneByOneAndEachSectionNamesItsSource() {
    let content = CandidateGlossCardContent(
      candidate: "你好",
      languages: [("英语", ("hello; hi；hello\nhow do you do", true)), ("日语", ("こんにちは", false))],
      route: .tencent(secretID: "id", secretKey: "key", region: "ap-guangzhou"))
    XCTAssertEqual(content.sections.map(\.sourceLine), ["英语 · 离线释义", "日语 · 腾讯翻译"])
    XCTAssertEqual(content.sections.first?.senses, ["hello", "hi", "how do you do"], "分号隔开的义项逐条列出，重复的只留一条")
    XCTAssertEqual(content.sections.last?.senses, ["こんにちは"], "在线译文是一整条")
    XCTAssertEqual(content.primaryTranslation, "hello", "上屏和复制的是第一目标语言的第一条译文")
  }

  func testEveryOnlineServiceHasItsOwnSourceName() {
    let routes: [(TranslationRoute, String)] = [
      (.account, "水杉账号"), (.niutrans(appID: "a", apiKey: "k"), "小牛翻译"),
      (.tencent(secretID: "i", secretKey: "k", region: "r"), "腾讯翻译"), (.custom(endpoint: "https://example.invalid", apiKey: ""), "自定义服务"),
      (.ai(provider: "p", endpoint: "https://example.invalid", model: "m", token: "t"), "AI 辅助"),
    ]
    for (route, name) in routes {
      let content = CandidateGlossCardContent(candidate: "你好", languages: [("英语", ("hello", false))], route: route)
      XCTAssertEqual(content.sections.first?.sourceLine, "英语 · \(name)")
    }
  }

  /// 第一目标语言还没有译文时两个按钮都不可用，即使第二目标语言有：上屏的必须是第一目标语言的译文。
  func testWithoutAPrimaryTranslationTheCardSaysSoAndBothButtonsAreDisabled() throws {
    let empty = CandidateGlossCardContent(candidate: "你好", languages: [("英语", nil)], route: .none)
    XCTAssertTrue(empty.sections.isEmpty)
    XCTAssertNil(empty.primaryTranslation)
    let secondaryOnly = CandidateGlossCardContent(
      candidate: "你好", languages: [("英语", nil), ("日语", ("こんにちは", true))], route: .none)
    XCTAssertNil(secondaryOnly.primaryTranslation)

    var commits = 0, copies = 0
    let card = CandidateGlossCardView(content: empty, skin: KeyboardTheme.current,
                                      onCommit: { commits += 1 }, onCopy: { copies += 1 }, onClose: {})
    card.frame = CGRect(x: 0, y: 0, width: 390, height: 216)
    card.layoutIfNeeded()
    XCTAssertEqual(label("glossCardEmpty", in: card)?.text, CandidateGlossCardContent.emptyTitle)
    XCTAssertEqual(label("glossCardEmptyHint", in: card)?.text, CandidateGlossCardContent.emptyHint)
    XCTAssertFalse(card.commitButton.isEnabled)
    XCTAssertFalse(card.copyButton.isEnabled)
    XCTAssertTrue(card.moreButton.isHidden, "没有可做的操作时不出「更多操作」")

    // 开着可用的在线翻译服务、只是还没答上来时，不叫用户去设置里开启服务。
    let pending = CandidateGlossCardView(
      content: CandidateGlossCardContent(candidate: "你好", languages: [("英语", nil)], route: .account, serviceAvailable: true),
      skin: KeyboardTheme.current, onCommit: {}, onCopy: {}, onClose: {})
    XCTAssertEqual(label("glossCardEmpty", in: pending)?.text, CandidateGlossCardContent.emptyTitle)
    XCTAssertNil(label("glossCardEmptyHint", in: pending))

    card.update(content: CandidateGlossCardContent(candidate: "你好", languages: [("英语", ("hello", true))], route: .none),
                moreMenu: UIMenu(children: [UIAction(title: "优先显示") { _ in }]))
    card.layoutIfNeeded()
    XCTAssertNil(label("glossCardEmpty", in: card))
    XCTAssertEqual(label("glossCardSource-0", in: card)?.text, "英语 · 离线释义")
    XCTAssertEqual(label("glossCardSource-0", in: card)?.accessibilityLabel, "英语，来源：离线释义")
    XCTAssertEqual(label("glossCardSense-0-0", in: card)?.text, "hello")
    XCTAssertTrue(card.commitButton.isEnabled)
    XCTAssertFalse(card.moreButton.isHidden)
    XCTAssertTrue(card.moreButton.showsMenuAsPrimaryAction)
    card.commitButton.sendActions(for: .primaryActionTriggered)
    card.copyButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual([commits, copies], [1, 1])
  }

  // MARK: - 键盘里的卡片

  func testLongPressOpensTheCardOverTheKeysAndEachWayOfClosingRestoresThem() throws {
    setCandidateGloss(false)
    let (controller, document) = try makeKeyboard()
    type("nihao", in: controller)
    let firstWord = try XCTUnwrap(chipWord(controller))

    try openCard(fromStripIn: controller)
    let card = try XCTUnwrap(card(in: controller))
    XCTAssertEqual(label("glossCardTitle", in: card)?.text, "释义")
    XCTAssertEqual(label("glossCardCandidate", in: card)?.text, firstWord)
    XCTAssertEqual(label("glossCardEmpty", in: card)?.text, "暂无译文", "关着离线释义、没有翻译服务时没有译文")
    XCTAssertEqual(label("glossCardEmptyHint", in: card)?.text, "可以在设置里开启翻译服务")
    XCTAssertTrue(keysCovered(controller), "卡片盖住整块按键区")
    XCTAssertFalse(try button("candidate-1", in: controller).isHidden, "候选栏留着")

    // 收起键。
    try XCTUnwrap(descendants(card).first { $0.accessibilityIdentifier == "glossCardClose" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertNil(self.card(in: controller))
    XCTAssertFalse(keysCovered(controller))
    XCTAssertNil(dismissArea(in: controller))

    // 点卡片外的候选栏：关掉卡片，不选词，组字还在。
    try openCard(fromStripIn: controller)
    try XCTUnwrap(dismissArea(in: controller)).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(self.card(in: controller))
    XCTAssertFalse(keysCovered(controller))
    XCTAssertEqual(chipWord(controller), firstWord, "点候选栏只关卡片，不上屏")

    // 换了输入框。
    try openCard(fromStripIn: controller)
    document.documentIdentifier = UUID()
    controller.textDidChange(nil)
    XCTAssertNil(self.card(in: controller), "换输入框关掉卡片")
    XCTAssertFalse(keysCovered(controller))

    // 键盘收起。
    type("nihao", in: controller)
    try openCard(fromStripIn: controller)
    controller.viewWillDisappear(false)
    XCTAssertNil(self.card(in: controller))
    XCTAssertFalse(keysCovered(controller))
  }

  func testCommitInsertsThePrimaryTranslationAndCopyExplainsMissingFullAccess() throws {
    setCandidateGloss(true)
    let (controller, document) = try makeKeyboard()
    type("nihao", in: controller)
    let gloss = try waitForGloss(in: controller)

    try openCard(fromStripIn: controller)
    var card = try XCTUnwrap(card(in: controller))
    XCTAssertEqual(label("glossCardSource-0", in: card)?.text, "英语 · 离线释义")
    XCTAssertEqual(card.content.primaryTranslation, gloss)
    XCTAssertTrue(card.copyButton.isEnabled)

    // 测试宿主没有完全访问：复制不了，卡片上说明原因，不写剪贴板。
    XCTAssertFalse(controller.hasFullAccess)
    let pasteboard = UIPasteboard.general.string
    card.copyButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(card.notice, CandidateGlossCardContent.copyNeedsFullAccess)
    XCTAssertEqual(UIPasteboard.general.string, pasteboard)
    XCTAssertNotNil(self.card(in: controller), "复制之后卡片留着")

    card.commitButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(document.text, gloss, "上屏的是译文，不是候选原文")
    XCTAssertNil(self.card(in: controller))
    XCTAssertFalse(keysCovered(controller))

    // 开着完全访问时写进系统剪贴板。
    try grantFullAccess()
    type("nihao", in: controller)
    _ = try waitForGloss(in: controller)
    try openCard(fromStripIn: controller)
    card = try XCTUnwrap(self.card(in: controller))
    let previous = UIPasteboard.general.string
    addTeardownBlock { UIPasteboard.general.string = previous }
    card.copyButton.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(UIPasteboard.general.string, gloss)
    XCTAssertEqual(card.notice, CandidateGlossCardContent.copied)
  }

  /// 换代后按文字重新定位：原位置上还是它就留在原位，否则取第一个同字的位置，没有了是 nil。
  func testRelocateFindsTheCandidateByTextAfterANewGeneration() {
    XCTAssertEqual(CandidateGlossCardTarget.relocate(word: "你好", previous: 1, in: ["妳好", "你好"]), 1)
    XCTAssertEqual(CandidateGlossCardTarget.relocate(word: "你好", previous: 1, in: ["云词", "妳好", "你好"]), 2)
    XCTAssertEqual(CandidateGlossCardTarget.relocate(word: "你好", previous: 5, in: ["你好"]), 0)
    XCTAssertNil(CandidateGlossCardTarget.relocate(word: "你好", previous: 0, in: ["妳好"]))
  }

  /// 同一个组字里候选换了一代（云候选、AI 候选在长按之后才到，把候选往后挤了一格）时卡片留着，跟着这个候选到新位置，「更多操作」里的词库操作也跟过去；组字变了才关。Android #10934 在设备上撞到过新的一代在长按后约 130 ms 到达、把卡片关掉。
  func testCardSurvivesALateGenerationInTheSameCompositionAndClosesWhenTheCompositionChanges() throws {
    setCandidateGloss(false)
    let (controller, _) = try makeKeyboard()
    type("nihao", in: controller)
    let shown = try separateSessionSnapshot(typing: "nihao")
    try XCTSkipIf(shown.candidates.count < 2, "the fixture spelling needs at least two candidates")
    let second = try XCTUnwrap(try button("candidate-2", in: controller).accessibilityLabel?
      .split(separator: "：").dropFirst().first.map { String($0.split(separator: "，").first ?? "") })

    let chip = try button("candidate-2", in: controller)
    let hold = try XCTUnwrap(chip.accessibilityCustomActions?.first { $0.name == "查看释义" })
    XCTAssertTrue(hold.actionHandler?(hold) ?? false)
    XCTAssertEqual(label("glossCardCandidate", in: try XCTUnwrap(card(in: controller)))?.text, second)

    // 晚到的在线回答：同一个组字，新的一代，前面多了一个候选。
    func padded<T>(_ values: [T], _ first: T) -> [T] { [first] + values }
    let late = MetasequoiaInputSnapshot(
      isHandled: true, preedit: shown.preedit, reading: shown.reading, phrasePrefix: shown.phrasePrefix,
      candidates: padded(shown.candidates, "合成云词"), candidateCodes: padded(shown.candidateCodes, ""),
      candidateIdentities: padded(shown.candidateIdentities, nil), generation: (shown.generation ?? 0) &+ 1_000,
      candidateGlosses: padded(shown.candidateGlosses, ""), candidateAnnotations: padded(shown.candidateAnnotations, ""),
      candidateSources: padded(shown.candidateSources, 0), candidateFixedPositions: padded(shown.candidateFixedPositions, 0),
      candidateHighlighted: padded(shown.candidateHighlighted.map { _ in false }, true),
      candidateConfidentCorrections: padded(shown.candidateConfidentCorrections, false),
      candidatePageCount: shown.candidatePageCount, editingText: shown.editingText, caretPosition: shown.caretPosition)
    controller.render(late)
    controller.view.layoutIfNeeded()
    let card = try XCTUnwrap(self.card(in: controller), "新的一代候选不关卡片")
    XCTAssertEqual(label("glossCardCandidate", in: card)?.text, second)
    XCTAssertTrue(keysCovered(controller))
    XCTAssertTrue(try button("candidate-3", in: controller).accessibilityLabel?.contains("：\(second)") == true,
                  "fixture: the candidate moved one seat right")
    let titles = try XCTUnwrap(card.moreButton.menu).children.map(\.title)
    XCTAssertTrue(titles.contains("优先显示") && titles.contains("固定排位"), "词库操作跟着候选到了新位置：\(titles)")

    // 组字变了（多打了一个字母）才关。
    controller.render(MetasequoiaInputSnapshot(
      isHandled: true, preedit: shown.preedit + "m", candidates: shown.candidates,
      generation: (shown.generation ?? 0) &+ 2_000, candidateSources: shown.candidateSources))
    XCTAssertNil(self.card(in: controller), "组字变了关卡片")
    XCTAssertFalse(keysCovered(controller))
  }

  /// 原来长按菜单里的操作原样在「更多操作」里，选一项先关掉卡片再做那件事。
  func testOldLongPressActionsLiveInTheMoreMenu() throws {
    setCandidateGloss(false)
    let (controller, document) = try makeKeyboard()
    type("nihao", in: controller)
    try openCard(fromStripIn: controller)
    let card = try XCTUnwrap(card(in: controller))
    let menu = try XCTUnwrap(card.moreButton.menu)
    XCTAssertFalse(card.moreButton.isHidden)
    XCTAssertEqual(menu.children.map(\.title), controller.candidateMenuElements(at: 0).map(\.title))
    XCTAssertEqual(menu.children.map(\.title), ["以词定字", "优先显示", "固定排位", "删除词条…"])
    let slots = try XCTUnwrap((menu.children[2] as? UIMenu)?.children as? [UIAction])
    XCTAssertEqual(slots.map(\.title), ["第 1 位", "第 2 位", "第 3 位", "第 4 位", "第 5 位"])
    XCTAssertEqual((menu.children[3] as? UIMenu)?.children.first?.title, "确认删除此词条")

    let first = try XCTUnwrap(actions(in: menu).first { $0.title.hasPrefix("只上屏首字") })
    first.performWithSender(nil, target: nil)
    XCTAssertNil(self.card(in: controller), "选了一项卡片就关上")
    XCTAssertEqual(document.text, "你", "以词定字照旧只上屏首字")
  }

  /// 展开网格里长按同样打开卡片，卡片叠在网格上；关掉卡片回到网格，网格里的长按菜单操作也在「更多操作」里。
  func testGridLongPressOpensTheCardAboveTheGridAndClosingReturnsToIt() throws {
    setCandidateGloss(false)
    let (controller, _) = try makeKeyboard()
    type("nihao", in: controller)
    try button("expandCandidates", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
    let chip = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "panelCandidate-1" } as? UIButton)
    let hold = try XCTUnwrap(chip.accessibilityCustomActions?.first { $0.name == "查看释义" })
    XCTAssertTrue(hold.actionHandler?(hold) ?? false)
    controller.view.layoutIfNeeded()

    let card = try XCTUnwrap(card(in: controller))
    XCTAssertEqual(card.frame, panel.frame, "卡片盖住网格所在的整块按键区")
    XCTAssertEqual(try XCTUnwrap(card.moreButton.menu).children.map(\.title),
                   ["以词定字", "优先显示", "固定排位", "删除词条…"])

    try XCTUnwrap(dismissArea(in: controller)).sendActions(for: .primaryActionTriggered)
    XCTAssertNil(self.card(in: controller))
    XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" }, "回到网格")
    XCTAssertTrue(keysCovered(controller), "网格还盖着按键")
  }

  // MARK: - 辅助

  private func makeKeyboard() throws -> (KeyboardViewController, Document) {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let document = Document()
    try attach(document)
    let controller = KeyboardViewController()
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 320))
    window.rootViewController = controller
    window.isHidden = false
    addTeardownBlock { window.isHidden = true }
    controller.view.frame = window.bounds
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return (controller, document)
  }

  /// 另起一个用临时目录的会话打同样的拼写，取它的快照：合成新一代候选时用它的拼写和候选，与键盘里的会话一致。
  private func separateSessionSnapshot(typing letters: String) throws -> MetasequoiaInputSnapshot {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-gloss-card-\(UUID().uuidString)", isDirectory: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.cancel()
    var typed = bridge.currentSnapshot()
    for letter in letters { typed = bridge.handleCharacter(String(letter)) }
    _ = bridge.cancel()
    return typed
  }

  private func type(_ letters: String, in controller: KeyboardViewController) {
    for letter in letters {
      (descendants(controller.view).first { $0.accessibilityLabel == "字母 \(String(letter).uppercased())" } as? UIButton)?
        .sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
  }

  private func openCard(fromStripIn controller: KeyboardViewController) throws {
    let chip = try button("candidate-1", in: controller)
    XCTAssertTrue(chip.gestureRecognizers?.contains { $0 is UILongPressGestureRecognizer } == true)
    XCTAssertNil(chip.menu, "长按不再弹系统菜单")
    let hold = try XCTUnwrap(chip.accessibilityCustomActions?.first { $0.name == "查看释义" })
    XCTAssertTrue(hold.actionHandler?(hold) ?? false)
    controller.view.layoutIfNeeded()
  }

  /// 离线释义在键盘线程外查好、晚一点才到，所以按菜单里出现的释义轮询，不按固定时长等。固定版本的词库里没有这个词的英文释义时跳过，与 `CandidateTranslationTests` 的长按用例相同。
  private func waitForGloss(in controller: KeyboardViewController) throws -> String {
    let deadline = Date().addingTimeInterval(10)
    var titles: [String] = []
    repeat {
      RunLoop.current.run(until: Date().addingTimeInterval(0.05))
      titles = controller.candidateMenuElements(at: 0).compactMap { ($0 as? UIAction)?.title }
    } while !titles.contains { $0.lowercased().contains("hello") } && Date() < deadline
    let gloss = titles.first { $0.lowercased().contains("hello") }
    try XCTSkipIf(gloss == nil, "No offline gloss for the fixture candidate in the pinned dictionary release.")
    return CandidateGlossCardContent.senses(try XCTUnwrap(gloss)).first ?? ""
  }

  private func chipWord(_ controller: KeyboardViewController) -> String? {
    (try? button("candidate-1", in: controller))?.accessibilityLabel?
      .split(separator: "：").dropFirst().first.map { String($0.split(separator: "，").first ?? "") }
  }

  private func actions(in menu: UIMenu) -> [UIAction] {
    menu.children.flatMap { element -> [UIAction] in
      if let menu = element as? UIMenu { return actions(in: menu) }
      return (element as? UIAction).map { [$0] } ?? []
    }
  }

  private func card(in controller: KeyboardViewController) -> CandidateGlossCardView? {
    descendants(controller.view).compactMap { $0 as? CandidateGlossCardView }.first
  }

  private func dismissArea(in controller: KeyboardViewController) -> UIButton? {
    descendants(controller.view).first { $0.accessibilityIdentifier == "glossCardDismissArea" } as? UIButton
  }

  /// 按键区被键区面板盖住时各排字母键不画、不收触摸（`setKeyRowsCovered`）。
  private func keysCovered(_ controller: KeyboardViewController) -> Bool {
    guard let key = descendants(controller.view).first(where: { $0.accessibilityLabel == "字母 Q" }) else { return false }
    var node: UIView? = key
    while let current = node, current !== controller.view {
      if current.alpha == 0 || current.isHidden { return true }
      node = current.superview
    }
    return false
  }

  private func label(_ identifier: String, in root: UIView) -> UILabel? {
    descendants(root).first { $0.accessibilityIdentifier == identifier } as? UILabel
  }

  private func descendants(_ node: UIView) -> [UIView] { [node] + node.subviews.flatMap { descendants($0) } }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  private func attach(_ document: Document) throws {
    try replace(#selector(getter: UIInputViewController.textDocumentProxy), with: { (_: AnyObject) -> AnyObject in document } as @convention(block) (AnyObject) -> AnyObject)
  }

  private func grantFullAccess() throws {
    try replace(#selector(getter: UIInputViewController.hasFullAccess), with: { (_: AnyObject) -> Bool in true } as @convention(block) (AnyObject) -> Bool)
  }

  /// 用例期间让 `KeyboardViewController` 的这个属性返回给定的值，结束时换回 `UIInputViewController` 自己的实现。
  private func replace(_ selector: Selector, with block: Any) throws {
    let inherited = try XCTUnwrap(class_getInstanceMethod(UIInputViewController.self, selector))
    let original = method_getImplementation(inherited)
    let replacement = imp_implementationWithBlock(block)
    if !class_addMethod(KeyboardViewController.self, selector, replacement, method_getTypeEncoding(inherited)) {
      method_setImplementation(try XCTUnwrap(class_getInstanceMethod(KeyboardViewController.self, selector)), replacement)
    }
    addTeardownBlock {
      if let own = class_getInstanceMethod(KeyboardViewController.self, selector) { method_setImplementation(own, original) }
      imp_removeBlock(replacement)
    }
  }
}
