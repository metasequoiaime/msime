import Foundation
import XCTest
import os

final class UsageReportingTests: XCTestCase {
  private static func syntheticAuthorization() -> UsageReporting.Authorization {
    .init(epoch: UUID(), sinceUnixMilliseconds: 1000, preferencesDirectory: "/synthetic/preferences")
  }

  private func authorization(in directory: URL) throws -> UsageReporting.Authorization {
    let preferences = directory.appendingPathComponent("preferences")
    guard case let .enabled(authorization)? = UsageReporting.captureConsent(preferencesDirectory: preferences.path) else {
      throw NSError(domain: "SyntheticConsent", code: 1)
    }
    // 留出毫秒转换余量，避免测试本身把刚保存的起点舍入到上一毫秒。
    Thread.sleep(forTimeInterval: 0.005)
    return authorization
  }

  func testDelayedDiagnosticsRequireContinuousHistoricalConsent() {
    let first = Date(timeIntervalSince1970: 1000)
    let later = Date(timeIntervalSince1970: 2000)
    XCTAssertTrue(UsageReporting.diagnosticAllowed(begin: first, end: later, consentSince: first, now: later))
    XCTAssertFalse(UsageReporting.diagnosticAllowed(begin: first, end: later, consentSince: later, now: later))
    XCTAssertFalse(UsageReporting.diagnosticAllowed(begin: first, end: later, consentSince: nil, now: later))
    XCTAssertFalse(UsageReporting.diagnosticAllowed(begin: later, end: first, consentSince: first, now: later))
    XCTAssertFalse(UsageReporting.diagnosticAllowed(begin: first, end: later, consentSince: first, now: first))
  }

  func testCaptureConsentRequiresSharedPersistentEpochAndStrictTypes() {
    let epoch = UUID().uuidString.lowercased()
    let valid: [String: Any] = ["enabled": true, "epoch": epoch, "since_unix_ms": 1000]
    let path = "/synthetic/preferences"
    let expected = UsageReporting.CaptureConsent.enabled(.init(epoch: UUID(uuidString: epoch)!,
      sinceUnixMilliseconds: 1000, preferencesDirectory: path))
    XCTAssertEqual(UsageReporting.captureConsent(from: valid, preferencesDirectory: path), expected)
    XCTAssertEqual(UsageReporting.captureConsent(from: ["enabled": false], preferencesDirectory: path), .disabled)
    XCTAssertNil(UsageReporting.captureConsent(from: valid, preferencesDirectory: "relative"))
    XCTAssertNil(UsageReporting.captureConsent(from: nil, preferencesDirectory: path))
    for value: Any in [1, "true", NSNull()] {
      var invalid = valid
      invalid["enabled"] = value
      XCTAssertNil(UsageReporting.captureConsent(from: invalid, preferencesDirectory: path))
    }
    for value: Any in [true, -1, 0, 1.5, "1000", 253_402_300_800_000 as UInt64] {
      var invalid = valid
      invalid["since_unix_ms"] = value
      XCTAssertNil(UsageReporting.captureConsent(from: invalid, preferencesDirectory: path))
    }
    for value: Any in [NSNull(), "00000000-0000-0000-0000-000000000000", "12345678-1234-4123-7123-123456789012"] {
      var invalid = valid
      invalid["epoch"] = value
      XCTAssertNil(UsageReporting.captureConsent(from: invalid, preferencesDirectory: path))
    }
    // 旧本地历史即使很早也不构成共享授权；缺少起点或代次都拒绝。
    for key in ["epoch", "since_unix_ms"] {
      var invalid = valid
      invalid.removeValue(forKey: key)
      XCTAssertNil(UsageReporting.captureConsent(from: invalid, preferencesDirectory: path))
    }
  }

