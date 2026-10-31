import XCTest

final class VocabularyReviewTests: XCTestCase {
  private func temporaryStore() -> VocabularyReviewStore {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    // 不用测试宿主自带的 EngineResources：它旁边可能暂存了真实的内置词书，这里的用例只看导入的词表。
    return VocabularyReviewStore(directory: directory, resources: nil)
  }

  /// 内置词书放在 EngineResources 的同级 wordbooks/ 里（stage-resources.sh 和键盘扩展的打包都按这个布局），共享层不向上找，所以存储传的必须是上一级目录。
  func testBundledBooksAreReadBesideTheEngineResources() throws {
    let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: base) }
    let engine = base.appendingPathComponent("bundle/EngineResources", isDirectory: true)
    let wordbooks = base.appendingPathComponent("bundle/wordbooks", isDirectory: true)
    try FileManager.default.createDirectory(at: engine, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: wordbooks, withIntermediateDirectories: true)
    let book = #"{"id":"cet4","name":"CET-4","entries":[{"word":"abandon","phonetic":"/a/","meaning":"v. 放弃"},{"word":"ability","phonetic":"","meaning":"n. 能力"}]}"#
    try Data(book.utf8).write(to: wordbooks.appendingPathComponent("cet4.json"))

    let store = VocabularyReviewStore(
      directory: base.appendingPathComponent("state", isDirectory: true), resources: engine)
    let status = try store.load()
    let bundled = try XCTUnwrap(status.wordbooks.first { $0.id == "cet4" })
    XCTAssertEqual(bundled.name, "CET-4")
    XCTAssertEqual(bundled.total, 2)
    XCTAssertTrue(bundled.builtin)
  }

  private let list = "alpha,/a/,adj. 甲\nbeta,adj. 乙\ngamma,adj. 丙\n"

  /// 键盘长按菜单的「加入生词本」写进背单词同一个目录：书目里出现「生词本」，提示由共享层给出，同一个词再加一次只说已经在了。
  func testPersonalWordbookAddsAGlossTheReviewPageLists() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    XCTAssertEqual(PersonalWordbookStore.add(candidate: "面试", gloss: "interview; audition\nmensetsu", directory: directory), "已加入生词本")
    XCTAssertEqual(PersonalWordbookStore.add(candidate: "面试", gloss: "interview", directory: directory), "已在生词本里")
    XCTAssertEqual(PersonalWordbookStore.add(candidate: "面试", gloss: " ", directory: directory), "这条释义不能加入生词本")
    let status = try VocabularyReviewStore(directory: directory, resources: nil).load()
    let book = try XCTUnwrap(status.wordbooks.first { $0.id == "personal" }, "\(status.wordbooks)")
    XCTAssertEqual(book.name, "生词本")
    XCTAssertEqual(book.total, 1)
    XCTAssertFalse(book.builtin)
  }

  /// 长按菜单只在有可用的释义时才列「加入生词本」，交给共享层的是显示着的几行释义、去掉空行。
  func testPersonalWordbookMenuNeedsAGloss() {
    XCTAssertEqual(PersonalWordbookStore.menuTitle, "加入生词本")
    XCTAssertEqual(PersonalWordbookStore.menuGloss([" interview ", "", "mensetsu\n"]), "interview\nmensetsu")
    XCTAssertNil(PersonalWordbookStore.menuGloss([]))
    XCTAssertNil(PersonalWordbookStore.menuGloss([" ", "\n"]))
  }

  /// 单词本插件装在共享状态目录的 `plugins/` 里（插件页和键盘读的是同一份），背单词把它列在导入的词表之前、标成插件；选它就能复习，在背单词里删不掉。
  func testWordbookPluginsAreListedFromTheSharedPluginsFolder() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let pack: [String: Any] = [
      "kind": "wordbook", "id": NSNull(), "name": "合成词书", "version": "1.0.0", "license": "CC0-1.0",
      "author": NSNull(), "description": NSNull(), "words": [["word": "delta", "phonetic": NSNull(), "gloss": "丁"]],
    ]
    guard case .success(let value) = MetasequoiaInputSessionBridge.plugins(
      ["operation": "authoring_save", "pack": pack, "replace": false], stateRoot: directory),
      let id = (value as? [String: Any])?["id"] as? String else { return XCTFail("authoring_save 失败") }

    let store = VocabularyReviewStore(directory: directory, resources: nil)
    let imported = try store.importWordbook(name: "合成词表", text: list)
    let book = try XCTUnwrap(imported.wordbooks.first { $0.id == "pack-\(id)" }, "\(imported.wordbooks)")
    XCTAssertTrue(book.pack)
    XCTAssertFalse(book.builtin)
    XCTAssertEqual(book.total, 1)
    XCTAssertEqual(imported.wordbooks.map(\.pack), [true, false], "插件词书排在导入的词表之前")
    let chosen = try store.setSettings(wordbook: book.id, newPerDay: imported.newPerDay, sessionLimit: imported.sessionLimit)
    XCTAssertEqual(chosen.wordbook, book.id)
    XCTAssertThrowsError(try store.removeWordbook(book.id), "插件词书在插件页卸载")
  }

  func testImportSelectsTheBookItCreatedAndDealsItsCards() throws {
    let store = temporaryStore()
    XCTAssertTrue(try store.load().wordbooks.isEmpty)

    let imported = try store.importWordbook(name: "合成词表", text: list)
    XCTAssertEqual(imported.wordbooks.count, 1)
    XCTAssertEqual(imported.wordbooks.first?.name, "合成词表")
    XCTAssertEqual(imported.wordbooks.first?.total, 3)
    XCTAssertEqual(imported.wordbooks.first?.builtin, false)
    // Leaving the user to pick the book they just imported out of a list is a step with one answer.
    XCTAssertEqual(imported.wordbook, imported.wordbooks.first?.id)
    XCTAssertEqual(imported.queue.count, 3)
    XCTAssertEqual(imported.current?.word, "alpha")
    XCTAssertEqual(imported.current?.phonetic, "/a/")
    XCTAssertEqual(imported.queue[1].phonetic, "", "two columns means no phonetic")
    XCTAssertFalse(imported.needsWordbook)
  }

  func testRecallLeavesTodaysQueueAndALapseStaysInIt() throws {
    let store = temporaryStore()
    let book = try store.importWordbook(name: "合成词表", text: list).wordbook

    let answered = try store.answer(wordbook: book, word: "alpha", known: true)
    XCTAssertEqual(answered.answeredToday, 1)
    XCTAssertEqual(answered.current?.word, "beta")

    let failed = try store.answer(wordbook: book, word: "beta", known: false)
    XCTAssertEqual(failed.answeredToday, 2)
    XCTAssertTrue(
      failed.queue.contains { $0.word == "beta" },
      "a failed card stays in the session it was failed in")
  }

  func testOldCardAnswersItsSourceBookAfterSelectingADifferentBook() throws {
    let store = temporaryStore()
    let first = try store.importWordbook(name: "合成词书甲", text: "alpha,合成释义甲\n")
    let second = try store.importWordbook(name: "合成词书乙", text: "alpha,合成释义乙\n")
    let answered = try store.answer(wordbook: first.wordbook, word: "alpha", known: true)
    XCTAssertEqual(answered.wordbook, second.wordbook)
    XCTAssertEqual(answered.current?.meaning, "合成释义乙")
    XCTAssertEqual(answered.answeredToday, 1)
    let original = try store.setSettings(wordbook: first.wordbook, newPerDay: 20, sessionLimit: 200)
    XCTAssertTrue(original.queue.isEmpty)
    _ = try store.removeWordbook(first.wordbook)
    _ = try store.setSettings(wordbook: second.wordbook, newPerDay: 20, sessionLimit: 200)
    XCTAssertThrowsError(try store.answer(wordbook: first.wordbook, word: "alpha", known: true))
    XCTAssertEqual(try store.load().current?.word, "alpha")
  }

  func testResetKeepsTheBooksAndRemoveTakesItsSchedule() throws {
    let store = temporaryStore()
    let book = try store.importWordbook(name: "合成词表", text: list).wordbook
    _ = try store.answer(wordbook: book, word: "alpha", known: true)

    let reset = try store.reset()
    XCTAssertEqual(reset.answeredToday, 0)
    XCTAssertEqual(reset.wordbook, book, "清空进度 is not 删除词表")
    XCTAssertEqual(reset.wordbooks.count, 1)

    let removed = try store.removeWordbook(book)
    XCTAssertTrue(removed.wordbooks.isEmpty)
    XCTAssertEqual(removed.wordbook, "")
    XCTAssertTrue(removed.queue.isEmpty)
    XCTAssertTrue(removed.needsWordbook)
  }

  func testSettingsRoundTripAndTheNewAllowanceTakesEffect() throws {
    let store = temporaryStore()
    let book = try store.importWordbook(name: "合成词表", text: list).wordbook
    let status = try store.setSettings(wordbook: book, newPerDay: 1, sessionLimit: 50)
    XCTAssertEqual(status.newPerDay, 1)
    XCTAssertEqual(status.sessionLimit, 50)
    XCTAssertEqual(status.introducing, 1)
  }

  func testAFileThatIsNotAWordListIsItsOwnAnswer() throws {
    let store = temporaryStore()
    // "storage" would read as the application being broken rather than the file being wrong.
    XCTAssertThrowsError(try store.importWordbook(name: "空的", text: "# 只有注释\n")) { error in
      XCTAssertEqual(error as? VocabularyReviewStore.Failure, .unreadableWordbook)
    }
    XCTAssertTrue(try store.load().wordbooks.isEmpty)
  }

  func testOversizedWordbookIsRejectedBeforeItIsLoaded() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: url) }
    try Data(repeating: 0x78, count: VocabularyReviewStore.maximumImportBytes + 1).write(to: url)
    XCTAssertThrowsError(try VocabularyReviewStore.readWordbookData(from: url)) { error in
      XCTAssertEqual(error as? VocabularyReviewStore.Failure, .unreadableWordbook)
    }
  }

  func testWordbookImportRejectsASymlinkedSource() throws {
    #if canImport(Darwin)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("msime-vocabulary-import-link-test-\(UUID().uuidString)")
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    let target = root.appendingPathComponent("target.txt")
    try Data(list.utf8).write(to: target)
    let selected = root.appendingPathComponent("selected.txt")
    try FileManager.default.createSymbolicLink(at: selected, withDestinationURL: target)

    XCTAssertThrowsError(try VocabularyReviewStore.readWordbookData(from: selected))
    #endif
  }

  func testNeedsWordbookSeparatesNoSelectionFromAFinishedDay() {
    var status = VocabularyReviewStatus()
    XCTAssertTrue(status.needsWordbook, "no selection asks for a book")

    status.wordbooks = [VocabularyWordbook(id: "cet-4", name: "CET-4", total: 4500, builtin: true)]
    status.wordbook = "cet-4"
    status.answeredToday = 30
    // Telling someone who has never picked a book that they have finished would be a lie, so the
    // finished day keeps its book and only the queue is empty.
    XCTAssertFalse(status.needsWordbook)
    XCTAssertNil(status.current)

    status.wordbook = "gone"
    XCTAssertTrue(status.needsWordbook, "a selection pointing at nothing asks for a book")
  }

  func testTodayIsTheShapeTheSharedLayerAccepts() {
    var components = DateComponents()
    components.year = 2026
    components.month = 9
    components.day = 3
    let date = Calendar(identifier: .gregorian).date(from: components)!
    XCTAssertEqual(
      VocabularyReviewStore.today(date, calendar: Calendar(identifier: .gregorian)),
      "2026-09-03",
      "the shared layer accepts exactly ten zero-padded bytes")
  }

  func testAMalformedStatusDecodesToSomethingThePageCanDraw() {
    // A card with no word could never be answered: the answer is keyed by it.
    let value: [String: Any] = [
      "wordbooks": [["name": "没有 id"], ["id": "user-1", "name": "有", "total": 2]],
      "settings": ["wordbook": "user-1", "newPerDay": 20, "sessionLimit": 200],
      "due": 1,
      "queue": [["phonetic": "/x/"], ["word": "alpha", "meaning": "adj. 甲"]],
    ]
    let status = VocabularyReviewStore.status(from: value)
    XCTAssertEqual(status.wordbooks.map(\.id), ["user-1"])
    XCTAssertEqual(status.queue.map(\.word), ["alpha"])
    XCTAssertEqual(status.queue.first?.phonetic, "")
    XCTAssertEqual(status.answeredToday, 0)
  }

  func testStatusIgnoresNonIntegerProgressMetadata() throws {
    let value: [String: Any] = [
      "wordbooks": [["id": "fixture", "total": true]],
      "settings": ["wordbook": "fixture", "newPerDay": 1.5, "sessionLimit": 20],
      "due": false,
      "answeredToday": 2.5,
      "introducing": 3,
      "remaining": 4,
    ]
    let roundTripped = try XCTUnwrap(
      JSONSerialization.jsonObject(with: JSONSerialization.data(withJSONObject: value)) as? [String: Any])
    let status = VocabularyReviewStore.status(from: roundTripped)
    XCTAssertEqual(status.wordbooks.first?.total, 0)
    XCTAssertEqual(status.newPerDay, 0)
    XCTAssertEqual(status.due, 0)
    XCTAssertEqual(status.answeredToday, 0)
    XCTAssertEqual(status.introducing, 3)
    XCTAssertEqual(status.remaining, 4)
  }
}
