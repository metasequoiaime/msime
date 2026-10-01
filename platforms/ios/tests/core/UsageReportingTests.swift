import Foundation
import XCTest

final class UsageReportingTests: XCTestCase {
  func testCallStackTreeBecomesBinaryAndOffsetLinesCrashedThreadFirst() throws {
    let tree: [String: Any] = [
      "callStackPerThread": true,
      "callStacks": [
        ["threadAttributed": false, "callStackRootFrames": [["binaryName": "Other", "offsetIntoBinaryTextSegment": 1]]],
        ["threadAttributed": true, "callStackRootFrames": [[
          "binaryName": "MSIMEApp", "offsetIntoBinaryTextSegment": 4096, "address": 123,
          "subFrames": [["binaryName": "/private/var/containers/Bundle/Application/X/MSIMEApp.app/Frameworks/Engine", "offsetIntoBinaryTextSegment": 77,
                         "subFrames": [["binaryName": "libdyld.dylib", "offsetIntoBinaryTextSegment": 9]]]],
        ]]],
      ],
    ]
    let data = try JSONSerialization.data(withJSONObject: tree)
    XCTAssertEqual(UsageReporting.frames(fromCallStackTree: data),
                   "0 MSIMEApp + 4096\n1 Engine + 77\n2 libdyld.dylib + 9")
    XCTAssertEqual(UsageReporting.frames(fromCallStackTree: data, limit: 1), "0 MSIMEApp + 4096")
    XCTAssertEqual(UsageReporting.frames(fromCallStackTree: Data("[]".utf8)), "")
  }

  func testCrashDiagnosticIsWrittenAsAPrivateRecordWithOneSummaryLine() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    UsageReporting.storeCrashDiagnostic(message: "Crash signal 11\nsecond line", stack: "0 MSIMEApp + 1", in: directory)
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let files = try FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: nil)
    XCTAssertEqual(files.count, 1)
    XCTAssertEqual(files[0].pathExtension, "crash")
    XCTAssertNotNil(UUID(uuidString: files[0].deletingPathExtension().lastPathComponent))
    XCTAssertEqual(try String(contentsOf: files[0], encoding: .utf8), "Crash signal 11\n0 MSIMEApp + 1")
    let permissions = try FileManager.default.attributesOfItem(atPath: files[0].path)[.posixPermissions] as? NSNumber
    XCTAssertEqual(permissions?.intValue, 0o600)
  }

  func testUsageReportingIsOnUnlessTurnedOff() {
    XCTAssertTrue(UsageReporting.isEnabled(in: nil))
    XCTAssertTrue(UsageReporting.isEnabled(in: [:]))
    XCTAssertFalse(UsageReporting.isEnabled(in: ["usage_reporting": false]))
  }

  func testNoticeMarkdownKeepsSafeLinksAndNeverInterpretsHTML() {
    let text = AppNotices.attributed("**更新** <b>原样</b>\n[官网](https://msime.app) [坏](javascript:alert(1)) ![图](https://msime.app/a.png)")
    XCTAssertTrue(String(text.characters).contains("<b>原样</b>"))
    XCTAssertTrue(String(text.characters).contains("\n"))
    let links = text.runs.compactMap { run in run.link.map { (String(text[run.range].characters), $0.absoluteString) } }
    XCTAssertEqual(links.map(\.0), ["官网", "图"])
    XCTAssertEqual(links.map(\.1), ["https://msime.app", "https://msime.app/a.png"])
    XCTAssertTrue(text.runs.allSatisfy { $0.imageURL == nil })
  }
}
