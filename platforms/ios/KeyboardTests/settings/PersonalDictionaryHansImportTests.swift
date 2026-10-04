import XCTest

/// A distinct letters-only code for fixture row `index` (0..<676): quick phrase codes take letters only, as the Windows source's `valid_code` does, so a numbered fixture spells its number in letters.
private func letterCode(_ prefix: String, _ index: Int) -> String {
  let letters = Array("abcdefghijklmnopqrstuvwxyz")
  return prefix + String(letters[index / 26]) + String(letters[index % 26])
}

/// Plain Chinese word lists are annotated by the shared Engine from the packaged dictionary, then previewed and queued like a JSON import.
final class PersonalDictionaryHansImportTests: XCTestCase {
  func testPackagedDictionaryAnnotatesWordsAndSkipsCommentsAndRepeats() throws {
    XCTAssertNotNil(PersonalDictionaryBridge.packagedResources, "the test host carries the Engine resources")
    let imported = try PersonalDictionaryImport.hans("你好\n# 注释\n\n世界\n你好\n")
    XCTAssertEqual(imported.entries.map(\.value), ["你好", "世界"])
    XCTAssertTrue(imported.entries.allSatisfy { $0.kind == .pinyin })
    XCTAssertEqual(imported.entries.map { $0.key.filter(\.isLetter) }, ["nihao", "shijie"])
    for word in imported.entries { XCTAssertEqual(try word.validated(), word) }
  }

  func testTextTheImportFormatRefusesSaysWhatToFix() {
    for text in ["hello", "你好 世界", "你好\t世界"] {
      XCTAssertThrowsError(try PersonalDictionaryBridge.hansEntries(text), text) {
        XCTAssertEqual($0 as? HansImportFailure, .invalidText)
      }
    }
  }

