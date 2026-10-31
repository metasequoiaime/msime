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
                     level: Float? = nil, endsOnPause: Bool? = nil, heartbeat: Date = Date(),
                     expiresAt: Date = Date().addingTimeInterval(300)) -> VoiceSessionState {
    VoiceSessionState(phase: phase, request: request, text: text, message: message, level: level,
                      endsOnPause: endsOnPause, heartbeat: heartbeat, expiresAt: expiresAt)
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

  /// 电平是后加的可选字段：旧版本 App 写的状态没有它也能读，有它时原样带过来。
  func testTheStateDecodesWithAndWithoutALevel() throws {
    let channel = VoiceSessionChannel(directory: directory)
    let request = UUID()
    let now = Int(Date().timeIntervalSince1970 * 1000)
    let legacy = """
      {"version":1,"phase":"listening","request":"\(request.uuidString)","text":"合成","message":"",\
      "heartbeat":\(now),"expiresAt":\(now + 300_000)}
      """
    let folder = directory.appendingPathComponent("VoiceSession", isDirectory: true)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    try Data(legacy.utf8).write(to: folder.appendingPathComponent("state.json"))
    let decoded = try XCTUnwrap(channel.readState())
    XCTAssertEqual(decoded.phase, .listening)
    XCTAssertEqual(decoded.request, request)
    XCTAssertEqual(decoded.text, "合成")
    XCTAssertNil(decoded.level)
    XCTAssertTrue(decoded.isAvailable())

    channel.publish(state(.listening, request: request, text: "合成", level: 0.5))
    XCTAssertEqual(channel.readState()?.level, 0.5)
    channel.publish(state(.recognizing, request: request, text: "合成"))
    XCTAssertNil(channel.readState()?.level)
    let written = try String(contentsOf: folder.appendingPathComponent("state.json"), encoding: .utf8)
    XCTAssertFalse(written.contains("level"), "没有电平时不写这个键，旧版本键盘照常读")
  }

  /// App 只在电平变了至少 0.02 时才再报，安静时不反复写文件。
  func testTheAppReportsALevelOnlyWhenItMoves() {
    XCTAssertEqual(VoiceSessionChannel.levelInterval, 0.1)
    XCTAssertFalse(VoiceSessionChannel.publishesLevel(0, after: nil))
    XCTAssertFalse(VoiceSessionChannel.publishesLevel(0.01, after: nil))
    XCTAssertTrue(VoiceSessionChannel.publishesLevel(0.3, after: nil))
    XCTAssertFalse(VoiceSessionChannel.publishesLevel(0.5, after: 0.49))
    XCTAssertTrue(VoiceSessionChannel.publishesLevel(0.5, after: 0.45))
    XCTAssertTrue(VoiceSessionChannel.publishesLevel(0.1, after: 0.6), "回落同样要报")
  }

  /// 正在听时状态里的电平交给面板；松开以后的电平不再报。
  func testTheDictationForwardsTheLevelWhileListening() throws {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    var events: [KeyboardVoiceDictation.Event] = []
    dictation.onEvent = { events.append($0) }
    dictation.start()
    let request = try XCTUnwrap(dictation.request)
    channel.publish(state(.listening, request: request, text: "合成"))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(events, [.listening("合成")], "没有电平就不报电平")
    channel.publish(state(.listening, request: request, text: "合成", level: 0.6))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(events.suffix(2), [.listening("合成"), .level(0.6)])
    dictation.finish()
    channel.publish(state(.listening, request: request, text: "合成文字", level: 0.9))
    RunLoop.main.run(until: Date().addingTimeInterval(0.1))
    XCTAssertEqual(events.last, .recognizing(text: "合成文字", message: ""))
    XCTAssertFalse(events.contains(.level(0.9)))
    dictation.cancel()
  }

  func testASessionIsOnlyAvailableWhileItsHeartbeatIsFreshAndItHasNotExpired() {
    let now = Date()
    XCTAssertTrue(state(.ready, request: nil, heartbeat: now).isAvailable(now: now))
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now.addingTimeInterval(-11)).isAvailable(now: now),
                   "an app the system ended stops beating")
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now, expiresAt: now.addingTimeInterval(-1)).isAvailable(now: now))
    XCTAssertFalse(state(.ready, request: nil, heartbeat: now.addingTimeInterval(60)).isAvailable(now: now))
  }

  /// 会话空闲时麦克风和音频引擎仍在后台运行，耗电发热，所以说完不久就要交还麦克风；心跳间隔要比判定 App 不在的时限短，否则开着的会话会被键盘当成已经不在。
  func testAnIdleSessionReleasesTheMicrophoneWithinAMinute() {
    XCTAssertLessThanOrEqual(VoiceSessionChannel.idleLifetime, 60)
    XCTAssertLessThan(VoiceSessionChannel.heartbeatInterval, VoiceSessionState.staleAfter)
    XCTAssertGreaterThan(VoiceSessionChannel.idleLifetime, VoiceSessionState.staleAfter)
  }

  func testWithoutASessionTheKeyboardMustOpenTheApp() {
    XCTAssertFalse(KeyboardVoiceDictation(channel: VoiceSessionChannel(directory: directory)).sessionAvailable)
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    XCTAssertTrue(KeyboardVoiceDictation(channel: channel).sessionAvailable)
  }

  /// 开始命令告诉 App 这次听写是不是点按开始的（#10310）：点按的本地识别由 App 按停顿结束，按住说话由松手结束。旧版本键盘写的命令没有这个字段，按按住读。
  func testTheStartCommandSaysWhetherTheDictationWasTapped() throws {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let tapped = KeyboardVoiceDictation(channel: channel)
    tapped.start(held: false)
    XCTAssertEqual(channel.readCommand()?.action, .start)
    XCTAssertEqual(channel.readCommand()?.tapStarted, true)
    tapped.cancel()

    let held = KeyboardVoiceDictation(channel: channel)
    held.start(held: true)
    XCTAssertEqual(channel.readCommand()?.tapStarted, false)
    held.cancel()

    let request = UUID()
    let now = Int(Date().timeIntervalSince1970 * 1000)
    let legacy = """
      {"version":1,"action":"start","request":"\(request.uuidString)","sentAt":\(now)}
      """
    let folder = directory.appendingPathComponent("VoiceSession", isDirectory: true)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    try Data(legacy.utf8).write(to: folder.appendingPathComponent("command.json"))
    let decoded = try XCTUnwrap(channel.readCommand())
    XCTAssertEqual(decoded.request, request)
    XCTAssertNil(decoded.tapStarted)
  }

  /// App 在点按开始的本地识别停顿后自己结束录音（#10310）：聆听状态先说明这次会在停顿后结束，键盘只报一次 `endsOnPause`；键盘没有发过停止命令，照样收到识别中和最终结果并上屏，与到时长上限时一样。
  func testATappedDictationTheAppEndsOnAPauseStillDeliversItsResult() throws {
    let channel = VoiceSessionChannel(directory: directory)
    channel.publish(state(.ready, request: nil))
    let dictation = KeyboardVoiceDictation(channel: channel)
    var events: [KeyboardVoiceDictation.Event] = []
    let listening = expectation(description: "listening")
    let announced = expectation(description: "ends on pause")
    let recognizing = expectation(description: "recognizing")
    let finished = expectation(description: "finished")
    dictation.onEvent = { event in
      events.append(event)
      switch event {
      case .listening("明天"): listening.fulfill()
      case .endsOnPause: announced.fulfill()
      case .recognizing: recognizing.fulfill()
      case .finished: finished.fulfill()
      default: break
      }
    }
    dictation.start(held: false)
    let request = try XCTUnwrap(dictation.request)
    // 状态是一个文件，键盘收到通知才去读；每一步等键盘读到再写下一步，免得几次写入合并成最后一次。
    channel.publish(state(.listening, request: request, endsOnPause: true))
    wait(for: [announced], timeout: 2)
    channel.publish(state(.listening, request: request, text: "明天", endsOnPause: true))
    wait(for: [listening], timeout: 2)
    channel.publish(state(.recognizing, request: request, text: "明天见"))
    wait(for: [recognizing], timeout: 2)
    channel.publish(state(.finished, request: request, text: "明天见。"))
    wait(for: [finished], timeout: 2)
    XCTAssertEqual(channel.readCommand()?.action, .start, "the keyboard never sent a stop")
    XCTAssertEqual(events.filter { $0 == .endsOnPause }.count, 1, "announced once per dictation")
    XCTAssertTrue(events.contains(.recognizing(text: "明天见", message: "")))
    XCTAssertEqual(events.last, .finished("明天见。"))
    XCTAssertNil(dictation.request)
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
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 227)
    panel.layoutIfNeeded()
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    XCTAssertEqual(panel.captionLabel.text, "松开结束，上滑取消")
    panel.showListening("今天天气不错")
    XCTAssertEqual(panel.statusLabel.text, "今天天气不错", "the partial takes the title")
    XCTAssertEqual(panel.captionLabel.text, "松开结束，上滑取消", "the instruction stays below it")
    XCTAssertEqual(panel.statusLabel.accessibilityLabel, "今天天气不错")
    panel.showListening(" 今天\n天气  不错 ")
    XCTAssertEqual(panel.statusLabel.text, "今天 天气 不错", "whitespace collapses to single spaces")
    panel.setCancelling(true)
    XCTAssertEqual(panel.statusLabel.text, "松开取消")
    XCTAssertTrue(panel.orb.isCancelling)
    panel.showListening("今天天气不错啊")
    XCTAssertEqual(panel.statusLabel.text, "松开取消", "a partial result does not leave the cancel zone")
    panel.setCancelling(false)
    XCTAssertEqual(panel.statusLabel.text, "今天天气不错啊", "back from the cancel zone the newest partial shows")
    panel.showListening("")
    XCTAssertEqual(panel.statusLabel.text, "正在聆听…")
    panel.showRecognizing(text: "今天天气不错啊", message: "")
    XCTAssertEqual(panel.statusLabel.text, "正在识别…")
    XCTAssertEqual(panel.captionLabel.text, "今天天气不错啊")
    panel.showRecognizing(text: "", message: "正在润色…")
    XCTAssertEqual(panel.statusLabel.text, "正在润色…")
    XCTAssertEqual(panel.captionLabel.text, "识别完成后自动上屏")
    let tapped = KeyboardVoicePanelView(dictationHeld: false, skin: KeyboardTheme.current)
    XCTAssertEqual(tapped.captionLabel.text, "说完后点「说完了」，识别后自动上屏")
    // App 用本地模型、会在停顿后自己结束时，提示与 Android、鸿蒙的本地识别相同（#10310）；按住说话不变。
    tapped.showEndsOnPause()
    XCTAssertEqual(tapped.captionLabel.text, "停顿后自动上屏，也可点「说完了」")
    tapped.showListening("明天见")
    XCTAssertEqual(tapped.captionLabel.text, "停顿后自动上屏，也可点「说完了」")
    let held = KeyboardVoicePanelView(dictationHeld: true, skin: KeyboardTheme.current)
    held.showEndsOnPause()
    XCTAssertEqual(held.captionLabel.text, "松开结束，上滑取消")
  }

  func testTheSessionLinkNamesThisEdition() {
    XCTAssertEqual(KeyboardAppLauncher.voiceSessionURL.absoluteString, "\(MSIMEAppEdition.urlScheme)://voice-session")
  }
}
