import XCTest

/// Leaving a composition through the 中/英 switch or Return keeps the letters typed, as Windows does; 「中文标点」 and 标点锁定 reach the runtime.
final class CompositionBoundaryTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-composition-boundary-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testPolicyCommitsRawLettersExceptWhereTheKeysAreNotLetters() {
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: false, scheme: .quanpin, boundary: .modeSwitch), .none)
    for boundary in [CompositionBoundary.modeSwitch, .returnKey] {
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .quanpin, boundary: boundary), .commitRaw)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .wubi, boundary: boundary), .commitRaw)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .nineKey, boundary: boundary), .finishComposition,
                     "nine-key raw keys are digits")
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .japanese, boundary: boundary), .finishComposition,
                     "Japanese confirms the kana")
    }
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .quanpin, boundary: .deactivate), .finishComposition)
  }

  /// 网址模式里数字和网址符号是 Engine 的输入：符号面板据此把它们作为字符交给会话，而不是选候选或结束组字。
  func testUrlModeDigitsAreEngineInputNotCandidatePicks() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    for letter in ["w", "w", "w"] { _ = bridge.handleCharacter(letter) }
    XCTAssertTrue(bridge.engineSpellsWhileComposing("."), "the . after www opens the URL mode")
    XCTAssertFalse(bridge.engineSpellsWhileComposing("1"), "a digit still picks before the URL mode opens")
    for symbol in [".", "1", "6", "3", ".", "c", "o", "m", ":", "8", "0"] {
      if symbol.first!.isLetter {
        _ = bridge.handleCharacter(symbol)
      } else {
        XCTAssertTrue(bridge.engineSpellsWhileComposing(symbol), symbol)
        XCTAssertTrue(bridge.handleCharacter(symbol).isHandled, symbol)
      }
    }
    XCTAssertFalse(bridge.engineSpellsWhileComposing("<"), "< ends the URL")
    XCTAssertEqual(bridge.commitRaw().commitText, "www.163.com:80")
    XCTAssertFalse(bridge.engineSpellsWhileComposing("/"), "with nothing composed / stays on the punctuation route")
  }

  /// U 模式里码点的数字是 Engine 的输入：宿主据此把数字作为字符交给会话，而不是按槽位选候选。
  func testUnicodeModeDigitsAreEngineInputNotCandidatePicks() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertFalse(bridge.engineSpellsWhileComposing("4"), "with nothing composed a digit is not spelled")
    XCTAssertTrue(bridge.openLocalMode("U").isInLocalMode, "Shift+U opens the Unicode mode")
    for digit in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] {
      XCTAssertTrue(bridge.engineSpellsWhileComposing(digit), digit)
    }
    XCTAssertTrue(bridge.handleCharacter("4").isHandled)
    XCTAssertTrue(bridge.engineSpellsWhileComposing("9"), "digits stay input once the code point has begun")
  }

  /// A Korean syllable is text already: Return commits it raw so the newline still follows, and every other boundary finishes it.
  func testKoreanCommitsTheSyllableAtEveryBoundary() {
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: false, scheme: .korean, boundary: .returnKey), .none)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .returnKey), .commitRaw)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .modeSwitch), .finishComposition)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .deactivate), .finishComposition)
    // With the syllable's Hanja list open Return chooses a Hanja, and the other boundaries still write the Hangul out.
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .returnKey, koreanHanjaListOpen: true), .commitCandidate)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .modeSwitch, koreanHanjaListOpen: true), .finishComposition)
    XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .korean, boundary: .deactivate, koreanHanjaListOpen: true), .finishComposition)

    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToKorean()
    for letter in "rk" { _ = bridge.handleCharacter(String(letter)) }
    let returned = bridge.commitRaw()
    XCTAssertFalse(returned.isHandled, "Return still types its newline after the syllable")
    XCTAssertEqual(returned.commitText, "가")
    XCTAssertTrue(returned.preedit.isEmpty)
    for letter in "rk" { _ = bridge.handleCharacter(String(letter)) }
    let finished = bridge.finishComposition()
    XCTAssertTrue(finished.isHandled)
    XCTAssertEqual(finished.commitText, "가")
  }

  /// Korean writes half-width ASCII marks whatever the Chinese punctuation switch and the width say.
  func testKoreanPunctuationStaysAsciiAndHalfWidth() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToKorean()
    bridge.setChinesePunctuation(true)
    bridge.setCharacterWidth(fullwidth: true)
    let idle = bridge.handlePunctuationWithContext(",", preceding: 0)
    XCTAssertFalse(idle.isHandled, "the keyboard types an idle mark itself")
    XCTAssertNil(idle.commitText)
    for letter in "rk" { _ = bridge.handleCharacter(String(letter)) }
    let marked = bridge.handlePunctuationWithContext(".", preceding: 0)
    XCTAssertTrue(marked.isHandled)
    XCTAssertEqual(marked.commitText, "가.")
    XCTAssertTrue(marked.preedit.isEmpty)
  }

  func testCommitRawKeepsTheTypedLetters() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    for letter in "nihao" { _ = bridge.handleCharacter(String(letter)) }
    let snapshot = bridge.commitRaw()
    XCTAssertTrue(snapshot.isHandled)
    XCTAssertEqual(snapshot.commitText, "nihao")
    XCTAssertTrue(snapshot.preedit.isEmpty)

    _ = bridge.switchToWubi()
    for letter in "wqvb" { _ = bridge.handleCharacter(String(letter)) }
    XCTAssertEqual(bridge.commitRaw().commitText, "wqvb")
  }

  func testChinesePunctuationSwitchReachesTheRuntime() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(bridge.handlePunctuationWithContext(",", preceding: 0).commitText, "，")
    bridge.setChinesePunctuation(false)
    XCTAssertNotEqual(bridge.handlePunctuationWithContext(",", preceding: 0).commitText, "，")
    bridge.setChinesePunctuation(true)
    XCTAssertEqual(bridge.handlePunctuationWithContext(",", preceding: 0).commitText, "，")
  }

  /// English mode asks the runtime only under 标点锁定为中文, and the runtime answers with the Chinese mark whatever the switch says.
  func testChineseLockOverridesTheSwitch() {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0["punctuation_lock"] = "chinese"
    })
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    bridge.setChinesePunctuation(false)
    XCTAssertEqual(bridge.handlePunctuationWithContext(",", preceding: 0).commitText, "，")
  }
}
