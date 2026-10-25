import XCTest
import UIKit

@MainActor
final class HandwritingTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  func testDownloadCompletionFromAnInactiveGenerationIsIgnored() {
    var gate = HandwritingDownloadGate()
    gate.setActive(true)
    let token = gate.beginDownload()
    gate.setActive(false)
    XCTAssertFalse(gate.accepts(token))
    gate.setActive(true)
    XCTAssertFalse(gate.accepts(token))
  }

  // Pen trajectories for 中国, not text rendered using a font.
  private var chineseInk: [[CGPoint]] {
    let points: [[(Double, Double)]] = [
      [(35, 40), (35, 105)], [(35, 40), (125, 40), (125, 105)], [(35, 105), (125, 105)], [(80, 15), (80, 140)],
      [(175, 20), (175, 140)], [(175, 20), (280, 20), (280, 140)], [(175, 140), (280, 140)],
      [(192, 45), (261, 45)], [(198, 78), (257, 78)], [(226, 45), (226, 112)], [(190, 112), (264, 112)], [(247, 91), (256, 101)],
    ]
    return points.map { $0.map { CGPoint(x: $0.0, y: $0.1) } }
  }
  #if canImport(MLKitDigitalInkRecognition)
  func testRealChineseInkRecognition() async throws {
    let canvas = HandwritingCanvas(frame: CGRect(x: 0, y: 0, width: 320, height: 155))
    canvas.setTestStrokes(chineseInk)
    let recognizer = HandwritingRecognizer()
    try await recognizer.download { _ in }
    let words = try await recognizer.recognize(Array(chineseInk.prefix(4)), width: 160, height: 155)
    XCTAssertEqual(words.first, "中", "Actual candidates: \(words)")
  }
  func testCommonCharactersFromPenTrajectories() async throws {
    let recognizer = HandwritingRecognizer()
    try await recognizer.download { _ in }
    let examples: [(String, [[(Double, Double)]])] = [
      ("人", [[(80,20),(75,55),(60,95),(30,140)],[(73,65),(90,100),(130,140)]]),
      ("大", [[(25,65),(135,65)],[(80,20),(75,70),(60,110),(25,145)],[(78,70),(95,110),(140,145)]]),
      ("木", [[(25,60),(135,60)],[(80,15),(80,145)],[(76,65),(55,95),(20,125)],[(85,70),(105,100),(140,125)]]),
      ("水", [[(80,15),(80,135),(73,145),(58,135)],[(20,65),(55,65),(45,90),(18,120)],[(130,40),(95,75)],[(85,60),(103,100),(140,130)]]),
      ("天", [[(35,30),(125,30)],[(20,65),(140,65)],[(80,30),(75,80),(55,120),(20,145)],[(78,80),(100,120),(140,145)]]),
      ("日", [[(40,20),(40,140)],[(40,20),(120,20),(120,140)],[(40,80),(120,80)],[(40,140),(120,140)]]),
    ]
    for (expected, strokes) in examples {
      let ink = strokes.map { $0.map { CGPoint(x: $0.0, y: $0.1) } }
      let words = try await recognizer.recognize(ink, width: 160, height: 160)
      print("Handwriting \(expected): \(words)")
      XCTAssertEqual(words.first, expected, "Actual candidates for \(expected): \(words)")
    }
  }
  func testCandidateSelectionInsertsOnlyAfterConfirmation() async throws {
    try await HandwritingRecognizer().download { _ in }
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 414, height: 160))
    panel.layoutIfNeeded()
    var inserted = ""
    var published: [String] = []
    panel.onInsert = { inserted += $0 }
    panel.onResults = { published = $0 }
    panel.canvas.setTestStrokes(chineseInk.prefix(4).map { $0.map { CGPoint(x: $0.x * 0.6 + 5, y: $0.y * 0.6 + 5) } })
    for _ in 0..<100 {
      if !panel.results.isEmpty { break }
      try await Task.sleep(nanoseconds: 50_000_000)
    }
    XCTAssertTrue(panel.results.contains("中"))
    XCTAssertEqual(published, panel.results, "Recognised candidates never reached the shared strip.")
    XCTAssertEqual(inserted, "")
    let index = try XCTUnwrap(panel.results.firstIndex(of: "中"))
    XCTAssertTrue(panel.use(at: index))
    XCTAssertEqual(inserted, "中")
    XCTAssertFalse(panel.hasInk)
    XCTAssertTrue(panel.results.isEmpty)
    XCTAssertEqual(published, [], "Confirming a candidate did not clear the shared strip.")
    XCTAssertFalse(panel.use(at: index), "A stale candidate must not insert again")
    XCTAssertEqual(inserted, "中", "A stale candidate must not insert again")
  }

  func testClearInvalidatesPendingRecognitionAndUndoRemovesOneStroke() async throws {
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 414, height: 160))
    panel.layoutIfNeeded()
    panel.canvas.setTestStrokes(chineseInk)
    panel.canvas.undo()
    XCTAssertEqual(panel.canvas.strokes.count, chineseInk.count - 1)
    panel.clear()
    try await Task.sleep(nanoseconds: 900_000_000)
    XCTAssertFalse(panel.hasInk)
    XCTAssertTrue(panel.results.isEmpty)
    XCTAssertTrue(panel.canvas.strokes.isEmpty)
  }
  /// 面板只有书写卡片本身；它的 ⌫ 和「重写」是旁边的键盘按键，所以面板上每一点都会落墨。
  func testEveryPointOnThePanelAcceptsInk() throws {
    let panel = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 320, height: 200))
    panel.layoutIfNeeded()
    for point in [CGPoint(x: 20, y: 10), CGPoint(x: 120, y: 100), CGPoint(x: 40, y: 190),
                  CGPoint(x: 160, y: 4), CGPoint(x: 310, y: 190)] {
      XCTAssertTrue(panel.hitTest(point, with: nil) === panel.canvas)
    }
  }
  #endif

  func testHandwritingSchemeKeepsToolbarAndSwitchesBackToLetters() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let controller = KeyboardViewController(); controller.loadViewIfNeeded()
    let writingHeight = KeyboardViewController.keyboardHeight(handwriting: true)
    // 竖屏手写比其他布局高一排键（52pt）和一道排间距。
    XCTAssertEqual(writingHeight - KeyboardViewController.defaultKeyboardHeight,
                   52 + CGFloat(KeyboardLayoutPreference.rowSpacing), accuracy: 0.5)
    for width in [320.0, 414.0] {
      let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
      XCTAssertEqual(height.constant, writingHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: height.constant); controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
      XCTAssertFalse(panel.isHidden)
      // 书写区有四排 52pt 键连同三道排间距那么高（底行的键只有 44pt，省下的 8pt 也归书写区）。
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, 4 * 52 + 3 * CGFloat(KeyboardLayoutPreference.rowSpacing))
      // 书写卡片铺满键盘宽度，只留键盘本身两侧的内边距；虚线框从卡片边缘内缩 8pt。
      let padding = KeyboardFormFactor.phone.padding
      XCTAssertEqual(panel.canvas.bounds.width, width - padding.leading - padding.trailing, accuracy: 0.5)
      let guide = try XCTUnwrap(HandwritingCanvasCard.guideRect(for: panel.canvas.bounds))
      XCTAssertEqual(panel.canvas.bounds.width - guide.width, 2 * HandwritingCanvasCard.guideInset, accuracy: 0.5)
      XCTAssertEqual(HandwritingCanvasCard.guideInset, 8)
      print("Handwriting pad at \(Int(width))pt: card \(panel.canvas.bounds.size), guide \(guide.size)")
      let shot = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { controller.view.layer.render(in: $0.cgContext) }); shot.name = "Handwriting keyboard \(Int(width))"; shot.lifetime = .keepAlways; add(shot)
    }
    let language = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, KeyboardViewController.defaultKeyboardHeight)
    XCTAssertTrue(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
    // 离开手写后底行的「重写」和 ⌫ 也一起收起。
    XCTAssertTrue(try control("handwritingRewrite", in: controller).isHidden)
    XCTAssertTrue(try control("handwritingDelete", in: controller).isHidden)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, writingHeight)
    XCTAssertFalse(try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" }).isHidden)
  }
  /// 功能菜单的「手写」再点一次回到进入手写前的布局和中英状态（#9703），不是启用列表里的第一个方案，也不是停在手写上。
  func testHandwritingTileReturnsToTheLayoutItWasOpenedFrom() throws {
    let previous = InputSchemePreference.scheme
    let memory = [ImeModeMemoryPreference.enabledKey, ImeModeMemoryPreference.lastChineseKey]
      .map { ($0, ImeModeMemoryPreference.defaults.object(forKey: $0)) }
    defer {
      InputSchemePreference.scheme = previous
      for (key, value) in memory { ImeModeMemoryPreference.defaults.set(value, forKey: key) }
    }
    // 新键盘从中文开始，不继承模拟器上一次留下的英文。
    ImeModeMemoryPreference.defaults.set(true, forKey: ImeModeMemoryPreference.enabledKey)
    ImeModeMemoryPreference.defaults.set(true, forKey: ImeModeMemoryPreference.lastChineseKey)
    XCTAssertEqual(InputSchemePreference.enabledSchemes.first, .quanpin, "全拼 26 键排在启用列表第一个，才看得出回到的是不是它")

    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController(); controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    try tapHandwritingTile(in: controller)
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    try tapHandwritingTile(in: controller)
    XCTAssertEqual(InputSchemePreference.scheme, .nineKey, "关掉手写回到了别的布局")
    XCTAssertEqual(try languageKey(in: controller).accessibilityValue, KeyboardViewController.languageKeyValue(.nineKey))

    // 从英文键盘（全拼 9 键上的英文）进入手写，关掉后回到英文，底下的方案仍是全拼 9 键。用同一个键盘：新建的键盘会先读共享文档里刚记下的方案，赋给 `InputSchemePreference.scheme` 的值盖不过它。
    try languageKey(in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try languageKey(in: controller).accessibilityValue, "英文输入")
    try tapHandwritingTile(in: controller)
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    XCTAssertNotEqual(try languageKey(in: controller).accessibilityValue, "英文输入")
    try tapHandwritingTile(in: controller)
    XCTAssertEqual(InputSchemePreference.scheme, .nineKey)
    XCTAssertEqual(try languageKey(in: controller).accessibilityValue, "英文输入", "关掉手写没有回到英文")
  }

  /// 离开手写回到哪个方案：记着的还启用着就是它，否则是启用列表里第一个不是手写的方案，与 Android、鸿蒙的 `handwritingReturn` 一致。
  func testHandwritingReturnPrefersTheSchemeItWasOpenedFrom() {
    let offered: [ChineseInputScheme] = [.quanpin, .nineKey, .shuangpin, .handwriting]
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: .nineKey, offered: offered), .nineKey)
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: .shuangpin, offered: offered), .shuangpin)
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: nil, offered: offered), .quanpin,
                   "经方案选择进入手写时，取第一个不是手写的方案")
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: .japanese, offered: offered), .quanpin,
                   "进入手写后被关掉的方案不再回去")
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: .handwriting, offered: offered), .quanpin)
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: nil, offered: [.handwriting, .wubi]), .wubi)
    XCTAssertEqual(InputSchemePreference.handwritingReturn(before: nil, offered: [.handwriting]),
                   ChineseInputScheme.editionFallback)
  }

  private func tapHandwritingTile(in controller: KeyboardViewController) throws {
    let more = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "moreShortcut" } as? UIButton)
    more.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let tile = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "moreCard-手写" } as? UIControl,
                             "No 手写 tile in the function menu.")
    tile.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
  }

  private func languageKey(in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
  }

  func testHandwritingHeightTracksOrientationAndSymbolMode() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let parent = UIViewController()
    let controller = KeyboardViewController()
    parent.addChild(controller)
    controller.loadViewIfNeeded()
    let height = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let toggle = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0 is HandwritingInputView } as? HandwritingInputView)
    let enter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "returnKey" })
    // Annotated rather than inferred: three tuples of four components, where one is an enum written with a leading dot and two are overloaded `+` on CGFloat, give the solver enough freedom that it gives up with "unable to type-check this expression in reasonable time". Naming the type leaves it nothing to solve.
    let layouts: [(UIUserInterfaceSizeClass, CGFloat, CGFloat, CGFloat)] = [
      (.regular, 414.0, KeyboardViewController.keyboardHeight(handwriting: true), KeyboardViewController.defaultKeyboardHeight),
      (.compact, 812.0, KeyboardViewController.keyboardHeight(landscape: true, handwriting: true), KeyboardViewController.keyboardHeight(landscape: true)),
      (.regular, 320.0, KeyboardViewController.keyboardHeight(handwriting: true), KeyboardViewController.defaultKeyboardHeight),
    ]
    // 横屏不加高：手写横屏仍是原来那四排的高度（书写区三排）。
    XCTAssertEqual(KeyboardFormFactor.phone.keyRows(numberRow: false, handwriting: true, landscape: true), 4)
    XCTAssertEqual(KeyboardFormFactor.phone.keyRows(numberRow: false, handwriting: true, landscape: false), 5)
    for (verticalSize, width, writingHeight, typingHeight) in layouts {
      parent.setOverrideTraitCollection(UITraitCollection(verticalSizeClass: verticalSize), forChild: controller)
      controller.viewDidLayoutSubviews()
      XCTAssertEqual(height.constant, writingHeight)
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: writingHeight)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(enter.bounds.height, 44, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(panel.canvas.bounds.height, verticalSize == .compact ? 90 : 229)
      toggle.sendActions(for: .primaryActionTriggered)
      XCTAssertTrue(panel.isHidden)
      XCTAssertEqual(height.constant, typingHeight)
      controller.view.frame.size.height = typingHeight
      controller.view.layoutIfNeeded()
      toggle.sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(panel.isHidden)
      XCTAssertEqual(height.constant, writingHeight)
    }
  }
  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func handwritingController(width: CGFloat) throws -> KeyboardViewController {
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .handwriting
    XCTAssertEqual(InputSchemePreference.scheme, .handwriting)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.keyboardHeight(handwriting: true))
    controller.view.layoutIfNeeded()
    return controller
  }

  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton, identifier)
  }

  private func frame(_ view: UIView, in controller: KeyboardViewController) -> CGRect {
    view.convert(view.bounds, to: controller.view)
  }

  /// 手写键盘：书写卡片铺满键盘宽度；底行是三个移动端共用的 123 | ， | 手写 | 。 | 中 | 重写 | ⌫ | return（Android `KeyboardActionRow.designEntries`、HarmonyOS `handwritingActionRow`），宽度按 `handwriting*Weight` 分配。原来书写区左侧的 ？ ！ 由 ， 和 。 的长按输入。
  func testThePadSpansTheRowAndTheBottomRowCarriesTheTools() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    for width in [320.0, 390.0, 414.0] {
      let controller = try handwritingController(width: width)
      let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
      XCTAssertFalse(panel.isHidden)
      let card = frame(panel, in: controller)
      let padding = KeyboardFormFactor.phone.padding
      XCTAssertEqual(card.minX, padding.leading, accuracy: 0.5)
      XCTAssertEqual(card.maxX, width - padding.trailing, accuracy: 0.5)
      XCTAssertFalse(nodes(controller.view).contains { ($0.accessibilityIdentifier ?? "").hasPrefix("handwritingPunctuation") },
                     "the side punctuation column is gone")
      let toggle = try control("layoutToggleButton", in: controller)
      let comma = try control("quickPunctuationKey", in: controller)
      let space = try control("spaceKey", in: controller)
      let period = try control("bottomPeriodKey", in: controller)
      let language = try control("bottomLanguageKey", in: controller)
      let rewrite = try control("handwritingRewrite", in: controller)
      let delete = try control("handwritingDelete", in: controller)
      let enter = try control("returnKey", in: controller)
      let row = [toggle, comma, space, period, language, rewrite, delete, enter]
      for key in row {
        XCTAssertFalse(key.isHidden, key.accessibilityIdentifier ?? "")
        XCTAssertGreaterThan(frame(key, in: controller).minY, card.maxY, key.accessibilityIdentifier ?? "")
        XCTAssertEqual(frame(key, in: controller).minY, frame(enter, in: controller).minY, accuracy: 0.5)
      }
      for (left, right) in zip(row, row.dropFirst()) {
        XCTAssertLessThan(frame(left, in: controller).maxX, frame(right, in: controller).minX,
                          "\(left.accessibilityIdentifier ?? "") before \(right.accessibilityIdentifier ?? "")")
      }
      XCTAssertEqual(rewrite.configuration?.title, "重写")
      let unit = space.bounds.width / KeyboardViewController.handwritingSpaceWeight
      XCTAssertEqual(comma.bounds.width, unit * KeyboardViewController.handwritingPunctuationWeight, accuracy: 0.5)
      XCTAssertEqual(period.bounds.width, unit * KeyboardViewController.handwritingPunctuationWeight, accuracy: 0.5)
      XCTAssertEqual(rewrite.bounds.width, unit * KeyboardViewController.handwritingRewriteWeight, accuracy: 0.5)
      XCTAssertEqual(delete.bounds.width, unit * KeyboardViewController.handwritingDeleteWeight, accuracy: 0.5)
      XCTAssertEqual(enter.bounds.width, unit * KeyboardViewController.handwritingReturnWeight, accuracy: 0.5)
      // 最窄的 320pt 上 ， 和 。 仍有约一个 26 键字母键宽，空格放得下麦克风和完整的「手写」。
      XCTAssertGreaterThanOrEqual(comma.bounds.width, 24, "\(Int(width))pt")
      let spaceTitle = try XCTUnwrap(space.titleLabel)
      XCTAssertEqual(spaceTitle.text, "手写")
      XCTAssertGreaterThanOrEqual(spaceTitle.bounds.width + 0.5, spaceTitle.intrinsicContentSize.width,
                                  "the space label is clipped at \(Int(width))pt")
      // ？ 和 ！ 是 ， 和 。 的长按角标。
      let commaKey = try XCTUnwrap(comma as? KeyboardKeyButton)
      let periodKey = try XCTUnwrap(period as? KeyboardKeyButton)
      XCTAssertEqual(commaKey.cornerHint, "？")
      XCTAssertEqual(periodKey.cornerHint, "！")
      XCTAssertNotNil(commaKey.onCornerHint)
      XCTAssertNotNil(periodKey.onCornerHint)
      XCTAssertNil(comma.menu, "a long press on ， types ？ instead of opening the quick punctuation menu")
      for hint in ["handwritingCommaHint", "handwritingPeriodHint"] {
        let label = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == hint } as? UILabel, hint)
        XCTAssertFalse(label.isHidden, hint)
      }
      print("Handwriting bottom row at \(Int(width))pt: \(row.map { "\($0.accessibilityIdentifier ?? "")=\($0.bounds.width)" }.joined(separator: " "))")
    }
    XCTAssertTrue(KeyboardViewController.usesPhoneBottomRow(formFactor: .phone, nineKeyFrame: false, handwriting: true, kana: false))
    XCTAssertTrue(KeyboardViewController.usesPhoneBottomRow(formFactor: .tablet, nineKeyFrame: false, handwriting: true, kana: false))
    let controller = try handwritingController(width: 390)
    let space = try control("spaceKey", in: controller)
    XCTAssertEqual(space.configuration?.title, "手写")
    XCTAssertNotNil(space.configuration?.image, "the space bar keeps its mic")
  }

  /// 有笔迹时 ⌫ 撤回最后一笔，与 Android 的 deleteFromHandwriting 一致；「重写」清空卡片。
  func testDeleteTakesBackAStrokeAndRewriteClears() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled; InputSchemePreference.scheme = previous }
    let controller = try handwritingController(width: 390)
    let panel = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "handwritingInput" } as? HandwritingInputView)
    panel.canvas.setTestStrokes([[CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 40)], [CGPoint(x: 20, y: 60), CGPoint(x: 80, y: 60)],
                                 [CGPoint(x: 50, y: 10), CGPoint(x: 50, y: 90)]])
    try control("handwritingDelete", in: controller).sendActions(for: .touchUpInside)
    XCTAssertEqual(panel.canvas.strokes.count, 2)
    try control("handwritingRewrite", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(panel.hasInk)
  }

  /// The simulator build says it cannot recognise, rather than just doing nothing.
  ///
  /// ML Kit's arm64 slice is device-only, so this target draws ink and recognises nothing. That is
  /// the honest answer and it has to be visible: silence is indistinguishable from a broken
  /// keyboard, because the user writes a character, nothing appears, and the panel explains none
  /// of it. On a device build this assertion does not apply — recognition is real there — so it
  /// only runs where the fallback is compiled.
  func testTheSimulatorPanelSaysItCannotRecognise() throws {
    #if targetEnvironment(simulator)
    let view = HandwritingInputView(frame: CGRect(x: 0, y: 0, width: 390, height: 200))
    view.layoutIfNeeded()
    let status = try XCTUnwrap(
      nodes(view).compactMap { $0 as? UILabel }
        .first { $0.accessibilityIdentifier == "handwritingStatus" },
      "the fallback panel has nothing that explains why writing produces no candidates")
    XCTAssertFalse(status.isHidden)
    XCTAssertEqual(status.text, HandwritingInputView.unavailableMessage)
    // It draws ink all the same: the panel is the same shape as the device one.
    view.canvas.setTestStrokes([[CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 40)]])
    XCTAssertTrue(view.hasInk)
    XCTAssertFalse(view.commitFirst(), "the fallback must never claim a recognition")
    #else
    throw XCTSkip("Device builds recognise for real; there is no fallback message to check.")
    #endif
  }

}
