import XCTest

/// Editing inside the spelling, as the Windows composition does with ← / →: the caret the runtime moves is the one the strip draws, and a key then acts there.
final class CompositionCaretTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-composition-caret-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testTheCaretMovesThroughTheSpellingAndEditsHappenThere() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = MetasequoiaInputSnapshot()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(snapshot.editingText, "nihao")
    XCTAssertEqual(snapshot.caretPosition, 5)
    XCTAssertNil(snapshot.editingTextWithCaret, "a caret at the end leaves the segmented pinyin in place")

    for _ in 0..<3 { snapshot = bridge.moveCaretLeft() }
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertEqual(snapshot.caretPosition, 2)
    XCTAssertEqual(snapshot.editingTextWithCaret, "ni|hao")

    snapshot = bridge.handleBackspace()
    XCTAssertEqual(snapshot.editingText, "nhao", "backspace deletes the letter before the caret, not the last one")
    XCTAssertEqual(snapshot.caretPosition, 1)

    snapshot = bridge.moveCaretRight()
    XCTAssertEqual(snapshot.caretPosition, 2)
    _ = bridge.cancel()
  }

  func testSegmentEditsTakeTheSpellingApartASyllableAtATime() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = MetasequoiaInputSnapshot()
    for letter in "nihaoshijie" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(snapshot.caretPosition, 11)

    snapshot = bridge.moveCaretLeftBySegment()
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertEqual(snapshot.caretPosition, 8, "one step lands on the boundary before jie")
    snapshot = bridge.moveCaretRightBySegment()
    XCTAssertEqual(snapshot.caretPosition, 11)

    snapshot = bridge.segmentBackspace()
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertEqual(snapshot.editingText, "nihaoshi", "the whole last syllable goes, not one letter")
    for _ in 0..<3 { snapshot = bridge.segmentBackspace() }
    XCTAssertEqual(snapshot.editingText, "")
    XCTAssertTrue(snapshot.candidates.isEmpty)
    _ = bridge.cancel()
  }

  func testOnlyALetteredPinyinSpellingIsEditedBySyllable() {
    XCTAssertEqual(ChineseInputScheme.allCases.filter(\.editsBySyllable),
                   [.quanpin, .shuangpin, .ziranma, .microsoft, .shoudao])
  }

  func testOnlyAQuickFlickMovesBySyllable() {
    XCTAssertFalse(SpaceCursorMovement.movesBySegment(velocity: 300))
    XCTAssertTrue(SpaceCursorMovement.movesBySegment(velocity: -1200))
    XCTAssertFalse(SpaceCursorMovement.movesBySegment(velocity: .nan))
  }

  func testHomeEndAndForwardDeleteActInsideTheSpelling() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = MetasequoiaInputSnapshot()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    snapshot = bridge.moveCaretToStart()
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertEqual(snapshot.caretPosition, 0)
    snapshot = bridge.deleteForward()
    XCTAssertEqual(snapshot.editingText, "ihao", "the letter after the caret goes")
    XCTAssertEqual(snapshot.caretPosition, 0)
    snapshot = bridge.moveCaretToEnd()
    XCTAssertEqual(snapshot.caretPosition, 4)
    _ = bridge.cancel()
  }

  /// 26 键全拼：拼写写进输入框后，候选行最左一段画一支笔（#10591），点它打开候选栏下面的放大拼写编辑条，键盘高出一行；在编辑条上放光标、再点入口保持打开、「完成」把光标移回末尾并收起。
  @MainActor
  func testTappingTheReadingOpensTheSpellingEditor() throws {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    let preedit = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "preeditButton"
    } as? UIButton)
    let bar = try XCTUnwrap(descendants(controller.view).first { $0 is CompositionEditorBar } as? CompositionEditorBar)
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let closedHeight = height.constant
    XCTAssertTrue(bar.isHidden)
    XCTAssertNotEqual(preedit.accessibilityLabel, "编辑拼写", "nothing to edit before a spelling")

    for letter in "NIHAO" { try key(letter, in: controller).sendActions(for: .primaryActionTriggered) }
    // 拼写在输入框里，读音那一段不再收成零宽，画的是笔。
    XCTAssertFalse(preedit.isHidden)
    XCTAssertEqual(preedit.configuration?.title ?? "", "")
    XCTAssertNotNil(preedit.configuration?.image, "a pen marks the entry while the spelling is in the field")
    XCTAssertNil(preedit.menu, "the spelling menu is gone, a tap opens the editor bar")
    XCTAssertEqual(preedit.accessibilityLabel, "编辑拼写")
    XCTAssertTrue(preedit.accessibilityTraits.contains(.button))
    XCTAssertTrue(bar.isHidden, "typing alone does not open the editor")
    // 读音那一段上下占满的整个高度都点得到读音（#10474）。
    controller.view.layoutIfNeeded()
    let row = try XCTUnwrap(preedit.superview)
    XCTAssertEqual(row.accessibilityIdentifier, "compositionRow")
    XCTAssertGreaterThanOrEqual(row.bounds.width, 40, "the pen keeps the segment wide enough to tap")
    for y in [row.bounds.minY + 0.5, row.bounds.midY, row.bounds.maxY - 0.5] {
      XCTAssertTrue(row.hitTest(CGPoint(x: row.bounds.midX, y: y), with: nil) === preedit, "y = \(y) in a \(row.bounds.height)pt row")
    }

    preedit.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(bar.isHidden)
    XCTAssertEqual(height.constant, closedHeight + CompositionEditorBar.height, "the keyboard grows by the editor row")
    XCTAssertEqual(bar.spelling.display.filter(\.isLetter), "nihao")
    XCTAssertEqual(bar.spelling.offset, bar.spelling.display.count, "the caret starts at the end")
    preedit.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(bar.isHidden, "tapping the entry again keeps the editor open")
    XCTAssertEqual(height.constant, closedHeight + CompositionEditorBar.height)

    // 点在拼写开头：光标到开头，读音行画出光标。
    bar.spelling.requestCaret(atDisplayOffset: 0)
    XCTAssertEqual(preedit.configuration?.title, "|nihao")
    XCTAssertEqual(bar.spelling.offset, 0)
    // 读屏的调整动作逐个按键走。
    bar.spelling.accessibilityIncrement()
    XCTAssertEqual(preedit.configuration?.title, "n|ihao")
    XCTAssertFalse(bar.isHidden)

    try XCTUnwrap(descendants(bar).first { $0.accessibilityIdentifier == "compositionEditorDone" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(bar.isHidden)
    XCTAssertEqual(height.constant, closedHeight)
    XCTAssertEqual(preedit.configuration?.title ?? "", "", "done sends the caret back to the end")
    XCTAssertNotNil(preedit.configuration?.image)
    controller.viewDidDisappear(false)
  }

  /// 编辑条开着时组字结束（选了候选），它自己收起，键盘回到原来的高度。
  @MainActor
  func testTheSpellingEditorClosesWhenTheCompositionEnds() throws {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let closedHeight = height.constant
    for letter in "NIHAO" { try key(letter, in: controller).sendActions(for: .primaryActionTriggered) }
    let preedit = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    let bar = try XCTUnwrap(descendants(controller.view).first { $0 is CompositionEditorBar } as? CompositionEditorBar)
    preedit.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(bar.isHidden)
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidate-1" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(bar.isHidden)
    XCTAssertEqual(height.constant, closedHeight)
    XCTAssertTrue(preedit.isHidden, "idle again, the inline segment collapses")
    controller.viewDidDisappear(false)
  }

  /// 九键：编辑条画拼音读音，光标按九键键位对到数字上，点读音里的字母把光标放到对应的数字前。
  @MainActor
  func testTheNineKeyEditorPlacesTheCaretAmongTheDigits() throws {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    for digit in "64426" {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "nineKey\(digit)" } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    let preedit = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    let bar = try XCTUnwrap(descendants(controller.view).first { $0 is CompositionEditorBar } as? CompositionEditorBar)
    XCTAssertEqual(preedit.accessibilityLabel, "编辑拼写")
    preedit.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(bar.isHidden)
    let display = bar.spelling.display
    XCTAssertTrue(display.contains { $0.isLetter }, "the editor shows the reading, not the digits: \(display)")
    let map = try XCTUnwrap(CompositionCaretPolicy.alignment(display, "64426"))
    // 点在第二个音节的第一个字母前：光标落在它对应的数字前。
    let second = try XCTUnwrap(Array(display).indices.dropFirst().first { map[$0] == 2 && Array(display)[$0].isLetter })
    bar.spelling.requestCaret(atDisplayOffset: second)
    XCTAssertEqual(preedit.configuration?.title, "64|426")
    XCTAssertEqual(bar.spelling.offset, second)
    controller.viewDidDisappear(false)
  }

  /// 读音单独一行时，它和候选行之间的皮肤行距也归读音；拼音选择条下面那段缝不归读音（#10591，与 Android 相同）。
  @MainActor
  func testTheGapBelowASeparateReadingRowOpensTheEditor() {
    let row = KeyboardViewController.ReadingRowView(frame: CGRect(x: 0, y: 0, width: 300, height: 14))
    XCTAssertTrue(row.hits(CGPoint(x: 290, y: 7)))
    XCTAssertFalse(row.hits(CGPoint(x: 20, y: 16)), "no gap, nothing below the row")
    row.hitBottomExtension = 6
    XCTAssertTrue(row.hits(CGPoint(x: 20, y: 16)))
    XCTAssertTrue(row.hits(CGPoint(x: 290, y: 19.5)))
    XCTAssertFalse(row.hits(CGPoint(x: 20, y: 20)), "the candidate row starts after the gap")
    let strip = UIView(frame: CGRect(x: 150, y: 0, width: 150, height: 14))
    row.addSubview(strip)
    row.bandLimit = strip
    XCTAssertTrue(row.hits(CGPoint(x: 100, y: 16)))
    XCTAssertFalse(row.hits(CGPoint(x: 200, y: 16)), "the gap under the spelling strip is not the reading's")
    strip.isHidden = true
    XCTAssertTrue(row.hits(CGPoint(x: 200, y: 16)))
  }

  private func key(_ letter: Character, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter)" } as? UIButton)
  }

  private func descendants(_ root: UIView) -> [UIView] {
    root.subviews + root.subviews.flatMap(descendants)
  }

  func testOnlyAnAsciiSpellingWithAnInnerCaretIsSplit() {
    XCTAssertEqual(MetasequoiaInputSnapshot(editingText: "nihao", caretPosition: 0).editingTextWithCaret, "|nihao")
    XCTAssertNil(MetasequoiaInputSnapshot(editingText: "nihao", caretPosition: 5).editingTextWithCaret)
    XCTAssertNil(MetasequoiaInputSnapshot(editingText: "nihao", caretPosition: 9).editingTextWithCaret)
    XCTAssertNil(MetasequoiaInputSnapshot(editingText: "にほ", caretPosition: 1).editingTextWithCaret)
  }
}
