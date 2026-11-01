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
    let contents = try String(contentsOf: record, encoding: .utf8)
    let lines = contents.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
    XCTAssertTrue(lines.first?.hasPrefix("MSIME-CRASH/1 ") == true, contents)
    XCTAssertEqual(Array(lines.dropFirst()), [
      "Crash exception 10 code 0 signal 6",
      "0 MSIMEKeyboardExtension + 1",
    ])
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

  /// 异常自带的返回地址写成与 MetricKit 相同的「序号 二进制 + 偏移」，偏移从映像起点算，同一份 dSYM 能符号化两者。
  func testReturnAddressesBecomeBinaryAndOffsetLines() throws {
    let addresses = Thread.callStackReturnAddresses
    var info = Dl_info()
    XCTAssertNotEqual(dladdr(#dsohandle, &info), 0)
    let binary = (String(cString: try XCTUnwrap(info.dli_fname)) as NSString).lastPathComponent
    let lines = UsageReporting.frames(fromReturnAddresses: addresses).split(separator: "\n").map(String.init)
    XCTAssertEqual(lines.count, min(addresses.count, 128))
    let base = UInt(bitPattern: try XCTUnwrap(info.dli_fbase))
    let index = try XCTUnwrap(lines.firstIndex { $0.contains(" \(binary) + ") }, "no frame from the test binary")
    XCTAssertEqual(lines[index], "\(index) \(binary) + \(addresses[index].uintValue - base)")
    XCTAssertEqual(UsageReporting.frames(fromReturnAddresses: [0, addresses[0]], limit: 1), "0 ??? + 0")
  }

  /// App 自己的未捕获异常：记录不属于任何会话、只带异常名；MetricKit 再送来同一进程的诊断时认出它，只认一次。
  func testUncaughtExceptionIsRecordedOnceAcrossItsMetricKitCopy() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let handled = directory.appendingPathComponent("uncaught-exception-pids")

    UsageReporting.recordUncaughtException(name: "NSInternalInconsistencyException", returnAddresses: [],
                                           pid: 4242, in: directory, handledProcesses: handled)
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let files = try FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: nil)
    XCTAssertEqual(files.count, 1)
    XCTAssertNotNil(UUID(uuidString: files[0].deletingPathExtension().lastPathComponent))
    XCTAssertEqual(try String(contentsOf: files[0], encoding: .utf8), "Uncaught NSInternalInconsistencyException\n")

    XCTAssertFalse(UsageReporting.takeHandledProcess(7, in: handled))
    XCTAssertTrue(UsageReporting.takeHandledProcess(4242, in: handled))
    XCTAssertFalse(UsageReporting.takeHandledProcess(4242, in: handled))
    XCTAssertFalse(FileManager.default.fileExists(atPath: handled.path))

    for pid in 1...40 {
      UsageReporting.recordUncaughtException(name: "Synthetic", returnAddresses: [], pid: pid_t(pid), in: directory,
                                             handledProcesses: handled)
    }
    XCTAssertFalse(UsageReporting.takeHandledProcess(999, in: handled))
    XCTAssertFalse(UsageReporting.takeHandledProcess(8, in: handled), "only the latest 32 are kept")
    XCTAssertTrue(UsageReporting.takeHandledProcess(9, in: handled))
  }

  func testFailedUncaughtExceptionWriteDoesNotSuppressMetricKit() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let blocked = root.appendingPathComponent("blocked")
    try Data("synthetic".utf8).write(to: blocked)
    let handled = root.appendingPathComponent("uncaught-exception-pids")
    UsageReporting.recordUncaughtException(name: "Synthetic", returnAddresses: [], pid: 4242,
                                           in: blocked, handledProcesses: handled)
    XCTAssertFalse(UsageReporting.takeHandledProcess(4242, in: handled))
    XCTAssertFalse(FileManager.default.fileExists(atPath: handled.path))
  }

  func testExceptionConsentFollowsChangesAndRejectsStaleStartupRead() {
    let consent = UsageReporting.ExceptionConsent()
    XCTAssertFalse(consent.isEnabled)
    XCTAssertTrue(consent.refresh { true })
    consent.update(false)
    XCTAssertFalse(consent.isEnabled)
    consent.update(true)
    XCTAssertTrue(consent.isEnabled)
    XCTAssertFalse(consent.refresh {
      consent.update(false)
      return true
    })
    XCTAssertFalse(consent.isEnabled)
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