  func testDiagnosticIdentityIsStableAndSeparatesHangsFromCrashes() {
    let data = Data("synthetic diagnostic".utf8)
    let id = UsageReporting.diagnosticID(kind: "crash", data: data)
    XCTAssertEqual(id, UsageReporting.diagnosticID(kind: "crash", data: data))
    XCTAssertNotEqual(id, UsageReporting.diagnosticID(kind: "hang", data: data))
    XCTAssertNotEqual(id, UsageReporting.diagnosticID(kind: "crash", data: Data("other".utf8)))
    XCTAssertFalse(id.contains("synthetic"))
  }

  func testDiagnosticHeaderKeepsSourceVersionAndRejectsPrivatePaths() throws {
    let header = try XCTUnwrap(UsageReporting.diagnosticHeader(version: "0.52.0"))
    let json = String(header.dropFirst("MSIME-CRASH/1 ".count))
    let context = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: String])
    XCTAssertEqual(context["version"], "0.52.0")
    XCTAssertEqual(context["revision"], "unknown")
    XCTAssertNil(UsageReporting.diagnosticHeader(version: "/private/synthetic"))
    XCTAssertNil(UsageReporting.diagnosticHeader(version: "0.1.0\nprivate"))
  }

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
    let authorization = try authorization(in: directory)
    let date = Date()
    let session = try XCTUnwrap(UsageReporting.begin(in: directory, authorization: authorization))
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let crash = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGABRT),
                                                 exceptionName: nil, terminationReason: nil)

    UsageReporting.recordDiagnosedCrash(crash, stack: "0 MSIMEKeyboardExtension + 1", pid: getpid(), in: directory,
                                        authorization: authorization, begin: date, end: date)
    let record = crashes.appendingPathComponent(session.id + ".crash")
    XCTAssertEqual(URL(fileURLWithPath: session.crashRecordPath).standardizedFileURL, record.standardizedFileURL)
    let contents = try String(contentsOf: record, encoding: .utf8)
    let lines = contents.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
    XCTAssertTrue(lines.first == session.crashRecordHeader.trimmingCharacters(in: .newlines), contents)
    XCTAssertEqual(Array(lines.dropFirst()), [
      "Crash exception 10 code 0 signal 6",
      "0 MSIMEKeyboardExtension + 1",
    ])
    // MetricKit 再送一次，或键盘自己的信号处理函数已经写过：保留第一份。
    UsageReporting.recordDiagnosedCrash(crash, stack: "0 other + 2", pid: getpid(), in: directory,
                                        authorization: authorization, begin: date, end: date)
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: crashes.path), [session.id + ".crash"])

    let killed = UsageReporting.diagnosticSummary(exceptionType: 10, exceptionCode: 0, signal: NSNumber(value: SIGKILL),
                                                  exceptionName: nil, terminationReason: nil)
    UsageReporting.recordDiagnosedCrash(killed, stack: "0 libsystem_kernel.dylib + 3", pid: getpid(), in: directory,
                                        authorization: authorization, begin: date, end: date)
    let files = try FileManager.default.contentsOfDirectory(atPath: crashes.path).filter { $0 != session.id + ".crash" }
    XCTAssertEqual(files.count, 1)
    XCTAssertNotNil(UUID(uuidString: (files[0] as NSString).deletingPathExtension))
  }

  func testCrashDiagnosticOutsideASessionIsAPrivateRecordWithOneSummaryLine() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let authorization = try authorization(in: directory)
    let date = Date()
    let summary = UsageReporting.DiagnosticSummary(message: "Crash signal 11\nsecond line", killed: false)
    // 没有会话的进程（App 自身）。
    UsageReporting.recordDiagnosedCrash(summary, stack: "0 MSIMEApp + 1", pid: getpid(), in: directory,
                                        authorization: authorization, begin: date, end: date)
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let files = try FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: nil)
    XCTAssertEqual(files.count, 1)
    XCTAssertEqual(files[0].pathExtension, "crash")
    XCTAssertNotNil(UUID(uuidString: files[0].deletingPathExtension().lastPathComponent))
    let record = try String(contentsOf: files[0], encoding: .utf8)
    XCTAssertTrue(record.hasSuffix("\nCrash signal 11\n0 MSIMEApp + 1"))
    let header = try XCTUnwrap(record.split(separator: "\n").first)
    let context = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(header.dropFirst("MSIME-CRASH/1 ".count).utf8)) as? [String: String])
    XCTAssertEqual(context["consent_epoch"], authorization.epoch.uuidString.lowercased())
    XCTAssertEqual(context["version"], "unknown")
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
    let header = try XCTUnwrap(UsageReporting.diagnosticHeader(version: "0.0.1"))

    let capture = UsageReporting.Session(id: UUID().uuidString.lowercased(), crashRecordPath: "", crashRecordHeader: header)
    UsageReporting.recordUncaughtException(name: "NSInternalInconsistencyException", returnAddresses: [],
                                           pid: 4242, in: directory, handledProcesses: handled, capture: capture)
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let files = try FileManager.default.contentsOfDirectory(at: crashes, includingPropertiesForKeys: nil)
    XCTAssertEqual(files.count, 1)
    XCTAssertNotNil(UUID(uuidString: files[0].deletingPathExtension().lastPathComponent))
    XCTAssertEqual(files[0].deletingPathExtension().lastPathComponent, capture.id)
    let record = try String(contentsOf: files[0], encoding: .utf8)
    XCTAssertTrue(record.hasSuffix("\nUncaught NSInternalInconsistencyException\n"))
    let storedHeader = try XCTUnwrap(record.split(separator: "\n").first)
    let prefix = "MSIME-CRASH/1 "
    XCTAssertTrue(storedHeader.hasPrefix(prefix))
    let storedContext = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(storedHeader.dropFirst(prefix.count).utf8)) as? [String: String])
    let expectedContext = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(header.dropFirst(prefix.count).utf8)) as? [String: String])
    XCTAssertEqual(storedContext, expectedContext)

    XCTAssertFalse(UsageReporting.takeHandledProcess(7, in: handled))
    XCTAssertTrue(UsageReporting.takeHandledProcess(4242, in: handled))
    XCTAssertFalse(UsageReporting.takeHandledProcess(4242, in: handled))
    XCTAssertFalse(FileManager.default.fileExists(atPath: handled.path))

    for pid in 1...40 {
      UsageReporting.recordUncaughtException(name: "Synthetic", returnAddresses: [], pid: pid_t(pid), in: directory,
                                             handledProcesses: handled,
                                             capture: .init(id: UUID().uuidString, crashRecordPath: "", crashRecordHeader: header))
    }
    XCTAssertFalse(UsageReporting.takeHandledProcess(999, in: handled))
    XCTAssertFalse(UsageReporting.takeHandledProcess(8, in: handled), "only the latest 32 are kept")
    XCTAssertTrue(UsageReporting.takeHandledProcess(9, in: handled))
  }

  /// App 的信号处理函数按捕获 UUID 写过现场记录：MetricKit 事后送来同一进程的诊断时按记下的身份补录，只留一份；现场没写成时由诊断补上同一身份。
  func testApplicationSignalRecordAndItsMetricKitCopyAreOneCrash() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let authorization = try authorization(in: directory)
    let identities = directory.appendingPathComponent("application-captures.json")
    let crashes = directory.appendingPathComponent("telemetry-crashes")
    let date = Date()
    let summary = UsageReporting.diagnosticSummary(exceptionType: 6, exceptionCode: 1, signal: NSNumber(value: SIGTRAP),
                                                   exceptionName: nil, terminationReason: nil)

    let capture = try XCTUnwrap(UsageReporting.prepareApplicationCapture(in: directory, authorization: authorization))
    UsageReporting.rememberApplicationCapture(capture, pid: 4242, in: identities)
    let signalRecord = capture.crashRecordHeader + "Fatal signal SIGTRAP\n<unknown>+0x1\n"
    try FileManager.default.createDirectory(at: crashes, withIntermediateDirectories: true)
    try Data(signalRecord.utf8).write(to: URL(fileURLWithPath: capture.crashRecordPath))

    XCTAssertNil(UsageReporting.takeApplicationCapture(7, in: identities))
    let remembered = try XCTUnwrap(UsageReporting.takeApplicationCapture(4242, in: identities))
    XCTAssertEqual(remembered.id, capture.id)
    XCTAssertEqual(remembered.crashRecordHeader, capture.crashRecordHeader)
    XCTAssertNil(UsageReporting.takeApplicationCapture(4242, in: identities))
    UsageReporting.recordDiagnosedCapture(summary, stack: "0 MSIMEApp + 1", capture: remembered, in: directory,
                                          authorization: authorization, begin: date, end: date)
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: crashes.path), [capture.id + ".crash"])
    XCTAssertEqual(try String(contentsOf: URL(fileURLWithPath: capture.crashRecordPath), encoding: .utf8), signalRecord)

    // 下一次准备会把现场记录排成事件；MetricKit 之后再送来同一进程的诊断也不再写第二份。
    let unwritten = try XCTUnwrap(UsageReporting.prepareApplicationCapture(in: directory, authorization: authorization))
    UsageReporting.recordDiagnosedCapture(summary, stack: "0 MSIMEApp + 1", capture: remembered, in: directory,
                                          authorization: authorization, begin: date, end: date)
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: crashes.path).isEmpty)
    let queue = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: directory.appendingPathComponent("telemetry.json"))) as? [[String: Any]])
    let queued = queue.filter { $0["kind"] as? String == "crash" }
    XCTAssertEqual(queued.count, 1)
    XCTAssertEqual((queued.first?["message"] as? String)?.hasPrefix("Fatal signal SIGTRAP"), true)

    // 信号处理函数没写成时，诊断按同一身份补上。
    UsageReporting.rememberApplicationCapture(unwritten, pid: 4343, in: identities)
    let fallback = try XCTUnwrap(UsageReporting.takeApplicationCapture(4343, in: identities))
    UsageReporting.recordDiagnosedCapture(summary, stack: "0 MSIMEApp + 2", capture: fallback, in: directory,
                                          authorization: authorization, begin: date, end: date)
    XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath: crashes.path), [unwritten.id + ".crash"])
    XCTAssertEqual(try String(contentsOf: URL(fileURLWithPath: unwritten.crashRecordPath), encoding: .utf8),
                   unwritten.crashRecordHeader + summary.message + "\n0 MSIMEApp + 2")
    XCTAssertFalse(FileManager.default.fileExists(atPath: identities.path))

    for pid in 1...40 {
      UsageReporting.rememberApplicationCapture(.init(id: UUID().uuidString.lowercased(), crashRecordPath: "",
                                                      crashRecordHeader: capture.crashRecordHeader),
                                                pid: pid_t(pid), in: identities)
    }
    XCTAssertNil(UsageReporting.takeApplicationCapture(8, in: identities), "only the latest 32 are kept")
    XCTAssertNotNil(UsageReporting.takeApplicationCapture(9, in: identities))
  }

  func testFailedUncaughtExceptionWriteDoesNotSuppressMetricKit() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let blocked = root.appendingPathComponent("blocked")
    try Data("synthetic".utf8).write(to: blocked)
    let handled = root.appendingPathComponent("uncaught-exception-pids")
    let header = try XCTUnwrap(UsageReporting.diagnosticHeader(version: "0.0.1"))
    UsageReporting.recordUncaughtException(name: "Synthetic", returnAddresses: [], pid: 4242,
                                           in: blocked, handledProcesses: handled,
                                           capture: .init(id: UUID().uuidString, crashRecordPath: "", crashRecordHeader: header))
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

    let authorization = try authorization(in: root)
    let date = Date()
    UsageReporting.recordDiagnosedCrash(.init(message: "Synthetic crash", killed: true), stack: "0 synthetic + 1",
                                        pid: getpid(), in: root,
                                        authorization: authorization, begin: date, end: date)

    let outsideFiles = try FileManager.default.contentsOfDirectory(at: outside, includingPropertiesForKeys: nil)
    XCTAssertTrue(outsideFiles.isEmpty)
  }

  func testUsageReportingIsOnUnlessTurnedOff() {
    XCTAssertFalse(UsageReporting.isEnabled(in: nil))
    XCTAssertTrue(UsageReporting.isEnabled(in: [:]))
    XCTAssertTrue(UsageReporting.isEnabled(in: ["usage_reporting": true]))
    XCTAssertFalse(UsageReporting.isEnabled(in: ["usage_reporting": false]))
    for value: Any in [NSNumber(value: 1), NSNumber(value: 0), "true", NSNull()] {
      XCTAssertNil(UsageReporting.consent(in: ["usage_reporting": value]))
      XCTAssertFalse(UsageReporting.isEnabled(in: ["usage_reporting": value]))
    }
  }

  func testSharedPreferenceReadFailureDoesNotBecomeDefaultConsent() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let defaults = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root))
    XCTAssertTrue(UsageReporting.isEnabled(in: defaults), "缺失文档由共享层提供产品默认值")
    try Data("{invalid synthetic document".utf8).write(to: root.appendingPathComponent("preferences.json"))
    let unreadable = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root)
    XCTAssertNil(unreadable)
    XCTAssertNil(UsageReporting.consent(in: unreadable))
    XCTAssertFalse(UsageReporting.isEnabled(in: unreadable))
  }

  func testMissingPreferenceDirectoryDoesNotClearExistingCrash() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let crashes = root.appendingPathComponent("telemetry-crashes")
    try FileManager.default.createDirectory(at: crashes, withIntermediateDirectories: true)
    let record = crashes.appendingPathComponent(UUID().uuidString.lowercased() + ".crash")
    let original = Data("Synthetic signal 11\n0 synthetic + 1".utf8)
    try original.write(to: record)
    XCTAssertNil(UsageReporting.begin(in: root, preferencesDirectory: ""))
    XCTAssertNil(UsageReporting.begin(in: root, preferencesDirectory: "relative/synthetic"))
    XCTAssertEqual(try Data(contentsOf: record), original)
  }

  private final class CaptureFixture {
    struct State {
      var authorization = UsageReportingTests.syntheticAuthorization()
      var consent: UsageReporting.CaptureConsent?
      init() { consent = .enabled(authorization) }
      var prepared = 0
      var cleared = 0
      var configured: UsageReporting.Session?
      var publishes = 0
      var rejectPublish = false
      var moduleRefreshes = 0
      var ended: [UsageReporting.Session] = []
    }
    let state = OSAllocatedUnfairLock(initialState: State())
    lazy var capture = UsageReporting.CaptureLifecycle(
      load: { self.state.withLock { $0.consent } },
      prepare: { _ in
        self.state.withLock {
          $0.prepared += 1
          return .init(id: "synthetic-\($0.prepared)", crashRecordPath: "/synthetic/\($0.prepared).crash",
                       crashRecordHeader: "synthetic-header-\($0.prepared)")
        }
      },
      configure: { session in
        self.state.withLock {
          $0.publishes += 1
          guard !$0.rejectPublish else { return false }
          $0.configured = session
          return true
        }
      },
      refreshModules: { self.state.withLock { $0.moduleRefreshes += 1 } },
      clear: { self.state.withLock { $0.cleared += 1 } },
      finish: { session in self.state.withLock { $0.ended.append(session) } })

    func refresh(_ consent: Bool?) {
      state.withLock {
        if consent == true {
          if $0.consent == .disabled { $0.authorization = UsageReportingTests.syntheticAuthorization() }
          $0.consent = .enabled($0.authorization)
        } else {
          $0.consent = consent == false ? .disabled : nil
        }
      }
      capture.refresh()
      capture.waitForPendingOperations()
    }

    func stop() { capture.stop(); capture.waitForPendingOperations() }
  }

  func testActiveCapturePausesOnReadFailureAndRenewsAfterWithdrawal() throws {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.capture.start()
    fixture.refresh(true)
    let first = try XCTUnwrap(fixture.capture.current)
    fixture.capture.start()
    fixture.refresh(true)
    XCTAssertEqual(fixture.state.withLock { $0.prepared }, 1)
    fixture.refresh(nil)
    XCTAssertNil(fixture.capture.current)
    XCTAssertNil(fixture.state.withLock { $0.configured })
    XCTAssertEqual(fixture.state.withLock { $0.cleared }, 0)
    fixture.refresh(true)
    XCTAssertEqual(fixture.capture.current, first)
    fixture.refresh(false)
    fixture.refresh(false)
    XCTAssertNil(fixture.capture.current)
    XCTAssertEqual(fixture.state.withLock { $0.cleared }, 1)
    fixture.refresh(true)
    let second = try XCTUnwrap(fixture.capture.current)
    XCTAssertNotEqual(first, second)
    XCTAssertNotEqual(first.crashRecordHeader, second.crashRecordHeader)
    fixture.stop()
    XCTAssertEqual(fixture.state.withLock { $0.ended }, [second])
    fixture.refresh(true)
    XCTAssertNil(fixture.capture.current)
    XCTAssertEqual(fixture.state.withLock { $0.prepared }, 2)
  }

  /// 每次发布都会短暂撤下信号记录并重新遍历映像：同一份快照只发布一次，变化或上次发布失败时才再发布；没变时每拍只让记录器补登记之后加载的映像。
  func testUnchangedCaptureIsPublishedOnce() throws {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.capture.start()
    fixture.refresh(true)
    let first = try XCTUnwrap(fixture.capture.current)
    let refreshes = fixture.state.withLock { $0.moduleRefreshes }
    for _ in 0..<3 { fixture.refresh(true) }
    XCTAssertEqual(fixture.state.withLock { $0.publishes }, 1)
    XCTAssertEqual(fixture.state.withLock { $0.configured }, first)
    XCTAssertGreaterThanOrEqual(fixture.state.withLock { $0.moduleRefreshes }, refreshes + 3)

    // 读失败撤下，恢复后同一身份再发布一次；撤下期间没有记录路径，不补登记映像。
    fixture.refresh(nil)
    let paused = fixture.state.withLock { $0.moduleRefreshes }
    fixture.refresh(nil)
    XCTAssertEqual(fixture.state.withLock { $0.publishes }, 2)
    XCTAssertEqual(fixture.state.withLock { $0.moduleRefreshes }, paused)
    fixture.refresh(true)
    fixture.refresh(true)
    XCTAssertEqual(fixture.state.withLock { $0.publishes }, 3)
    XCTAssertEqual(fixture.state.withLock { $0.configured }, first)

    // 撤回后重开换新身份，发布新的那份。
    fixture.capture.revoke()
    fixture.capture.waitForPendingOperations()
    fixture.refresh(true)
    let second = try XCTUnwrap(fixture.capture.current)
    XCTAssertNotEqual(first, second)
    XCTAssertEqual(fixture.state.withLock { $0.configured }, second)
    let afterRenewal = fixture.state.withLock { $0.publishes }
    fixture.refresh(true)
    XCTAssertEqual(fixture.state.withLock { $0.publishes }, afterRenewal)
  }

  /// 配置锁被故障线程占用时记录器没有接受发布：下一拍必须重试，不能停在撤下状态。
  func testRejectedPublishIsRetried() throws {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.state.withLock { $0.rejectPublish = true }
    fixture.capture.start()
    fixture.refresh(true)
    let session = try XCTUnwrap(fixture.capture.current)
    XCTAssertNil(fixture.state.withLock { $0.configured })
    let attempts = fixture.state.withLock { $0.publishes }
    fixture.refresh(true)
    XCTAssertGreaterThan(fixture.state.withLock { $0.publishes }, attempts)
    fixture.state.withLock { $0.rejectPublish = false }
    fixture.refresh(true)
    XCTAssertEqual(fixture.state.withLock { $0.configured }, session)
    let published = fixture.state.withLock { $0.publishes }
    fixture.refresh(true)
    XCTAssertEqual(fixture.state.withLock { $0.publishes }, published)
  }

  func testEpochChangeWithoutObservedWithdrawalRenewsCapture() throws {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.capture.start()
    fixture.refresh(true)
    let first = try XCTUnwrap(fixture.capture.current)
    fixture.state.withLock {
      $0.authorization = Self.syntheticAuthorization()
      $0.consent = .enabled($0.authorization)
    }
    fixture.refresh(true)
    let second = try XCTUnwrap(fixture.capture.current)
    XCTAssertNotEqual(first, second)
    XCTAssertEqual(fixture.state.withLock { $0.cleared }, 0)
    XCTAssertTrue(fixture.state.withLock { $0.ended.isEmpty })
    fixture.refresh(nil)
    fixture.refresh(true)
    XCTAssertEqual(fixture.capture.current, second)
  }

  func testEpochChangeDuringPreparationDoesNotPublishOldCapture() throws {
    let first = Self.syntheticAuthorization()
    let second = Self.syntheticAuthorization()
    let consent = OSAllocatedUnfairLock(initialState: first)
    let prepared = OSAllocatedUnfairLock(initialState: 0)
    let capture = UsageReporting.CaptureLifecycle(load: { .enabled(consent.withLock { $0 }) }, prepare: { authorization in
      prepared.withLock { $0 += 1 }
      consent.withLock { $0 = second }
      return .init(id: authorization.epoch.uuidString, crashRecordPath: "/synthetic/record.crash")
    }, configure: { _ in true }, clear: {})
    defer { capture.stop(); capture.waitForPendingOperations() }
    capture.start()
    capture.waitForPendingOperations()
    XCTAssertNil(capture.current)
    capture.refresh()
    capture.waitForPendingOperations()
    XCTAssertEqual(capture.current?.id, second.epoch.uuidString)
    XCTAssertEqual(prepared.withLock { $0 }, 2)
  }

  func testPersistentWithdrawalRejectsOldBeginAndDelayedDiagnostic() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let first = try authorization(in: directory)
    let date = Date()
    let old = try XCTUnwrap(UsageReporting.begin(in: directory, authorization: first))
    let preferences = URL(fileURLWithPath: first.preferencesDirectory)
    for enabled in [false, true] {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: preferences) {
        $0[UsageReporting.preferenceKey] = enabled
      })
    }
    let second = try authorization(in: directory)
    XCTAssertNotEqual(first.epoch, second.epoch)
    let current = try XCTUnwrap(UsageReporting.begin(in: directory, authorization: second))
    XCTAssertNil(UsageReporting.begin(in: directory, authorization: first))
    // 旧撤回回调不能清除新授权队列，旧原生进程可以晚写，但不能被补录。
    UsageReporting.clear(in: directory, preferencesDirectory: first.preferencesDirectory)
    try Data((old.crashRecordHeader + "Synthetic old fault\n").utf8).write(to: URL(fileURLWithPath: old.crashRecordPath))
    UsageReporting.recordDiagnosedCrash(.init(message: "Synthetic delayed fault", killed: true), stack: "",
      pid: 0, in: directory, authorization: first, begin: date, end: date)
    let next = try XCTUnwrap(UsageReporting.begin(in: directory, authorization: second))
    XCTAssertNotEqual(current.id, next.id)
    XCTAssertFalse(FileManager.default.fileExists(atPath: old.crashRecordPath))
    XCTAssertTrue(try FileManager.default.contentsOfDirectory(atPath: directory.appendingPathComponent("telemetry-crashes").path).isEmpty)
    let queue = try String(contentsOf: directory.appendingPathComponent("telemetry.json"), encoding: .utf8)
    XCTAssertFalse(queue.contains("Synthetic"))
    XCTAssertFalse(queue.contains("consent_epoch"))
  }

  func testQueuedDismissDoesNotEndNextPresentation() throws {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.capture.start()
    fixture.refresh(true)
    let first = try XCTUnwrap(fixture.capture.current)
    fixture.capture.stop()
    fixture.capture.start()
    fixture.refresh(true)
    let second = try XCTUnwrap(fixture.capture.current)
    XCTAssertNotEqual(first, second)
    XCTAssertEqual(fixture.state.withLock { $0.ended }, [first])
    XCTAssertEqual(fixture.state.withLock { $0.configured }, second)
  }

  func testWithdrawalDuringSlowPreparationCannotRearmOldCapture() {
    let entered = DispatchSemaphore(value: 0)
    let proceed = DispatchSemaphore(value: 0)
    let configured = OSAllocatedUnfairLock<UsageReporting.Session?>(initialState: nil)
    let authorization = Self.syntheticAuthorization()
    let capture = UsageReporting.CaptureLifecycle(load: { .enabled(authorization) }, prepare: { _ in
      entered.signal()
      _ = proceed.wait(timeout: .now() + 5)
      return .init(id: "old", crashRecordPath: "/synthetic/old.crash")
    }, configure: { session in configured.withLock { $0 = session }; return true }, clear: {})
    capture.start()
    XCTAssertEqual(entered.wait(timeout: .now() + 5), .success)
    capture.revoke()
    XCTAssertNil(capture.current)
    XCTAssertNil(configured.withLock { $0 })
    // 撤回后随即正常收起，旧的 prepare 即使返回成功也不能重新武装。
    capture.stop()
    proceed.signal()
    capture.waitForPendingOperations()
    XCTAssertNil(capture.current)
    XCTAssertNil(configured.withLock { $0 })
  }

  func testCaptureReadyPrecedesTransportAndDismissDoesNotWaitForIt() throws {
    let fixture = CaptureFixture()
    let transport = DispatchQueue(label: "synthetic.blocked-transport")
    let entered = DispatchSemaphore(value: 0)
    let release = DispatchSemaphore(value: 0)
    defer { release.signal(); fixture.stop() }
    fixture.capture.start {
      XCTAssertNotNil(fixture.capture.current, "先准备并补录，再投递发送任务")
      transport.async {
        entered.signal()
        _ = release.wait(timeout: .now() + 5)
      }
    }
    XCTAssertEqual(entered.wait(timeout: .now() + 5), .success)
    let session = try XCTUnwrap(fixture.capture.current)
    fixture.stop()
    XCTAssertNil(fixture.capture.current)
    XCTAssertEqual(fixture.state.withLock { $0.ended }, [session])
  }

  func testActiveCaptureObservesChangesWithoutNewPresentation() {
    let fixture = CaptureFixture()
    defer { fixture.stop() }
    fixture.capture.start()
    fixture.refresh(true)
    fixture.state.withLock { $0.consent = .disabled }
    let observed = expectation(description: "active observer sees withdrawal")
    DispatchQueue.global(qos: .utility).async {
      let deadline = Date().addingTimeInterval(3)
      while Date() < deadline {
        if fixture.state.withLock({ $0.cleared }) == 1 { observed.fulfill(); return }
        Thread.sleep(forTimeInterval: 0.01)
      }
    }
    wait(for: [observed], timeout: 4)
    XCTAssertNil(fixture.capture.current)
    XCTAssertNil(fixture.state.withLock { $0.configured })
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
