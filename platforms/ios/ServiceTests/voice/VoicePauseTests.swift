import XCTest

/// 点按开始的本地识别的停顿时间、判停灵敏度和停顿计时（#10310）。
@MainActor
final class VoicePauseTests: XCTestCase {
  func testMissingOrInvalidValuesReadAsTheOldFixedOnes() {
    let empty = VoicePauseSettings(nil)
    XCTAssertEqual(empty.pauseMilliseconds, 1500)
    XCTAssertEqual(empty.stopSensitivity, 50)
    XCTAssertEqual(empty.vadThreshold, 0.5, "the default is the threshold that used to be fixed")
    for value: Any in [0, 400, 550, 5100, 1500.5, "2000", true] {
      XCTAssertEqual(VoicePauseSettings(["voice_input": ["pause_millis": value]]).pauseMilliseconds, 1500, "\(value)")
    }
    for value: Any in [0, 9, 91, 50.5, "70"] {
      XCTAssertEqual(VoicePauseSettings(["voice_input": ["stop_sensitivity": value]]).stopSensitivity, 50, "\(value)")
    }
    let stored = VoicePauseSettings(["voice_input": ["pause_millis": 3000, "stop_sensitivity": 80]])
    XCTAssertEqual(stored.pauseMilliseconds, 3000)
    XCTAssertEqual(stored.stopSensitivity, 80)
  }

  /// 与鸿蒙的 `LocalAsrPolicy.vadThreshold` 同一个换算：越高越容易判定说完。
  func testAHigherStopSensitivityRaisesTheVADThreshold() {
    var settings = VoicePauseSettings()
    settings.stopSensitivity = 10
    XCTAssertEqual(settings.vadThreshold, 0.1, accuracy: 0.0001)
    settings.stopSensitivity = 90
    XCTAssertEqual(settings.vadThreshold, 0.9, accuracy: 0.0001)
    var previous: Float = 0
    for sensitivity in VoicePauseSettings.sensitivityRange {
      settings.stopSensitivity = sensitivity
      XCTAssertGreaterThan(settings.vadThreshold, previous)
      previous = settings.vadThreshold
    }
  }

  /// `VoiceSessionService` 按这个决定一次本地识别要不要停顿计时和用哪个 VAD 阈值：点按开始的才有，按住说话没有，阈值保持 0.5。
  func testOnlyATappedDictationGetsPauseSettings() throws {
    let document: [String: Any] = ["voice_input": ["pause_millis": 2500, "stop_sensitivity": 70]]
    let tapped = try XCTUnwrap(VoicePauseSettings.forDictation(tapStarted: true, preferences: document))
    XCTAssertEqual(tapped.pauseMilliseconds, 2500)
    XCTAssertEqual(tapped.vadThreshold, 0.7, accuracy: 0.0001)
    XCTAssertEqual(VoicePauseSettings.forDictation(tapStarted: true, preferences: nil), VoicePauseSettings())
    XCTAssertNil(VoicePauseSettings.forDictation(tapStarted: false, preferences: document), "a held dictation ends on release")
  }

  func testWritingKeepsTheRestOfVoiceInput() {
    var document: [String: Any] = ["voice_input": ["asr_provider": "local", "language": "zh-cn"]]
    var settings = VoicePauseSettings()
    settings.pauseMilliseconds = 800
    settings.stopSensitivity = 70
    settings.write(into: &document)
    let voice = document["voice_input"] as? [String: Any]
    XCTAssertEqual(voice?["pause_millis"] as? Int, 800)
    XCTAssertEqual(voice?["stop_sensitivity"] as? Int, 70)
    XCTAssertEqual(voice?["asr_provider"] as? String, "local")
    XCTAssertEqual(VoicePauseSettings(document), settings)
  }

  func testNothingEndsBeforeAnyTextIsHeard() async {
    var stopped = 0
    let pause = VoicePauseStop(milliseconds: 20) { stopped += 1 }
    pause.speech(true)
    pause.speech(false)
    try? await Task.sleep(nanoseconds: 120_000_000)
    XCTAssertEqual(stopped, 0)
    pause.cancel()
  }

  func testAPauseAfterTextEndsTheDictationOnce() async {
    var stopped = 0
    let pause = VoicePauseStop(milliseconds: 30) { stopped += 1 }
    pause.heard()
    try? await Task.sleep(nanoseconds: 300_000_000)
    XCTAssertEqual(stopped, 1)
    pause.heard()
    try? await Task.sleep(nanoseconds: 150_000_000)
    XCTAssertEqual(stopped, 1, "an ended dictation is not ended again")
  }

  /// 留足几百毫秒的余量：CI 机同时跑着别的任务，一次 `Task.sleep` 多睡几十毫秒也不能让结论翻转。
  func testNewTextRestartsTheCount() async {
    var stopped = 0
    let pause = VoicePauseStop(milliseconds: 1000) { stopped += 1 }
    pause.heard()
    try? await Task.sleep(nanoseconds: 600_000_000)
    pause.heard()
    try? await Task.sleep(nanoseconds: 600_000_000)
    XCTAssertEqual(stopped, 0, "1200 ms after the first text, but only 600 ms after the latest")
    try? await Task.sleep(nanoseconds: 1_000_000_000)
    XCTAssertEqual(stopped, 1)
  }

  /// 经过 VAD 的整段模型还在说话时到点不结束，说完后从这一刻再等一个完整的停顿。
  func testSpeechStillHeardPostponesTheEnd() async {
    var stopped = 0
    let pause = VoicePauseStop(milliseconds: 40) { stopped += 1 }
    pause.heard()
    pause.speech(true)
    try? await Task.sleep(nanoseconds: 250_000_000)
    XCTAssertEqual(stopped, 0)
    pause.speech(false)
    try? await Task.sleep(nanoseconds: 300_000_000)
    XCTAssertEqual(stopped, 1)
  }

  func testCancelledCountsNeverFire() async {
    var stopped = 0
    let pause = VoicePauseStop(milliseconds: 20) { stopped += 1 }
    pause.heard()
    pause.cancel()
    try? await Task.sleep(nanoseconds: 150_000_000)
    XCTAssertEqual(stopped, 0)
    pause.heard()
    try? await Task.sleep(nanoseconds: 150_000_000)
    XCTAssertEqual(stopped, 0)
  }
}
