import UIKit
import XCTest

/// 符号混输的行（打「立方米」出的「㎥」「m³」，来源是 emoji）永远没有释义，候选条上的词块只按符号本身取宽，不因为释义开着就留出一栏（候选栏的三分之一）。鸿蒙曾经一律留一栏，一屏只剩三个候选、符号后面空出一大段。
@MainActor
final class SymbolCandidateChipTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
                  "No button with accessibility identifier \(identifier).")
  }

  func testSymbolRowsStayAsNarrowAsTheirSymbolWithGlossesOn() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    setCandidateGloss(true)
    // 九键的 54 3264 64 是 li fang mi，7464 3264 64 是 ping fang mi。
    for (digits, word, symbols) in [("54326464", "立方米", ["㎥", "m³"]), ("7464326464", "平方米", ["㎡", "m²"])] {
      InputSchemePreference.scheme = .nineKey
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.viewWillAppear(false)
      for digit in digits { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
      controller.view.layoutIfNeeded()
      let chips = descendants(controller.view).compactMap { $0 as? KeyboardKeyButton }
        .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
      func chip(_ text: String) throws -> KeyboardKeyButton {
        try XCTUnwrap(chips.first { $0.accessibilityLabel?.hasSuffix("：\(text)") == true }, "候选条上没有 \(text)")
      }
      XCTAssertNotNil(try chip(word).constraints.first { $0.identifier == "candidateChipWidth" && $0.isActive },
                      "释义开着时词按释义行取宽")
      let strip = try XCTUnwrap(chips.first?.superview?.superview)
      for symbol in symbols {
        let symbolChip = try chip(symbol)
        XCTAssertNil(symbolChip.constraints.first { $0.identifier == "candidateChipWidth" && $0.isActive },
                     "\(symbol) 没有释义行，不按释义列取宽")
        XCTAssertFalse(symbolChip.configuration?.attributedTitle.map { String($0.characters) }?.contains("\n") ?? false,
                       "\(symbol) 不画释义行")
        XCTAssertLessThan(symbolChip.bounds.width, strip.bounds.width / 6,
                          "\(symbol) 的词块只按符号取宽，不能占去一栏：\(symbolChip.bounds.width) / \(strip.bounds.width)")
      }
      controller.viewDidDisappear(false)
    }
  }

  /// 皮肤的 `item_min_width` 是桌面候选窗的格宽，不套到候选条上：`base = "microsoft"` 的皮肤出厂 3.5em，候选字号 24 时原先每个词块至少 84pt，「立方米」「㎥」「m³」三个等宽大格占满一屏。候选条的词块只按内容取宽，下限是 30pt 的触控宽度。
  func testDesktopSkinMinimumWidthDoesNotPadStripChips() throws {
    preserveSharedTheme()
    setCandidateGloss(false)
    let defaults = CandidatePalette.defaults
    let follows = defaults.object(forKey: CandidatePalette.followsDesktopKey)
    let previousScheme = InputSchemePreference.scheme
    let previousFont = MetasequoiaInputSessionBridge.loadSharedPreferences()?[CandidateFontPreference.candidateKey]
    addTeardownBlock {
      defaults.set(follows, forKey: CandidatePalette.followsDesktopKey)
      InputSchemePreference.scheme = previousScheme
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[CandidateFontPreference.candidateKey] = previousFont }
    }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let id = "ios-microsoft-" + UUID().uuidString.lowercased()
    let source = root.appendingPathComponent(id)
    try FileManager.default.createDirectory(at: source, withIntermediateDirectories: true)
    addTeardownBlock { try? FileManager.default.removeItem(at: root) }
    try """
    schema_version = 1
    id = '\(id)'
    name = '微软基底'
    version = '1.0.0'
    base = 'microsoft'
    [supports]
    layouts = ['horizontal', 'vertical']
    themes = ['light', 'dark']
    [candidate_window]
    """.write(to: source.appendingPathComponent("skin.toml"), atomically: true, encoding: .utf8)
    let installed = root.appendingPathComponent("installed")
    XCTAssertEqual(ExternalCandidateSkin.importFolder(source, root: installed), .success(id))
    defaults.set(true, forKey: CandidatePalette.followsDesktopKey)
    XCTAssertTrue(GlobalThemePreference.update {
      $0["global_theme"] = "custom"
      $0["custom_theme"] = ["candidate_skin": id, "candidate_skin_dark": id]
      $0[CandidateFontPreference.candidateKey] = 24
    })
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.candidateSkinsRoot = installed
    controller.overrideUserInterfaceStyle = .light
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    // 皮肤的配色在后台解析完才交给候选条。
    RunLoop.main.run(until: Date().addingTimeInterval(0.6))
    for digit in "54326464" { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
    controller.view.layoutIfNeeded()
    let chips = descendants(controller.view).compactMap { $0 as? KeyboardKeyButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
    let stack = try XCTUnwrap(chips.first?.superview as? UIStackView)
    XCTAssertEqual(stack.spacing, 10, accuracy: 0.5, "皮肤版式确实生效：间距是 0.15em 加 6")
    func width(_ text: String) throws -> CGFloat {
      try XCTUnwrap(chips.first { $0.accessibilityLabel?.hasSuffix("：\(text)") == true }, "候选条上没有 \(text)").bounds.width
    }
    let word = try width("立方米")
    let symbol = try width("㎥")
    XCTAssertLessThan(symbol, 84 - 0.5, "㎥ 不能被皮肤的 3.5em 撑到 84pt")
    XCTAssertGreaterThanOrEqual(symbol, KeyboardViewController.candidateChipMinimumSize.width - 0.5, "触控下限照旧")
    XCTAssertGreaterThan(word, symbol + 20, "词块按内容取宽，三字词明显比一个符号宽：\(word) / \(symbol)")
    controller.viewDidDisappear(false)
  }
}
