import XCTest

/// 语音标点（#10595）：对着真实的 `msime_client_voice_punctuation` 跑，例子与 Android `VoicePunctuationSmoke`、共享层测试相同。
final class VoicePunctuationTests: XCTestCase {
  func testModesAndLabelsMatchAndroid() {
    XCTAssertEqual(VoicePunctuation.modes, ["auto", "omit_trailing", "omit_all", "space"])
    XCTAssertEqual(VoicePunctuation.labels, ["智能添加", "省略句末标点", "省略所有标点", "空格代替标点"])
    XCTAssertEqual(VoicePunctuation.label("omit_all"), "省略所有标点")
    XCTAssertEqual(VoicePunctuation.label("bogus"), "智能添加", "an unknown value shows as the default")
  }

  func testModeReadsTheSharedPreferenceAndFallsBackToAuto() {
    XCTAssertEqual(VoicePunctuation.mode(nil), "auto")
    XCTAssertEqual(VoicePunctuation.mode([:]), "auto")
    XCTAssertEqual(VoicePunctuation.mode(["voice_input": ["language": "zh-cn"]]), "auto")
    XCTAssertEqual(VoicePunctuation.mode(["voice_input": ["punctuation_mode": 3]]), "auto", "not a string")
    XCTAssertEqual(VoicePunctuation.mode(["voice_input": ["punctuation_mode": "later_mode"]]), "auto", "a newer version's value")
    for mode in VoicePunctuation.modes {
      XCTAssertEqual(VoicePunctuation.mode(["voice_input": ["punctuation_mode": mode]]), mode)
    }
  }

  func testWriteChangesOnlyThePunctuationMode() throws {
    var preferences: [String: Any] = ["theme": "dark", "voice_input": ["language": "yue", "pause_millis": 2000]]
    VoicePunctuation.write("space", into: &preferences)
    let voice = try XCTUnwrap(preferences["voice_input"] as? [String: Any])
    XCTAssertEqual(voice["punctuation_mode"] as? String, "space")
    XCTAssertEqual(voice["language"] as? String, "yue")
    XCTAssertEqual(voice["pause_millis"] as? Int, 2000)
    XCTAssertEqual(preferences["theme"] as? String, "dark")

    var empty: [String: Any] = [:]
    VoicePunctuation.write("omit_all", into: &empty)
    XCTAssertEqual(VoicePunctuation.mode(empty), "omit_all")
  }

  func testApplyFollowsTheSharedRule() {
    XCTAssertEqual(VoicePunctuation.apply("你好。", mode: "auto"), "你好。")
    XCTAssertEqual(VoicePunctuation.apply("你好。", mode: "omit_trailing"), "你好")
    XCTAssertEqual(VoicePunctuation.apply("天啊！他是谁？", mode: "omit_all"), "天啊他是谁")
    XCTAssertEqual(VoicePunctuation.apply("天啊！他是谁？", mode: "space"), "天啊 他是谁")
    XCTAssertEqual(VoicePunctuation.apply("你好。", mode: "bogus"), "你好。", "the shared layer treats an unknown mode as auto")
    XCTAssertEqual(VoicePunctuation.apply("", mode: "omit_all"), "")
    XCTAssertEqual(VoicePunctuation.apply("。", mode: "omit_trailing"), "", "only punctuation leaves nothing")
  }

  func testNumbersAndTimesKeepTheirPoints() {
    for mode in ["omit_trailing", "omit_all", "space"] {
      XCTAssertEqual(VoicePunctuation.apply("3.5", mode: mode), "3.5", mode)
      XCTAssertEqual(VoicePunctuation.apply("10:30", mode: mode), "10:30", mode)
    }
  }

  /// 上屏前的最后一步：识别出了内容、处理后什么都不剩时为 nil，调用方提示没有识别到文字；本来就为空的结果原样返回，由调用方按没有听到内容处理。
  func testFinalTextTellsAPunctuationOnlyResultFromSilence() {
    XCTAssertEqual(VoicePunctuation.finalText("你好。", mode: "omit_trailing"), "你好")
    XCTAssertEqual(VoicePunctuation.finalText("你好。", mode: "auto"), "你好。")
    XCTAssertNil(VoicePunctuation.finalText("。", mode: "omit_trailing"))
    XCTAssertNil(VoicePunctuation.finalText("？！", mode: "space"))
    XCTAssertEqual(VoicePunctuation.finalText("", mode: "omit_all"), "")
    XCTAssertEqual(VoicePunctuation.finalText("  ", mode: "omit_all"), "  ")
    XCTAssertEqual(VoicePunctuation.finalText("。", mode: "auto"), "。", "auto never empties a result")
  }
}
