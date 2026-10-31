import UIKit
import XCTest

final class InputBridgeResponseTests: XCTestCase {
  func testQuickPhraseCursorSuffixDecodingAndCommitReplacement() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    for suffix in ["", "🌲e\u{301}\""] {
      let snapshot = try bridge.snapshot(from: ["commit": "title:\"" + suffix, "commit_cursor_suffix": suffix, "view": [:]])
      XCTAssertEqual(snapshot.commitCursorSuffix, suffix)
      XCTAssertEqual(snapshot.replacingCommit(snapshot.commitText).commitCursorSuffix, suffix)
      XCTAssertNil(snapshot.replacingCommit("changed").commitCursorSuffix)
      XCTAssertNil(snapshot.replacingCommit(nil).commitCursorSuffix)
    }
    for invalid: Any in [NSNull(), 7, false, ["suffix"]] {
      XCTAssertNil(try bridge.snapshot(from: ["commit_cursor_suffix": invalid, "view": [:]]).commitCursorSuffix)
    }
    XCTAssertNil(try bridge.snapshot(from: ["view": [:]]).commitCursorSuffix)
    let decomposed = MetasequoiaInputSnapshot(commitText: "e\u{301}", commitCursorSuffix: "e\u{301}")
    XCTAssertNil(decomposed.replacingCommit("é").commitCursorSuffix, "规范等价不表示原来的 UTF-16 位置仍适用")
    XCTAssertNil(MetasequoiaInputSnapshot(commitCursorSuffix: "").replacingCommit("").commitCursorSuffix)
  }

  func testMalformedEnvelopeReleasesNativeResponseOnce() throws {
    let pointer = strdup("[]")!
    var releases = 0
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.decodeResponse(pointer) { pointer in
      releases += 1
      free(pointer)
    })
    XCTAssertEqual(releases, 1)
  }

  /// 全拼九键的读音和筛选状态随每次回应一起到，键盘不再另外问一次。
  func testSnapshotDecodesNineKeyReadingAndFilter() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-nine-key-decode-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let snapshot = try bridge.snapshot(from: [
      "handled": true,
      "view": [
        "preedit": "6464'224", "nine_key_spellings": ["bai", "cai", "B", "C"],
        "nine_key_reading": "ning'bai", "nine_key_single_character": true, "nine_key_strokes": "hs",
      ] as [String: Any],
    ])
    XCTAssertEqual(snapshot.nineKeyReading, "ning'bai")
    XCTAssertTrue(snapshot.nineKeySingleCharacter)
    XCTAssertEqual(snapshot.nineKeyStrokes, "hs")
    XCTAssertEqual(snapshot.nineKeySpellings, ["bai", "cai", "B", "C"])

    let bare = try bridge.snapshot(from: ["handled": true, "view": ["preedit": "64"]])
    XCTAssertEqual(bare.nineKeyReading, "")
    XCTAssertFalse(bare.nineKeySingleCharacter)
    XCTAssertEqual(bare.nineKeyStrokes, "")
    XCTAssertThrowsError(try bridge.snapshot(from: ["view": ["nine_key_single_character": 1]]))
  }

  /// 来源 18 的提交已在运行时逐段转换，解码与标点替换不能恢复整行转换。
  func testMultilingualCommitKeepsItsConversionBoundary() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let snapshot = try bridge.snapshot(from: [
      "commit": "hello網絡学校", "commit_context": ["script_conversion": false],
      "view": ["candidates": [["text": "hello網絡学校", "source": 18]]],
    ])
    XCTAssertFalse(snapshot.scriptConversion)
    XCTAssertFalse(snapshot.replacingCommit("hello網絡学校。").scriptConversion)
    XCTAssertEqual(snapshot.candidateSources, [18])
    XCTAssertTrue(try bridge.snapshot(from: ["view": [:]]).scriptConversion)
    XCTAssertThrowsError(try bridge.snapshot(from: [
      "commit_context": ["script_conversion": 0], "view": [:],
    ]))
  }

  func testCandidateHighlightIsStrictAndAlignedWithRows() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let rows: [[String: Any]] = [["text": "合成一", "highlighted": false],
      ["text": "合成二", "highlighted": true], ["text": "合成三"]]
    let snapshot = try bridge.snapshot(from: ["view": ["candidates": rows]])
    XCTAssertEqual(snapshot.candidateHighlighted, [false, true, false])
    let visible = CandidateMarker.visibleIndices(sources: [0, 15, 0])
    XCTAssertEqual(visible.map { snapshot.candidateHighlighted[$0] }, [false, false])
    XCTAssertThrowsError(try bridge.snapshot(from: ["view": ["candidates": [["text": "合成", "highlighted": 1]]]]))
  }

  /// 双拼触点纠错坐了首选时，View 的那一行候选带 `confident_correction: true`（#10018），其余行和默认文档里这个字段不出现。解码与行对齐，缺省为假，字符串冒充时整份回应作废（与 `highlighted` 走同一个 strictBool），换上屏文字时留着。
  func testConfidentCorrectionIsOptionalStrictAndAlignedWithRows() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let snapshot = try bridge.snapshot(from: try Self.viewFixture(confident: "true"))
    XCTAssertEqual(snapshot.candidates, ["我们今天去公园", "我们今天去公", "我们"])
    XCTAssertEqual(snapshot.candidateConfidentCorrections, [true, false, false])
    XCTAssertEqual(snapshot.candidateAnnotations.first, "(womfjbtnqugsyr)", "纠错前的拼写照旧作为注释")
    XCTAssertEqual(snapshot.replacingCommit("，").candidateConfidentCorrections, [true, false, false])
    let visible = CandidateMarker.visibleIndices(sources: snapshot.candidateSources)
    XCTAssertEqual(visible.map { snapshot.candidateConfidentCorrections[$0] }, [true, false, false])

    let plain = try bridge.snapshot(from: try Self.viewFixture(confident: nil))
    XCTAssertEqual(plain.candidateConfidentCorrections, [false, false, false], "默认文档里没有这个字段")
    XCTAssertThrowsError(try bridge.snapshot(from: try Self.viewFixture(confident: "\"true\"")))
  }

  /// host-api 的 View JSON：首行是改过一键、坐了首选的纠错整句，第二行是没坐首选的纠错，`confident` 为 nil 时不写 `confident_correction`。
  static func viewFixture(confident: String?) throws -> [String: Any] {
    let flag = confident.map { #", "confident_correction": \#($0)"# } ?? ""
    let json = """
    {"handled": true, "view": {"preedit": "womfjbtnqugsyr", "generation": 3, "page_count": 1, "candidates": [
      {"id": {"session": 1, "generation": 3, "index": 0}, "text": "我们今天去公园", "code": "womfjbtmqugsyr", "annotation": "(womfjbtnqugsyr)", "source": 0, "corrected": true, "fixed_position": 0, "highlighted": true\(flag)},
      {"id": {"session": 1, "generation": 3, "index": 1}, "text": "我们今天去公", "code": "womfjbtmqugs", "annotation": "(womfjbtnqugs)", "source": 0, "corrected": true, "fixed_position": 0, "highlighted": false},
      {"id": {"session": 1, "generation": 3, "index": 2}, "text": "我们", "code": "womf", "annotation": "", "source": 0, "corrected": false, "fixed_position": 0, "highlighted": false}
    ]}}
    """
    return try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
  }

  func testAllSessionCallsUseOneOwningThread() {
    let owner = MSIMEInputThread.shared.sync { Thread.current }
    XCTAssertFalse(owner.isMainThread)
    let second = MSIMEInputThread.shared.sync { MSIMEInputThread.shared.sync { Thread.current } }
    XCTAssertTrue(owner === second, "重入不能派到另一线程或等待自己")
  }

  @MainActor
  func testAsyncNineKeyCommandsFlushInOrderBeforeBackspaceAndCommit() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    _ = bridge.setPrivateSession(true)
    let gate = DispatchSemaphore(value: 0)
    MSIMEInputThread.shared.async { gate.wait() }
    var delivered: [String] = []
    for digit in "64426" {
      XCTAssertTrue(bridge.enqueueCharacter(String(digit)) { snapshot in
        XCTAssertTrue(Thread.isMainThread)
        XCTAssertNil(snapshot.diagnosticText)
        XCTAssertNil(snapshot.commitText)
        delivered.append(snapshot.preedit)
      })
    }
    XCTAssertTrue(bridge.hasPendingCharacters)
    XCTAssertTrue(delivered.isEmpty, "投递不能同步等待被阻塞的引擎线程")
    gate.signal()
    let deleted = bridge.handleBackspace()
    XCTAssertEqual(delivered, ["6", "64", "644", "6442", "64426"])
    XCTAssertFalse(bridge.hasPendingCharacters)
    XCTAssertEqual(deleted.preedit, "6442")
    let committed = bridge.commitCandidate()
    XCTAssertNotNil(committed.commitText)
    XCTAssertNil(committed.diagnosticText)
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    XCTAssertNil(bridge.handleCharacter("6").diagnosticText)
    _ = bridge.cancel()
  }

  /// 26 键全拼的字母异步投递（#10031）：投递不等被挡住的会话线程，结果按顺序回到主线程，紧随的删除和提交先排空此前的字母。
  @MainActor
  func testAsyncQuanpinLettersFlushInOrderBeforeBackspaceAndCommit() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed)
    _ = bridge.setLearningEnabled(false)
    _ = bridge.setPrivateSession(true)
    let gate = DispatchSemaphore(value: 0)
    MSIMEInputThread.shared.async { gate.wait() }
    var delivered: [String] = []
    for letter in "nihao" {
      XCTAssertTrue(bridge.enqueueCharacter(String(letter)) { snapshot in
        XCTAssertTrue(Thread.isMainThread)
        XCTAssertNil(snapshot.diagnosticText)
        XCTAssertNil(snapshot.commitText)
        delivered.append(snapshot.editingText)
      })
    }
    XCTAssertTrue(bridge.hasPendingCharacters)
    XCTAssertTrue(delivered.isEmpty, "投递不能同步等待被阻塞的引擎线程")
    gate.signal()
    let deleted = bridge.handleBackspace()
    XCTAssertEqual(delivered, ["n", "ni", "nih", "niha", "nihao"])
    XCTAssertFalse(bridge.hasPendingCharacters)
    XCTAssertEqual(deleted.editingText, "niha")
    XCTAssertTrue(bridge.enqueueCharacter("o") { delivered.append($0.editingText) })
    let committed = bridge.commitCandidate()
    XCTAssertEqual(delivered.last, "nihao", "提交之前先处理此前投递的字母")
    XCTAssertNotNil(committed.commitText)
    XCTAssertNil(committed.diagnosticText)
    XCTAssertTrue(committed.editingText.isEmpty)
  }

  /// 26 键双拼带按下点的字母异步投递时与同步触点入口组出同样的拼写和候选；九键网格不收字母、不带按下点，26 键不收大写、分词符和数字。
  @MainActor
  func testAsyncShuangpinTouchLettersMatchTheSynchronousTouchEntry() throws {
    func shuangpin() -> MetasequoiaInputSessionBridge {
      let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
      addTeardownBlock { try? FileManager.default.removeItem(at: state) }
      let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
      _ = bridge.switch(toShuangpinProfile: "xiaohe")
      _ = bridge.setLearningEnabled(false)
      return bridge
    }
    // 手机 26 键的排法：30×40 的键、36pt 一格，第二行缩进半格，第三行缩进一格半。
    var frames = [CGRect](repeating: .null, count: 26)
    for (row, (letters, indent)) in [("qwertyuiop", 0.0), ("asdfghjkl", 0.5), ("zxcvbnm", 1.5)].enumerated() {
      for (column, letter) in letters.unicodeScalars.enumerated() {
        frames[Int(letter.value - 97)] = CGRect(x: (CGFloat(column) + indent) * 36, y: CGFloat(row) * 48, width: 30, height: 40)
      }
    }
    let request = try XCTUnwrap(TouchPointPolicy.request(id: 1, layout: .init(letters: frames, semicolon: nil)))
    let synchronous = shuangpin()
    let asynchronous = shuangpin()
    XCTAssertTrue(synchronous.setKeyGeometry(request, id: 1))
    XCTAssertTrue(asynchronous.setKeyGeometry(request, id: 1))
    var expected = MetasequoiaInputSnapshot()
    var delivered: [MetasequoiaInputSnapshot] = []
    for letter in "womfjbtn".unicodeScalars {
      let frame = frames[Int(letter.value - 97)]
      let touch = MetasequoiaInputSessionBridge.TouchPoint(
        geometryID: 1, x: Float(frame.midX + frame.width * 0.3), y: Float(frame.midY))
      expected = synchronous.handleTouchCharacter(String(letter), geometryID: 1, x: touch.x, y: touch.y)
      XCTAssertTrue(asynchronous.enqueueCharacter(String(letter), touch: touch) { delivered.append($0) })
    }
    asynchronous.flushPendingCharacters()
    XCTAssertEqual(delivered.count, 8)
    XCTAssertEqual(delivered.last?.editingText, expected.editingText)
    XCTAssertEqual(delivered.last?.candidates, expected.candidates)
    XCTAssertFalse(expected.candidates.isEmpty)

    for rejected in ["W", "'", "6", "ni", "", "你"] {
      XCTAssertFalse(asynchronous.enqueueCharacter(rejected) { _ in XCTFail(rejected) }, rejected)
    }
    _ = asynchronous.cancel()
    XCTAssertNil(asynchronous.switchToNineKey().diagnosticText)
    XCTAssertFalse(asynchronous.enqueueCharacter("n") { _ in XCTFail("九键网格不收字母") })
    XCTAssertFalse(asynchronous.enqueueCharacter("6", touch: .init(geometryID: 1, x: 0, y: 0)) { _ in XCTFail("九键不带按下点") })
    XCTAssertFalse(asynchronous.hasPendingCharacters)
    _ = synchronous.cancel()
  }

  func testOnlyNineKeyDigitsAndTwentySixKeyLettersAreAsynchronous() {
    typealias Bridge = MetasequoiaInputSessionBridge
    for byte in UInt8(0)...UInt8(127) {
      let letter = (97...122).contains(byte)
      XCTAssertEqual(Bridge.acceptsAsynchronousCharacter(byte, grid: .none, touched: false), letter || byte == 59, "\(byte)")
      XCTAssertEqual(Bridge.acceptsAsynchronousCharacter(byte, grid: .none, touched: true), letter || byte == 59, "\(byte)")
      XCTAssertEqual(Bridge.acceptsAsynchronousCharacter(byte, grid: .nineKey, touched: false), (50...57).contains(byte), "\(byte)")
      XCTAssertFalse(Bridge.acceptsAsynchronousCharacter(byte, grid: .nineKey, touched: true), "\(byte)")
      for grid in [Bridge.KeyGrid.fourteenKey, .nineKeyShuangpin, .eighteenKey, .eighteenKeyShuangpin] {
        XCTAssertFalse(Bridge.acceptsAsynchronousCharacter(byte, grid: grid, touched: false), "\(grid) \(byte)")
      }
    }
  }

  @MainActor
  func testAsyncNineKeyCompletionReturnsToMainAndKeepsCandidateIdentity() async throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    let done = expectation(description: "异步数字交回主线程")
    var previous: MetasequoiaCandidateIdentity?
    XCTAssertTrue(bridge.enqueueCharacter("6") { snapshot in
      XCTAssertTrue(Thread.isMainThread)
      XCTAssertEqual(snapshot.preedit, "6")
      previous = snapshot.candidateIdentities.first ?? nil
      done.fulfill()
    })
    await fulfillment(of: [done], timeout: 10)
    let identity = try XCTUnwrap(previous)
    XCTAssertTrue(bridge.enqueueCharacter("4") { _ in })
    let stale = bridge.selectCandidate(generation: identity.generation, globalIndex: identity.index)
    XCTAssertNil(stale.commitText, "旧画面的候选不能按新页的位置提交")
    XCTAssertEqual(stale.preedit, "64")
    let rejectedActions: [() -> MetasequoiaInputSnapshot] = [
      { bridge.selectAnyCandidate(generation: identity.generation, globalIndex: identity.index) },
      { bridge.selectCandidateEdge(generation: identity.generation, globalIndex: identity.index, last: false) },
      { bridge.chooseNineKeySpelling(generation: identity.generation, at: 0) },
      { bridge.editCandidate(generation: identity.generation, globalIndex: identity.index, action: .promote) },
      { bridge.editCandidate(generation: identity.generation, globalIndex: identity.index, action: .remove) },
      { bridge.editCandidate(generation: identity.generation, globalIndex: identity.index, action: .fix(position: 1)) },
      { bridge.editCandidate(generation: identity.generation, globalIndex: identity.index, action: .clearPosition) },
    ]
    for action in rejectedActions {
      let rejected = action()
      XCTAssertNotNil(rejected.diagnosticText)
      XCTAssertNil(rejected.commitText)
      XCTAssertEqual(rejected.preedit, "64", "被拒的旧画面操作不能清掉仍在运行时里的组字")
    }
    _ = bridge.cancel()
  }

  /// 空闲渲染每次都把同样的学习设置再推一遍，会话已经应用的同一份偏好不再发。组字时被推迟的那次不算应用：组字结束后同样的请求要再发一次，如实报告已经生效；会话重建后也照常发送。
  @MainActor
  func testRepeatedPreferencesSkipOnlyWhatTheSessionAlreadyApplied() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.initializationFailed)
    XCTAssertTrue(bridge.setLearningEnabled(false))
    XCTAssertTrue(bridge.setLearningEnabled(false))
    XCTAssertTrue(bridge.setPrivateSession(true))
    XCTAssertTrue(bridge.setPrivateSession(true))

    XCTAssertFalse(bridge.handleCharacter("n").preedit.isEmpty)
    XCTAssertFalse(bridge.setLearningEnabled(true), "组字中的偏好改动推迟到空闲时应用")
    XCTAssertFalse(bridge.setLearningEnabled(true), "推迟的那次不能当作已经应用")
    XCTAssertEqual(bridge.cancel().preedit, "")
    XCTAssertTrue(bridge.setLearningEnabled(true))
    XCTAssertTrue(bridge.setLearningEnabled(true))

    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    XCTAssertTrue(bridge.setLearningEnabled(false))
    XCTAssertTrue(bridge.setPrivateSession(true))
    XCTAssertTrue(bridge.setPrivateSession(false))
    XCTAssertNil(bridge.handleCharacter("n").diagnosticText)
    _ = bridge.cancel()
  }

  @MainActor
  func testControlReenteredFromCompletionDrainsTheRemainingSnapshotsFirst() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    var order: [String] = []
    XCTAssertTrue(bridge.enqueueCharacter("6") { snapshot in
      order.append(snapshot.preedit)
      XCTAssertEqual(bridge.cancel().preedit, "")
      order.append("cancelled")
    })
    XCTAssertTrue(bridge.enqueueCharacter("4") { order.append($0.preedit) })
    bridge.flushPendingCharacters()
    XCTAssertEqual(order, ["6", "64", "cancelled"])
    XCTAssertEqual(bridge.handleCharacter("8").preedit, "8")
    _ = bridge.cancel()
  }

}
