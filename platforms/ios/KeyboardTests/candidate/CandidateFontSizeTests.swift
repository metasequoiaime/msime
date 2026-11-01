import UIKit
import XCTest

/// The candidate and composition sizes come from the shared document the desktop settings also write, so a size synced from a desktop has to fit the surface drawing it.
@MainActor
final class CandidateFontSizeTests: XCTestCase {
  private var state: URL!

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-candidate-font-\(UUID().uuidString)", isDirectory: true)
  }

  override func tearDown() {
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  func testDefaultsDrawTheStylesTheKeyboardAlreadyUsed() {
    XCTAssertEqual(CandidateFontPreference.candidateScale(in: [:], tablet: false), 1)
    XCTAssertEqual(CandidateFontPreference.preeditScale(in: nil, tablet: true), 1)
    XCTAssertEqual(
      CandidateFontPreference.font(.body, scale: 1).pointSize,
      UIFont.preferredFont(forTextStyle: .body).pointSize)
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(glossLines: 1), KeyboardViewController.glossedCandidateRowHeight,
      "the first gloss line sits inside the 42pt candidate line, and the reading shares that line")
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(glossLines: 1, separateReadingRow: true),
      KeyboardViewController.readingRowHeight + KeyboardViewController.glossedCandidateRowHeight,
      "the 14-key reading line sits over the candidate line, as on Android")
    XCTAssertEqual(KeyboardViewController.preeditFont(scale: 1).pointSize, 12, "the design's 12pt spelling")
  }

  /// 顶栏是工具栏和候选共用的一行：读音并入候选行，整行只有候选行那么高（#7405），但不低于工具栏按钮 40pt 的底板加上下各 1pt；不论显示什么，都和输入中布局需要的一样高。全拼 14 键的读音单独一行，多出 14pt。
  func testTheTopRowIsOnlyAsTallAsTheCandidateLine() {
    XCTAssertEqual(KeyboardViewController.topRowMinimumHeight, KeyboardToolbarButton.padSide + 2)
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 0), 42, "a bare 38pt candidate line still fits the toolbar pad")
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 1), 42, "one gloss line costs 4pt, not a 14pt line of its own")
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 2), 42 + KeyboardViewController.glossLineHeight)
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 0, separateReadingRow: true), 52)
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 1, separateReadingRow: true), 56)
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(glossLines: 0, candidateScale: 12.0 / 18.0, preeditScale: 12.0 / 15.0), 42)
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(glossLines: 1, preeditScale: 2), 42,
      "a larger spelling only takes width when it shares the candidate line")
  }

  /// 候选条的词块使用设计稿的 18pt 候选字号，像桌面候选窗一样按同步的「候选字号」缩放，并使用所选的字体链。
  func testStripChipsDrawAtEighteenPointsTimesTheSize() {
    XCTAssertEqual(KeyboardViewController.candidateChipFontSize, 18)
    XCTAssertEqual(KeyboardViewController.candidateChipFont(scale: 1, families: []).pointSize, 18)
    XCTAssertEqual(KeyboardViewController.candidateChipFont(scale: 24.0 / 18.0, families: []).pointSize, 24)
    XCTAssertEqual(KeyboardViewController.candidateChipFont(scale: 12.0 / 18.0, families: []).pointSize, 12)
    let chained = KeyboardViewController.candidateChipFont(scale: 1, families: ["Georgia", "PingFang SC"])
    XCTAssertEqual(chained.familyName, "Georgia")
    XCTAssertEqual(chained.pointSize, 18)
    XCTAssertEqual(KeyboardViewController.candidateGlossFontSize, 10)
  }

  /// The chain runs Latin face, family, fallbacks, drops what the device lacks and repeats, and is empty for a document synced from Windows, which leaves the system font.
  func testFamilyChainKeepsInstalledFacesInDesktopOrder() {
    let has: Set<String> = ["Helvetica Neue", "PingFang SC", "Georgia"]
    let document: [String: Any] = [
      CandidateFontPreference.englishFamilyKey: "Georgia", CandidateFontPreference.familyKey: "PingFang SC",
      CandidateFontPreference.fallbackFamiliesKey: ["Microsoft YaHei", "PingFang SC", "Helvetica Neue"],
    ]
    XCTAssertEqual(CandidateFontPreference.families(in: document, installed: has.contains),
                   ["Georgia", "PingFang SC", "Helvetica Neue"])
    XCTAssertEqual(CandidateFontPreference.families(in: [:], installed: has.contains), [])
    XCTAssertEqual(CandidateFontPreference.families(in: [CandidateFontPreference.familyKey: "Noto Sans SC",
      CandidateFontPreference.fallbackFamiliesKey: ["Noto Sans SC", "Microsoft YaHei"]]), [],
      "the shared defaults name no face iOS ships")
  }

  /// The leading face draws the letters, the cascade the Han characters, and the size still follows the scale.
  func testFamilyFontLeadsWithTheFirstFaceAtTheScaledSize() {
    let font = CandidateFontPreference.font(.body, scale: 1, families: ["Georgia", "PingFang SC"])
    XCTAssertEqual(font.familyName, "Georgia")
    let cascade = font.fontDescriptor.fontAttributes[.cascadeList] as? [UIFontDescriptor]
    XCTAssertEqual(cascade?.map { $0.fontAttributes[.family] as? String }, ["PingFang SC"])
    XCTAssertEqual(font.pointSize, CandidateFontPreference.font(.body, scale: 1).pointSize, accuracy: 0.5)
    XCTAssertEqual(CandidateFontPreference.font(.body, scale: 1, families: []), CandidateFontPreference.font(.body, scale: 1))
  }

  func testAPhoneClampsADesktopSizeThatAnIPadKeeps() {
    let synced: [String: Any] = [CandidateFontPreference.candidateKey: 30, CandidateFontPreference.preeditKey: 24]
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: synced, tablet: false), 24)
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: synced, tablet: true), 30)
    XCTAssertEqual(CandidateFontPreference.preeditSize(in: synced, tablet: false), 20)
    XCTAssertEqual(CandidateFontPreference.preeditSize(in: synced, tablet: true), 24)
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: [CandidateFontPreference.candidateKey: 4], tablet: true), 12)
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: [CandidateFontPreference.candidateKey: 20.5], tablet: false), 18)
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: [CandidateFontPreference.candidateKey: true], tablet: false), 18)
  }

  /// 「候选栏大小」（#10137）：共享偏好 `candidate_scale_percent`，与 client-core、Android 和鸿蒙一样只认 50–200 的整数，其余按 100。
  func testCandidateBarSizeReadsTheSharedRange() {
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: nil), 100)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [:]), 100)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: 50]), 50)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: 200]), 200)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: 49]), 100)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: 201]), 100)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: 87.5]), 100)
    XCTAssertEqual(CandidateFontPreference.scalePercent(in: [CandidateFontPreference.scaleKey: true]), 100)
    XCTAssertEqual(CandidateFontPreference.barScale(in: [CandidateFontPreference.scaleKey: 80]), 0.8, accuracy: 0.0001)
    XCTAssertEqual(CandidateFontPreference.scaleRange, 50...200)
    XCTAssertEqual((CandidateFontPreference.defaultScalePercent - CandidateFontPreference.scaleRange.lowerBound)
      % CandidateFontPreference.scaleStep, 0, "the slider can land on 100%")
  }

  /// 调小候选栏时顶栏这一行连同它的下限一起变矮，空闲时的工具栏和组字时的候选栏仍然同高，按键不跳；100% 时与原来完全一样。
  func testCandidateBarSizeScalesTheWholeTopRow() {
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 0, barScale: 1), 42)
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 1, barScale: 1), 42)
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 0, candidateScale: 0.5, preeditScale: 0.5, barScale: 0.5), 21)
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(
        glossLines: 0, candidateScale: 0.5, preeditScale: 0.5, separateReadingRow: true, barScale: 0.5), 26,
      "the 14-key reading line shrinks with the candidates")
    XCTAssertEqual(KeyboardViewController.topRowHeight(glossLines: 0, candidateScale: 2, preeditScale: 2, barScale: 2), 84)
    XCTAssertEqual(
      KeyboardViewController.topRowHeight(glossLines: 2, candidateScale: 0.5, preeditScale: 0.5, barScale: 0.5),
      21 + KeyboardViewController.glossLineHeight / 2, "extra gloss lines shrink with the bar")
    XCTAssertLessThan(
      KeyboardFormFactor.phone.keyboardHeight(
        topRow: KeyboardViewController.topRowHeight(glossLines: 0, candidateScale: 0.8, preeditScale: 0.8, barScale: 0.8),
        rowSpacing: 7, landscape: false, handwriting: false),
      KeyboardFormFactor.phone.keyboardHeight(
        topRow: KeyboardViewController.topRowHeight(glossLines: 0), rowSpacing: 7, landscape: false, handwriting: false),
      "a smaller bar gives its height back instead of padding the row")
  }

  /// 工具栏按钮的图标、底板和品牌标志随这一行缩小，不伸出被裁掉；行更高时保持设计尺寸。
  func testToolbarContentShrinksWithAShortRowOnly() {
    XCTAssertEqual(KeyboardToolbarButton.contentScale(rowHeight: KeyboardViewController.topRowMinimumHeight), 1)
    XCTAssertEqual(KeyboardToolbarButton.contentScale(rowHeight: 21), 0.5, accuracy: 0.0001)
    XCTAssertEqual(KeyboardToolbarButton.contentScale(rowHeight: 84), 1)
    XCTAssertEqual(KeyboardToolbarButton.contentScale(rowHeight: 0), 1)
  }

  func testLargerTextGrowsTheStripButSmallerTextKeepsTheTouchTarget() {
    let base = KeyboardViewController.topRowHeight(glossLines: 0)
    let larger = KeyboardViewController.topRowHeight(
      glossLines: 0, candidateScale: 24.0 / 18.0, preeditScale: 20.0 / 15.0)
    let smaller = KeyboardViewController.topRowHeight(
      glossLines: 0, candidateScale: 12.0 / 18.0, preeditScale: 12.0 / 15.0)
    XCTAssertGreaterThan(larger, base)
    XCTAssertEqual(smaller, base)
    XCTAssertEqual(
      KeyboardFormFactor.phone.keyboardHeight(topRow: larger, rowSpacing: 7, landscape: false, handwriting: false)
        - KeyboardFormFactor.phone.keyboardHeight(topRow: base, rowSpacing: 7, landscape: false, handwriting: false),
      larger - base, "the keyboard grows by what the top row grew instead of taking it out of the keys")
  }

  func testTheExpandedPanelDrawsCandidatesAtTheChosenSize() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["水杉"], preedit: "shuishan", candidateScale: 24.0 / 18.0, preeditScale: 1,
      display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    panel.layoutIfNeeded()
    let chip = try XCTUnwrap(find("panelCandidate-1", in: panel) as? UIButton)
    let title = try XCTUnwrap(chip.configuration?.attributedTitle)
    let font = try XCTUnwrap(NSAttributedString(title).attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
    XCTAssertGreaterThan(font.pointSize, UIFont.preferredFont(forTextStyle: .body).pointSize)
  }

  func testTheExpandedPanelHeaderLeadsWithTheBrandMark() throws {
    let panel = KeyboardCandidatePanelView(
      candidates: ["水杉"], preedit: "shuishan", display: { $0 }, onSelect: { _ in }, onClose: {})
    panel.frame = CGRect(x: 0, y: 0, width: 390, height: 300)
    panel.layoutIfNeeded()
    let mark = try XCTUnwrap(find("candidatePanelBrandIcon", in: panel) as? UIImageView)
    let spelling = try XCTUnwrap(find("candidatePanelSpelling", in: panel))
    let header = try XCTUnwrap(mark.superview as? UIStackView)
    XCTAssertTrue(header.arrangedSubviews.first === mark, "the mark leads the header")
    XCTAssertTrue(spelling.superview === header)
    XCTAssertNotNil(mark.image)
    // 固定配色的完整标志，原色绘制，不随皮肤强调色着色。
    XCTAssertEqual(mark.image?.renderingMode, .alwaysOriginal)
    XCTAssertEqual(mark.bounds.width, 16, accuracy: 0.1)
    XCTAssertEqual(mark.bounds.height, 16, accuracy: 0.1)
    XCTAssertEqual(spelling.frame.minX - mark.frame.maxX, 6, accuracy: 0.5)
    XCTAssertFalse(mark.isUserInteractionEnabled)
    XCTAssertFalse(mark.isAccessibilityElement)
  }

  func testTheLiveSessionSeesASizeTheSettingsAppWrote() async {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0[CandidateFontPreference.candidateKey] = 22
    })
    let reloaded = expectation(description: "reload")
    var accepted = false
    bridge.reloadSharedPreferences { accepted = $0; reloaded.fulfill() }
    await fulfillment(of: [reloaded], timeout: 15)
    XCTAssertTrue(accepted, "the reload was refused")
    XCTAssertEqual(CandidateFontPreference.candidateSize(in: bridge.sharedPreferences, tablet: false), 22)
  }

  private func find(_ identifier: String, in view: UIView) -> UIView? {
    if view.accessibilityIdentifier == identifier { return view }
    for subview in view.subviews {
      if let found = find(identifier, in: subview) { return found }
    }
    return nil
  }

  func testFallbackFamiliesKeepTheStoredOrderAndAppendOnceWithinTheLimit() {
    XCTAssertEqual(CandidateFontPreference.fallbackFamilies(in: nil), ["Noto Sans SC", "Microsoft YaHei"])
    XCTAssertEqual(CandidateFontPreference.fallbackFamilies(in: [CandidateFontPreference.fallbackFamiliesKey: []]), [])
    let stored: [String: Any] = [CandidateFontPreference.fallbackFamiliesKey: ["Segoe UI", 3, "PingFang SC"]]
    XCTAssertEqual(CandidateFontPreference.fallbackFamilies(in: stored), ["Segoe UI", "PingFang SC"])

    XCTAssertEqual(CandidateFontPreference.appending("Kailasa", to: ["PingFang SC"]), ["PingFang SC", "Kailasa"])
    XCTAssertEqual(CandidateFontPreference.appending("PingFang SC", to: ["PingFang SC"]), ["PingFang SC"])
    XCTAssertEqual(CandidateFontPreference.appending("", to: []), [])
    let full = (0..<CandidateFontPreference.maximumFallbackFamilies).map { "Family \($0)" }
    XCTAssertEqual(CandidateFontPreference.appending("One More", to: full), full)
  }
}