  func testMissingDictionaryIsReportedAsUnavailable() throws {
    let empty = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-hans-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: empty, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: empty) }
    XCTAssertThrowsError(try PersonalDictionaryBridge.hansEntries("你好", resources: empty)) {
      XCTAssertEqual($0 as? HansImportFailure, .dictionaryUnavailable)
    }
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: empty.path), [])
  }

  func testImportKeepsToTheQueueLimit() {
    let words = (0..<129).map { PersonalWord(key: "a\($0)", value: "词\($0)") }
    XCTAssertThrowsError(try PersonalDictionaryImport.hans("fixture") { _ in words })
    XCTAssertThrowsError(try PersonalDictionaryImport.hans("fixture") { _ in [] })
    XCTAssertEqual(try PersonalDictionaryImport.hans("fixture") { _ in Array(words.prefix(128)) }.entries.count, 128)
  }

  func testSharedImportFormatsQueueWhatTheEngineAcceptsAndReportTheRest() throws {
    // Standard puts the word first; the second row is jianpin the Engine refuses and the third repeats the first.
    let standard = try PersonalDictionaryBridge.importEntries(
      kind: "pinyin", format: "standard", text: "水杉\tshui'shan\t100\n你好\tnihaoma\t100\n水杉\tshui'shan\t100\n在家\tzai'jia\t100\n")
    XCTAssertEqual(standard.entries.map(\.value), ["水杉", "在家"])
    XCTAssertEqual(standard.entries.map(\.key), ["shui'shan", "zai'jia"])
    XCTAssertEqual(standard.report["applied"] as? Int, 2)
    XCTAssertEqual(standard.report["failed"] as? Int, 1)
    XCTAssertEqual((standard.report["first_failures"] as? [[String: Any]])?.first?["line"] as? Int, 2)
    XCTAssertNil(standard.report["entries"], "the words travel separately from the report")

    let windows = try PersonalDictionaryBridge.importEntries(kind: "quick_phrase", format: "windows", text: "zjd\t在家等\t100\n")
    XCTAssertEqual(windows.entries, [PersonalWord(kind: .quickPhrase, key: "zjd", value: "在家等", weight: 100)])

    // The queue holds 128 words, so a longer file is queued in part and says so instead of being refused.
    let long = (0..<200).map { "短语\($0)\t\(letterCode("q", $0))\t100\n" }.joined()
    let capped = try PersonalDictionaryBridge.importEntries(kind: "quick_phrase", format: "standard", text: long)
    XCTAssertEqual(capped.entries.count, 128)
    XCTAssertEqual(capped.report["truncated"] as? Bool, true)
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-import-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = PersonalDictionaryStore(directory: directory)
    try store.enqueueImport(capped.entries, requestID: "ui-import-1")
    XCTAssertEqual(try store.read().pendingCount, 128)

    let hans = try PersonalDictionaryBridge.importEntries(kind: "pinyin", format: "hans", text: "你好\n世界\n")
    XCTAssertEqual(hans.entries.map { $0.key.filter(\.isLetter) }, ["nihao", "shijie"])

    XCTAssertThrowsError(try PersonalDictionaryBridge.importEntries(kind: "pinyin", format: "standard", text: "你好\tnihaoma\t100\n")) {
      XCTAssertEqual($0 as? DictionaryFileImportFailure, .rejected)
    }
    XCTAssertThrowsError(try PersonalDictionaryBridge.importEntries(kind: "pinyin", format: "csv", text: "x")) {
      XCTAssertEqual($0 as? DictionaryFileImportFailure, .rejected)
    }
  }

  func testTheNativePagesDictionaryFileImportPreviewsWhatItSkipped() throws {
    let standard = try PersonalDictionaryImport.file(
      "水杉\tshui'shan\t100\n你好\tni\t100\n在家\tzai'jia\t100\n", kind: .pinyin, format: "standard")
    XCTAssertEqual(standard.file.entries.map(\.value), ["水杉", "在家"])
    XCTAssertEqual(standard.notice, "跳过 1 行，首先出现在第 2 行。")
    // A row that parsed but that the Engine refused gets the desktop's extra hint.
    XCTAssertEqual(
      PersonalDictionaryImport.notice(["failed": 2, "first_failures": [["line": 4, "issue": "pinyin"], ["line": 9, "issue": "rejected"]]]),
      "跳过 2 行，首先出现在第 4、9 行。其中部分行的编码与词不匹配，例如简拼、或音节数与汉字数不一致。")

    let rime = try PersonalDictionaryImport.file(
      "---\nname: demo\n...\n你好\tni hao\tc=3\n世界\tshi jie\n", kind: .pinyin, format: "rime")
    XCTAssertEqual(rime.file.entries.map(\.value), ["你好", "世界"])
    XCTAssertEqual(rime.notice, "")

    // A word-first file read as code-first is read the way round it was written, and the page says so.
    let swapped = try PersonalDictionaryImport.file("在家等\tzjd\t100\n", kind: .quickPhrase, format: "windows")
    XCTAssertEqual(swapped.file.entries, [PersonalWord(kind: .quickPhrase, key: "zjd", value: "在家等", weight: 100)])
    XCTAssertTrue(swapped.notice.contains("两列与所选格式相反"), swapped.notice)

    let long = (0..<200).map { "短语\($0)\t\(letterCode("q", $0))\t100\n" }.joined()
    let capped = try PersonalDictionaryImport.file(long, kind: .quickPhrase, format: "standard")
    XCTAssertEqual(capped.file.entries.count, 128)
    XCTAssertTrue(capped.notice.contains("仅导入前 128 条"), capped.notice)

    XCTAssertThrowsError(try PersonalDictionaryImport.file("你好\tnihaoma\t100\n", kind: .pinyin, format: "standard")) {
      XCTAssertEqual($0.localizedDescription, "文件里没有能导入的拼音词条，请确认词库类型和文件格式。")
    }
    XCTAssertThrowsError(try PersonalDictionaryImport.file("x", kind: .pinyin, format: "standard") { _, _, _ in
      throw DictionaryFileImportFailure.unavailable
    }) { XCTAssertEqual($0.localizedDescription, "键盘词典暂时无法读取，请稍后再试。") }
  }

  func testImportNoticeIgnoresNonIntegerFailureMetadata() {
    let report: [String: Any] = [
      "failed": 1.5,
      "first_failures": [["line": true]],
    ]
    XCTAssertEqual(PersonalDictionaryImport.notice(report), "")
  }

  func testEditedWeightIsQueuedAsWritten() throws {
    XCTAssertEqual(PersonalWord.weightRange, 1...100_000_000)
    XCTAssertEqual(PersonalWord(key: "ni hao", value: "你好").weight, PersonalWord.defaultWeight)
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-weight-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = PersonalDictionaryStore(directory: directory)
    let previous = PersonalWord(key: "ni hao", value: "你好", weight: 100_000)
    var replacement = previous
    replacement.weight = 7
    try store.enqueue(previous: previous, replacement: try replacement.validated())
    let request = try XCTUnwrap(store.read().requests.first)
    XCTAssertEqual(request.previous?.weight, 100_000)
    XCTAssertEqual(request.replacement?.weight, 7)
  }
}
