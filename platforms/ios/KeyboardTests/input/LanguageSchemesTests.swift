import XCTest
import UIKit

/// Cantonese, Zhuyin and Vietnamese: which of them the keyboard offers, how they reach the shared preferences, the Dachen keys, and the in-place compositions as the keyboard sees them through the bridge.
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
    XCTAssertEqual(enabled, ChineseInputScheme.allCases.filter { ![.cantonese, .zhuyin, .vietnamese].contains($0) })
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
    for (scheme, engine, remembered) in [(ChineseInputScheme.cantonese, "cantonese", true), (.zhuyin, "zhuyin", true), (.vietnamese, "vietnamese", false)] {
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
    XCTAssertFalse(ChineseInputScheme.japanese.writesAsciiPunctuation, "Japanese keeps its own marks")
    for scheme in ChineseInputScheme.allCases where ![.cantonese, .zhuyin, .vietnamese].contains(scheme) {
      XCTAssertFalse(scheme.needsLanguageDictionary, scheme.rawValue)
      XCTAssertEqual(scheme.composesInPlace, scheme.isKorean, scheme.rawValue)
      XCTAssertEqual(scheme.hasSpellingCaret, scheme.writesChinese, scheme.rawValue)
    }
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.cantonese), "粤")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.zhuyin), "注")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.vietnamese), "越")
    XCTAssertEqual(KeyboardViewController.languageKeyTitle(.quanpin), "中")
    XCTAssertEqual(KeyboardViewController.languageKeyValue(.vietnamese), "越南语输入")
    XCTAssertEqual(TypingSource.cantonese.rawValue, "cantonese")
    XCTAssertEqual(TypingSource.zhuyin.rawValue, "zhuyin")
    XCTAssertEqual(TypingSource.vietnamese.rawValue, "vietnamese")
  }

  // MARK: - Boundaries

  func testZhuyinAndVietnameseCommitAtEveryBoundary() {
    for scheme in [ChineseInputScheme.zhuyin, .vietnamese] {
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

  // MARK: - Through the bridge

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
}
