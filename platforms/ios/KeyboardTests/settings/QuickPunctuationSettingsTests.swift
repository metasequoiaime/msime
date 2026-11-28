import XCTest
import UIKit

/// 快捷符号栏只接受单字符、去重后的有限列表，避免异常 App Group 数据把键盘菜单撑大。
final class QuickPunctuationSettingsTests: XCTestCase {
  private var stored: Any?
  private var storedReturnSymbols: Any?

  override func setUp() {
    super.setUp()
    stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    storedReturnSymbols = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
  }

  override func tearDown() {
    if let stored {
      KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    } else {
      KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    }
    if let storedReturnSymbols {
      KeyboardLayoutPreference.defaults.set(storedReturnSymbols, forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
    } else {
      KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
    }
    super.tearDown()
  }

  func testDefaultsAndCustomSymbols() {
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols,
                   KeyboardLayoutPreference.defaultQuickPunctuationSymbols)
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    XCTAssertTrue(KeyboardLayoutPreference.saveQuickPunctuationSymbols(["☆", "☆", "", "\n", "😀"], stateRoot: root))
    XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root)?["touch_quick_punctuation_symbols"] as? [String], ["☆", "😀"])
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols, ["☆", "😀"])
  }

  func testInvalidStoredValueFallsBackAndInputIsBounded() {
    KeyboardLayoutPreference.defaults.set(["ab", "\u{0000}"],
                                           forKey: KeyboardLayoutPreference.quickPunctuationSymbolsKey)
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols,
                   KeyboardLayoutPreference.defaultQuickPunctuationSymbols)
    let text = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳"
    XCTAssertEqual(KeyboardLayoutPreference.quickPunctuationSymbols(from: text).count, 12)
  }
  @MainActor
  func testCustomSymbolsReachPhoneKeyAndMenu() throws {
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_quick_punctuation_symbols"]
    let scheme = InputSchemePreference.scheme
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_quick_punctuation_symbols"] = previous }
      InputSchemePreference.scheme = scheme
    }
    XCTAssertTrue(KeyboardLayoutPreference.saveQuickPunctuationSymbols(["☆", "=", "！"]))
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "quickPunctuationKey" } as? KeyboardKeyButton)
    XCTAssertFalse(key.isHidden)
    XCTAssertEqual(key.configuration?.title, "☆")
    XCTAssertEqual(key.menu?.children.map(\.title), ["☆", "=", "！"])
    XCTAssertNil(key.onCornerHint, "自定义菜单使用系统长按，不与固定句号手势竞争")
    controller.viewDidDisappear(false)
  }

  @MainActor
  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  @MainActor
  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton)
  }

  /// 判定只比字面：填 `，` 命中符号页画的 `，`，也命中「符」面板送出 `,` 的那个键；中文标点关着时只认 `,`。
  func testSymbolReturnPolicyComparesEveryFace() {
    XCTAssertFalse(SymbolReturnPolicy.shouldReturn(face: "，", configured: []), "空清单就是关闭")
    XCTAssertTrue(SymbolReturnPolicy.shouldReturn(face: "，", configured: ["，", "。"]))
    XCTAssertFalse(SymbolReturnPolicy.shouldReturn(face: "；", configured: ["，"]))
    let faces = SymbolReturnPolicy.faces(symbol: ",", drawnFace: "，")
    XCTAssertTrue(SymbolReturnPolicy.shouldReturn(faces: faces, configured: ["，"]))
    XCTAssertTrue(SymbolReturnPolicy.shouldReturn(faces: faces, configured: [","]))
    XCTAssertFalse(SymbolReturnPolicy.shouldReturn(faces: faces, configured: ["；"]))
    XCTAssertEqual(SymbolReturnPolicy.faces(symbol: ",", drawnFace: nil), Set([","]))
  }

  /// 与快捷符号共用单字素规则；清空后空列表不写进文档，缺键即关闭。
  func testReturnSymbolsSaveNormalizesAndEmptyTurnsItOff() {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    XCTAssertTrue(KeyboardLayoutPreference.saveReturnToLettersSymbols(
      ["，", "，", "", "ab", "\n", "。", "😀"], stateRoot: root))
    XCTAssertEqual(
      MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root)?["touch_return_to_letters_symbols"] as? [String],
      ["，", "。", "😀"])
    XCTAssertEqual(KeyboardLayoutPreference.returnToLettersSymbols, ["，", "。", "😀"])
    XCTAssertTrue(KeyboardLayoutPreference.saveReturnToLettersSymbols([], stateRoot: root))
    // 空列表按共享文档规则不落盘：读回没有这个键，键盘缺省即关闭。
    XCTAssertNil(
      MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: root)?["touch_return_to_letters_symbols"],
      "清空后空列表不写进文档，缺键即关闭")
    XCTAssertEqual(KeyboardLayoutPreference.returnToLettersSymbols, [String]())
  }

  /// 本机还没镜像过这一项时按缺省的空列表处理：没配置过就是关闭。
  func testReturnSymbolsFallBackToEmpty() {
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
    XCTAssertEqual(KeyboardLayoutPreference.returnToLettersSymbols, [String]())
    XCTAssertEqual(KeyboardLayoutPreference.cachedReturnToLettersSymbols, [String]())
    // 镜像一个空数组之后仍是关闭。
    KeyboardLayoutPreference.defaults.set([String](), forKey: KeyboardLayoutPreference.returnToLettersSymbolsKey)
    XCTAssertEqual(KeyboardLayoutPreference.cachedReturnToLettersSymbols, [String]())
    XCTAssertEqual(KeyboardLayoutPreference.returnToLettersSymbols, [String]())
  }

  /// 符号页上写出清单里的符号后回到字母页；没命中就留在符号页。
  @MainActor
  func testReturnSymbolsSendTheSymbolLayerBackToLetters() throws {
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()?["touch_return_to_letters_symbols"]
    let scheme = InputSchemePreference.scheme
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0["touch_return_to_letters_symbols"] = previous }
      InputSchemePreference.scheme = scheme
    }
    // 清单同时收全角与半角，中文标点开或关都能命中 `symbolLayerKey2_1`；`symbolLayerKey2_2` 两边都不在清单里
    //（中文层是 、，英文层是 ?）。
    XCTAssertTrue(KeyboardLayoutPreference.saveReturnToLettersSymbols(["，", ","]))
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    // 123 → 数字九宫格，#+= 打开设计稿的符号层。符号页收起字母底行（`quickPunctuationKey` 只画在字母层），
    // 回到字母页时它又出现。
    try control("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try control("numberPadMoreSymbols", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try control("quickPunctuationKey", in: controller).isHidden)
    try control("symbolLayerKey2_2", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try control("quickPunctuationKey", in: controller).isHidden, "不在清单里的符号留在符号页")
    try control("symbolLayerKey2_1", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try control("quickPunctuationKey", in: controller).isHidden, "命中清单后回到字母页")
    XCTAssertEqual(try control("layoutToggleButton", in: controller).configuration?.title, "123")
    controller.viewDidDisappear(false)
  }

}
