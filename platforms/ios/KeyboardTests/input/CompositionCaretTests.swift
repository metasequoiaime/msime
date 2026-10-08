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

  func testTheSpellingMenuDimsWhatWouldDoNothing() {
    let atStart = KeyboardViewController.spellingEditMenu(("nihao", 0))
    XCTAssertEqual(atStart.map(\.title), ["光标移到开头", "光标移到末尾", "删除光标后的字母"])
    XCTAssertEqual(atStart.map(\.enabled), [false, true, true])
    XCTAssertEqual(KeyboardViewController.spellingEditMenu(("nihao", 5)).map(\.enabled), [true, false, false])
    XCTAssertEqual(KeyboardViewController.spellingEditMenu(("nihao", 2)).map(\.enabled), [true, true, true])
  }

  @MainActor
  func testTappingTheSpellingOffersHomeEndAndDelete() throws {
    enableAllInputSchemes()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let preedit = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "preeditButton"
    } as? UIButton)
    XCTAssertNotEqual(preedit.menu?.title, "编辑拼写", "nothing to edit before a spelling")

    for letter in "NIHAO" {
      try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(letter)" } as? UIButton)
        .sendActions(for: .primaryActionTriggered)
    }
    let menu = try XCTUnwrap(preedit.menu)
    XCTAssertEqual(menu.title, "编辑拼写")
    XCTAssertTrue(preedit.accessibilityTraits.contains(.button))
    let home = try XCTUnwrap(menu.children.first as? UIAction)
    home.performWithSender(nil, target: nil)
    XCTAssertEqual(preedit.configuration?.title, "|nihao")
    let delete = try XCTUnwrap(preedit.menu?.children.last as? UIAction)
    XCTAssertFalse(delete.attributes.contains(.disabled))
    delete.performWithSender(nil, target: nil)
    XCTAssertEqual(preedit.configuration?.title, "|ihao")
    try XCTUnwrap(preedit.menu?.children[1] as? UIAction).performWithSender(nil, target: nil)
    XCTAssertNotEqual(preedit.configuration?.title, "|ihao", "the caret is back at the end")
    XCTAssertTrue(try XCTUnwrap(preedit.menu?.children.last as? UIAction).attributes.contains(.disabled))
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
