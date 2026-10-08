import UIKit
import XCTest

/// 功能菜单（「功能」）是分页网格，所以没有东西藏在折叠线以下：每个工具都在某一页上，顺序与 Android 的 `FunctionPanelModel` 一致，翻页能到达每一个。这些测试固定这个顺序、每页数量，以及那些所做的事面板本身显示不出来的工具。
@MainActor
final class MorePanelFoldTests: XCTestCase {
  private var savedDefaults: [String: Any] = [:]
  /// 「完成」写入共享文档的触屏几何参数，测试后恢复原值，免得之后的键盘画得更高。
  private var savedGeometry: [String: Any] = [:]
  private let geometryKeys = ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment", "touch_voice_shortcut"]
  private let defaultsKeys = [
    KeyboardPrivacyPreference.incognitoKey, KeyboardFeedbackPreference.soundKey, KeyboardFeedbackPreference.hapticsKey,
    KeyboardLayoutPreference.heightAdjustmentKey,
  ]

  override func setUp() {
    super.setUp()
    for key in defaultsKeys {
      savedDefaults[key] = KeyboardFeedbackPreference.defaults.object(forKey: key)
    }
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences() ?? [:]
    savedGeometry = document.filter { geometryKeys.contains($0.key) }
  }

  override func tearDown() {
    for key in defaultsKeys {
      if let value = savedDefaults[key] { KeyboardFeedbackPreference.defaults.set(value, forKey: key) }
      else { KeyboardFeedbackPreference.defaults.removeObject(forKey: key) }
    }
    let saved = savedGeometry, keys = geometryKeys
    MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      for key in keys { document[key] = saved[key] }
    }
    super.tearDown()
  }

  /// 先是设计稿里的各项，再是在别处没有位置的 iOS 工具；振动相关的项只在有 Taptic Engine 的设备上出现。
  private var expectedTitles: [String] {
    let haptics = KeyboardFeedbackPreference.hapticsAvailable
    return ["全角", "中文标点", "模糊音", "繁体", "手写", "词库", "键盘高度", "设置", "按键音"]
      + (haptics ? ["振动"] : [])
      + ["单手模式", "隐私模式", "反馈", "关于", "AI 润色", "高情商回复", "本地输入", "语音结果"]
      + (haptics ? ["振动强度 \(KeyboardFeedbackPreference.hapticStrength.title)"] : [])
      + ["表情", "剪贴板历史", "清除候选缓存"]
  }

  func testEveryToolIsReachableByPagingInTheDesignsOrder() throws {
    let controller = makeController(width: 390)
    let panel = try openMenu(in: controller)
    let grid = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagedGridView }.first)
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    let tiles = descendants(panel).compactMap { $0 as? KeyboardFunctionTileView }

    // 像用户翻页那样，一页一页、一行一行地读出图块。
    let ordered = tiles.sorted {
      let a = $0.convert($0.bounds, to: scroll), b = $1.convert($1.bounds, to: scroll)
      let pageA = page(of: $0, in: scroll), pageB = page(of: $1, in: scroll)
      if pageA != pageB { return pageA < pageB }
      if abs(a.minY - b.minY) > 0.5 { return a.minY < b.minY }
      return a.minX < b.minX
    }
    XCTAssertEqual(ordered.map(\.tool.title), expectedTitles)
    XCTAssertEqual(panel.pageCount, (expectedTitles.count + 7) / 8)
    for index in 0..<panel.pageCount {
      XCTAssertEqual(tiles.filter { page(of: $0, in: scroll) == index }.count, min(8, expectedTitles.count - index * 8),
                     "page \(index + 1) holds 4 × 2 tiles")
    }
    // 翻页能让每个图块都进入视野。
    for tile in ordered {
      let target = page(of: tile, in: scroll)
      grid.scrollToPage(target, animated: false)
      XCTAssertEqual(panel.currentPage, target)
      let visible = scroll.convert(scroll.bounds, to: panel)
      XCTAssertTrue(visible.contains(tile.convert(tile.bounds, to: panel).insetBy(dx: 1, dy: 1)), tile.tool.title)
    }
    for tile in tiles { XCTAssertEqual(tile.bounds.height, KeyboardFunctionTileView.height, accuracy: 0.5) }
    XCTAssertEqual(KeyboardFunctionTileView.height, 52)
  }

  /// iPad 键盘每页显示 6 × 2 个图块。
  func testTheTabletMenuHoldsTwelveTilesAPage() {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    let tablet = KeyboardMorePickerView(tools: tools, formFactor: .tablet)
    tablet.frame = CGRect(x: 0, y: 0, width: 820, height: 237)
    tablet.layoutIfNeeded()
    XCTAssertEqual(tablet.pageCount, 2)
    let phone = KeyboardMorePickerView(tools: tools, formFactor: .phone)
    phone.frame = CGRect(x: 0, y: 0, width: 380, height: 201)
    phone.layoutIfNeeded()
    XCTAssertEqual(phone.pageCount, 3)
    let grid = descendants(tablet).compactMap { $0 as? KeyboardPagedGridView }.first
    XCTAssertEqual(grid?.tilesPerPage, 12)
  }

  /// Windows 上的 Ctrl+Shift+Alt+C 组合键在 iOS 上没有键可按，所以菜单把它放在最后一页；点这个图块会关闭菜单并提示已生效。
  func testTheClearCacheCardClearsAndSaysSo() throws {
    let controller = makeController(width: 390)
    let panel = try openMenu(in: controller)
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    let card = try tile("moreCard-清除候选缓存", in: controller)
    XCTAssertEqual(page(of: card, in: scroll), 2, "清除候选缓存 is on the third page")
    card.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()

    XCTAssertFalse(
      descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" },
      "the panel stayed open over the message")
    let label = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "diagnosticLabel" } as? UILabel)
    XCTAssertEqual(label.text, "已清除候选缓存")
    XCTAssertFalse(label.isHidden)
  }

  /// 「隐私模式」是开关：点按后菜单不关闭，值存在 App Group 中，新建的键盘也保持这个状态。
  func testIncognitoIsASwitchThatPersists() throws {
    KeyboardPrivacyPreference.incognito = false
    let controller = makeController(width: 390)
    let panel = try openMenu(in: controller)
    let incognito = try tile("moreCard-隐私模式", in: controller)
    XCTAssertEqual(incognito.accessibilityValue, "已关闭")
    incognito.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(KeyboardPrivacyPreference.incognito)
    XCTAssertEqual(try tile("moreCard-隐私模式", in: controller).accessibilityValue, "已开启")
    XCTAssertNotNil(panel.superview, "a switch keeps the menu open")
    XCTAssertTrue(KeyboardFeedbackPreference.defaults.bool(forKey: KeyboardPrivacyPreference.incognitoKey))

    let next = makeController(width: 390)
    _ = try openMenu(in: next)
    XCTAssertEqual(try tile("moreCard-隐私模式", in: next).accessibilityValue, "已开启")
    try tile("moreCard-隐私模式", in: next).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(KeyboardPrivacyPreference.incognito)
  }

  /// 「模糊音」只对中文模式下的拼音方案有意义，所以其他情况下图块变暗。
  func testFuzzyPinyinFollowsTheScheme() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    InputSchemePreference.scheme = .quanpin
    let controller = makeController(width: 390)
    _ = try openMenu(in: controller)
    XCTAssertTrue(try tile("moreCard-模糊音", in: controller).isEnabled)
    // 英文模式不动拼写规则。
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    _ = try openMenu(in: controller)
    XCTAssertFalse(try tile("moreCard-模糊音", in: controller).isEnabled)
    XCTAssertEqual(try tile("moreCard-模糊音", in: controller).accessibilityValue, "不可用")
  }

  /// 菜单自己的开关把 `fuzzy_pinyin` 同时写入会话和文档：用户选过的规则集保持不变，空的规则集则填满。
  func testFuzzyPinyinSwitchKeepsTheChosenRules() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-fuzzy-switch-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0[FuzzyPinyinPreference.documentKey] = ["enabled": false, "seeded": true, "rules": ["z-zh", "n-l"]]
    })
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setFuzzyPinyinEnabled(true))
    var stored = FuzzyPinyinPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(stored?.enabled, true)
    XCTAssertEqual(stored?.rules, ["z-zh", "n-l"])
    XCTAssertEqual(FuzzyPinyinPreference.settings(in: bridge.sharedPreferences), stored, "the session runs what the document says")
    XCTAssertEqual(bridge.fuzzyPinyinRulesApplied, stored?.bits)

    XCTAssertTrue(bridge.setFuzzyPinyinEnabled(false))
    stored = FuzzyPinyinPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(stored?.enabled, false)
    XCTAssertEqual(stored?.rules, ["z-zh", "n-l"], "switching off keeps the chosen rules for next time")
    XCTAssertEqual(bridge.fuzzyPinyinRulesApplied, 0)

    // 规则被全部清空的文档在打开开关时得到所有规则，所以打开开关总会有效果。
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0[FuzzyPinyinPreference.documentKey] = ["enabled": false, "seeded": true, "rules": [String]()]
    })
    let fresh = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(fresh.setFuzzyPinyinEnabled(true))
    stored = FuzzyPinyinPreference.settings(in: MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(stored?.rules, Set(FuzzyPinyinPreference.ruleIDs))
    XCTAssertEqual(stored?.seeded, true)
  }

  /// 「键盘高度」把工具栏换成内联调节条，调节时实时预览，只在点「完成」时保存；「取消」把高度恢复原样。
  func testKeyboardHeightTileAdjustsInline() throws {
    KeyboardLayoutPreference.heightAdjustment = 0
    let controller = makeController(width: 390)
    let heightConstraint = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let start = heightConstraint.constant
    _ = try openMenu(in: controller)
    try tile("moreCard-键盘高度", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
    let bar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "inlineHeightBar" } as? InlineHeightBar)
    let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" })
    XCTAssertTrue(toolbar.isHidden, "the bar takes the toolbar's place")
    XCTAssertEqual(bar.percent, 100)
    XCTAssertEqual(try button("spaceKey", in: controller).superview?.alpha, 1, "the keys stay live under the bar")
    let keyBlock = KeyboardViewController.keyBlockHeight(keyHeight: 42, rows: 4, rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing))
    XCTAssertEqual(bar.range.lowerBound, KeyboardViewController.heightPercent(adjustment: -12, keyBlock: keyBlock))
    XCTAssertEqual(bar.range.upperBound, KeyboardViewController.heightPercent(adjustment: 48, keyBlock: keyBlock))

    // VoiceOver 的增大操作对应调节条的 5% 步长；键盘只预览，不保存。
    let handle = try XCTUnwrap(descendants(bar).first { $0.accessibilityIdentifier == "inlineHeightHandle" })
    handle.accessibilityIncrement()
    let preview = KeyboardViewController.heightAdjustment(percent: 105, keyBlock: keyBlock)
    XCTAssertEqual(heightConstraint.constant, start + preview, accuracy: 0.5)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0, "nothing is saved before 完成")
    try control("inlineHeightCancel", in: bar).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(heightConstraint.constant, start, accuracy: 0.5)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "inlineHeightBar" })
    XCTAssertFalse(toolbar.isHidden)

    _ = try openMenu(in: controller)
    try tile("moreCard-键盘高度", in: controller).sendActions(for: .primaryActionTriggered)
    let again = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "inlineHeightBar" })
    try XCTUnwrap(descendants(again).first { $0.accessibilityIdentifier == "inlineHeightHandle" }).accessibilityIncrement()
    try control("inlineHeightDone", in: again).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, Double(preview))
    XCTAssertEqual(heightConstraint.constant, start + preview, accuracy: 0.5)
  }

  /// 调节条开着时按键照常可用，所以按键打开的面板（这里是 123 层的表情键）不会结束调整、也不会丢掉预览的高度；与 Android 一样，只有「取消」「完成」或收起键盘才结束，收起键盘按「取消」处理。
  func testKeyboardHeightPreviewSurvivesKeyPanelsAndIsCancelledWhenTheKeyboardGoesAway() throws {
    KeyboardLayoutPreference.heightAdjustment = 0
    let controller = makeController(width: 390)
    let heightConstraint = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" })
    let start = heightConstraint.constant
    _ = try openMenu(in: controller)
    try tile("moreCard-键盘高度", in: controller).sendActions(for: .primaryActionTriggered)
    let bar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "inlineHeightBar" })
    try XCTUnwrap(descendants(bar).first { $0.accessibilityIdentifier == "inlineHeightHandle" }).accessibilityIncrement()
    let previewed = heightConstraint.constant
    XCTAssertGreaterThan(previewed, start + 0.5)

    let hasBar = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "inlineHeightBar" } }
    let hasPicker = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardEmojiPicker" } }
    try button("layerEmojiKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(hasPicker())
    XCTAssertTrue(hasBar(), "a panel opened from the keys leaves the bar in place")
    XCTAssertEqual(heightConstraint.constant, previewed, accuracy: 0.5, "and keeps the previewed height")
    try control("closeEmojiPicker", in: controller.view).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(hasPicker())
    XCTAssertTrue(hasBar())
    XCTAssertEqual(heightConstraint.constant, previewed, accuracy: 0.5)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0, "nothing is saved before 完成")

    controller.viewWillDisappear(false)
    XCTAssertFalse(hasBar())
    XCTAssertEqual(heightConstraint.constant, start, accuracy: 0.5, "putting the keyboard away cancels the preview")
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0)
  }

  func testHeightPercentRoundTripsThroughPoints() {
    let keyBlock = KeyboardViewController.keyBlockHeight(keyHeight: 42, rows: 4, rowSpacing: 7)
    XCTAssertEqual(keyBlock, 189, "four of the design's 42pt phone keys and three 7pt row gaps")
    XCTAssertEqual(KeyboardHeightPercent.portraitKeyBlockHeight(tablet: false, numberRow: true, rowSpacing: 7), keyBlock,
                   "a phone has no digit row")
    XCTAssertEqual(KeyboardHeightPercent.portraitKeyBlockHeight(tablet: true, numberRow: true, rowSpacing: 7), 5 * 54 + 4 * 7)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: 0, keyBlock: keyBlock), 100)
    XCTAssertEqual(KeyboardViewController.heightAdjustment(percent: 100, keyBlock: keyBlock), 0)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: -12, keyBlock: keyBlock), 94)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: 48, keyBlock: keyBlock), 125)
    // 无论调节条要求多少，点数都限制在共享字段的范围内。
    XCTAssertEqual(KeyboardViewController.heightAdjustment(percent: 200, keyBlock: keyBlock), 48)
    XCTAssertEqual(KeyboardViewController.heightAdjustment(percent: 50, keyBlock: keyBlock), -12)
  }

  private func makeController(width: CGFloat) -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func openMenu(in controller: KeyboardViewController) throws -> KeyboardMorePickerView {
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    return try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" } as? KeyboardMorePickerView)
  }

  private func page(of view: UIView, in scroll: UIScrollView) -> Int {
    guard scroll.bounds.width > 0 else { return 0 }
    return Int((view.convert(view.bounds, to: scroll).midX / scroll.bounds.width).rounded(.down))
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton,
      "No button with accessibility identifier \(identifier).")
  }

  private func tile(_ identifier: String, in controller: KeyboardViewController) throws -> UIControl {
    try control(identifier, in: controller.view)
  }

  private func control(_ identifier: String, in view: UIView) throws -> UIControl {
    try XCTUnwrap(
      descendants(view).first { $0.accessibilityIdentifier == identifier } as? UIControl,
      "No control with accessibility identifier \(identifier).")
  }
}
