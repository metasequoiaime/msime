import UIKit
import XCTest

/// 按住说话：键盘和后台主 App 之间的会话通道、键盘这一侧的听写流程、上滑取消区和聆听面板。App 一侧在这里由测试按协议写状态来扮演。
@MainActor
final class VoiceSessionTests: XCTestCase {
  private var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent("voice-session-" + UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  private func state(_ phase: VoiceSessionState.Phase, request: UUID?, text: String = "", message: String = "",
                     heartbeat: Date = Date(), expiresAt: Date = Date().addingTimeInterval(300)) -> VoiceSessionState {
    VoiceSessionState(phase: phase, request: request, text: text, message: message, heartbeat: heartbeat, expiresAt: expiresAt)
  }

  func testTheChannelCarriesStateAndCommands() {
    let channel = VoiceSessionChannel(directory: directory)
    XCTAssertNil(channel.readState())
    let published = state(.listening, request: UUID(), text: "你好")
    channel.publish(published)
    XCTAssertEqual(channel.readState()?.phase, .listening)
    XCTAssertEqual(channel.readState()?.text, "你好")
    let command = VoiceSessionCommand(action: .stop, request: UUID(), sentAt: Date())
    channel.send(command)
    // 时间按毫秒存，只比较动作和请求。
    XCTAssertEqual(channel.readCommand()?.action, .stop)
    XCTAssertEqual(channel.readCommand()?.request, command.request)
    XCTAssertEqual(channel.readCommand()?.sentAt.timeIntervalSince1970 ?? 0, command.sentAt.timeIntervalSince1970, accuracy: 0.001)
    channel.clearState()
    XCTAssertNil(channel.readState())
  }

  func testASessionIsOnlyAvailableWhileItsHeartbeatIsFreshAndItHasNotExpired() {
    let now = Date()
    XCTAssertTrue(state(.ready, request: nil, heartbeat: now).isAvailable(now: now))
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now.addingTimeInterval(-11)).isAvailable(now: now),
                   "an app the system ended stops beating")
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now, expiresAt: now.addingTimeInterval(-1)).isAvailable(now: now))
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now.addingTimeInterval(60)).isAvailable(now: now))
  }

  func testWithoutASessionTheKeyboardMustOpenTheApp() {
    XCTAssertFalse(KeyboardVoiceDictation(channel: VoiceSessionChannel(directory: directory)).sessionAvailable)
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    XCTAssertTrue(KeyboardVoiceDictation(channel: channel).sessionAvailable)
  }

  /// 松开得比 App 回应开始命令还早：停止命令等到回应以后才发，不会盖掉还没读到的开始命令。
  func testAHoldReleasedBeforeTheAppAnswersStopsOnceItDoes() throws {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    var events: [KeyboardVoiceDictation.Event] = []
    let finished = expectation(description: "finished")
    dictation.onEvent = { event in
      events.append(event)
      if case .finished = event { finished.fulfill() }
    }
    dictation.start()
    let start = try XCTUnwrap(channel.readCommand())
    XCTAssertEqual(start.action, .start)
    dictation.finish()
    XCTAssertEqual(channel.readCommand()?.action, .start, "no stop before the app has started")

    channel.publish(state(.listening, request: start.request, text: "明天"))
    let stopped = expectation(description: "stop sent")
    Task { @MainActor in
      while channel.readCommand()?.action != .stop { try? await Task.sleep(nanoseconds: 10_000_000) }
      stopped.fulfill()
    }
    wait(for: [stopped], timeout: 2)
    XCTAssertEqual(channel.readCommand()?.request, start.request)
    channel.publish(state(.recognizing, request: start.request, text: "明天见"))
    channel.publish(state(.finished, request: start.request, text: "明天见。"))
    wait(for: [finished], timeout: 2)
    XCTAssertEqual(events.last, .finished("明天见。"))
    XCTAssertNil(dictation.request)
  }

  /// 结束操作马上进入识别状态；迟到的聆听回调不能重新开放结束按钮或显示仍在录音。
  func testFinishBeforeAcknowledgementKeepsRecognizingUntilTheFinalResult() throws {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    var events: [KeyboardVoiceDictation.Event] = []
    let finished = expectation(description: "结束后收到最终转写")
    dictation.onEvent = { event in
      events.append(event)
      if case .finished = event { finished.fulfill() }
    }
    dictation.start()
    let request = try XCTUnwrap(dictation.request)
    dictation.finish()
    XCTAssertEqual(events.last, .recognizing(text: "", message: ""))
    channel.publish(state(.listening, request: request, text: "合成"))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(events.last, .recognizing(text: "合成", message: ""))
    XCTAssertEqual(channel.readCommand()?.action, .stop)
    channel.publish(state(.finished, request: request, text: "合成文字"))
    wait(for: [finished], timeout: 2)
    XCTAssertEqual(events.last, .finished("合成文字"))
    XCTAssertNil(dictation.request)
  }

  func testAnotherDictationsStateIsIgnored() {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    var events: [KeyboardVoiceDictation.Event] = []
    dictation.onEvent = { events.append($0) }
    dictation.start()
    channel.publish(state(.finished, request: UUID(), text: "别人的"))
    RunLoop.main.run(until: Date().addingTimeInterval(0.2))
    XCTAssertTrue(events.isEmpty)
    dictation.cancel()
    XCTAssertEqual(channel.readCommand()?.action, .cancel)
  }

  func testAFailureFromTheAppIsReportedWithItsReason() {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    let failed = expectation(description: "failed")
    dictation.onEvent = { event in
      if case .failed(let message, let reopens) = event {
        XCTAssertEqual(message, "请在系统设置中允许水杉使用语音识别。")
        XCTAssertFalse(reopens)
        failed.fulfill()
      }
    }
    dictation.start()
    channel.publish(state(.failed, request: dictation.request, message: "请在系统设置中允许水杉使用语音识别。"))
    wait(for: [failed], timeout: 2)
  }

  /// App 没有回应开始命令（被系统结束了但心跳还没过期）：报告失败并要求重新打开 App。
  func testAnUnansweredStartAsksToReopenTheSession() {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    let failed = expectation(description: "failed")
    dictation.onEvent = { event in
      if case .failed(_, let reopens) = event, reopens { failed.fulfill() }
    }
    dictation.start()
    wait(for: [failed], timeout: KeyboardVoiceDictation.startTimeout + 2)
    XCTAssertEqual(channel.readCommand()?.action, .cancel)
  }

  /// Android 的 `VoiceHoldPolicy`：上滑超过键盘高度的四分之一进入取消区。
  func testSlidingUpAQuarterOfTheKeyboardCancels() {
    XCTAssertFalse(KeyboardViewController.voiceHoldCancels(originY: 250, y: 250, height: 260))
    XCTAssertFalse(KeyboardViewController.voiceHoldCancels(originY: 250, y: 186, height: 260))
    XCTAssertTrue(KeyboardViewController.voiceHoldCancels(originY: 250, y: 184, height: 260))
    XCTAssertFalse(KeyboardViewController.voiceHoldCancels(originY: 250, y: 300, height: 260), "sliding down never cancels")
  }

  func testTheListeningPanelReadsLikeAndroids() {
    let panel = KeyboardVoicePanelView(dictationHeld: true, skin: KeyboardTheme.current)
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    XCTAssertEqual(panel.captionLabel.text, "松开结束，上滑取消")
    panel.showListening("今天天气不错")
    XCTAssertEqual(panel.captionLabel.text, "今天天气不错")
    XCTAssertEqual(panel.captionLabel.lineBreakMode, .byTruncatingHead, "the newest words stay in view")
    panel.setCancelling(true)
    XCTAssertEqual(panel.statusLabel.text, "松开取消")
    XCTAssertTrue(panel.orb.isCancelling)
    panel.showListening("今天天气不错啊")
    XCTAssertEqual(panel.statusLabel.text, "松开取消", "a partial result does not leave the cancel zone")
    panel.setCancelling(false)
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    panel.showRecognizing(text: "今天天气不错啊", message: "")
    XCTAssertEqual(panel.statusLabel.text, "正在识别…")
    panel.showRecognizing(text: "今天天气不错啊", message: "正在润色…")
    XCTAssertEqual(panel.statusLabel.text, "正在润色…")
    let tapped = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    XCTAssertEqual(tapped.captionLabel.text, "说完后点「说完了」，识别后自动上屏")
  }

  func testTheSessionLinkNamesThisEdition() {
    XCTAssertEqual(KeyboardAppLauncher.voiceSessionURL.absoluteString, "\(MSIMEAppEdition.urlScheme)://voice-session")
  }
}
