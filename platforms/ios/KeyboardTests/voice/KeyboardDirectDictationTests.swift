import XCTest

/// 键盘自己录音的听写（`KeyboardDirectDictation`）：麦克风和识别服务都换成合成的，看它向键盘报的事件、交给服务的录音，以及什么时候退回 App 录音。
@MainActor
final class KeyboardDirectDictationTests: XCTestCase {
  private final class SyntheticMicrophone: KeyboardAudioInput {
    var onStarted: (() -> Void)?
    var onAudio: ((Data) -> Void)?
    var onStopped: ((_ reason: String, _ denied: Bool) -> Void)?
    private(set) var starts = 0
    private(set) var stops = 0
    func start() { starts += 1 }
    func stop() { stops += 1 }
  }

  private let configuration = KeyboardVoiceConfiguration(
    endpoint: "https://asr.example/v1/audio/transcriptions", model: "whisper-1", chatAudio: false,
    language: "zh", startSound: false, endSound: false)

  /// `count` 个取值都是 `value` 的 16 位小端样本。
  private func samples(_ count: Int, value: Int16 = 1_000) -> Data {
    var data = Data()
    for _ in 0..<count { withUnsafeBytes(of: value.littleEndian) { data.append(contentsOf: $0) } }
    return data
  }

  private func record(_ dictation: KeyboardDirectDictation, until done: XCTestExpectation? = nil)
    -> () -> [KeyboardVoiceDictation.Event] {
    var events: [KeyboardVoiceDictation.Event] = []
    dictation.onEvent = { event in
      events.append(event)
      switch event {
      case .finished, .failed: done?.fulfill()
      default: break
      }
    }
    return { events }
  }

  func testTheWholeRecordingGoesToTheServiceAfterFinishingAndItsTextComesBack() async throws {
    let microphone = SyntheticMicrophone()
    var uploaded: Data?
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { wav in
      uploaded = wav
      return "今天天气不错"
    }
    let done = expectation(description: "result")
    let events = record(dictation, until: done)
    dictation.start(held: true)
    XCTAssertEqual(microphone.starts, 1)
    XCTAssertTrue(events().isEmpty, "麦克风打开以前不报在听")

    microphone.onStarted?()
    microphone.onAudio?(samples(1_600))
    microphone.onAudio?(samples(1_600))
    dictation.finish()
    await fulfillment(of: [done], timeout: 2)

    XCTAssertEqual(events().first, .listening(""))
    XCTAssertTrue(events().contains { if case .level(let level) = $0 { return level > 0 } else { return false } })
    XCTAssertTrue(events().contains(.recognizing(text: "", message: "")))
    XCTAssertEqual(events().last, .finished("今天天气不错"))
    let wav = try XCTUnwrap(uploaded)
    XCTAssertEqual(wav.prefix(4), Data("RIFF".utf8))
    XCTAssertEqual(wav.count, 44 + 2 * 3_200, "WAV 头之后是两段录音的全部样本")
    XCTAssertGreaterThanOrEqual(microphone.stops, 1, "说完了就关麦克风")
  }

  /// 主 App 没有麦克风权限，或者键盘里就是打不开麦克风：退回打开 App 录音，不上传。
  func testAMicrophoneThatDoesNotOpenFallsBackToTheApp() {
    let microphone = SyntheticMicrophone()
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { _ in
      XCTFail("没录到声音不该上传")
      return ""
    }
    let events = record(dictation)
    dictation.start(held: false)
    microphone.onStopped?("failed: NotAllowedError: The request is not allowed", true)

    XCTAssertEqual(events().count, 1)
    guard case .failed(_, let reopensSession) = events().first else { return XCTFail("应该报失败：\(events())") }
    XCTAssertTrue(reopensSession)
  }

  /// 麦克风还没打开就松手：与 App 收到没开始过的停止命令一样交回空结果，键盘提示没有识别到文字。
  func testReleasingBeforeTheMicrophoneOpensGivesAnEmptyResult() {
    let microphone = SyntheticMicrophone()
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { _ in
      XCTFail("没录到声音不该上传")
      return ""
    }
    let events = record(dictation)
    dictation.start(held: true)
    dictation.finish()

    XCTAssertEqual(events(), [.recognizing(text: "", message: ""), .finished("")])
    XCTAssertGreaterThanOrEqual(microphone.stops, 1)
  }

  /// 录到一半被系统打断：已经录到的照样识别。
  func testAnInterruptionMidwayStillRecognizesWhatWasRecorded() async throws {
    let microphone = SyntheticMicrophone()
    var uploaded: Data?
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { wav in
      uploaded = wav
      return "合成文字"
    }
    let done = expectation(description: "result")
    let events = record(dictation, until: done)
    dictation.start(held: false)
    microphone.onStarted?()
    microphone.onAudio?(samples(800))
    microphone.onStopped?("interrupted: track-ended", false)
    await fulfillment(of: [done], timeout: 2)

    XCTAssertEqual(events().last, .finished("合成文字"))
    XCTAssertEqual(try XCTUnwrap(uploaded).count, 44 + 2 * 800)
  }

  /// 服务出错时把原因交给键盘，不重新打开 App：换到 App 录音，同一个服务照样会出错。
  func testAServiceFailureIsReportedWithoutReopeningTheApp() async {
    let microphone = SyntheticMicrophone()
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { _ in
      throw ServiceFailure(message: "服务请求失败（HTTP 401），请检查地址、模型和密钥。")
    }
    let done = expectation(description: "failure")
    let events = record(dictation, until: done)
    dictation.start(held: true)
    microphone.onStarted?()
    microphone.onAudio?(samples(800))
    dictation.finish()
    await fulfillment(of: [done], timeout: 2)

    XCTAssertEqual(events().last, .failed("服务请求失败（HTTP 401），请检查地址、模型和密钥。", reopensSession: false))
  }

  /// 取消之后不再报任何事件，正在进行的上传也不再交回结果。
  func testCancellingStopsTheMicrophoneAndReportsNothingMore() async {
    let microphone = SyntheticMicrophone()
    let uploading = expectation(description: "upload started")
    let dictation = KeyboardDirectDictation(configuration: configuration, microphone: microphone) { _ in
      uploading.fulfill()
      try await Task.sleep(nanoseconds: 200_000_000)
      return "不该上屏"
    }
    let events = record(dictation)
    dictation.start(held: true)
    microphone.onStarted?()
    microphone.onAudio?(samples(800))
    dictation.finish()
    await fulfillment(of: [uploading], timeout: 2)
    let reported = events().count
    dictation.cancel()
    try? await Task.sleep(nanoseconds: 400_000_000)

    XCTAssertEqual(events().count, reported, "取消以后没有新事件")
    XCTAssertFalse(events().contains(.finished("不该上屏")))
    XCTAssertGreaterThanOrEqual(microphone.stops, 1)
  }
}
