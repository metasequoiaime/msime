import XCTest
import UIKit

/// The Korean scheme: Dubeolsik keycaps and Shift on the keyboard, and the Engine's syllable automaton as the keyboard sees it through the bridge.
@MainActor
final class KoreanDubeolsikTests: XCTestCase {
  private var state: URL!

  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-korean-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func button(_ identifier: String, in controller: UIViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  private func key(labelled label: String, in controller: UIViewController) throws -> UIButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == label } as? UIButton,
                  "No key labelled \(label).")
  }

  private func koreanBridge() -> MetasequoiaInputSessionBridge {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToKorean()
    return bridge
  }

  private func type(_ keys: String, into bridge: MetasequoiaInputSessionBridge) -> MetasequoiaInputSnapshot {
    var snapshot = MetasequoiaInputSnapshot()
    for key in keys { snapshot = bridge.handleCharacter(String(key), shifted: key.isUppercase) }
    return snapshot
  }

  // MARK: - Layout

  func testEveryLetterHasItsDubeolsikJamo() {
    let expected: [String: String] = [
      "q": "ㅂ", "w": "ㅈ", "e": "ㄷ", "r": "ㄱ", "t": "ㅅ", "y": "ㅛ", "u": "ㅕ", "i": "ㅑ", "o": "ㅐ", "p": "ㅔ",
      "a": "ㅁ", "s": "ㄴ", "d": "ㅇ", "f": "ㄹ", "g": "ㅎ", "h": "ㅗ", "j": "ㅓ", "k": "ㅏ", "l": "ㅣ",
      "z": "ㅋ", "x": "ㅌ", "c": "ㅊ", "v": "ㅍ", "b": "ㅠ", "n": "ㅜ", "m": "ㅡ",
    ]
    XCTAssertEqual(expected.count, 26)
    for (letter, jamo) in expected {
      XCTAssertEqual(DubeolsikKeyLayout.keycap(for: letter, shifted: false), jamo, letter)
      XCTAssertEqual(DubeolsikKeyLayout.keyInput(for: letter, shifted: false), letter, letter)
    }
    XCTAssertNil(DubeolsikKeyLayout.keycap(for: ";", shifted: false))
    XCTAssertNil(DubeolsikKeyLayout.keyInput(for: "1", shifted: true))
  }

  func testShiftTypesTheTenseConsonantsAndExtraVowelsOnly() {
    let shifted: [String: String] = ["q": "ㅃ", "w": "ㅉ", "e": "ㄸ", "r": "ㄲ", "t": "ㅆ", "o": "ㅒ", "p": "ㅖ"]
    for letter in "abcdefghijklmnopqrstuvwxyz".map(String.init) {
      if let jamo = shifted[letter] {
        XCTAssertTrue(DubeolsikKeyLayout.hasShiftedJamo(letter), letter)
        XCTAssertEqual(DubeolsikKeyLayout.keycap(for: letter, shifted: true), jamo, letter)
        XCTAssertEqual(DubeolsikKeyLayout.keyInput(for: letter, shifted: true), letter.uppercased(), letter)
      } else {
        XCTAssertFalse(DubeolsikKeyLayout.hasShiftedJamo(letter), letter)
        XCTAssertEqual(DubeolsikKeyLayout.keycap(for: letter, shifted: true),
                       DubeolsikKeyLayout.keycap(for: letter, shifted: false), letter)
        XCTAssertEqual(DubeolsikKeyLayout.keyInput(for: letter, shifted: true), letter,
                       "\(letter) has no second jamo, so Shift sends it unchanged")
      }
    }
  }

  func testKoreanIsItsOwnSchemeWithoutChineseTools() {
    XCTAssertTrue(ChineseInputScheme.korean.isKorean)
    XCTAssertFalse(ChineseInputScheme.korean.isJapanese)
    XCTAssertFalse(ChineseInputScheme.korean.writesChinese)
    XCTAssertFalse(ChineseInputScheme.korean.editsBySyllable)
    XCTAssertNil(ChineseInputScheme.korean.shuangpinProfile)
    XCTAssertEqual(ChineseInputScheme.korean.sharedIdentifier, "korean")
    XCTAssertEqual(ChineseInputScheme.scheme(sharedIdentifier: "korean"), .korean)
    XCTAssertEqual(ChineseInputScheme(rawValue: "korean"), .korean)
    XCTAssertEqual(TypingSource(rawValue: "korean"), .korean)
    XCTAssertTrue(ChineseInputScheme.quanpin.writesChinese)
    XCTAssertFalse(ChineseInputScheme.japanese.writesChinese)
  }

  // MARK: - Keyboard

  func testKeysShowJamoAndShiftPicksTheDoubles() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .korean
    XCTAssertEqual(InputSchemePreference.scheme, .korean)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()

    XCTAssertEqual(try button("bottomLanguageKey", in: controller).configuration?.title, "한")
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, ChineseInputScheme.korean.title)
    let q = try key(labelled: "字母 ㅂ", in: controller)
    let a = try key(labelled: "字母 ㅁ", in: controller)
    XCTAssertEqual(q.configuration?.title, "ㅂ")
    XCTAssertEqual(a.configuration?.title, "ㅁ")
    XCTAssertTrue(try button("microsoftFinalKey", in: controller).isHidden)

    // Shift is the layout's shift here, not the switch to English capitals.
    try button("shiftButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).configuration?.title, "한")
    XCTAssertEqual(q.configuration?.title, "ㅃ")
    XCTAssertEqual(q.accessibilityLabel, "字母 ㅃ")
    XCTAssertEqual(a.configuration?.title, "ㅁ", "a key without a second jamo keeps its face under Shift")
    XCTAssertEqual(try button("shiftButton", in: controller).accessibilityLabel, "双辅音")

    // One shifted key, then the keyboard is back to the plain jamo, and the syllable is on the strip.
    q.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(q.configuration?.title, "ㅂ")
    try key(labelled: "字母 ㅏ", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "빠")
    // Return commits the syllable and still does the field's own action, so it keeps the field's name.
    XCTAssertEqual(try button("returnKey", in: controller).configuration?.title, "换行")
    XCTAssertNil(nodes(controller.view).first { $0.accessibilityIdentifier == "candidate-1" && !$0.isHidden })

    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    attachment.name = "Korean Dubeolsik keyboard"
    attachment.lifetime = .keepAlways
    add(attachment)

    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(q.configuration?.title, "q", "English puts the Latin letters back")
    XCTAssertEqual(q.accessibilityLabel, "字母 Q")
  }

  func testPunctuationIsAsciiAndChineseSwitchesAreOff() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .korean
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()

    let punctuation = try button("quickPunctuationKey", in: controller)
    XCTAssertEqual(punctuation.configuration?.title, ",")
    XCTAssertEqual(punctuation.menu?.children.compactMap { ($0 as? UIAction)?.title },
                   [",", ".", "?", "!", ":", ";", "@"])
    XCTAssertFalse(try button("punctuationShortcut", in: controller).isEnabled)
    XCTAssertFalse(try button("characterSetShortcut", in: controller).isEnabled)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(nodes(controller.view).first { $0.accessibilityLabel == "符号 ;" })
    XCTAssertNil(nodes(controller.view).first { $0.accessibilityLabel == "符号 ；" })
  }

  /// 漢 shows only while a syllable composes, lists its Hanja with their 훈음 on the strip, and turns Return into choosing one; with the list closed Korean is unchanged.
  func testHanjaButtonListsTheComposingSyllablesHanja() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .korean
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()
    func visibleChip(_ number: Int) -> UIButton? {
      nodes(controller.view).first { $0.accessibilityIdentifier == "candidate-\(number)" && !$0.isHidden } as? UIButton
    }
    func press(_ label: String) throws {
      try key(labelled: label, in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
    }

    let hanja = try button("hanjaButton", in: controller)
    XCTAssertTrue(hanja.isHidden, "nothing is composing")
    for jamo in ["ㅎ", "ㅏ", "ㄴ"] { try press("字母 \(jamo)") }
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "한")
    XCTAssertFalse(hanja.isHidden)
    XCTAssertEqual(hanja.accessibilityLabel, "转换为汉字")
    XCTAssertNil(visibleChip(1), "Korean has no candidates until 漢")
    XCTAssertEqual(try button("returnKey", in: controller).accessibilityLabel, "换行")

    hanja.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let first = try XCTUnwrap(visibleChip(1))
    let firstTitle = String(try XCTUnwrap(first.configuration?.attributedTitle).characters)
    XCTAssertTrue(firstTitle.hasPrefix("韓"), firstTitle)
    XCTAssertTrue(firstTitle.contains("나라 이름 한"), "the 훈음 follows the Hanja: \(firstTitle)")
    XCTAssertFalse(firstTitle.contains("gks"), "the key letters are not drawn")
    XCTAssertEqual(first.accessibilityLabel, "候选词 1：韓，训音 나라 이름 한, 한나라 한")
    XCTAssertTrue(String(try XCTUnwrap(visibleChip(2)?.configuration?.attributedTitle).characters).hasPrefix("漢"))
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "한", "the syllable keeps composing")
    XCTAssertEqual(hanja.accessibilityLabel, "关闭汉字列表")
    XCTAssertTrue(hanja.accessibilityTraits.contains(.selected))
    XCTAssertEqual(try button("returnKey", in: controller).accessibilityLabel, "确认", "Return chooses a Hanja")

    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    attachment.name = "Korean Hanja list"
    attachment.lifetime = .keepAlways
    add(attachment)

    // 漢 again closes the list and keeps the syllable.
    hanja.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(visibleChip(1))
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "한")
    XCTAssertEqual(hanja.accessibilityLabel, "转换为汉字")
    XCTAssertEqual(try button("returnKey", in: controller).accessibilityLabel, "换行")

    // Choosing a Hanja ends the composition, and 漢 goes with it.
    hanja.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    try XCTUnwrap(visibleChip(2)).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(visibleChip(1))
    XCTAssertTrue(hanja.isHidden)
    XCTAssertEqual(try button("returnKey", in: controller).accessibilityLabel, "换行")

    // Return with the list open chooses the leading Hanja rather than writing the Hangul out.
    for jamo in ["ㄱ", "ㅏ"] { try press("字母 \(jamo)") }
    hanja.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(visibleChip(1))
    try button("returnKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(visibleChip(1))
    XCTAssertTrue(hanja.isHidden)

    // A lone jamo has no Hanja: 漢 shows, the Engine declines, and the jamo keeps composing.
    try press("字母 ㄱ")
    XCTAssertFalse(hanja.isHidden)
    hanja.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(visibleChip(1))
    XCTAssertEqual(hanja.accessibilityLabel, "转换为汉字")
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "ㄱ")
  }

  func testSchemeSelectionWritesKoreanAndKeepsTheChineseScheme() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.wubi, enabledSchemes: [.quanpin, .wubi, .korean]))
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.korean, enabledSchemes: [.quanpin, .wubi, .korean]))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["scheme"] as? String, "korean")
    XCTAssertEqual(preferences["last_chinese_scheme"] as? String, "wubi", "Korean does not replace the Chinese scheme to return to")
    XCTAssertEqual(preferences["touch_keyboard_layout"] as? String, "twenty_six_key")
    let schemes = try XCTUnwrap(preferences["touch_keyboard_schemes"] as? [String: Any])
    XCTAssertEqual(schemes["enabled"] as? [String], ["quanpin", "wubi", "korean"])
    XCTAssertEqual(schemes["selected"] as? String, "korean")
  }

  // MARK: - Automaton through the bridge

  func testFinishedSyllableCommitsWhenTheNextOneStarts() {
    let bridge = koreanBridge()
    XCTAssertEqual(type("dks", into: bridge).preedit, "안")
    let fourth = bridge.handleCharacter("s")
    XCTAssertTrue(fourth.isHandled)
    XCTAssertEqual(fourth.commitText, "안")
    XCTAssertEqual(fourth.preedit, "ㄴ")
    let composing = type("ud", into: bridge)
    XCTAssertNil(composing.commitText)
    XCTAssertEqual(composing.preedit, "녕")
    XCTAssertEqual(composing.reading, "녕")
    XCTAssertEqual(composing.editingText, "sud")
    XCTAssertTrue(composing.candidates.isEmpty)
    XCTAssertEqual(composing.candidatePageCount, 0)
    XCTAssertTrue(composing.phrasePrefix.isEmpty, "a finished syllable is never held back as a phrase")
    XCTAssertNil(bridge.onlineQuery())

    // Space ends the syllable and hands the key back, so the keyboard types the space after the commit.
    let space = bridge.commitCandidate()
    XCTAssertFalse(space.isHandled)
    XCTAssertEqual(space.commitText, "녕")
    XCTAssertTrue(space.preedit.isEmpty)
    XCTAssertTrue(space.editingText.isEmpty)
  }

  func testVowelTakesTheFinalConsonantIntoTheNextSyllable() {
    let bridge = koreanBridge()
    _ = type("rks", into: bridge)
    let moved = bridge.handleCharacter("k")
    XCTAssertEqual(moved.commitText, "가")
    XCTAssertEqual(moved.preedit, "나")

    _ = bridge.cancel()
    _ = type("ekfr", into: bridge)
    let split = bridge.handleCharacter("k")
    XCTAssertEqual(split.commitText, "달")
    XCTAssertEqual(split.preedit, "가")
  }

  func testShiftedLettersAndLoneJamo() {
    let bridge = koreanBridge()
    XCTAssertEqual(type("Rk", into: bridge).preedit, "까")
    _ = bridge.cancel()
    XCTAssertEqual(type("r", into: bridge).preedit, "ㄱ")
    let cluster = bridge.handleCharacter("t")
    XCTAssertEqual(cluster.commitText, "ㄱ", "consonants never cluster outside a syllable")
    XCTAssertEqual(cluster.preedit, "ㅅ")
    _ = bridge.cancel()
    XCTAssertEqual(type("hk", into: bridge).preedit, "ㅘ")
  }

  func testBackspaceRemovesOneJamoAtATime() {
    let bridge = koreanBridge()
    XCTAssertEqual(type("ekfr", into: bridge).preedit, "닭")
    for expected in ["달", "다", "ㄷ", ""] {
      let snapshot = bridge.handleBackspace()
      XCTAssertTrue(snapshot.isHandled)
      XCTAssertEqual(snapshot.preedit, expected)
    }
    XCTAssertFalse(bridge.handleBackspace().isHandled, "an idle backspace deletes document text")

    XCTAssertEqual(type("rhkd", into: bridge).preedit, "광")
    for expected in ["과", "고", "ㄱ"] { XCTAssertEqual(bridge.handleBackspace().preedit, expected) }
  }

  /// The Hanja list as the bridge carries it: MSIME_CONVERT_HANJA opens and closes it over a syllable that keeps composing, the candidate commands and digits choose from it, Cancel closes it, Finish writes the Hangul, and a lone jamo has none.
  func testHanjaListThroughTheBridge() {
    let bridge = koreanBridge()
    XCTAssertEqual(type("gks", into: bridge).preedit, "한")
    let opened = bridge.convertHanja()
    XCTAssertTrue(opened.isHandled)
    XCTAssertNil(opened.commitText)
    XCTAssertEqual(opened.preedit, "한")
    XCTAssertEqual(Array(opened.candidates.prefix(2)), ["韓", "漢"])
    XCTAssertEqual(opened.candidateAnnotations.first, "나라 이름 한, 한나라 한")
    XCTAssertEqual(opened.candidateCodes.first, "gks")

    let closed = bridge.convertHanja()
    XCTAssertTrue(closed.isHandled)
    XCTAssertTrue(closed.candidates.isEmpty)
    XCTAssertEqual(closed.preedit, "한")

    _ = bridge.convertHanja()
    let cancelled = bridge.cancel()
    XCTAssertTrue(cancelled.isHandled)
    XCTAssertTrue(cancelled.candidates.isEmpty, "Cancel only closes the list")
    XCTAssertEqual(cancelled.preedit, "한")
    XCTAssertTrue(bridge.cancel().preedit.isEmpty, "a second Cancel drops the syllable")

    _ = type("gks", into: bridge)
    _ = bridge.convertHanja()
    let chosen = bridge.commitCandidate()
    XCTAssertTrue(chosen.isHandled, "Return and Space take no newline or space after a Hanja")
    XCTAssertEqual(chosen.commitText, "韓")
    XCTAssertTrue(chosen.preedit.isEmpty)

    _ = type("gks", into: bridge)
    _ = bridge.convertHanja()
    let digit = bridge.handleCharacter("2")
    XCTAssertTrue(digit.isHandled)
    XCTAssertEqual(digit.commitText, "漢")

    _ = type("gks", into: bridge)
    _ = bridge.convertHanja()
    let selected = bridge.selectCandidate(at: 1)
    XCTAssertEqual(selected.commitText, "漢")

    _ = type("gks", into: bridge)
    _ = bridge.convertHanja()
    let letter = bridge.handleCharacter("r")
    XCTAssertEqual(letter.commitText, "한", "a letter closes the list and composes on")
    XCTAssertTrue(letter.candidates.isEmpty)
    XCTAssertEqual(letter.preedit, "ㄱ")
    let lone = bridge.convertHanja()
    XCTAssertFalse(lone.isHandled, "a lone jamo has no Hanja")
    XCTAssertTrue(lone.candidates.isEmpty)
    XCTAssertEqual(lone.preedit, "ㄱ")

    _ = bridge.cancel()
    _ = type("gks", into: bridge)
    _ = bridge.convertHanja()
    let finished = bridge.finishComposition()
    XCTAssertEqual(finished.commitText, "한", "finishing writes the Hangul, never a Hanja")
    XCTAssertTrue(finished.candidates.isEmpty)
  }

  func testKeysThatEndTheSyllableCommitItUnhandled() {
    let bridge = koreanBridge()
    _ = type("rk", into: bridge)
    let digit = bridge.handleCharacter("1")
    XCTAssertFalse(digit.isHandled, "the keyboard types the digit after the commit")
    XCTAssertEqual(digit.commitText, "가")
    XCTAssertFalse(bridge.handleCharacter("1").isHandled)
    XCTAssertNil(bridge.handleCharacter("1").commitText)

    _ = type("rk", into: bridge)
    let caret = bridge.moveCaretLeft()
    XCTAssertFalse(caret.isHandled)
    XCTAssertEqual(caret.commitText, "가")

    _ = type("rk", into: bridge)
    let cancelled = bridge.cancel()
    XCTAssertTrue(cancelled.isHandled)
    XCTAssertNil(cancelled.commitText)
    XCTAssertTrue(cancelled.preedit.isEmpty)

    _ = type("rk", into: bridge)
    XCTAssertFalse(bridge.cycleKanaVariant().isHandled)
    XCTAssertEqual(bridge.finishComposition().commitText, "가")
  }
}
