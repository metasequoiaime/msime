import UIKit
import XCTest

/// 滑行输入（#5347）：什么时候算滑行、抬手时交给 Engine 的请求，以及键盘上的开关、键区和会话怎么接起来。
@MainActor
final class GlideTypingTests: XCTestCase {
  private var storedGlide: Any?

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    storedGlide = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.glideTypingKey)
  }

  override func tearDown() {
    if let storedGlide { KeyboardLayoutPreference.defaults.set(storedGlide, forKey: KeyboardLayoutPreference.glideTypingKey) }
    else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.glideTypingKey) }
    super.tearDown()
  }

  // MARK: - 合成的键盘

  /// 手机 26 键的排法：30×40 的键、6pt 键距，第二行缩进半个键距，第三行缩进一个半；按 a..z 排列。
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

  private static func center(_ letter: Character, in frames: [CGRect] = frames) -> CGPoint {
    let frame = frames[GlideTyping.letters.firstIndex(of: letter)!]
    return CGPoint(x: frame.midX, y: frame.midY)
  }

  /// 依次经过 `word` 各字母键中心的一笔：大约每 6pt 一个采样、每个采样 8ms。
  private static func stroke(_ word: String, in frames: [CGRect] = frames) -> [GlideTyping.Sample] {
    let centers = word.map { center($0, in: frames) }
    var samples: [GlideTyping.Sample] = []
    func add(_ point: CGPoint) {
      samples.append(GlideTyping.Sample(x: point.x, y: point.y, milliseconds: Double(samples.count) * 8))
    }
    for (from, to) in zip(centers, centers.dropFirst()) {
      let steps = max(1, Int((hypot(to.x - from.x, to.y - from.y) / 6).rounded(.up)))
      for step in 0..<steps {
        let t = CGFloat(step) / CGFloat(steps)
        add(CGPoint(x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t))
      }
    }
    add(centers[centers.count - 1])
    return samples
  }

  private func decode(_ data: Data) throws -> [String: Any] {
    try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
  }

  // MARK: - 判定

  func testAGlideStartsOverAnotherLetterFarEnoughSideways() {
    let frames = Self.frames
    let g = GlideTyping.letters.firstIndex(of: "g")!
    let down = Self.center("g")
    // 还在按下的键上，走多远都不算。
    XCTAssertFalse(GlideTyping.startsGlide(downKey: g, down: down, current: CGPoint(x: down.x + 14, y: down.y), frames: frames))
    // 进了隔壁的 h，横向已超过 0.4 个键宽。
    XCTAssertTrue(GlideTyping.startsGlide(downKey: g, down: down, current: Self.center("h"), frames: frames))
    // 键距里的点不在任何键上。
    XCTAssertFalse(GlideTyping.startsGlide(downKey: g, down: down, current: CGPoint(x: frames[g].maxX + 3, y: down.y), frames: frames))
    // 竖直往上滑到 t 上，横向只挪了不到 0.4 个键宽：留给同一个键上的竖直手势，不算滑行。
    let t = Self.frames[GlideTyping.letters.firstIndex(of: "t")!]
    XCTAssertTrue(t.contains(CGPoint(x: down.x - 6, y: t.midY)))
    XCTAssertFalse(GlideTyping.startsGlide(downKey: g, down: down, current: CGPoint(x: down.x - 6, y: t.midY), frames: frames))
    XCTAssertTrue(GlideTyping.startsGlide(downKey: g, down: down, current: CGPoint(x: down.x - 12, y: t.midY), frames: frames))
    XCTAssertFalse(GlideTyping.startsGlide(downKey: 26, down: down, current: Self.center("h"), frames: frames), "按下的键不存在")
  }

  func testKeyIndexSkipsHiddenKeysAndGaps() {
    var frames = Self.frames
    XCTAssertEqual(GlideTyping.keyIndex(at: Self.center("q"), in: frames), GlideTyping.letters.firstIndex(of: "q"))
    XCTAssertNil(GlideTyping.keyIndex(at: CGPoint(x: 33, y: 20), in: frames), "q 与 w 之间的键距")
    frames[GlideTyping.letters.firstIndex(of: "q")!] = .null
    XCTAssertNil(GlideTyping.keyIndex(at: Self.center("q", in: Self.frames), in: frames))
  }

  // MARK: - 请求

  func testDownsamplingKeepsBothEndsWithinTheLimit() {
    let short = Array(0..<10)
    XCTAssertEqual(GlideTyping.downsampled(short, limit: 10), short)
    let long = Array(0..<5_000)
    let kept = GlideTyping.downsampled(long)
    XCTAssertEqual(kept.count, GlideTyping.pointLimit)
    XCTAssertEqual(kept.first, 0)
    XCTAssertEqual(kept.last, 4_999)
    XCTAssertEqual(kept, kept.sorted())
    XCTAssertEqual(Set(kept).count, kept.count, "均匀抽取不会重复取同一个采样")
    XCTAssertEqual(GlideTyping.downsampled(Array(0..<1_025)).count, 1_024)
  }

  func testLiveStrokeBufferIsBoundedBeforeTheRequestIsBuilt() {
    var samples: [GlideTyping.Sample] = []
    for index in 0..<(GlideTyping.sampleBufferLimit * 3) {
      GlideTyping.appendBounded(
        GlideTyping.Sample(x: CGFloat(index), y: 0, milliseconds: Double(index)),
        to: &samples)
    }
    XCTAssertLessThanOrEqual(samples.count, GlideTyping.sampleBufferLimit)
    XCTAssertEqual(samples.first?.x, 0)
    XCTAssertEqual(samples.last?.x, CGFloat(GlideTyping.sampleBufferLimit * 3 - 1))
  }

  func testRequestCarriesTheKeyCentresTheKeySizeAndTheStroke() throws {
    var frames = Self.frames
    // 第二行的键稍宽一些，一个字母键的大小取最小的那个。
    for letter in "asdfghjkl" {
      let index = GlideTyping.letters.firstIndex(of: letter)!
      frames[index].size.width = 33
    }
    let samples = [
      GlideTyping.Sample(x: 10.04, y: 20.06, milliseconds: 0),
      GlideTyping.Sample(x: 50.5, y: 21, milliseconds: 16.4),
      GlideTyping.Sample(x: 88.26, y: 19.74, milliseconds: 40.6),
    ]
    let data = try XCTUnwrap(GlideTyping.request(frames: frames, samples: samples))
    let request = try decode(data)
    XCTAssertEqual(Set(request.keys), ["keys", "key_width", "key_height", "points"], "Engine 拒绝未知字段")
    let keys = try XCTUnwrap(request["keys"] as? [[Double]])
    XCTAssertEqual(keys.count, 26)
    XCTAssertEqual(keys[0], [Double(frames[0].midX), Double(frames[0].midY)], "a 在最前")
    XCTAssertEqual(keys[25], [Double(Self.center("z").x), Double(Self.center("z").y)], "z 在最后")
    XCTAssertEqual(request["key_width"] as? Double, 30)
    XCTAssertEqual(request["key_height"] as? Double, 40)
    let points = try XCTUnwrap(request["points"] as? [[Double]])
    XCTAssertEqual(points, [[10.0, 20.1, 0], [50.5, 21.0, 16], [88.3, 19.7, 41]])
  }

  func testRequestNeedsTheWholeKeyboardAndAStroke() {
    let samples = Self.stroke("nihao")
    XCTAssertNotNil(GlideTyping.request(frames: Self.frames, samples: samples))
    XCTAssertNil(GlideTyping.request(frames: Array(Self.frames.dropLast()), samples: samples))
    var hidden = Self.frames
    hidden[3] = .null
    XCTAssertNil(GlideTyping.request(frames: hidden, samples: samples))
    XCTAssertNil(GlideTyping.request(frames: Self.frames, samples: Array(samples.prefix(1))))
    var broken = samples
    broken[1].x = .nan
    XCTAssertNil(GlideTyping.request(frames: Self.frames, samples: broken))
  }

  func testALongStrokeStaysWithinTheEngineLimits() throws {
    let samples = (0..<6_000).map { index in
      GlideTyping.Sample(x: 9_999.99 - CGFloat(index), y: -9_999.99, milliseconds: Double(index) * 4.17)
    }
    let data = try XCTUnwrap(GlideTyping.request(frames: Self.frames, samples: samples))
    XCTAssertLessThanOrEqual(data.count, GlideTyping.requestByteLimit)
    let points = try XCTUnwrap(try decode(data)["points"] as? [[Double]])
    XCTAssertEqual(points.count, GlideTyping.pointLimit)
    XCTAssertEqual(points.first, [10_000.0, -10_000.0, 0])
    XCTAssertEqual(points.last?[2], (5_999 * 4.17).rounded())
  }

  func testTrailPathRunsFromTheFirstPointToTheLast() throws {
    XCTAssertNil(GlideTrailView.path(through: []))
    let points = [CGPoint(x: 10, y: 10), CGPoint(x: 60, y: 30), CGPoint(x: 110, y: 10), CGPoint(x: 160, y: 40)]
    let path = try XCTUnwrap(GlideTrailView.path(through: points))
    XCTAssertEqual(path.currentPoint, points[3])
    let box = path.boundingBoxOfPath
    XCTAssertEqual(box.minX, 10, accuracy: 0.01)
    XCTAssertEqual(box.maxX, 160, accuracy: 0.01)
    XCTAssertNotNil(GlideTrailView.path(through: Array(points.prefix(2))))
  }

  // MARK: - 会话

  func testTheSessionTypesAQuanpinGlideAndRefusesOtherSchemes() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-glide-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switch(toShuangpin: false)
    let request = try XCTUnwrap(GlideTyping.request(frames: Self.frames, samples: Self.stroke("nihao")))
    let typed = bridge.glide(request)
    XCTAssertTrue(typed.isHandled, String(describing: typed.diagnosticText))
    XCTAssertEqual(typed.editingText, "nihao")
    XCTAssertFalse(typed.candidates.isEmpty)
    _ = bridge.cancel()

    XCTAssertFalse(bridge.glide(Data("{}".utf8)).isHandled, "Engine 拒绝不完整的请求")
    XCTAssertFalse(bridge.glide(Data()).isHandled)

    _ = bridge.switchToWubi()
    let refused = bridge.glide(request)
    XCTAssertFalse(refused.isHandled, "只有全拼解码滑行")
    XCTAssertTrue(refused.editingText.isEmpty)
  }

  // MARK: - 键盘

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func keyboard(_ scheme: ChineseInputScheme, glide: Bool) -> KeyboardViewController {
    InputSchemePreference.scheme = scheme
    KeyboardLayoutPreference.glideTyping = glide
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func keyArea(_ controller: KeyboardViewController) throws -> KeyAreaStackView {
    try XCTUnwrap(nodes(controller.view).first { $0 is KeyAreaStackView } as? KeyAreaStackView)
  }

  private func gesture(_ controller: KeyboardViewController) throws -> GlideTypingGestureRecognizer {
    try XCTUnwrap(try keyArea(controller).gestureRecognizers?.first { $0 is GlideTypingGestureRecognizer }
      as? GlideTypingGestureRecognizer)
  }

  func testTheSettingIsOffByDefault() {
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.glideTypingKey)
    XCTAssertEqual(KeyboardLayoutPreference.glideTypingKey, "keyboard.gesture.glide")
    XCTAssertFalse(KeyboardLayoutPreference.glideTyping)
    KeyboardLayoutPreference.glideTyping = true
    XCTAssertEqual(KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.glideTypingKey) as? Bool, true)
  }

  func testGlideIsArmedOnlyOnTheQuanpinLettersWithTheSettingOn() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    XCTAssertTrue(try gesture(keyboard(.quanpin, glide: true)).isArmed())
    XCTAssertFalse(try gesture(keyboard(.quanpin, glide: false)).isArmed(), "默认关")
    XCTAssertFalse(try gesture(keyboard(.nineKey, glide: true)).isArmed())
    XCTAssertFalse(try gesture(keyboard(.shuangpin, glide: true)).isArmed())
    XCTAssertFalse(try gesture(keyboard(.korean, glide: true)).isArmed(), "韩文键帽")

    // 切到 123 符号层后不滑行。
    let symbols = keyboard(.quanpin, glide: true)
    try XCTUnwrap(nodes(symbols.view).first { $0.accessibilityIdentifier == "layoutToggleButton" } as? UIButton)
      .sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try gesture(symbols).isArmed())
    // 英文模式不滑行；切回中文又能滑行（同时把「沿用上次」可能记下的模式放回中文）。
    let english = keyboard(.quanpin, glide: true)
    let language = try XCTUnwrap(nodes(english.view).first { $0.accessibilityIdentifier == "bottomLanguageKey" } as? UIButton)
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try gesture(english).isArmed())
    language.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try gesture(english).isArmed())
  }

  func testTheKeyboardMapsItsLetterKeysAndTypesTheStrokeOnLift() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = keyboard(.quanpin, glide: true)
    let area = try keyArea(controller)
    let glide = try gesture(controller)
    let frames = glide.keyFrames()
    XCTAssertEqual(frames.count, 26)
    XCTAssertFalse(frames.contains { $0.isEmpty })
    // 每个字母键都认得出来，`;` 和其他键不算字母键。
    for (index, letter) in GlideTyping.letters.enumerated() {
      let key = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 \(letter.uppercased())" })
      XCTAssertEqual(glide.letterIndex(key), index, "\(letter)")
      XCTAssertEqual(KeyAreaStackView.layoutFrame(of: key, in: area), frames[index])
    }
    let space = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "spaceKey" })
    XCTAssertNil(glide.letterIndex(space))
    XCTAssertTrue(nodes(controller.view).contains { $0 is GlideTrailView })

    // 抬手：这一笔经会话变成全拼组字。
    glide.onEnded(Self.stroke("nihao", in: frames), frames)
    let preedit = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "preeditButton" } as? UIButton)
    XCTAssertEqual(preedit.configuration?.title?.filter(\.isLetter), "nihao")
  }

  func testNewFingersHitTheKeyAreaItselfWhileAGlideRuns() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    let controller = keyboard(.quanpin, glide: true)
    let area = try keyArea(controller)
    let glide = try gesture(controller)
    let q = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 Q" })
    let point = q.convert(CGPoint(x: q.bounds.midX, y: q.bounds.midY), to: controller.view)
    XCTAssertTrue(controller.view.hitTest(point, with: nil) === q)
    glide.onBegan()
    XCTAssertTrue(area.suppressesKeyHits)
    XCTAssertTrue(controller.view.hitTest(point, with: nil) === area, "滑行中落下的手指不按任何键")
    glide.onCancelled()
    XCTAssertFalse(area.suppressesKeyHits)
    XCTAssertTrue(controller.view.hitTest(point, with: nil) === q)
  }
}
