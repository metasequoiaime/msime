import XCTest

/// A distinct letters-only code for fixture row `index` (0..<676): quick phrase codes take letters only, as the Windows source's `valid_code` does, so a numbered fixture spells its number in letters.
private func letterCode(_ prefix: String, _ index: Int) -> String {
  let letters = Array("abcdefghijklmnopqrstuvwxyz")
  return prefix + String(letters[index / 26]) + String(letters[index % 26])
}

final class PersonalDictionaryStoreTests: XCTestCase {
  func testTauriPagingRejectsBooleanIntegers() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let request = try JSONSerialization.data(withJSONObject: [
      "operation": "list", "offset": true, "limit": 1,
    ])
    XCTAssertThrowsError(try TauriPersonalDictionaryBridge.request(
      request, store: PersonalDictionaryStore(directory: root)))
  }

  func testReadRejectsASymlinkedPersonalDictionaryDirectory() throws {
    #if canImport(Darwin)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-personal-directory-link-\(UUID().uuidString)")
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent("msime-personal-directory-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(at: root.appendingPathComponent("PersonalDictionary", isDirectory: true),
                                               withDestinationURL: outside)

    XCTAssertThrowsError(try PersonalDictionaryStore(directory: root).read())
    XCTAssertFalse(FileManager.default.fileExists(atPath: outside.appendingPathComponent("sync.lock").path))
    #endif
  }

  func testEnqueueRejectsASymlinkedStateFileWithoutWritingExternalFile() throws {
    #if canImport(Darwin)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-personal-file-link-\(UUID().uuidString)")
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent("msime-personal-file-target-\(UUID().uuidString)")
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    let directory = root.appendingPathComponent("PersonalDictionary", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    let externalState = outside.appendingPathComponent("state.json")
    try Data("synthetic-state".utf8).write(to: externalState)
    try FileManager.default.createSymbolicLink(at: directory.appendingPathComponent("sync.json"),
                                               withDestinationURL: externalState)

    XCTAssertThrowsError(try PersonalDictionaryStore(directory: root).enqueue(previous: nil,
      replacement: PersonalWord(key: "ni'hao", value: "拟好")))
    XCTAssertEqual(try Data(contentsOf: externalState), Data("synthetic-state".utf8))
    #endif
  }

  func testQueueAcknowledgementFailureAndPaging() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = PersonalDictionaryStore(directory: root)
    let keyboard = PersonalDictionaryStore(directory: root)
    let word = PersonalWord(key: "ni'hao", value: "拟好")
    let id = try host.enqueue(previous: nil, replacement: word)
    XCTAssertEqual(try host.read().pendingCount, 1)
    XCTAssertTrue(try host.read().entries.isEmpty)
    XCTAssertThrowsError(try host.enqueue(previous: nil, replacement: word))
    var calls = [String]()
    try keyboard.synchronize(apply: { calls.append($0.id) }, page: { _ in .init(entries: [word], hasMore: true) })
    XCTAssertEqual(calls, [id])
    XCTAssertEqual(try host.read().requests.first?.status, .applied)
    XCTAssertEqual(try host.read().entries, [word])
    try keyboard.synchronize(apply: { _ in XCTFail("Acknowledged edit was applied twice") },
                             page: { _ in .init(entries: [word], hasMore: true) })
    let removal = try host.enqueue(previous: word, replacement: nil)
    enum Failure: Error { case injected }
    try keyboard.synchronize(apply: { _ in throw Failure.injected }, page: { _ in throw Failure.injected })
    XCTAssertEqual(try host.read().requests.last?.status, .failed)
    XCTAssertEqual(try host.read().entries, [word])
    XCTAssertNotNil(try host.read().snapshotError)
    try host.retry(removal)
    try host.requestPage(offset: 100)
    try keyboard.synchronize(apply: { XCTAssertEqual($0.id, removal) }, page: {
      XCTAssertEqual($0.offset, 100)
      return .init(entries: [], hasMore: false)
    })
    XCTAssertEqual(try host.read().pageOffset, 100)
    XCTAssertEqual(try host.read().pendingCount, 0)
    XCTAssertTrue(try host.read().entries.isEmpty)
    XCTAssertNil(try host.read().snapshotError)
  }

  @MainActor
  func testQueueRetryReachesActualKeyboardEngine() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = PersonalDictionaryStore(directory: root)
    let keyboard = PersonalDictionaryStore(directory: root)
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let session = MetasequoiaInputSessionBridge(resources: resources,
                                                 stateRoot: root.appendingPathComponent("EngineState"))
    let word = try PersonalWord(kind: .quickPhrase, key: "msimefixture", value: "private fixture text").validated()
    let id = try host.enqueue(previous: nil, replacement: word)
    defer {
      _ = session.cancel()
      try? session.applyPersonalPrevious(word.bridgeValue, replacement: nil, requestID: UUID().uuidString)
    }
    // Simulate a committed edit followed by process interruption before its shared acknowledgement.
    try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: id)
    func synchronize() throws {
      try keyboard.synchronize(apply: {
        try session.applyPersonalPrevious($0.previous?.bridgeValue, replacement: $0.replacement?.bridgeValue,
                                          requestID: $0.id)
      }, page: {
        let result = try session.personalEntries(atOffset: UInt($0.offset))
        let entries = try XCTUnwrap(result["entries"] as? [[String: Any]])
        return .init(entries: try entries.map { try PersonalWord(bridgeValue: $0) },
                     hasMore: try XCTUnwrap(result["hasMore"] as? Bool))
      })
    }
    try synchronize()
    XCTAssertEqual(try host.read().requests.first?.status, .applied)
    XCTAssertTrue(try host.read().entries.contains(word))
    _ = session.openLocalMode("K")
    var snapshot = session.handleCharacter("m")
    for letter in word.key.dropFirst() { snapshot = session.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains(word.value))
    _ = session.cancel()
    try host.enqueue(previous: word, replacement: nil)
    try synchronize()
    XCTAssertEqual(try host.read().requests.last?.status, .applied)
    XCTAssertFalse(try host.read().entries.contains { $0.id == word.id })
  }

  func testEngineValidationAndBridgePayload() throws {
    let word = try PersonalWord(key: "NI HAO", value: "拟好").validated()
    XCTAssertEqual(word.key, "ni'hao")
    XCTAssertEqual(try PersonalWord(bridgeValue: word.bridgeValue), word)
    XCTAssertThrowsError(try PersonalWord(bridgeValue: [
      "kind": "pinyin", "key": "ni", "value": "你好", "weight": NSNumber(value: 2.5),
    ]))
    XCTAssertThrowsError(try PersonalWord(bridgeValue: [
      "kind": "pinyin", "key": "ni", "value": "你好", "weight": NSNumber(value: true),
    ]))
    XCTAssertThrowsError(try PersonalWord(key: "nihao", value: "你好").validated())
    XCTAssertThrowsError(try PersonalWord(key: "ni'hao", value: "你").validated())
    XCTAssertThrowsError(try PersonalWord(kind: .wubi, key: "abcde", value: "词").validated())
    XCTAssertEqual(try PersonalWord(kind: .english, key: "dont", value: "don't").validated().value, "don't")
    XCTAssertThrowsError(try PersonalWord(kind: .english, key: "w0rd", value: "Word").validated())
    XCTAssertThrowsError(try PersonalWord(key: "ni", value: "a\0b").validated())
    XCTAssertEqual(try PersonalWord(kind: .quickPhrase, key: "HELLO", value: "第一行\n第二行").validated().key, "hello")
    // A quick phrase code takes letters only, and the refusal names that rule rather than a generic one.
    XCTAssertThrowsError(try PersonalWord(kind: .quickPhrase, key: "hello1", value: "你好").validated()) {
      XCTAssertEqual($0.localizedDescription, "快捷短语编码只能包含英文字母，长度 1 到 32。")
    }
  }

  func testImportValidatesAllEntriesAndRejectsMalformedOrDuplicateData() throws {
    let example = try PersonalDictionaryImport.decode(PersonalDictionaryImport.example.encoded())
    XCTAssertEqual(example.entries.count, 4)
    XCTAssertEqual(example.entries[0].key, "ni'hao")
    XCTAssertEqual(example.entries[3].value, "你好！\n很高兴认识你。")
    var invalid = example
    invalid.entries.append(.init(key: "nihao", value: "你好"))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(invalid.encoded())) { error in
      XCTAssertTrue(error.localizedDescription.contains("第 5 条"))
    }
    invalid = example
    invalid.entries.append(.init(key: "NI HAO", value: "你好"))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(invalid.encoded())) { error in
      XCTAssertTrue(error.localizedDescription.contains("重复"))
    }
    invalid = example
    invalid.version = 2
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(invalid.encoded()))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(Data("{}".utf8)))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(Data(repeating: 32, count: 1_048_577)))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(PersonalDictionaryImport(entries: []).encoded()))
    let oversized = PersonalDictionaryImport(entries: Array(repeating: example.entries[0], count: 129))
    XCTAssertThrowsError(try PersonalDictionaryImport.decode(oversized.encoded()))
  }

  func testImportQueueIsAtomicOnValidationConflictAndCapacityFailure() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = PersonalDictionaryStore(directory: root)
    let words = try PersonalDictionaryImport.decode(PersonalDictionaryImport.example.encoded()).entries
    try store.enqueueImport(words)
    let file = root.appendingPathComponent("PersonalDictionary/sync.json")
    let original = try Data(contentsOf: file)
    XCTAssertEqual(try store.read().pendingCount, 4)
    let fresh = PersonalWord(kind: .quickPhrase, key: "newfixture", value: "fixture")
    XCTAssertThrowsError(try store.enqueueImport([fresh, words[0]]))
    XCTAssertEqual(try Data(contentsOf: file), original)
    XCTAssertThrowsError(try store.enqueueImport([fresh, .init(key: "nihao", value: "你好")]))
    XCTAssertEqual(try Data(contentsOf: file), original)
    let tooMany = (0..<125).map { PersonalWord(kind: .quickPhrase, key: letterCode("fixture", $0), value: "fixture") }
    XCTAssertThrowsError(try store.enqueueImport(tooMany))
    XCTAssertEqual(try Data(contentsOf: file), original)
    try store.synchronize(apply: { _ in }, page: { _ in .init(entries: [], hasMore: false) })
    try store.enqueueImport(tooMany)
    XCTAssertEqual(try store.read().pendingCount, 125)
    XCTAssertEqual(Set(try store.read().requests.map(\.id)).count, 129)
  }

  func testLargeImportYieldsBetweenSyncBatchesAndPreservesReceipts() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = PersonalDictionaryStore(directory: root)
    try store.enqueueImport((0..<9).map { .init(kind: .quickPhrase, key: letterCode("batch", $0), value: "fixture") })
    let ids = try store.read().requests.map(\.id)
    var applied = [String]()
    try store.synchronize(apply: { applied.append($0.id) }, page: { _ in .init(entries: [], hasMore: false) })
    XCTAssertEqual(applied, Array(ids.prefix(4)))
    XCTAssertEqual(try store.read().pendingCount, 5)
    XCTAssertNotEqual(try store.read().refreshID, try store.read().completedRefreshID)
    try store.synchronize(apply: { applied.append($0.id) }, page: { _ in .init(entries: [], hasMore: false) })
    XCTAssertEqual(applied, Array(ids.prefix(8)))
    try store.synchronize(apply: { applied.append($0.id) }, page: { _ in .init(entries: [], hasMore: false) })
    XCTAssertEqual(applied, ids)
    XCTAssertEqual(try store.read().pendingCount, 0)
    XCTAssertEqual(try store.read().refreshID, try store.read().completedRefreshID)
  }

  func testCoordinatedFileImportReadsCompleteContentAndRejectsOversizedFiles() throws {
    let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".json")
    defer { try? FileManager.default.removeItem(at: file) }
    // Multiple read chunks, including escaped newlines and multibyte text,
    // while every entry stays under the Engine's quick-phrase limit.
    let words = (0..<128).map {
      PersonalWord(kind: .quickPhrase, key: letterCode("file", $0), value: String(repeating: "你好\n", count: 60))
    }
    let data = try PersonalDictionaryImport(entries: words).encoded()
    XCTAssertGreaterThan(data.count, 65_536)
    try data.write(to: file)
    XCTAssertEqual(try PersonalDictionaryImport.read(from: file).entries, words)
    try Data(repeating: 32, count: PersonalDictionaryImport.maximumBytes + 1).write(to: file)
    XCTAssertThrowsError(try PersonalDictionaryImport.read(from: file)) { error in
      XCTAssertTrue(error.localizedDescription.contains("1 MB"))
    }
    try FileManager.default.removeItem(at: file)
    XCTAssertThrowsError(try PersonalDictionaryImport.read(from: file))
  }

  func testMalformedStateIsPreservedAndReadDoesNotCreateFiles() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = PersonalDictionaryStore(directory: root)
    XCTAssertTrue(try store.read().entries.isEmpty)
    XCTAssertFalse(FileManager.default.fileExists(atPath: root.path))
    let directory = root.appendingPathComponent("PersonalDictionary")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let file = directory.appendingPathComponent("sync.json")
    let original = Data("broken fixture".utf8)
    try original.write(to: file)
    XCTAssertThrowsError(try store.enqueue(previous: nil, replacement: .init(key: "ni", value: "拟")))
    XCTAssertEqual(try Data(contentsOf: file), original)
    XCTAssertThrowsError(try PersonalDictionaryStore(directory: nil).read())
  }

  func testTheKeyboardWritesTheRequestedExportAfterTheQueuedEdits() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = PersonalDictionaryStore(directory: root)
    let keyboard = PersonalDictionaryStore(directory: root)
    let empty: (PersonalPageRequest) throws -> PersonalWordPage = { _ in .init(entries: [], hasMore: false) }
    XCTAssertThrowsError(try host.requestExport(kind: .pinyin, format: "rime"), "the Engine exports only the two TSV layouts")

    // An edit still waiting is applied first, and the export waits for the queue to drain.
    try host.enqueueImport((0..<5).map { PersonalWord(kind: .quickPhrase, key: letterCode("q", $0), value: "短语\($0)") })
    let request = try host.requestExport(kind: .quickPhrase, format: "windows")
    var asked: [PersonalExportRequest] = []
    let export: (PersonalExportRequest) throws -> PersonalExportText = {
      asked.append($0)
      return PersonalExportText(text: "q0\t短语0\t100\nq1\t短语1\t100\n", complete: true)
    }
    try keyboard.synchronize(apply: { _ in }, page: empty, export: export)
    XCTAssertEqual(try host.read().pendingCount, 1)
    XCTAssertTrue(asked.isEmpty)
    try keyboard.synchronize(apply: { _ in }, page: empty, export: export)
    XCTAssertEqual(asked, [request])
    let result = try XCTUnwrap(host.read().exportResult)
    XCTAssertEqual(result.request, request)
    XCTAssertEqual(result.rows, 2)
    XCTAssertFalse(result.truncated)
    XCTAssertNil(result.error)

    // Written once per request, and shared under the desktop's name.
    try keyboard.synchronize(apply: { _ in }, page: empty, export: export)
    XCTAssertEqual(asked.count, 1)
    let copy = try host.exportCopy(for: result)
    XCTAssertEqual(copy.lastPathComponent, "水杉IME-快捷短语用户词库.txt")
    XCTAssertEqual(try String(contentsOf: copy, encoding: .utf8), "q0\t短语0\t100\nq1\t短语1\t100\n")

    // A newer export must not be copied under an older result's filename.
    let newer = try host.requestExport(kind: .pinyin, format: "standard")
    try keyboard.synchronize(apply: { _ in }, page: empty, export: { _ in
      PersonalExportText(text: "new\t新\t1\n", complete: true)
    })
    XCTAssertEqual(try host.read().exportResult?.request, newer)
    XCTAssertThrowsError(try host.exportCopy(for: result))

    // A failed export leaves no stale file behind and says why.
    enum Failure: LocalizedError { case injected; var errorDescription: String? { "injected" } }
    let failed = try host.requestExport(kind: .pinyin, format: "standard")
    try keyboard.synchronize(apply: { _ in }, page: empty, export: { _ in throw Failure.injected })
    XCTAssertEqual(try host.read().exportResult?.request, failed)
    XCTAssertEqual(try host.read().exportResult?.error, "injected")
    XCTAssertFalse(FileManager.default.fileExists(atPath: try XCTUnwrap(host.exportFile).path))
    XCTAssertThrowsError(try host.exportCopy(for: try XCTUnwrap(host.read().exportResult)))

    try host.requestExport(kind: .pinyin, format: "standard")
    try keyboard.synchronize(apply: { _ in }, page: empty, export: { _ in PersonalExportText(text: "你好\tni'hao\t1\n", complete: false) })
    XCTAssertEqual(try host.read().exportResult?.truncated, true)
  }

  @MainActor
  func testTheKeyboardEngineExportsTheUsersWords() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let session = MetasequoiaInputSessionBridge(resources: resources,
                                                 stateRoot: root.appendingPathComponent("EngineState"))
    let word = try PersonalWord(kind: .quickPhrase, key: "msimeexport", value: "export fixture", weight: 42).validated()
    defer {
      _ = session.cancel()
      try? session.applyPersonalPrevious(word.bridgeValue, replacement: nil, requestID: UUID().uuidString)
    }
    try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: UUID().uuidString)
    let windows = try session.personalExport(kind: .quickPhrase, format: "windows")
    XCTAssertTrue(windows.complete)
    XCTAssertTrue(windows.text.contains("msimeexport\texport fixture\t42\n"), windows.text)
    let standard = try session.personalExport(kind: .quickPhrase, format: "standard")
    XCTAssertTrue(standard.text.contains("export fixture\tmsimeexport\t42\n"), standard.text)
    // The session is reopened after the export, so typing still works.
    _ = session.openLocalMode("K")
    var snapshot = session.handleCharacter("m")
    for letter in word.key.dropFirst() { snapshot = session.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains(word.value))
  }

  @MainActor
  func testACodeSearchReachesTheWholeStoreThroughTheKeyboardEngine() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = PersonalDictionaryStore(directory: root)
    let keyboard = PersonalDictionaryStore(directory: root)
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let session = MetasequoiaInputSessionBridge(resources: resources,
                                                 stateRoot: root.appendingPathComponent("EngineState"))
    let wanted = try PersonalWord(kind: .quickPhrase, key: "msimesearch", value: "search fixture").validated()
    let other = try PersonalWord(kind: .quickPhrase, key: "msimeother", value: "other fixture").validated()
    defer {
      _ = session.cancel()
      for word in [wanted, other] {
        try? session.applyPersonalPrevious(word.bridgeValue, replacement: nil, requestID: UUID().uuidString)
      }
    }
    for word in [wanted, other] {
      try session.applyPersonalPrevious(nil, replacement: word.bridgeValue, requestID: UUID().uuidString)
    }

    try host.requestPage(offset: 0, kind: .quickPhrase, query: " msimes ")
    var asked: PersonalPageRequest?
    try keyboard.synchronize(apply: { _ in }, page: { request in
      asked = request
      let result = try session.personalEntries(atOffset: UInt(request.offset), kind: request.kind, query: request.query)
      let entries = try XCTUnwrap(result["entries"] as? [[String: Any]])
      return .init(entries: try entries.map { try PersonalWord(bridgeValue: $0) },
                   hasMore: try XCTUnwrap(result["hasMore"] as? Bool))
    })
    XCTAssertEqual(asked, PersonalPageRequest(offset: 0, kind: .quickPhrase, query: "msimes"))
    let state = try host.read()
    XCTAssertEqual(state.entries, [wanted])
    XCTAssertEqual(state.pageKind, .quickPhrase)
    XCTAssertEqual(state.pageQuery, "msimes")

    // The filter is written under the keys the Rust queue reads.
    let file = root.appendingPathComponent("PersonalDictionary/sync.json")
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    XCTAssertEqual(object["requestedKind"] as? String, "quickPhrase")
    XCTAssertEqual(object["pageQuery"] as? String, "msimes")
    XCTAssertThrowsError(try host.requestPage(offset: 0, query: String(repeating: "a", count: 257)))
  }

  @MainActor
  func testABundledWordIsFoundReweightedAndDeletedThroughTheQueue() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let host = PersonalDictionaryStore(directory: root)
    let keyboard = PersonalDictionaryStore(directory: root)
    let resources = try XCTUnwrap(Bundle.main.resourceURL?.appendingPathComponent("EngineResources", isDirectory: true))
    let session = MetasequoiaInputSessionBridge(resources: resources,
                                                 stateRoot: root.appendingPathComponent("EngineState"))
    defer { _ = session.cancel() }
    func sync() throws {
      try keyboard.synchronize(apply: { request in
        try session.applyPersonalPrevious(request.previous?.bridgeValue, replacement: request.replacement?.bridgeValue,
                                          requestID: request.id)
      }, page: { request in
        let result = try session.personalEntries(atOffset: UInt(request.offset), kind: request.kind, query: request.query)
        let entries = try XCTUnwrap(result["entries"] as? [[String: Any]])
        return .init(entries: try entries.map { try PersonalWord(bridgeValue: $0) },
                     hasMore: try XCTUnwrap(result["hasMore"] as? Bool))
      })
    }

    // Without a kind the page stays the user's own words, as before.
    try host.requestPage(offset: 0, query: "nihao")
    try sync()
    XCTAssertFalse(try host.read().entries.contains(where: \.isBundled))

    try host.requestPage(offset: 0, kind: .pinyin, query: "nihao")
    try sync()
    let listed = try XCTUnwrap(host.read().entries.first { $0.value == "你好" })
    XCTAssertTrue(listed.isBundled)
    XCTAssertEqual(try PersonalWord(bridgeValue: listed.bridgeValue), listed, "the mark survives the queue")
    var validatedCopy = listed
    validatedCopy.source = nil
    XCTAssertFalse(try validatedCopy.validated().isBundled)

    var reweighted = listed
    reweighted.weight = 7
    _ = try host.enqueue(previous: listed, replacement: reweighted)
    try sync()
    var state = try host.read()
    XCTAssertEqual(state.pendingCount, 0, "\(state.requests.map { $0.error ?? "" })")
    XCTAssertEqual(state.entries.first { $0.value == "你好" }?.weight, 7)

    // A bundled row's code and word are fixed; the Engine refuses anything else.
    let current = try XCTUnwrap(state.entries.first { $0.value == "你好" })
    var renamed = current
    renamed.value = "拟好"
    _ = try host.enqueue(previous: current, replacement: renamed)
    try sync()
    state = try host.read()
    XCTAssertEqual(state.requests.last?.status, .failed)
    try host.dismissFailure(try XCTUnwrap(state.requests.last?.id))

    _ = try host.enqueue(previous: current, replacement: nil)
    try sync()
    state = try host.read()
    XCTAssertEqual(state.pendingCount, 0, "\(state.requests.map { $0.error ?? "" })")
    XCTAssertNil(state.entries.first { $0.value == "你好" && $0.key == current.key })
  }

  func testStateUsesRustCompatibleRefreshKeys() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = PersonalDictionaryStore(directory: root)
    _ = try store.enqueue(previous: nil, replacement: .init(key: "ni", value: "拟"))
    let file = root.appendingPathComponent("PersonalDictionary/sync.json")
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? [String: Any])
    XCTAssertNotNil(object["refreshId"])
    XCTAssertNil(object["refreshID"])
    let fixture = try JSONSerialization.data(withJSONObject: [
      "version": 1, "requests": [], "entries": [], "hasMore": false,
      "pageOffset": 0, "requestedPageOffset": 0,
      "refreshId": UUID().uuidString, "completedRefreshId": NSNull()
    ])
    try fixture.write(to: file, options: .atomic)
    XCTAssertNoThrow(try store.read())
  }
}
