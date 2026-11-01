import UIKit
import XCTest

/// 触屏 26 键双拼纠错（#10018）：哪一键带着按下点、注册给引擎的键位几何、按键记下的按下点，以及会话和键盘怎么接起来。
@MainActor
final class TouchPointTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  // MARK: - 合成的键盘

  /// 手机 26 键的排法：30×40 的键、36pt 一格，第二行缩进半格，第三行缩进一格半；按 a..z 排列。
  private nonisolated static let frames: [CGRect] = {
    let rows: [(String, CGFloat)] = [("qwertyuiop", 0), ("asdfghjkl", 0.5), ("zxcvbnm", 1.5)]
    var frames = [CGRect](repeating: .null, count: 26)
    for (row, (letters, indent)) in rows.enumerated() {
      for (column, letter) in letters.unicodeScalars.enumerated() {
        let index = Int(letter.value - UnicodeScalar("a").value)
        frames[index] = CGRect(x: (CGFloat(column) + indent) * 36, y: CGFloat(row) * 48, width: 30, height: 40)
      }
    }
    return frames
  }()

  /// 第二行 l 右边一格的 `;`。
  private nonisolated static let semicolon = CGRect(x: 9.5 * 36, y: 48, width: 30, height: 40)

  private static func frame(_ letter: Character) -> CGRect {
    frames[GlideTyping.letters.firstIndex(of: letter)!]
  }

  private func decode(_ data: Data) throws -> [String: Any] {
    try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
  }

  // MARK: - 判定

  func testOnlyShuangpinLettersAndTheSemicolonCarryATouch() {
    for key in ["a", "q", "z", ";"] {
      XCTAssertTrue(TouchPointPolicy.sendsTouch(character: key, shuangpinLetters: true, hasTouchDown: true, voiceOver: false), key)
    }
    // 辅助码的大写、分词的 `'`、数字和标点照常走 msime_client_character。
    for key in ["A", "'", "1", ",", "", "ab"] {
      XCTAssertFalse(TouchPointPolicy.sendsTouch(character: key, shuangpinLetters: true, hasTouchDown: true, voiceOver: false), key)
    }
    XCTAssertFalse(TouchPointPolicy.sendsTouch(character: "a", shuangpinLetters: false, hasTouchDown: true, voiceOver: false),
                   "全拼、14/18 键、本地模式和英文不带落点")
    XCTAssertFalse(TouchPointPolicy.sendsTouch(character: "a", shuangpinLetters: true, hasTouchDown: false, voiceOver: false),
                   "sendActions 这类激活没有按下点")
    XCTAssertFalse(TouchPointPolicy.sendsTouch(character: "a", shuangpinLetters: true, hasTouchDown: true, voiceOver: true),
                   "读屏的双击不是手指的落点")
  }

  // MARK: - 几何

  func testTheGeometryListsTheLettersAlphabeticallyAndTheSemicolon() throws {
    let plain = try decode(try XCTUnwrap(TouchPointPolicy.request(id: 7, layout: .init(letters: Self.frames, semicolon: nil))))
    XCTAssertEqual(Set(plain.keys), ["id", "keys", "semicolon", "duplicates"], "引擎拒绝未知字段")
    XCTAssertEqual(plain["id"] as? Int, 7)
    let keys = try XCTUnwrap(plain["keys"] as? [[Double]])
    XCTAssertEqual(keys.count, 26)
    XCTAssertEqual(keys[0], [15.0 + 18, 68, 30, 40], "a 在最前，[cx, cy, w, h]")
    XCTAssertEqual(keys[25], [Double(Self.frame("z").midX), Double(Self.frame("z").midY), 30, 40], "z 在最后")
    XCTAssertTrue(plain["semicolon"] is NSNull)
    XCTAssertEqual((plain["duplicates"] as? [Any])?.count, 0, "iOS 的分离键盘不重复字母键")

    let microsoft = try decode(try XCTUnwrap(
      TouchPointPolicy.request(id: 8, layout: .init(letters: Self.frames, semicolon: Self.semicolon))))
    XCTAssertEqual(microsoft["semicolon"] as? [Double], [357, 68, 30, 40])
    // 坐标保留一位小数。
    var fractional = Self.frames
    fractional[0] = CGRect(x: 0.04, y: 0.26, width: 30.33, height: 40.07)
    let rounded = try decode(try XCTUnwrap(TouchPointPolicy.request(id: 1, layout: .init(letters: fractional, semicolon: nil))))
    XCTAssertEqual((rounded["keys"] as? [[Double]])?.first, [15.2, 20.3, 30.3, 40.1])
  }

  func testTheGeometryNeedsAllTwentySixLetterKeysAndANonZeroID() throws {
    let layout = TouchPointPolicy.Layout(letters: Self.frames, semicolon: nil)
    XCTAssertNil(TouchPointPolicy.request(id: 0, layout: layout), "0 是清除")
    XCTAssertNil(TouchPointPolicy.request(id: 1, layout: .init(letters: Array(Self.frames.dropLast()), semicolon: nil)))
    for broken in [CGRect.null, .zero, CGRect(x: 0, y: 0, width: 30, height: 0), CGRect(x: CGFloat.nan, y: 0, width: 30, height: 40),
                   CGRect(x: 0, y: CGFloat.infinity, width: 30, height: 40)] {
      var letters = Self.frames
      letters[3] = broken
      XCTAssertNil(TouchPointPolicy.request(id: 1, layout: .init(letters: letters, semicolon: nil)), "\(broken)")
    }
    // `;` 没量出来时写 null，字母照常带落点。
    let empty = try decode(try XCTUnwrap(TouchPointPolicy.request(id: 1, layout: .init(letters: Self.frames, semicolon: .zero))))
    XCTAssertTrue(empty["semicolon"] is NSNull)
    XCTAssertLessThanOrEqual(
      try XCTUnwrap(TouchPointPolicy.request(id: .max, layout: .init(letters: Self.frames, semicolon: Self.semicolon))).count,
      TouchPointPolicy.requestByteLimit)
  }

  func testTheGeometryIsRegisteredAgainOnlyWhenItChangesOrTheSessionLostIt() {
    let layout = TouchPointPolicy.Layout(letters: Self.frames, semicolon: nil)
    var registration = TouchPointPolicy.Registration()
    XCTAssertTrue(registration.needsRegistering(layout, sessionID: nil))
    XCTAssertEqual(registration.advance(to: layout), 1)
    XCTAssertFalse(registration.needsRegistering(layout, sessionID: 1), "布局没变、会话里还是这一份：不再发")
    XCTAssertTrue(registration.needsRegistering(layout, sessionID: nil), "会话重建后没有几何")
    XCTAssertTrue(registration.needsRegistering(layout, sessionID: 9), "会话里是别的代次")
    // 单手、分离、旋转都会挪动键：换一个代次。
    var narrower = Self.frames
    narrower[0].size.width -= 4
    let moved = TouchPointPolicy.Layout(letters: narrower, semicolon: nil)
    XCTAssertTrue(registration.needsRegistering(moved, sessionID: 1))
    XCTAssertTrue(registration.needsRegistering(.init(letters: Self.frames, semicolon: Self.semicolon), sessionID: 1),
                  "微软双拼的 `;` 显示出来")
    XCTAssertEqual(registration.advance(to: moved), 2)
    XCTAssertFalse(registration.needsRegistering(moved, sessionID: 2))
    registration.reset()
    XCTAssertTrue(registration.needsRegistering(moved, sessionID: 2), "注册失败后下一键再试")
    // 代次用到头以后从 1 重新数，跳过表示清除的 0。
    var wrapping = TouchPointPolicy.Registration(id: .max - 1)
    XCTAssertEqual(wrapping.advance(to: layout), .max)
    XCTAssertEqual(wrapping.advance(to: layout), 1)
  }

  // MARK: - 按键

  private func keyInArea(received: @escaping (CGPoint?) -> Void) -> (KeyAreaStackView, KeyboardKeyButton) {
    let area = KeyAreaStackView(frame: CGRect(x: 0, y: 0, width: 200, height: 60))
    let row = UIStackView()
    area.addArrangedSubview(row)
    let key = KeyboardKeyButton(configuration: .plain(), primaryAction: UIAction { action in
      received((action.sender as? KeyboardKeyButton)?.takeTouchDownLocation())
    })
    row.addArrangedSubview(key)
    area.layoutIfNeeded()
    return (area, key)
  }

  func testAKeyRemembersTheTouchDownOnlyUntilTheTouchIsOver() {
    var received: [CGPoint?] = []
    // 键区要留着：按钮只被它的父视图持有，丢掉键区就不在键区里了。
    let (area, key) = keyInArea { received.append($0) }
    let touch = UITouch()

    // 不记按下点的按钮（功能键、候选）什么都不记。
    XCTAssertTrue(key.beginTracking(touch, with: nil))
    XCTAssertNil(key.touchDownLocation)

    key.recordsTouchDown = true
    XCTAssertTrue(key.beginTracking(touch, with: nil))
    XCTAssertNotNil(key.touchDownLocation, "字母键记下按下点")
    key.touchesCancelled([touch], with: nil)
    XCTAssertNil(key.touchDownLocation, "触摸被取消（滑行、横滑光标、系统打断）")

    XCTAssertTrue(key.beginTracking(touch, with: nil))
    key.touchesEnded([touch], with: nil)
    XCTAssertNil(key.touchDownLocation, "触摸结束后不留给下一次没有触摸的激活")

    key.cornerHint = "1"
    XCTAssertTrue(key.beginTracking(touch, with: nil))
    XCTAssertNotNil(key.touchDownLocation)
    key.cornerHint = nil
    key.removeFromSuperview()
    XCTAssertTrue(key.beginTracking(touch, with: nil))
    XCTAssertNil(key.touchDownLocation, "不在键区里的按钮没有键区坐标")
    withExtendedLifetime(area) {}

    // 按键的动作经 UIAction.sender 取走按下点，取过就没了。
    let (_, other) = keyInArea { received.append($0) }
    received.removeAll()
    other.touchDownLocation = CGPoint(x: 12, y: 34)
    other.sendActions(for: .primaryActionTriggered)
    other.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(received, [CGPoint(x: 12, y: 34), nil])
    XCTAssertNil(other.touchDownLocation)
  }

  /// 手指位置固定的触摸，用来驱动按键的跟踪：`location(in:)` 不论问哪个视图都回答同一个点。
  private final class PinnedTouch: UITouch {
    var point: CGPoint
    init(_ point: CGPoint) {
      self.point = point
      super.init()
    }
    override func location(in view: UIView?) -> CGPoint { point }
  }

  /// 上滑改输角标的那一刻按下点就清掉，松手时动作也拿不到它；按键离开窗口（键盘收起、键区重建）时同样清掉。
  func testTheCornerHintAndLeavingTheWindowDropTheTouchDown() {
    let (area, key) = keyInArea { _ in }
    key.recordsTouchDown = true
    key.cornerHint = "1"
    key.swipesCornerHint = true
    key.cornerHintSwipeDirection = .up
    let touch = PinnedTouch(CGPoint(x: 20, y: 100))
    XCTAssertTrue(key.beginTracking(touch, with: nil))
    XCTAssertEqual(key.touchDownLocation, CGPoint(x: 20, y: 100))
    touch.point.y = 100 - SwipeHintPolicy.threshold / 2
    _ = key.continueTracking(touch, with: nil)
    XCTAssertNotNil(key.touchDownLocation, "没滑到阈值，还是这个字母的按下点")
    touch.point.y = 100 - SwipeHintPolicy.threshold - 1
    _ = key.continueTracking(touch, with: nil)
    XCTAssertNil(key.touchDownLocation, "改输角标，按下点不再有用")

    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 200))
    window.addSubview(area)
    XCTAssertTrue(key.beginTracking(PinnedTouch(CGPoint(x: 30, y: 20)), with: nil))
    XCTAssertNotNil(key.touchDownLocation)
    area.removeFromSuperview()
    XCTAssertNil(key.window)
    XCTAssertNil(key.touchDownLocation, "离开窗口的按键不留按下点")
    withExtendedLifetime(area) {}
  }

  // MARK: - 会话

  private func shuangpinBridge() -> MetasequoiaInputSessionBridge {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-\(UUID().uuidString)", isDirectory: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switch(toShuangpinProfile: "xiaohe")
    return bridge
  }

  private func candidateTexts(_ bridge: MetasequoiaInputSessionBridge) throws -> [String] {
    let rows = try XCTUnwrap(try bridge.allCandidates()["candidates"] as? [[String: Any]])
    return rows.compactMap { $0["text"] as? String }
  }

  /// 按下点：被按键的键心往 `toward` 那个键挪 `fraction` 个键宽（键高）。
  private static func point(on pressed: Character, toward intended: Character, fraction: CGFloat = 0.4) -> CGPoint {
    let from = frame(pressed), to = frame(intended)
    let dx = to.midX - from.midX, dy = to.midY - from.midY
    let length = max(hypot(dx, dy), 1)
    return CGPoint(x: from.midX + dx / length * from.width * fraction, y: from.midY + dy / length * from.height * fraction)
  }

  func testTheSessionKeepsTheRegisteredGeometryAndRefusesABrokenOne() throws {
    let bridge = shuangpinBridge()
    XCTAssertNil(bridge.keyGeometryID)
    let request = try XCTUnwrap(TouchPointPolicy.request(id: 3, layout: .init(letters: Self.frames, semicolon: nil)))
    XCTAssertTrue(bridge.setKeyGeometry(request, id: 3))
    XCTAssertEqual(bridge.keyGeometryID, 3)
    XCTAssertFalse(bridge.setKeyGeometry(Data("{\"id\":4,\"keys\":[]}".utf8), id: 4), "引擎拒绝不是 26 个字母键的几何")
    XCTAssertNil(bridge.keyGeometryID, "被拒之后不再按任何代次带落点")
    XCTAssertFalse(bridge.setKeyGeometry(Data(), id: 5))
    XCTAssertFalse(bridge.setKeyGeometry(request, id: 0))
    XCTAssertTrue(bridge.setKeyGeometry(request, id: 3))
    XCTAssertEqual(bridge.keyGeometryID, 3)
  }

  /// 代次不符、坐标不是有限数时，带落点的按键与不带落点的逐字相同。
  func testATouchWithoutAUsableGeometryTypesLikeAPlainCharacter() throws {
    let plain = shuangpinBridge()
    let touched = shuangpinBridge()
    let request = try XCTUnwrap(TouchPointPolicy.request(id: 2, layout: .init(letters: Self.frames, semicolon: nil)))
    XCTAssertTrue(touched.setKeyGeometry(request, id: 2))
    for (letter, id, x) in [("w", UInt32(9), Float(50)), ("o", 2, .nan), ("m", 2, .infinity), ("f", 1, 100)] {
      let expected = plain.handleCharacter(letter)
      let actual = touched.handleTouchCharacter(letter, geometryID: id, x: x, y: 20)
      XCTAssertEqual(actual.editingText, expected.editingText, letter)
      XCTAssertEqual(actual.candidates, expected.candidates, letter)
    }
    XCTAssertEqual(try candidateTexts(touched), try candidateTexts(plain))
  }

  /// 带着 `touches` 给出的按下点打 `code`（没给的位置按在键心），返回整张候选表；`touches` 为 nil 时不带落点。
  private func type(_ code: String, touches: [Int: CGPoint]?) throws -> [String] {
    let bridge = shuangpinBridge()
    guard let touches else {
      for letter in code { _ = bridge.handleCharacter(String(letter)) }
      return try candidateTexts(bridge)
    }
    let request = try XCTUnwrap(TouchPointPolicy.request(id: 1, layout: .init(letters: Self.frames, semicolon: nil)))
    XCTAssertTrue(bridge.setKeyGeometry(request, id: 1))
    for (index, letter) in code.enumerated() {
      let point = touches[index] ?? CGPoint(x: Self.frame(letter).midX, y: Self.frame(letter).midY)
      let snapshot = bridge.handleTouchCharacter(String(letter), geometryID: 1, x: Float(point.x), y: Float(point.y))
      XCTAssertTrue(snapshot.isHandled, String(describing: snapshot.diagnosticText))
    }
    return try candidateTexts(bridge)
  }

  /// 「我们今天去公园」小鹤是 wo mf jb tm qu gs yr。把一个键按成它的邻键、落点在靠原键的那一侧时，纠错整句进第一页；不带落点时同一串输入它沉在表尾或根本不出现，落点按在被按键正中时也不进第一页。
  func testATouchNearTheIntendedKeyBringsTheCorrectedSentenceForward() throws {
    let gold = "我们今天去公园"
    // m 按成左边的 n（tn 是「条」），y 按成左边的 t（tr 是「团」）。
    for (code, index, intended) in [("womfjbtnqugsyr", 7, Character("m")), ("womfjbtmqugstr", 12, Character("y"))] {
      let pressed = Array(code)[index]
      let edge = try type(code, touches: [index: Self.point(on: pressed, toward: intended)])
      let touchedRank = try XCTUnwrap(edge.firstIndex(of: gold), "\(code): \(edge.prefix(12))")
      XCTAssertLessThan(touchedRank, 9, "\(code): 带着落点的纠错整句在第一页")
      let plain = try type(code, touches: nil)
      if let plainRank = plain.firstIndex(of: gold) { XCTAssertGreaterThanOrEqual(plainRank, 9, "\(code): 不带落点时沉底") }
      let center = try type(code, touches: [:])
      if let centerRank = center.firstIndex(of: gold) {
        XCTAssertGreaterThanOrEqual(centerRank, 9, "\(code): 按在键心的那一键改过去太贵，纠错整句不留在座位上")
      }
    }
  }

  // MARK: - 键盘

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func keyboard(_ scheme: ChineseInputScheme, width: CGFloat? = nil) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width ?? UIScreen.main.bounds.width,
                                   height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func keyArea(_ controller: KeyboardViewController) throws -> KeyAreaStackView {
    try XCTUnwrap(nodes(controller.view).first { $0 is KeyAreaStackView } as? KeyAreaStackView)
  }

  private func letterKey(_ letter: Character, in controller: KeyboardViewController) throws -> KeyboardKeyButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(letter.uppercased())" } as? KeyboardKeyButton,
                  "\(letter)")
  }

  /// 设了 `MSIME_KEY_GEOMETRY_EXPORT`（xcodebuild 前加 `TEST_RUNNER_`）时，把键盘此刻会注册的几何写进那个目录，评测拿它替换参考布局。
  private func export(_ data: Data, as name: String) throws {
    guard let directory = ProcessInfo.processInfo.environment["MSIME_KEY_GEOMETRY_EXPORT"], !directory.isEmpty else { return }
    let url = URL(fileURLWithPath: directory, isDirectory: true)
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    try data.write(to: url.appendingPathComponent("\(name).json"))
  }

  func testTheKeyboardExportsTheGeometryOfItsLetterKeys() throws {
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    defer {
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
    }
    KeyboardLayoutPreference.oneHanded = .off
    let xiaohe = keyboard(.shuangpin)
    let area = try keyArea(xiaohe)
    let data = try XCTUnwrap(xiaohe.keyGeometryExport)
    let geometry = try decode(data)
    let keys = try XCTUnwrap(geometry["keys"] as? [[Double]])
    XCTAssertEqual(keys.count, 26)
    for (index, letter) in GlideTyping.letters.enumerated() {
      let frame = KeyAreaStackView.layoutFrame(of: try letterKey(letter, in: xiaohe), in: area)
      XCTAssertEqual(keys[index][0], Double(frame.midX), accuracy: 0.051, "\(letter)")
      XCTAssertEqual(keys[index][1], Double(frame.midY), accuracy: 0.051, "\(letter)")
      XCTAssertEqual(keys[index][2], Double(frame.width), accuracy: 0.051, "\(letter)")
    }
    XCTAssertTrue(geometry["semicolon"] is NSNull, "小鹤不显示 `;`")
    try export(data, as: "ios-iphone-portrait")

    let microsoft = keyboard(.microsoft)
    let microsoftData = try XCTUnwrap(microsoft.keyGeometryExport)
    let semicolonKey = try XCTUnwrap(nodes(microsoft.view).first { $0.accessibilityIdentifier == "microsoftFinalKey" })
    let semicolonFrame = KeyAreaStackView.layoutFrame(of: semicolonKey, in: try keyArea(microsoft))
    let semicolon = try XCTUnwrap(try decode(microsoftData)["semicolon"] as? [Double])
    XCTAssertEqual(semicolon[0], Double(semicolonFrame.midX), accuracy: 0.051)
    XCTAssertNotEqual(try decode(microsoftData)["keys"] as? [[Double]], keys, "第二行多一个 `;`，字母键变窄")
    try export(microsoftData, as: "ios-iphone-portrait-microsoft")

    // 单手模式把键收窄靠到一侧，几何随之改变。
    KeyboardLayoutPreference.oneHanded = .left
    let oneHanded = keyboard(.shuangpin)
    let narrow = try XCTUnwrap(try decode(try XCTUnwrap(oneHanded.keyGeometryExport))["keys"] as? [[Double]])
    XCTAssertLessThan(narrow[0][2], keys[0][2])
    KeyboardLayoutPreference.oneHanded = .off

    // 14 键不画 26 键的字母行，没有几何可导出。
    let fourteen = keyboard(.fourteenKey)
    XCTAssertNil(fourteen.keyGeometryExport)
  }

  /// iPad 横屏分离式键盘：T 和 Y 分在两半，中间隔着四分之一个键区。测试里的控制器没有窗口场景，横屏用 compact 的竖直 size class 模拟。
  func testTheSplitTabletKeyboardExportsBothHalves() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey)
    let previous = InputSchemePreference.scheme
    defer {
      defaults.set(stored, forKey: KeyboardLayoutPreference.tabletSplitKey)
      InputSchemePreference.scheme = previous
    }
    KeyboardLayoutPreference.tabletSplit = true
    InputSchemePreference.scheme = .shuangpin
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.traitOverrides.verticalSizeClass = .compact
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 1194, height: KeyboardViewController.keyboardHeight(.tablet, landscape: true, numberRow: true))
    controller.view.layoutIfNeeded()
    let data = try XCTUnwrap(controller.keyGeometryExport)
    let keys = try XCTUnwrap(try decode(data)["keys"] as? [[Double]])
    let t = keys[Int(UnicodeScalar("t").value - UnicodeScalar("a").value)]
    let y = keys[Int(UnicodeScalar("y").value - UnicodeScalar("a").value)]
    let r = keys[Int(UnicodeScalar("r").value - UnicodeScalar("a").value)]
    XCTAssertGreaterThan(y[0] - t[0], 3 * (t[0] - r[0]), "T 和 Y 之间隔着中缝")
    try export(data, as: "ios-ipad-split")
  }

  /// iPad 全宽键盘的单手模式把 26 键收进手机大小的一块、贴到一侧（#10203）：注册的几何跟着按键的实际矩形走，比两手的 iPad 键盘窄，全部落在靠左的那一块里。键盘这边不用另做什么，量的本来就是布局矩形。
  func testTheOneHandedTabletKeyboardRegistersTheDockedBlock() throws {
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    defer {
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
    }
    func tablet() -> KeyboardViewController {
      InputSchemePreference.scheme = .shuangpin
      let controller = KeyboardViewController()
      controller.traitOverrides.userInterfaceIdiom = .pad
      controller.traitOverrides.horizontalSizeClass = .regular
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 1032, height: KeyboardViewController.keyboardHeight(.tablet))
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      return controller
    }
    KeyboardLayoutPreference.oneHanded = .off
    let full = try XCTUnwrap(try decode(try XCTUnwrap(tablet().keyGeometryExport))["keys"] as? [[Double]])
    KeyboardLayoutPreference.oneHanded = .left
    let controller = tablet()
    let docked = try XCTUnwrap(try decode(try XCTUnwrap(controller.keyGeometryExport))["keys"] as? [[Double]])
    XCTAssertEqual(docked.count, 26)
    XCTAssertLessThan(docked[0][2], full[0][2], "the keys are narrower")
    // 键区坐标从键区左边起算（iPad 左右内边距 8 之内），按键那一段宽约 360。
    let keysWidth = KeyboardOneHandLayout.keysWidth(available: 1032 - 16, mode: .left, formFactor: .tablet)
    for key in docked {
      XCTAssertLessThanOrEqual(key[0] + key[2] / 2, Double(keysWidth) + 0.5, "every letter sits in the docked block")
    }
    try press("w", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 1)
    XCTAssertEqual(controller.touchCharacterCount, 1)
  }

  /// 26 键双拼的字母键取走按下点后照常组字；`sendActions` 没有按下点，走原来的 msime_client_character，两条路组出同样的字母。
  func testLetterKeysTypeWithTheirTouchDownAndDropIt() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let touched = keyboard(.shuangpin)
    let plain = keyboard(.shuangpin)
    let area = try keyArea(touched)
    for letter in "womf" {
      let key = try letterKey(letter, in: touched)
      let frame = KeyAreaStackView.layoutFrame(of: key, in: area)
      key.touchDownLocation = CGPoint(x: frame.midX + frame.width * 0.3, y: frame.midY)
      key.sendActions(for: .primaryActionTriggered)
      XCTAssertNil(key.touchDownLocation, "\(letter)")
      try letterKey(letter, in: plain).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertEqual(preedit(touched), "womf")
    XCTAssertEqual(preedit(plain), "womf")
    XCTAssertEqual(touched.touchCharacterCount, 4, "四键都带着按下点进了引擎")
    XCTAssertEqual(plain.touchCharacterCount, 0, "sendActions 没有按下点，走 msime_client_character")
  }

  /// 像松手输入那样按 `letter`：`touched` 时先给它一个偏离键心 0.3 个键宽的按下点，`sendActions` 本身没有。按键的动作总会取走按下点。
  private func press(_ letter: Character, in controller: KeyboardViewController, touched: Bool = true) throws {
    try press(try letterKey(letter, in: controller), in: controller, touched: touched)
  }

  private func press(_ key: KeyboardKeyButton, in controller: KeyboardViewController, touched: Bool = true) throws {
    if touched {
      let frame = KeyAreaStackView.layoutFrame(of: key, in: try keyArea(controller))
      key.touchDownLocation = CGPoint(x: frame.midX + frame.width * 0.3, y: frame.midY)
    }
    key.sendActions(for: .primaryActionTriggered)
    XCTAssertNil(key.touchDownLocation, key.accessibilityLabel ?? "")
  }

  /// 组字区里还没上屏的拉丁字母；组字全部提交后为空。
  private func preedit(_ controller: KeyboardViewController) -> String {
    (nodes(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)?
      .accessibilityValue?.filter { $0.isASCII && $0.isLetter } ?? ""
  }

  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIControl {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIControl, identifier)
  }

  /// 26 键双拼里带按下点的字母先注册一次几何再走触点入口，同一布局不再注册；没有按下点、开着 VoiceOver、Shift 让字母成为辅助码时走原来的入口，之后的字母照常带按下点。
  func testShuangpinLettersRegisterTheGeometryOnceAndTypeWithTheTouch() throws {
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    defer {
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
    }
    KeyboardLayoutPreference.oneHanded = .off
    let controller = keyboard(.shuangpin)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 0, "键盘出现时不注册，等第一个带按下点的字母")
    XCTAssertEqual(controller.touchCharacterCount, 0)

    try press("w", in: controller)
    XCTAssertEqual(controller.touchCharacterCount, 1)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 1, "第一键之前注册几何")
    let registered = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertEqual(try XCTUnwrap(controller.keyGeometryExport), TouchPointPolicy.request(id: 1, layout: registered),
                   "注册的是键区此刻的布局矩形")
    try press("o", in: controller)
    XCTAssertEqual(controller.touchCharacterCount, 2)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 1, "布局没变，不再注册")

    try press("m", in: controller, touched: false)
    XCTAssertEqual(controller.touchCharacterCount, 2, "没有按下点的激活走 msime_client_character")
    controller.isVoiceOverRunning = { true }
    try press("f", in: controller)
    XCTAssertEqual(controller.touchCharacterCount, 2, "读屏的激活不带落点")
    controller.isVoiceOverRunning = { false }

    // 组字中按 Shift，下一个字母是辅助码的大写，不带落点；辅助码之后 Shift 复原，字母又带按下点。Shift 之后键面读作「大写 J」，所以先拿住这个键。
    let j = try letterKey("j", in: controller)
    try control("shiftButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(j.accessibilityLabel, "大写 J", "Shift 让下一个字母成为辅助码")
    try press(j, in: controller)
    XCTAssertEqual(controller.touchCharacterCount, 2, "辅助码的大写不带落点")
    try press("b", in: controller)
    XCTAssertEqual(controller.touchCharacterCount, 3)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 1)
  }

  /// 单手换边、退出单手、键盘变宽（旋转）、微软双拼显示 `;` 都挪动字母键：下一个带按下点的字母之前换一个代次重新注册，注册的是挪动后的布局。
  func testALayoutChangeRegistersANewGeometryBeforeTheNextTouch() throws {
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    defer {
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
    }
    KeyboardLayoutPreference.oneHanded = .right
    let controller = keyboard(.shuangpin, width: 390)
    try press("w", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 1)
    let right = try XCTUnwrap(controller.keyGeometryRegistration.layout)

    try control("oneHandSwap", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    try press("o", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 2, "单手换到另一侧")
    let left = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertLessThan(left.letters[0].minX, right.letters[0].minX)

    try control("oneHandExit", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    try press("m", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 3, "退出单手")
    let full = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertGreaterThan(full.letters[0].width, left.letters[0].width)

    controller.view.frame.size.width = 844
    controller.view.layoutIfNeeded()
    try press("f", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 4, "键盘变宽")
    XCTAssertGreaterThan(try XCTUnwrap(controller.keyGeometryRegistration.layout).letters[0].width, full.letters[0].width)
    XCTAssertEqual(controller.touchCharacterCount, 4)

    // 组字提交以后才换方案：不带按下点的一键让键盘读到微软双拼，排版之后下一个带按下点的字母注册带 `;` 的几何。
    for _ in 0..<8 where !preedit(controller).isEmpty {
      try control("spaceKey", in: controller).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertEqual(preedit(controller), "", "组字已提交")
    InputSchemePreference.scheme = .microsoft
    try press("w", in: controller, touched: false)
    controller.view.layoutIfNeeded()
    let semicolonKey = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "microsoftFinalKey" })
    XCTAssertFalse(semicolonKey.isHidden)
    try press("o", in: controller)
    XCTAssertEqual(controller.keyGeometryRegistration.id, 5, "微软双拼显示 `;`")
    let microsoft = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertEqual(microsoft.semicolon, KeyAreaStackView.layoutFrame(of: semicolonKey, in: try keyArea(controller)))
    XCTAssertEqual(controller.touchCharacterCount, 5)
  }

  /// 全拼、双拼 14 键、本地模式下字母键的按下点不进引擎，也不注册几何。
  func testOtherKeyboardsNeverTakeTheTouchEntry() throws {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let previous = InputSchemePreference.scheme
    let previousLayout = TouchShuangpinLayoutPreference.layout
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = document }
      TouchShuangpinLayoutPreference.layout = previousLayout
      InputSchemePreference.scheme = previous
    }
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.twentySixKey))

    let quanpin = keyboard(.quanpin)
    for letter in "nihao" { try press(letter, in: quanpin) }
    XCTAssertEqual(quanpin.touchCharacterCount, 0, "全拼")
    XCTAssertEqual(quanpin.keyGeometryRegistration.id, 0)

    let local = keyboard(.shuangpin)
    local.openLocalInputMode("U")
    local.view.layoutIfNeeded()
    for letter in "abc" { try press(letter, in: local) }
    XCTAssertEqual(local.touchCharacterCount, 0, "本地模式")
    XCTAssertEqual(local.keyGeometryRegistration.id, 0)

    // 14 键画的是字母网格，切换之前按下的 26 键字母不开始组字，更不带落点。
    XCTAssertTrue(TouchShuangpinLayoutPreference.save(.fourteenKey))
    let fourteen = keyboard(.shuangpin)
    XCTAssertNil(fourteen.keyGeometryExport, "14 键没有 26 键字母的几何")
    for letter in "wo" { try press(letter, in: fourteen) }
    XCTAssertEqual(fourteen.touchCharacterCount, 0, "双拼 14 键")
    XCTAssertEqual(fourteen.keyGeometryRegistration.id, 0)
  }

  /// 调试导出：诊断日志开着时导出目录是共享状态目录下的 `debug`，关着时没有；每注册一次就把注册给引擎的那份 JSON 原样写成 `key-geometry.json`，布局没变不重写。
  func testTheRegisteredGeometryIsExportedWhileTheDiagnosticLogIsOn() throws {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences())
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-geometry-\(UUID().uuidString)")
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0 = document }
      DiagnosticLog.shared.configure(directory: nil, enabled: false)
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
      try? FileManager.default.removeItem(at: directory)
    }
    KeyboardLayoutPreference.oneHanded = .off
    func setDiagnosticLog(_ on: Bool) {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
        var diagnostic = $0["diagnostic_log"] as? [String: Any] ?? [:]
        diagnostic["server"] = on
        $0["diagnostic_log"] = diagnostic
      })
    }
    setDiagnosticLog(false)
    XCTAssertNil(keyboard(.shuangpin).keyGeometryExportDirectory, "诊断日志关着时不导出")
    setDiagnosticLog(true)
    XCTAssertEqual(keyboard(.shuangpin).keyGeometryExportDirectory?.path,
                   URL(fileURLWithPath: MetasequoiaInputSessionBridge.sharedStateDirectory).appendingPathComponent("debug").path)

    let controller = keyboard(.shuangpin)
    controller.keyGeometryExportDirectory = directory
    let file = directory.appendingPathComponent(KeyboardViewController.keyGeometryExportFile)
    try press("w", in: controller)
    let layout = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertEqual(try Data(contentsOf: file), TouchPointPolicy.request(id: controller.keyGeometryRegistration.id, layout: layout),
                   "写出的就是注册给引擎的那份")
    try FileManager.default.removeItem(at: file)
    try press("o", in: controller)
    XCTAssertFalse(FileManager.default.fileExists(atPath: file.path), "没有重新注册就不重写")
    controller.view.frame.size.width += 40
    controller.view.layoutIfNeeded()
    try press("m", in: controller)
    let moved = try XCTUnwrap(controller.keyGeometryRegistration.layout)
    XCTAssertEqual(try Data(contentsOf: file), TouchPointPolicy.request(id: controller.keyGeometryRegistration.id, layout: moved))
  }

  /// 带按下点和不带按下点打同一串双拼，量逐键耗时（按键动作加排版），以及每键核对几何时量一遍 26 个键的布局矩形要多久。只报数，与 KeystrokeLatencyTests 一样。
  func testWhatTheTouchPathCosts() throws {
    let previous = InputSchemePreference.scheme
    let previousOneHanded = KeyboardLayoutPreference.oneHanded
    defer {
      InputSchemePreference.scheme = previous
      KeyboardLayoutPreference.oneHanded = previousOneHanded
    }
    KeyboardLayoutPreference.oneHanded = .off
    var samples: [Bool: [Double]] = [true: [], false: []]
    for touched in [false, true, false, true] {
      let controller = keyboard(.shuangpin)
      let area = try keyArea(controller)
      let keys = try "womfjbtm".map { letter in
        let key = try letterKey(letter, in: controller)
        let frame = KeyAreaStackView.layoutFrame(of: key, in: area)
        return (key, CGPoint(x: frame.midX + frame.width * 0.3, y: frame.midY))
      }
      let space = try control("spaceKey", in: controller)
      for _ in 0..<20 {
        for (key, point) in keys {
          if touched { key.touchDownLocation = point }
          let start = CACurrentMediaTime()
          key.sendActions(for: .primaryActionTriggered)
          controller.view.layoutIfNeeded()
          samples[touched, default: []].append((CACurrentMediaTime() - start) * 1000)
        }
        space.sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
      }
      XCTAssertEqual(controller.touchCharacterCount, touched ? 160 : 0)
      XCTAssertEqual(controller.keyGeometryRegistration.id, touched ? 1 : 0, "整段只注册一次")
    }
    report("shuangpin-plain-letter", samples[false] ?? [])
    report("shuangpin-touch-letter", samples[true] ?? [])

    let controller = keyboard(.shuangpin)
    var layout: [Double] = []
    for _ in 0..<500 {
      let start = CACurrentMediaTime()
      _ = controller.keyGeometryExport
      layout.append((CACurrentMediaTime() - start) * 1000)
    }
    report("key-geometry-measure-and-encode", layout)
  }

  /// 坐了首选的双拼纠错（View 的 `confident_correction`）在候选条上：字和纠错前的拼写用首选的字色（`candidateAccent`），读屏标签以「，已纠正」结尾；没坐首选的纠错照旧画次要色。下一次回应没有这个标记时，复用的词块放回原样。
  func testTheStripMarksAConfidentCorrection() throws {
    let previous = InputSchemePreference.scheme
    let follows = CandidatePalette.defaults.object(forKey: CandidatePalette.followsDesktopKey)
    defer {
      InputSchemePreference.scheme = previous
      CandidatePalette.defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
    }
    CandidatePalette.defaults.set(false, forKey: CandidatePalette.followsDesktopKey)
    setCandidateGloss(false)
    let controller = keyboard(.shuangpin)
    defer { controller.viewDidDisappear(false) }
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    func chip(_ number: Int) throws -> KeyboardKeyButton {
      try XCTUnwrap(nodes(controller.view).first {
        $0.accessibilityIdentifier == "candidate-\(number)" && !$0.isHidden
      } as? KeyboardKeyButton, "候选条上没有第 \(number) 个词块")
    }
    func hintColor(_ button: UIButton, _ hint: String) throws -> UIColor? {
      let title = NSAttributedString(try XCTUnwrap(button.configuration?.attributedTitle))
      let range = (title.string as NSString).range(of: hint)
      XCTAssertNotEqual(range.location, NSNotFound, title.string)
      return title.attribute(.foregroundColor, at: range.location, effectiveRange: nil) as? UIColor
    }
    let skin = KeyboardTheme.current

    controller.render(try bridge.snapshot(from: try InputBridgeResponseTests.viewFixture(confident: "true")))
    controller.view.layoutIfNeeded()
    let first = try chip(1)
    XCTAssertEqual(first.accessibilityLabel, "候选词 1：我们今天去公园，还需输入 womfjbtnqugsyr，已纠正")
    XCTAssertEqual(first.configuration?.baseForegroundColor, skin.candidateAccent)
    XCTAssertEqual(try hintColor(first, "womfjbtnqugsyr"), skin.candidateAccent, "纠错前的拼写用首选的字色")
    XCTAssertEqual(try chip(2).accessibilityLabel, "候选词 2：我们今天去公，还需输入 womfjbtnqugs")
    XCTAssertEqual(try chip(2).configuration?.baseForegroundColor, skin.keyForeground)
    XCTAssertEqual(try hintColor(chip(2), "womfjbtnqugs"), skin.secondary, "没坐首选的纠错注释照旧是次要色")
    XCTAssertEqual(try chip(3).accessibilityLabel, "候选词 3：我们")

    controller.render(try bridge.snapshot(from: try InputBridgeResponseTests.viewFixture(confident: nil)))
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try chip(1).accessibilityLabel, "候选词 1：我们今天去公园，还需输入 womfjbtnqugsyr")
    XCTAssertEqual(try chip(1).configuration?.baseForegroundColor, skin.candidateAccent, "键盘自己的配色里首选本来就是首选的字色")
    XCTAssertEqual(try hintColor(chip(1), "womfjbtnqugsyr"), skin.secondary, "默认文档的纠错首选不画强调色注释")
  }

  /// 跟随桌面调色板时（#10018）：首个词块画在选中底色上，坐了首选的纠错照用调色板给那里的颜色。字是选中文字色，不换成强调色；纠错前的拼写从次要色换成选中翻译色、没给时用选中文字色，标记照样看得出来。这里取色器选中色设成强调色本身，client-services 于是按亮度给出白色的选中文字色。
  func testTheStripMarksAConfidentCorrectionOnADesktopPalette() throws {
    preserveSharedTheme()
    setCandidateGloss(false)
    let previous = InputSchemePreference.scheme
    let follows = CandidatePalette.defaults.object(forKey: CandidatePalette.followsDesktopKey)
    addTeardownBlock {
      InputSchemePreference.scheme = previous
      CandidatePalette.defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
    }
    CandidatePalette.defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    XCTAssertTrue(GlobalThemePreference.update {
      $0["global_theme"] = GlobalThemeCatalog.customId
      $0["custom_theme"] = ["base": "paper", "candidate_colors": ["selected": "#2C7A4B"]]
    })
    let palette = try XCTUnwrap(CandidatePalette.active(in: MetasequoiaInputSessionBridge.loadSharedPreferences(), systemDark: false))
    let selectedText = try XCTUnwrap(palette.selectedText)
    XCTAssertNotEqual(selectedText, palette.accent, "夹具要让选中文字色和强调色不同")
    let markedCaption = palette.selectedTranslation ?? selectedText
    let plainCaption = palette.selectedTranslation ?? palette.secondary
    XCTAssertNotEqual(markedCaption, plainCaption, "夹具要让标记改变注释颜色")

    InputSchemePreference.scheme = .shuangpin
    let controller = KeyboardViewController()
    controller.overrideUserInterfaceStyle = .light
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: UIScreen.main.bounds.width, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    defer { controller.viewDidDisappear(false) }
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    func chip(_ number: Int) throws -> KeyboardKeyButton {
      try XCTUnwrap(nodes(controller.view).first {
        $0.accessibilityIdentifier == "candidate-\(number)" && !$0.isHidden
      } as? KeyboardKeyButton, "候选条上没有第 \(number) 个词块")
    }
    func hintColor(_ button: UIButton, _ hint: String) throws -> UIColor? {
      let title = NSAttributedString(try XCTUnwrap(button.configuration?.attributedTitle))
      let range = (title.string as NSString).range(of: hint)
      XCTAssertNotEqual(range.location, NSNotFound, title.string)
      return title.attribute(.foregroundColor, at: range.location, effectiveRange: nil) as? UIColor
    }

    controller.render(try bridge.snapshot(from: try InputBridgeResponseTests.viewFixture(confident: "true")))
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try chip(1).accessibilityLabel, "候选词 1：我们今天去公园，还需输入 womfjbtnqugsyr，已纠正")
    XCTAssertEqual(try chip(1).configuration?.baseForegroundColor, selectedText, "选中底色上的字不换成强调色")
    XCTAssertEqual(try hintColor(chip(1), "womfjbtnqugsyr"), markedCaption)
    XCTAssertEqual(try chip(2).configuration?.baseForegroundColor, palette.text)
    XCTAssertEqual(try hintColor(chip(2), "womfjbtnqugs"), palette.secondary)

    controller.render(try bridge.snapshot(from: try InputBridgeResponseTests.viewFixture(confident: nil)))
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try chip(1).accessibilityLabel, "候选词 1：我们今天去公园，还需输入 womfjbtnqugsyr")
    XCTAssertEqual(try chip(1).configuration?.baseForegroundColor, selectedText)
    XCTAssertEqual(try hintColor(chip(1), "womfjbtnqugsyr"), plainCaption, "没有标记时注释回到调色板原来的颜色")
  }

  private func report(_ label: String, _ samples: [Double]) {
    let sorted = samples.sorted()
    let at = { (q: Double) in sorted[Int((Double(sorted.count - 1) * q).rounded())] }
    print(String(format: "LATENCY %@ n=%d mean=%.3fms p50=%.3fms p95=%.3fms max=%.3fms",
                 label, samples.count, samples.reduce(0, +) / Double(samples.count), at(0.5), at(0.95), at(1.0)))
  }
}
