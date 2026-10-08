import XCTest

/// The keyboard's diagnostic log follows `diagnostic_log.server`, keeps to event labels and rotates like the desktop hosts' logs.
final class DiagnosticLogTests: XCTestCase {
  private var state: URL!
  private let log = DiagnosticLog()

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-diagnostic-log-\(UUID().uuidString)", isDirectory: true)
    try? FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private var file: URL { state.appendingPathComponent(DiagnosticLog.fileName) }

  func testWriteRejectsASymlinkedDirectoryBeforeCreatingExternalLog() throws {
    #if canImport(Darwin)
    let linked = state.appendingPathComponent("linked", isDirectory: true)
    let outside = state.appendingPathComponent("outside", isDirectory: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)

    log.configure(directory: linked.path, enabled: true)
    log.write("synthetic_event")

    XCTAssertFalse(FileManager.default.fileExists(atPath: outside.appendingPathComponent(DiagnosticLog.fileName).path))
    #endif
  }

  func testReadTailRejectsAHardLinkedLogOutsideTheStateDirectory() throws {
    #if canImport(Darwin)
    let outside = state.appendingPathComponent("outside", isDirectory: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    let external = outside.appendingPathComponent("private.log")
    try Data("synthetic external log\n".utf8).write(to: external)
    try FileManager.default.linkItem(at: external, to: file)

    XCTAssertThrowsError(try DiagnosticLog.readTail(from: file, maximumBytes: 1024))
    #endif
  }

  /// Only a real `true` under `diagnostic_log.server` turns it on; the Windows-only `tsf` field does not.
  func testOnlyTheServerBooleanEnablesTheLog() {
    XCTAssertTrue(DiagnosticLog.isEnabled(in: ["diagnostic_log": ["server": true, "tsf": false]]))
    XCTAssertFalse(DiagnosticLog.isEnabled(in: ["diagnostic_log": ["server": false, "tsf": true]]))
    XCTAssertFalse(DiagnosticLog.isEnabled(in: ["diagnostic_log": ["server": NSNumber(value: 1)]]))
    XCTAssertFalse(DiagnosticLog.isEnabled(in: ["diagnostic_log": true]))
    XCTAssertFalse(DiagnosticLog.isEnabled(in: nil))
  }

  /// A record never carries anything but printable ASCII, and never more than 192 bytes of it.
  func testEventsAreCutToPrintableAscii() {
    XCTAssertEqual(DiagnosticLog.sanitize("focus_in"), "focus_in")
    XCTAssertEqual(DiagnosticLog.sanitize("a\nb\u{7f}"), "a?b?")
    XCTAssertEqual(DiagnosticLog.sanitize("水"), "???")
    XCTAssertEqual(DiagnosticLog.sanitize(String(repeating: "x", count: 500)).utf8.count, DiagnosticLog.maxEventBytes)
  }

  /// Off writes nothing; on appends one owner-only line per event; off again stops.
  func testWritesOnlyWhileEnabled() throws {
    log.configure(directory: state.path, enabled: false)
    log.write("focus_in")
    XCTAssertFalse(FileManager.default.fileExists(atPath: file.path))

    log.configure(directory: state.path, enabled: true)
    log.write("focus_in")
    log.write("focus_out")
    log.configure(directory: state.path, enabled: false)
    log.write("memory_warning")

    let lines = try String(contentsOf: file, encoding: .utf8).split(separator: "\n")
    XCTAssertEqual(lines.count, 2)
    XCTAssertTrue(lines[0].hasSuffix("] focus_in"))
    XCTAssertTrue(lines[1].hasSuffix("] focus_out"))
    let permissions = try FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as? NSNumber
    XCTAssertEqual(permissions?.intValue, 0o600)
  }

  /// A relative directory is refused rather than written next to wherever the process happens to be.
  func testRelativeDirectoryIsRefused() {
    XCTAssertNil(DiagnosticLog.url(in: "relative/dir"))
    XCTAssertNil(DiagnosticLog.url(in: ""))
  }

  /// Past 1 MiB the log moves to `.1`, replacing an older copy, and starts again.
  func testRotatesPastOneMebibyte() throws {
    try Data("old".utf8).write(to: file.appendingPathExtension("1"))
    try Data(count: DiagnosticLog.maxBytes + 1).write(to: file)
    log.configure(directory: state.path, enabled: true)
    log.write("focus_in")

    let rotated = try Data(contentsOf: file.appendingPathExtension("1"))
    XCTAssertEqual(rotated.count, DiagnosticLog.maxBytes + 1)
    XCTAssertTrue(try String(contentsOf: file, encoding: .utf8).hasSuffix("] focus_in\n"))
  }

  /// Reading the preview keeps the allocation bounded while still reporting the complete file size.
  func testReadTailBoundsThePreviewForAnOversizedLog() throws {
    let expectedTail = Data(repeating: 0x42, count: 32 * 1024)
    var oversized = Data(repeating: 0x11, count: 2 * 1024 * 1024)
    oversized.append(expectedTail)
    try oversized.write(to: file)

    let result = try DiagnosticLog.readTail(from: file, maximumBytes: expectedTail.count)

    XCTAssertEqual(result.size, 2 * 1024 * 1024 + expectedTail.count)
    XCTAssertEqual(result.data, expectedTail)
  }

  func testReadTailRejectsLeafSymlink() throws {
    let outside = state.appendingPathComponent("outside.log")
    let linked = state.appendingPathComponent(DiagnosticLog.fileName)
    try Data("synthetic diagnostic log".utf8).write(to: outside)
    try FileManager.default.createSymbolicLink(at: linked, withDestinationURL: outside)

    XCTAssertThrowsError(try DiagnosticLog.readTail(from: linked, maximumBytes: 32 * 1024))
  }

  func testWriteRejectsLeafHardLink() throws {
    let outside = state.appendingPathComponent("outside-hardlink.log")
    try Data("outside\n".utf8).write(to: outside)
    try FileManager.default.linkItem(atPath: outside.path, toPath: file.path)

    log.configure(directory: state.path, enabled: true)
    log.write("must_not_modify_hardlink")

    XCTAssertEqual(try Data(contentsOf: outside), Data("outside\n".utf8))
  }

  /// The App's switch writes `diagnostic_log.server` and leaves the Windows-only field as stored.
  func testSwitchKeepsTheWindowsField() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["diagnostic_log"] = ["server": false, "tsf": true]
    })
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      var diagnostic = $0["diagnostic_log"] as? [String: Any] ?? [:]
      diagnostic["server"] = true
      $0["diagnostic_log"] = diagnostic
    })

    let preferences = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertTrue(DiagnosticLog.isEnabled(in: preferences))
    XCTAssertEqual((preferences?["diagnostic_log"] as? [String: Any])?["tsf"] as? Bool, true)
  }
}
