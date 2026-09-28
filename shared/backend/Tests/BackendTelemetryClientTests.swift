import XCTest
@testable import MSIMEBackend

private final class TelemetryFlushProtocol: URLProtocol {
  static let started = DispatchSemaphore(value: 0)
  static let release = DispatchSemaphore(value: 0)
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    Self.started.signal()
    Self.release.wait()
    let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

private final class TelemetryOversizedResponseProtocol: URLProtocol {
  override class func canInit(with request: URLRequest) -> Bool { true }
  override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
  override func startLoading() {
    let body = Data(repeating: 0x41, count: 65 * 1024)
    let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                   headerFields: nil)!
    client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
    client?.urlProtocol(self, didLoad: body)
    client?.urlProtocolDidFinishLoading(self)
  }
  override func stopLoading() {}
}

final class BackendTelemetryClientTests: XCTestCase {
  func testCrashIsBoundedAndPersistedSynchronously() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-test-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }

    BackendTelemetryClient.persistCrash(message: String(repeating: "m", count: 3000),
                                        stack: String(repeating: "s", count: 20_000), queueURL: queue)
    let data = try Data(contentsOf: queue)
    let events = try JSONDecoder().decode([BackendTelemetryEvent].self, from: data)
    XCTAssertEqual(events.count, 1)
    XCTAssertEqual(events[0].message?.count, 2048)
    XCTAssertEqual(events[0].stack?.count, 12_000)
  }

  func testManyLargeCrashesStayWithinBoundAndKeepNewest() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-test-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }

    for index in 0..<10 {
      BackendTelemetryClient.persistCrash(message: "crash \(index)", stack: String(repeating: "frame\n", count: 2_000), queueURL: queue)
    }
    let data = try Data(contentsOf: queue)
    XCTAssertLessThanOrEqual(data.count, BackendTelemetryClient.maxPayloadBytes)
    let events = try JSONDecoder().decode([BackendTelemetryEvent].self, from: data)
    XCTAssertGreaterThan(events.count, 1)
    XCTAssertEqual(events.last?.message, "crash 9")
    XCTAssertEqual(BackendTelemetryClient.readQueue(queue).last?.message, "crash 9")
  }

  func testOversizedSingleEventHasStackShortened() throws {
    // Control characters escape to \uXXXX, so 12 000 of them exceed 64 KiB on their own.
    let event = BackendTelemetryEvent(kind: "crash", platform: "test", version: "1", message: "big",
                                      stack: String(repeating: "\u{1}", count: 12_000))
    let data = try XCTUnwrap(BackendTelemetryClient.boundedEncode([event]))
    XCTAssertLessThanOrEqual(data.count, BackendTelemetryClient.maxPayloadBytes)
    let events = try JSONDecoder().decode([BackendTelemetryEvent].self, from: data)
    XCTAssertEqual(events.map(\.message), ["big"])
    XCTAssertLessThan(events[0].stack?.count ?? 0, 12_000)
  }

  func testQueueOverPayloadBoundIsReadNotDiscarded() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-test-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let events = (0..<10).map { BackendTelemetryEvent(kind: "crash", platform: "test", version: "1", message: "old \($0)",
                                                       stack: String(repeating: "s", count: 12_000)) }
    try JSONEncoder().encode(events).write(to: queue)
    BackendTelemetryClient.persistCrash(message: "new", queueURL: queue)
    let kept = try JSONDecoder().decode([BackendTelemetryEvent].self, from: Data(contentsOf: queue))
    XCTAssertEqual(kept.last?.message, "new")
    XCTAssertEqual(kept.dropLast().last?.message, "old 9")
  }

  func testQueueReaderRejectsAFileThatGrowsPastTheLimit() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-limit-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try Data(repeating: 0x41, count: BackendTelemetryClient.maxQueueFileBytes + 1).write(to: queue)
    XCTAssertNil(BackendTelemetryClient.readQueueBytes(queue))
  }

  func testCrashPersistedDuringFlushSurvivesSuccessfulUpload() async throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-test-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let original = BackendTelemetryEvent(kind: "download", platform: "test", version: "1")
    try JSONEncoder().encode([original]).write(to: queue)
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [TelemetryFlushProtocol.self]
    let client = BackendTelemetryClient(configuration: configuration, queueURL: queue)
    let flushing = Task { await client.flush() }
    XCTAssertEqual(TelemetryFlushProtocol.started.wait(timeout: .now() + 2), .success)
    BackendTelemetryClient.persistCrash(message: "during flush", queueURL: queue)
    TelemetryFlushProtocol.release.signal()
    await flushing.value
    let events = try JSONDecoder().decode([BackendTelemetryEvent].self, from: Data(contentsOf: queue))
    XCTAssertEqual(events.map(\.message), ["during flush"])
  }

  func testOversizedResponseKeepsEventQueued() async throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent("msime-telemetry-test-\(UUID().uuidString)")
    let queue = directory.appendingPathComponent("events.json")
    defer { try? FileManager.default.removeItem(at: directory) }
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    let original = BackendTelemetryEvent(kind: "download", platform: "test", version: "1")
    try JSONEncoder().encode([original]).write(to: queue)
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [TelemetryOversizedResponseProtocol.self]
    let client = BackendTelemetryClient(configuration: configuration, queueURL: queue)

    await client.flush()

    let events = try JSONDecoder().decode([BackendTelemetryEvent].self, from: Data(contentsOf: queue))
    XCTAssertEqual(events, [original])
  }
}
