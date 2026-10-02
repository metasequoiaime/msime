import XCTest

final class CustomTranslationsTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-custom-translations-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testWriteRejectsASymlinkedUserDirectoryBeforeWritingExternalFile() throws {
    #if canImport(Darwin)
    let linked = state.appendingPathComponent("linked", isDirectory: true)
    let outside = state.appendingPathComponent("outside", isDirectory: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)
    let url = linked.appendingPathComponent(CustomTranslations.fileName)

    XCTAssertThrowsError(try CustomTranslations.write("你好\thello\n", to: url))
    XCTAssertFalse(FileManager.default.fileExists(atPath: outside.appendingPathComponent(CustomTranslations.fileName).path))
    #endif
  }

  func testParsingFollowsTheEngineRules() {
    let text = "\u{FEFF}# comment\r\n你好\thello\n\n刚才\ta moment ago\r\nserendipity\t意外发现\n\tno source\nno gloss\t\nnotab\n你好\thi again\n  spaced \t  gloss  \n"
    XCTAssertEqual(CustomTranslations.parse(text),
                   CustomTranslations.Report(entries: 4, chineseSources: 2, skipped: 3),
                   "a repeated source counts once, and a leading tab, a trailing tab and a line without one are skipped")
    XCTAssertEqual(CustomTranslations.parse(""), CustomTranslations.Report(entries: 0, chineseSources: 0, skipped: 0))
  }

  func testTheFileLandsInTheUserDirectoryTheKeyboardEngineIsGiven() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    let user = try XCTUnwrap(bridge.dictionarySnapshotContext()["user"] as? URL)
    let url = CustomTranslations.url(stateRoot: state)
    XCTAssertEqual(url.deletingLastPathComponent().standardizedFileURL.path, user.standardizedFileURL.path)
    _ = bridge.cancel()
  }

  func testWritingRoundTripsRemovesAnEmptyDocumentAndRefusesAnOversizedOne() throws {
    let url = CustomTranslations.url(stateRoot: state)
    XCTAssertEqual(try CustomTranslations.read(at: url), "", "no file yet is an empty document")

    try CustomTranslations.write("你好\thello\n", to: url)
    XCTAssertEqual(try CustomTranslations.read(at: url), "你好\thello\n")
    try Data("\u{FEFF}测试\ttest\n".utf8).write(to: url)
    XCTAssertEqual(try CustomTranslations.read(at: url), "测试\ttest\n", "a BOM is an encoding marker, not part of the first source")

    XCTAssertThrowsError(try CustomTranslations.write(String(repeating: "a", count: CustomTranslations.maximumBytes + 1), to: url))
    XCTAssertThrowsError(try CustomTranslations.write("你好\thello\0\n", to: url))
    XCTAssertEqual(try CustomTranslations.read(at: url), "测试\ttest\n", "a refused write leaves the previous file")

    try CustomTranslations.write(" \n\t\n", to: url)
    XCTAssertFalse(FileManager.default.fileExists(atPath: url.path), "an emptied document removes the file")
    try CustomTranslations.write("", to: url)
  }
}
