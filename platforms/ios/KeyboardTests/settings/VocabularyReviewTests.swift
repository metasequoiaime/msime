import XCTest

final class VocabularyReviewTests: XCTestCase {
  private func temporaryStore() -> VocabularyReviewStore {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString)
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return VocabularyReviewStore(directory: directory)
  }

  private let list = "alpha,/a/,adj. 甲\nbeta,adj. 乙\ngamma,adj. 丙\n"

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
    _ = try store.importWordbook(name: "合成词表", text: list)

    let answered = try store.answer("alpha", known: true)
    XCTAssertEqual(answered.answeredToday, 1)
    XCTAssertEqual(answered.current?.word, "beta")

    let failed = try store.answer("beta", known: false)
    XCTAssertEqual(failed.answeredToday, 2)
    XCTAssertTrue(
      failed.queue.contains { $0.word == "beta" },
      "a failed card stays in the session it was failed in")
  }

  func testResetKeepsTheBooksAndRemoveTakesItsSchedule() throws {
    let store = temporaryStore()
    let book = try store.importWordbook(name: "合成词表", text: list).wordbook
    _ = try store.answer("alpha", known: true)

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
