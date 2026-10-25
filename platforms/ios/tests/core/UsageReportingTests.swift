import Foundation
import XCTest

final class UsageReportingTests: XCTestCase {
  func testSharedClientRequestEnvelope() {
    var request: [String: Any]?
    let reply = ClientRequestEnvelope.call({ bytes, count in
      if let bytes {
        request = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: Int(count))) as? [String: Any]
      }
      return strdup("{\"ok\":true,\"value\":{\"ready\":true}}")
    }, ["kind": "synthetic"]) as? [String: Any]
    XCTAssertEqual(request?["kind"] as? String, "synthetic")
    XCTAssertEqual(reply?["ready"] as? Bool, true)
    XCTAssertNil(ClientRequestEnvelope.call({ _, _ in strdup("{\"ok\":false,\"value\":1}") }, [:]))
    XCTAssertNil(ClientRequestEnvelope.call({ _, _ in strdup("invalid") }, [:]))
  }

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

  func testCallStackTreeIgnoresNonIntegerOffsets() throws {
    let tree: [String: Any] = [
      "callStacks": [["callStackRootFrames": [[
        "binaryName": "MSIMEApp", "offsetIntoBinaryTextSegment": true,
        "subFrames": [["binaryName": "Engine", "offsetIntoBinaryTextSegment": 1.5]],
      ]]]],
    ]
    let data = try JSONSerialization.data(withJSONObject: tree)
    XCTAssertEqual(UsageReporting.frames(fromCallStackTree: data), "0 MSIMEApp + 0\n1 Engine + 0")
  }

  func testDiagnosticSummaryTellsSystemKillsFromCrashes() {
    let killed = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGKILL),
                                                  exceptionName: nil,
                                                  terminationReason: "Namespace RUNNINGBOARD, Code 0xdead10cc")
    XCTAssertEqual(killed, .init(message: "Killed exception 10 code 0 signal 9 Namespace RUNNINGBOARD, Code 0xdead10cc", killed: true))
    let crashed = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGABRT),
                                                   exceptionName: "NSInvalidArgumentException", terminationReason: "line\nbreak")
    XCTAssertEqual(crashed, .init(message: "Crash exception 10 code 0 signal 6 NSInvalidArgumentException line break", killed: false))
    XCTAssertFalse(UsageReporting.diagnosticSummary(exceptionType: nil, exceptionCode: nil, signal: nil,
                                                    exceptionName: nil, terminationReason: nil).killed)
  }

  /// 键盘进程崩溃的诊断由 App 事后收到：要记到这个进程当时未结束的会话名下（下次启动报 `session_crash`），同一次崩溃只记一份；被系统强制结束的不算会话崩溃。
  func testCrashDiagnosticOfAKeyboardProcessBelongsToItsOpenSessionOnce() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let session = try XCTUnwrap(UsageReporting.begin(in: directory))
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let crash = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGABRT),
                                                 exceptionName: nil, terminationReason: nil)

    UsageReporting.recordDiagnosedCrash(crash, stack: "0 MSIMEKeyboardExtension + 1", pid: getpid(), in: directory)
    let record = crashes.appendingPathComponent(session.id + ".crash")
    XCTAssertEqual(URL(fileURLWithPath: session.crashRecordPath).standardizedFileURL, record.standardizedFileURL)
    XCTAssertEqual(try String(contentsOf: record, encoding: .utf8),
                   "Crash exception 10 code 0 signal 6\n0 MSIMEKeyboardExtension + 1")
    // MetricKit 再送一次，或键盘自己的信号处理函数已经写过：保留第一份。
    UsageReporting.recordDiagnosedCrash(crash, stack: "0 other + 2", pid: getpid(), in: directory)
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: crashes.path), [session.id + ".crash"])

    let killed = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGKILL),
                                                  exceptionName: nil, terminationReason: nil)
    UsageReporting.recordDiagnosedCrash(killed, stack: "0 libsystem_kernel.dylib + 3", pid: getpid(), in: directory)
    let files = try FileManager.default.contentsOfDirectory(atPath: crashes.path).filter { $0 != session.id + ".crash" }
    XCTAssertEqual(files.count, 1)
    XCTAssertNotNil(UUID(uuidString: (files[0] as NSString).deletingPathExtension))
  }

  func testCrashDiagnosticOutsideASessionIsAPrivateRecordWithOneSummaryLine() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let summary = UsageReporting.DiagnosticSummary(message: "Crash signal 11\nsecond line", killed: false)
    // 没有会话的进程（App 自身）。
    UsageReporting.recordDiagnosedCrash(summary, stack: "0 MSIMEApp + 1", pid: getpid(), in: directory)
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let files = try FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: nil)
    XCTAssertEqual(files.count, 1)
    XCTAssertEqual(files[0].pathExtension, "crash")
    XCTAssertNotNil(UUID(uuidString: files[0].deletingPathExtension().lastPathComponent))
    XCTAssertEqual(try String(contentsOf: files[0], encoding: .utf8), "Crash signal 11\n0 MSIMEApp + 1")
    let permissions = try FileManager.default.attributesOfItem(atPath: files[0].path)[.posixPermissions] as? NSNumber
    XCTAssertEqual(permissions?.intValue, 0o600)
  }

  func testCrashDiagnosticDoesNotFollowCrashDirectorySymlink() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let outside = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer {
      try? FileManager.default.removeItem(at: root)
      try? FileManager.default.removeItem(at: outside)
    }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: outside, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(
      at: root.appendingPathComponent("telemetry-crashes"),
      withDestinationURL: outside
    )

    UsageReporting.recordDiagnosedCrash(.init(message: "Synthetic crash", killed: true), stack: "0 synthetic + 1",
                                        pid: getpid(), in: root)

    let outsideFiles = try FileManager.default.contentsOfDirectory(at: outside, includingPropertiesForKeys: nil)
    XCTAssertTrue(outsideFiles.isEmpty)
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
