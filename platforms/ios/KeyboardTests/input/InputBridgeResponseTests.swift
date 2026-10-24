import XCTest

final class InputBridgeResponseTests: XCTestCase {
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
      XCTAssertTrue(bridge.enqueueNineKeyCharacter(String(digit)) { snapshot in
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

  @MainActor
  func testAsyncNineKeyCompletionReturnsToMainAndKeepsCandidateIdentity() async throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    let done = expectation(description: "异步数字交回主线程")
    var previous: MetasequoiaCandidateIdentity?
    XCTAssertTrue(bridge.enqueueNineKeyCharacter("6") { snapshot in
      XCTAssertTrue(Thread.isMainThread)
      XCTAssertEqual(snapshot.preedit, "6")
      previous = snapshot.candidateIdentities.first ?? nil
      done.fulfill()
    })
    await fulfillment(of: [done], timeout: 10)
    let identity = try XCTUnwrap(previous)
    XCTAssertTrue(bridge.enqueueNineKeyCharacter("4") { _ in })
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

  @MainActor
  func testControlReenteredFromCompletionDrainsTheRemainingSnapshotsFirst() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(bridge.switchToNineKey().diagnosticText)
    _ = bridge.setLearningEnabled(false)
    var order: [String] = []
    XCTAssertTrue(bridge.enqueueNineKeyCharacter("6") { snapshot in
      order.append(snapshot.preedit)
      XCTAssertEqual(bridge.cancel().preedit, "")
      order.append("cancelled")
    })
    XCTAssertTrue(bridge.enqueueNineKeyCharacter("4") { order.append($0.preedit) })
    bridge.flushPendingCharacters()
    XCTAssertEqual(order, ["6", "64", "cancelled"])
    XCTAssertEqual(bridge.handleCharacter("8").preedit, "8")
    _ = bridge.cancel()
  }

}
