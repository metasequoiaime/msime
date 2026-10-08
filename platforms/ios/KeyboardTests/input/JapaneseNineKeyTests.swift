import XCTest
import UIKit

@MainActor
final class JapaneseNineKeyTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  func testKanaKeysFeedJapaneseEngineCandidates() throws {
    let previous = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    // The candidate the Engine returns is what is being checked. A gloss appends a second line to
    // the chip's title, so leaving it on turns a suffix check into a check of the gloss layout.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = false
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .japaneseNineKey
    XCTAssertEqual(InputSchemePreference.scheme, .japaneseNineKey)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(nodes(controller.view).compactMap { $0 as? JapaneseNineKeyView }.first)
    XCTAssertFalse(panel.isHidden)
    let modeColumn = try XCTUnwrap(nodes(panel).first { $0.accessibilityIdentifier == "japaneseModeColumn" } as? UIStackView)
    XCTAssertEqual(modeColumn.arrangedSubviews.count, 4)
    let japaneseReturn = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "japaneseReturn" } as? UIButton)
    XCTAssertFalse(japaneseReturn.isHidden)
    let sharedReturn = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "returnKey" })
    XCTAssertTrue(sharedReturn.superview?.isHidden == true)
    panel.select(4, direction: 1) // に
    panel.select(5, direction: 4) // ほ
    panel.select(9, direction: 2) // ん
    let first = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "candidate-1" } as? UIButton)
    XCTAssertTrue(first.configuration?.title?.hasSuffix("日本") == true, first.configuration?.title ?? "No candidate")
    let screenshot = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    screenshot.name = "Japanese nine-key candidates"; screenshot.lifetime = .keepAlways; add(screenshot)
    let picker = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeButton" } as? UIButton)
    picker.sendActions(for: .primaryActionTriggered)
    for name in ["japanese", "japaneseNineKey"] {
      XCTAssertNotNil(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-\(name)" })
    }
    let roman = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-japanese" } as? UIButton)
    roman.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .japanese)
    XCTAssertTrue(panel.isHidden)
    controller.view.layoutIfNeeded()
    let letter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == "字母 A" } as? UIButton)
    XCTAssertGreaterThan(letter.bounds.height, 40)
    picker.sendActions(for: .primaryActionTriggered)
    let nine = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "schemeCard-japaneseNineKey" } as? UIButton)
    nine.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .japaneseNineKey)
    XCTAssertFalse(panel.isHidden)
  }

  func testJapaneseCommitReadingKeepsTheTypedKana() {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    let typed = bridge.handleCharacter("a")
    XCTAssertEqual(typed.reading, "あ")
    let committed = bridge.commitReading()
    XCTAssertEqual(committed.commitText, "あ")
    XCTAssertTrue(committed.reading.isEmpty)
  }

  func testJapaneseConversionUsesSpaceToSelectAndReturnToCommit() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .japaneseNineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414,
                                   height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(nodes(controller.view).compactMap { $0 as? JapaneseNineKeyView }.first)
    let space = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "japaneseSpace" } as? UIButton)
    let enter = try XCTUnwrap(nodes(controller.view).first { $0.accessibilityIdentifier == "japaneseReturn" } as? UIButton)

    panel.select(0, direction: 0)
    XCTAssertEqual(space.configuration?.title, "変換")
    XCTAssertEqual(enter.configuration?.title, "確定")
    space.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(space.configuration?.title, "変換")
    XCTAssertEqual(enter.configuration?.title, "確定")
    enter.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(space.configuration?.title, "空白")
    XCTAssertEqual(enter.configuration?.title, "改行")
  }

  /// A lone Fallback row is the raw composition, which Space commits the way Windows does rather than converting.
  func testJapaneseSpaceCommitsALoneFallbackRow() {
    let fallback = KeyboardViewController.candidateSourceFallback
    XCTAssertFalse(KeyboardViewController.japaneseSpaceConverts(candidateCount: 1, firstSource: fallback))
    XCTAssertTrue(KeyboardViewController.japaneseSpaceConverts(candidateCount: 1, firstSource: 0))
    XCTAssertTrue(KeyboardViewController.japaneseSpaceConverts(candidateCount: 2, firstSource: fallback))
    XCTAssertFalse(KeyboardViewController.japaneseSpaceConverts(candidateCount: 0, firstSource: -1))
  }

  func testSnapshotCarriesEachCandidateSource() {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    _ = bridge.handleCharacter("k")
    let snapshot = bridge.handleCharacter("a")
    XCTAssertFalse(snapshot.candidates.isEmpty)
    XCTAssertEqual(snapshot.candidateSources.count, snapshot.candidates.count)
    XCTAssertFalse(snapshot.candidateSources.contains(-1), "every Engine row names its source")
  }

  func testEveryKanaKeyConvertsAndLayoutsKeepFullHeight() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    for key in JapaneseNineKeyView.keys {
      for (kana, input) in zip(key.kana, key.strokes) where !input.isEmpty {
        _ = bridge.cancel()
        var snapshot: MetasequoiaInputSnapshot?
        for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
        XCTAssertTrue(snapshot?.candidates.contains(kana) == true,
                      "\(input) → \(kana): \(snapshot?.diagnosticText ?? String(describing: snapshot?.candidates ?? []))")
      }
    }
    for width in [320.0, 414.0] {
      let panel = JapaneseNineKeyView { title, _, action in
        var config = UIButton.Configuration.plain(); config.title = title
        return UIButton(configuration: config, primaryAction: UIAction { _ in action() })
      }
      panel.frame = CGRect(x: 0, y: 0, width: width, height: 176)
      panel.applyLayout(); panel.layoutIfNeeded()
      let buttons = nodes(panel).compactMap { $0 as? UIButton }
      XCTAssertEqual(buttons.count, 13)
      for button in buttons {
        XCTAssertGreaterThan(button.bounds.height, 45)
        XCTAssertGreaterThan(button.bounds.width, 44)
        XCTAssertLessThanOrEqual(button.convert(button.bounds, to: panel).maxX, width + 0.5)
      }
    }
  }

  /// The mode column covers exactly the four rows of the kana grid, in the proportions a real
  /// kana keyboard uses: one row each, except the script key, which takes two whenever the system
  /// draws the globe itself.
  ///
  /// The ratios alone do not say this. Writing each key's height against the column - a `.fill`
  /// stack, whose own height its children decide - computes the right numbers through a cycle,
  /// and a solver is free to settle it differently: iOS 27 made 123 2.33pt taller than ^_^ and
  /// overflowed the column by 2pt while every ratio here still looked plausible. So this checks
  /// that the one-row keys match each other exactly and that the parts add up to the whole.
  func testModeColumnGivesScriptKeyTwoRowsWhenTheGlobeIsSystemOwned() throws {
    let modeKeys = (0..<4).map { index in
      let button = UIButton(type: .system)
      button.accessibilityIdentifier = "mode-\(index)"
      return button
    }
    let panel = JapaneseNineKeyView(makeKey: { title, _, action in
      UIButton(type: .system, primaryAction: UIAction { _ in action() })
    }, modeKeys: modeKeys)
    panel.frame = CGRect(x: 0, y: 0, width: 414, height: 220)
    panel.layoutIfNeeded()
    let column = try XCTUnwrap(
      nodes(panel).first { $0.accessibilityIdentifier == "japaneseModeColumn" } as? UIStackView)

    // The system draws the globe below the keyboard, so ours is hidden and the script key covers
    // the row it would have taken.
    modeKeys[3].isHidden = true
    panel.setModeColumnFull(false)
    panel.layoutIfNeeded()
    let single = modeKeys[0].bounds.height
    XCTAssertGreaterThan(single, 40, "a mode key that short is not a touch target")
    XCTAssertEqual(modeKeys[1].bounds.height, single, accuracy: 1,
                   "the one-row mode keys have to be the same height as each other")
    XCTAssertEqual(modeKeys[2].bounds.height, single * 2 + column.spacing, accuracy: 1.5,
                   "two rows means two keys plus the gap between them")
    // The three visible keys and their two gaps fill the column, with nothing left over.
    //
    // The tolerance scales with the key count rather than being a flat constant: one row's ideal
    // height carries a fraction (53.5pt in a 220pt column), layout rounds each key to a point, and
    // three roundings accumulate. What this is actually asking - whether a gap or a key was left
    // out of the split - is off by 7pt or more, which this still catches.
    let used = [modeKeys[0], modeKeys[1], modeKeys[2]].map(\.bounds.height).reduce(0, +)
      + column.spacing * 2
    XCTAssertEqual(used, column.bounds.height, accuracy: 3, "the column was not filled")

    // With the globe ours to draw, all four keys take one row each.
    modeKeys[3].isHidden = false
    panel.setModeColumnFull(true)
    panel.layoutIfNeeded()
    let full = modeKeys[0].bounds.height
    for key in modeKeys.dropFirst() {
      XCTAssertEqual(key.bounds.height, full, accuracy: 1,
                     "every key takes one row once the globe is ours")
    }
    XCTAssertEqual(full * 4 + column.spacing * 3, column.bounds.height, accuracy: 4,
                   "the column was not filled")
  }

  func testJapaneseNineKeyDigitLayerUsesSymbolsAndKeepsKanaPunctuation() throws {
    var symbols: [String] = []
    let panel = JapaneseNineKeyView { title, _, action in
      var config = UIButton.Configuration.plain(); config.title = title
      return UIButton(configuration: config, primaryAction: UIAction { _ in action() })
    }
    panel.onSymbol = { symbols.append($0) }
    panel.setDigits(true)
    panel.select(0, direction: 0)
    panel.select(0, direction: 1)
    panel.select(0, direction: 4) // Empty fifth slot must not emit an empty symbol.
    panel.select(9, direction: 3)
    XCTAssertEqual(symbols, ["1", "☆", "ー"])
    panel.setDigits(false)
    panel.select(7, direction: 1)
    XCTAssertEqual(symbols.last, "「")
    panel.select(10, direction: 1)
    XCTAssertEqual(symbols.last, "。")
  }

  func testKanaVariantButtonDelegatesToEngineOnlyWhileComposing() throws {
    var variantActivations = 0
    let panel = JapaneseNineKeyView { title, _, action in
      var config = UIButton.Configuration.plain(); config.title = title
      return UIButton(configuration: config, primaryAction: UIAction { _ in action() })
    }
    panel.onVariant = { variantActivations += 1 }
    let variants = try XCTUnwrap(nodes(panel).first { $0.accessibilityIdentifier == "japaneseVariants" } as? UIButton)
    variants.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(variantActivations, 0)
    panel.setComposing(true)
    variants.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(variantActivations, 1)
    panel.setDigits(true)
    variants.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(variantActivations, 1)
  }

  func testKanaKeysUseFlickPreviewInsteadOfLongPressMenus() throws {
    let panel = JapaneseNineKeyView { title, _, action in
      var config = UIButton.Configuration.plain(); config.title = title
      return UIButton(configuration: config, primaryAction: UIAction { _ in action() })
    }
    let kana = try XCTUnwrap(nodes(panel).first { $0.accessibilityIdentifier == "japaneseKana0" } as? UIButton)
    XCTAssertNil(kana.menu)
    panel.setDigits(true)
    XCTAssertNotNil(kana.menu, "The numeric/symbol layer still exposes its alternate symbols")
    panel.setDigits(false)
    XCTAssertNil(kana.menu)
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { nodes($0) } }
}
