import XCTest
import UIKit

/// 粤拼、注音、越南语和藏文：键盘提供其中哪些、它们怎样写进共享偏好、大千键位，以及键盘经桥接层看到的就地组字。
@MainActor
final class LanguageSchemesTests: XCTestCase {
  private var state: URL!

  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-language-schemes-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func type(_ keys: String, into bridge: MetasequoiaInputSessionBridge) -> MetasequoiaInputSnapshot {
    var snapshot = MetasequoiaInputSnapshot()
    for key in keys { snapshot = bridge.handleCharacter(String(key)) }
    return snapshot
  }

  // MARK: - Availability

  func testTheThreeSchemesStartDisabledOnAFreshInstall() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previous = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    defer { defaults.set(previous, forKey: InputSchemePreference.enabledSchemesKey) }
    defaults.removeObject(forKey: InputSchemePreference.enabledSchemesKey)
    let enabled = InputSchemePreference.enabledSchemes
    XCTAssertEqual(enabled, ChineseInputScheme.allCases.filter { ![.cantonese, .zhuyin, .vietnamese, .tibetan].contains($0) })
  }

  func testAMissingDictionaryHidesOnlyItsScheme() {
    let enabled: [ChineseInputScheme] = [.quanpin, .cantonese, .zhuyin, .vietnamese]
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: nil), enabled,
                   "a process that cannot tell hides nothing")
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: [.cantonese, .zhuyin]), enabled)
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: [.zhuyin]), [.quanpin, .zhuyin, .vietnamese])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: enabled, installed: []), [.quanpin, .vietnamese],
                   "Vietnamese needs no data")
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: [.cantonese], installed: []), [.quanpin])
    XCTAssertEqual(InputSchemePreference.offeredSchemes(enabled: [.quanpin, .tibetan], installed: []), [.quanpin, .tibetan],
                   "藏文不需要词库")
  }

  func testInstalledSchemesAreReadFromTheDictionaryFiles() throws {
    XCTAssertNil(InputSchemePreference.installedLanguageSchemes(in: nil))
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [])
    try Data().write(to: state.appendingPathComponent("cantonese.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.cantonese])
    try Data().write(to: state.appendingPathComponent("zhuyin.db"))
    XCTAssertEqual(InputSchemePreference.installedLanguageSchemes(in: state), [.cantonese, .zhuyin])
  }

  /// The test host bundles EngineResources, so it can tell what is installed; the App bundle carries no Engine and cannot.
  func testTheTestHostLooksBesideItsEngineResources() throws {
    let directory = try XCTUnwrap(InputSchemePreference.languageDictionaryDirectory(in: .main))
    XCTAssertEqual(directory.lastPathComponent, "language-dictionaries")
    XCTAssertNotNil(InputSchemePreference.installedLanguageSchemes)
  }

  // MARK: - Shared preferences

  func testCantoneseAndZhuyinAreChineseSchemesAndVietnameseIsNot() {
    let enabled = ChineseInputScheme.allCases
    for (scheme, engine, remembered) in [(ChineseInputScheme.cantonese, "cantonese", true), (.zhuyin, "zhuyin", true), (.vietnamese, "vietnamese", false), (.tibetan, "tibetan", false)] {
      var document: [String: Any] = ["last_chinese_scheme": "wubi"]
      MetasequoiaInputSessionBridge.schemeMapping(scheme, enabledSchemes: enabled)?(&document)
      XCTAssertEqual(document["scheme"] as? String, engine)
      XCTAssertEqual(document["last_chinese_scheme"] as? String, remembered ? engine : "wubi", engine)
      XCTAssertEqual(document["touch_keyboard_layout"] as? String, "twenty_six_key", engine)
      XCTAssertEqual(ChineseInputScheme.scheme(sharedIdentifier: scheme.sharedIdentifier), scheme)
    }
  }

  func testSchemeTraits() {
    XCTAssertTrue(ChineseInputScheme.cantonese.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.cantonese.composesInPlace)
    XCTAssertFalse(ChineseInputScheme.cantonese.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.zhuyin.composesInPlace)
    XCTAssertFalse(ChineseInputScheme.zhuyin.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.zhuyin.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.vietnamese.composesInPlace)
    XCTAssertTrue(ChineseInputScheme.vietnamese.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.tibetan.composesInPlace)
    XCTAssertTrue(ChineseInputScheme.tibetan.writesAsciiPunctuation)
    XCTAssertTrue(ChineseInputScheme.tibetan.typesCasedLetters)
    XCTAssertFalse(ChineseInputScheme.tibetan.writesChinese)
    XCTAssertFalse(ChineseInputScheme.tibetan.hasSpellingCaret)
    XCTAssertFalse(ChineseInputScheme.tibetan.needsLanguageDictionary)
    XCTAssertTrue(ChineseInputScheme.optInSchemes.contains(.tibetan))
    XCTAssertEqual(ChineseInputScheme.allCases.filter(\.typesCasedLetters), [.vietnamese, .tibetan])
    XCTAssertFalse(ChineseInputScheme.japanese.writesAsciiPunctuation, "Japanese keeps its own marks")
    for scheme in ChineseInputScheme.allCases where ![.cantonese, .zhuyin, .vietnamese, .tibetan].contains(scheme) {
      XCTAssertFalse(scheme.needsLanguageDictionary, scheme.rawValue)
      XCTAssertEqual(scheme.composesInPlace, scheme.isKorean, scheme.rawValue)
      XCTAssertEqual(scheme.hasSpellingCaret, scheme.writesChinese, scheme.rawValue)
    }
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.cantonese), "粤")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.zhuyin), "注")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.vietnamese), "越")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.quanpin), "中")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.vietnamese), "越南语输入")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.tibetan), "藏")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.tibetan), "藏文输入")
    XCTAssertEqual(ChineseInputScheme.tibetan.title, "藏文 26 键")
    XCTAssertEqual(ChineseInputScheme.tibetan.sharedIdentifier, "tibetan")
    XCTAssertEqual(TypingSource.tibetan.rawValue, "tibetan")
    XCTAssertEqual(TypingSource.tibetan.title, "藏文")
    XCTAssertEqual(KeyboardViewController.tibetanSpellingSymbols, ["'", "+", "-", ".", "/"])
    // 藏文的符号页把 `=` 键换成叠写用的 `+`，其他键和其他方案不变。
    XCTAssertEqual(KeyboardViewController.symbolRowKey("=", tibetan: true), "+")
    XCTAssertEqual(KeyboardViewController.symbolRowKey("=", tibetan: false), "=")
    XCTAssertEqual(KeyboardViewController.symbolRowKey("_", tibetan: true), "_")
    XCTAssertEqual(TypingSource.cantonese.rawValue, "cantonese")
    XCTAssertEqual(TypingSource.zhuyin.rawValue, "zhuyin")
    XCTAssertEqual(TypingSource.vietnamese.rawValue, "vietnamese")
  }

  // MARK: - Boundaries

  func testZhuyinAndVietnameseCommitAtEveryBoundary() {
    for scheme in [ChineseInputScheme.zhuyin, .vietnamese, .tibetan] {
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: false, scheme: scheme, boundary: .returnKey), .none)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .returnKey), .commitRaw)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .modeSwitch), .finishComposition)
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: scheme, boundary: .deactivate), .finishComposition)
    }
    for boundary in [CompositionBoundary.modeSwitch, .returnKey] {
      XCTAssertEqual(CompositionBoundaryPolicy.action(composing: true, scheme: .cantonese, boundary: boundary), .commitRaw,
                     "Cantonese keeps its letters like pinyin")
    }
  }

  func testAnInPlaceCompositionIsMarkedWhateverThePreeditSetting() {
    XCTAssertEqual(InlineCompositionPolicy.markedText(
      inPlace: true, style: .off, phrasePrefix: "", preedit: "việt", editingText: "vieejt", japaneseReading: nil), "việt")
  }

  // MARK: - Dachen keys

  func testEveryDachenKeyHasItsBopomofoOrTone() {
    let keys = ZhuyinKeyLayout.rows.flatMap { $0 }
    XCTAssertEqual(keys.count, 41)
    XCTAssertEqual(Set(keys).count, 41)
    let faces = keys.compactMap(ZhuyinKeyLayout.keycap(for:))
    XCTAssertEqual(faces.count, 41)
    XCTAssertEqual(Set(faces).count, 41)
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "1"), "ㄅ")
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "3"), "ˇ")
    XCTAssertEqual(ZhuyinKeyLayout.keycap(for: "/"), "ㄥ")
    XCTAssertNil(ZhuyinKeyLayout.keycap(for: "["))
    for digit in "123456789".map(String.init) { XCTAssertTrue(ZhuyinKeyLayout.selectsWhileListOpen(digit), digit) }
    for key in ["0", "-", "a", ";", ","] { XCTAssertFalse(ZhuyinKeyLayout.selectsWhileListOpen(key), key) }
  }

  func testTheDachenRowsReplaceTheLettersWhileZhuyinIsActive() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "zhuyin.db is not staged into the test host")
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .zhuyin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.viewWillAppear(false)
    let all = nodes(controller.view)
    let rows = all.filter { $0.accessibilityIdentifier == "zhuyinRow" }
    XCTAssertEqual(rows.count, 4)
    XCTAssertTrue(rows.allSatisfy { !$0.isHidden })
    let key = try XCTUnwrap(all.first { $0.accessibilityIdentifier == "zhuyinKeyq" } as? UIButton)
    XCTAssertEqual(key.accessibilityLabel, "注音 ㄆ")
  }

  func testTheZhuyinSymbolPanelFollowsTheChinesePunctuationSwitch() {
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText(",", chinesePunctuation: true), "，")
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText(".", chinesePunctuation: false), ".")
    XCTAssertEqual(KeyboardViewController.zhuyinSymbolText("@", chinesePunctuation: true), "@")
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: nil))
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: "follow"))
    XCTAssertFalse(KeyboardViewController.writesChinesePunctuation(switchOn: false, punctuationLock: "follow"))
    XCTAssertFalse(KeyboardViewController.writesChinesePunctuation(switchOn: true, punctuationLock: "english"))
    XCTAssertTrue(KeyboardViewController.writesChinesePunctuation(switchOn: false, punctuationLock: "chinese"))
  }

  // MARK: - Through the bridge

  func testAnIdleZhuyinToneKeyIsLeftToTheKeyboardToType() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "zhuyin.db is not staged into the test host")
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToZhuyin()
    for tone in ["6", "3", "4", "7"] {
      let snapshot = bridge.handleCharacter(tone)
      XCTAssertFalse(snapshot.isHandled, "the Engine leaves \(tone) to the keyboard, which inserts it")
      XCTAssertNil(snapshot.commitText, tone)
      XCTAssertEqual(snapshot.preedit, "", tone)
    }
    let pending = type("1", into: bridge)
    XCTAssertTrue(pending.isHandled)
    XCTAssertTrue(bridge.handleCharacter("6").isHandled, "a tone key on a pending syllable stays the Engine's")
  }


  func testZhuyinComposesInPlaceAndReturnCommitsTheConversion() throws {
    try XCTSkipUnless(InputSchemePreference.installedLanguageSchemes?.contains(.zhuyin) == true,
                      "zhuyin.db is not staged into the test host")
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToZhuyin()
    let composing = type("su3cl3", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertNil(composing.commitText)
    XCTAssertEqual(composing.preedit, "你好")
    let committed = bridge.commitRaw()
    XCTAssertTrue(committed.isHandled, "Return confirms the conversion, so no newline follows")
    XCTAssertEqual(committed.commitText, "你好")
  }

  func testVietnameseComposesTheWordAndTheFirstCancelRestoresTheKeys() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToVietnamese()
    let composing = type("vieejt", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertEqual(composing.preedit, "việt")
    XCTAssertTrue(composing.candidates.isEmpty)
    let restored = bridge.cancel()
    XCTAssertEqual(restored.preedit, "vieejt")
    XCTAssertEqual(bridge.cancel().preedit, "")
    _ = type("chaof", into: bridge)
    let committed = bridge.commitRaw()
    XCTAssertFalse(committed.isHandled, "Return still inserts its newline after the word")
    XCTAssertEqual(committed.commitText, "chào")
  }

  func testTibetanComposesWylieAndCommitsWithTshegOrShad() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    let composing = type("bkra", into: bridge)
    XCTAssertTrue(composing.isHandled)
    XCTAssertNil(composing.commitText)
    XCTAssertEqual(composing.preedit, "བཀྲ")
    XCTAssertTrue(composing.candidates.isEmpty)
    // 空格（MSIME_COMMIT_CANDIDATE）上屏藏文并加音节点，以已处理返回，键盘不再插入空格。
    let tsheg = bridge.commitCandidate()
    XCTAssertTrue(tsheg.isHandled)
    XCTAssertEqual(tsheg.commitText, "བཀྲ་")
    _ = type("shis", into: bridge)
    let shad = bridge.handleCharacter("/")
    XCTAssertTrue(shad.isHandled)
    XCTAssertEqual(shad.commitText, "ཤིས།")
    let idleShad = bridge.handleCharacter("/")
    XCTAssertTrue(idleShad.isHandled, "没有组字时斜杠单独输入垂符")
    XCTAssertEqual(idleShad.commitText, "།")
  }

  func testTibetanReturnCommitsWithoutTshegAndUppercaseIsSpelling() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    // 威利转写区分大小写：`Ta` 是反写的 ཊ，不是 ཏ。
    XCTAssertTrue(bridge.handleCharacter("T", shifted: true).isHandled)
    XCTAssertEqual(type("a", into: bridge).preedit, "ཊ")
    let committed = bridge.commitRaw()
    XCTAssertTrue(committed.isHandled, "回车只确认藏文，不换行")
    XCTAssertEqual(committed.commitText, "ཊ")
    // `'` 在没有组字时也能开头（achung），`+` 叠写。
    XCTAssertEqual(type("'od", into: bridge).preedit, "འོད")
    XCTAssertEqual(bridge.commitRaw().commitText, "འོད")
    XCTAssertEqual(type("pad+ma", into: bridge).preedit, "པདྨ")
    XCTAssertEqual(bridge.commitRaw().commitText, "པདྨ")
  }

  func testTibetanFirstCancelRestoresTheWylie() {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToTibetan()
    _ = type("bkra", into: bridge)
    XCTAssertEqual(bridge.cancel().preedit, "bkra")
    XCTAssertEqual(bridge.cancel().preedit, "")
  }

  func testTibetanNeverReplacesTheRememberedChineseScheme() {
    var document: [String: Any] = ["last_chinese_scheme": "shuangpin"]
    MetasequoiaInputSessionBridge.schemeMapping(.tibetan, enabledSchemes: [.quanpin, .tibetan])?(&document)
    XCTAssertEqual(document["scheme"] as? String, "tibetan")
    XCTAssertEqual(document["last_chinese_scheme"] as? String, "shuangpin")
  }
}
