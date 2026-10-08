import UIKit
import XCTest

/// 单手模式（dc.html `oneHand`），行为与 Android 一致：功能菜单卡片在关闭和右侧之间切换，长按换边；侧栏的按钮负责换边和退出；值存在 App Group 里；只有手机键盘会绘制它。
@MainActor
final class OneHandModeTests: XCTestCase {
  private var stored: Any?
  private var storedScheme = InputSchemePreference.scheme

  override func setUp() {
    super.setUp()
    stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.oneHandedKey)
    storedScheme = InputSchemePreference.scheme
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.oneHandedKey)
    InputSchemePreference.scheme = .quanpin
  }

  override func tearDown() {
    if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.oneHandedKey) }
    else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.oneHandedKey) }
    InputSchemePreference.scheme = storedScheme
    super.tearDown()
  }

  func testTheModeTogglesAsAndroidsToggleOneHanded() {
    XCTAssertEqual(KeyboardOneHandedMode.off.toggled(swapSide: false), .right)
    XCTAssertEqual(KeyboardOneHandedMode.right.toggled(swapSide: false), .off)
    XCTAssertEqual(KeyboardOneHandedMode.left.toggled(swapSide: false), .off)
    XCTAssertEqual(KeyboardOneHandedMode.off.toggled(swapSide: true), .left)
    XCTAssertEqual(KeyboardOneHandedMode.left.toggled(swapSide: true), .right)
    XCTAssertEqual(KeyboardOneHandedMode.right.toggled(swapSide: true), .left)
  }

  func testTheStoredValueUsesAndroidsSpellingAndDefaultsOff() {
    XCTAssertEqual(KeyboardLayoutPreference.oneHandedKey, "keyboard.oneHanded")
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .off)
    KeyboardLayoutPreference.oneHanded = .left
    XCTAssertEqual(KeyboardLayoutPreference.defaults.string(forKey: KeyboardLayoutPreference.oneHandedKey), "left")
    KeyboardLayoutPreference.defaults.set("middle", forKey: KeyboardLayoutPreference.oneHandedKey)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .off, "an unknown value reads as off")
  }

  func testTheKeysGiveTheColumnFifteenPercentAndTheGap() {
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 384, mode: .off), 384)
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 384, mode: .right), 384 - 384 * 0.15 - 6, accuracy: 0.001)
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 384, mode: .left), 384 - 384 * 0.15 - 6, accuracy: 0.001)
    XCTAssertEqual(KeyboardOneHandLayout.effective(.right, formFactor: .phone), .right)
    XCTAssertEqual(KeyboardOneHandLayout.effective(.left, formFactor: .tablet), .off)
    XCTAssertTrue(KeyboardOneHandLayout.gutterLeads(.right))
    XCTAssertFalse(KeyboardOneHandLayout.gutterLeads(.left))
  }

  /// 点击卡片会打开右侧单手；侧栏随即位于左边，换到另一侧 指向左边，按键填满其余空间。换到另一侧 把按键移到左边，退出单手 恢复全宽按键。
  func testTheTileAndTheColumnDriveTheLayout() throws {
    let controller = makeController()
    let fullSpace = try frame("spaceKey", in: controller)
    _ = try openMenu(in: controller)
    let tile = try control("moreCard-单手模式", in: controller)
    XCTAssertEqual(tile.accessibilityValue, "已关闭")
    tile.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
    XCTAssertEqual(try control("moreCard-单手模式", in: controller).accessibilityValue, "已开启")
    XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" }, "a switch keeps the menu open")
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()

    let gutter = try view("oneHandGutter", in: controller)
    XCTAssertFalse(gutter.isHidden)
    let available = controller.view.bounds.width - 6
    let column = gutter.convert(gutter.bounds, to: controller.view)
    XCTAssertEqual(column.width, available * KeyboardOneHandLayout.gutterRatio, accuracy: 0.5)
    XCTAssertEqual(column.minX, 3, accuracy: 0.5)
    let space = try frame("spaceKey", in: controller)
    let delete = try frame("letterDeleteKey", in: controller)
    XCTAssertEqual(delete.maxX, controller.view.bounds.width - 3, accuracy: 0.5, "the keys reach the right edge")
    XCTAssertGreaterThanOrEqual(space.minX, column.maxX)
    let letterQ = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" })
    XCTAssertEqual(letterQ.convert(letterQ.bounds, to: controller.view).minX, column.maxX + KeyboardOneHandLayout.gap, accuracy: 0.5)
    let swap = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "oneHandSwap" } as? KeyboardOneHandGutterButton)
    XCTAssertEqual(swap.icon, .oneHandSwapLeft)
    XCTAssertEqual(swap.bounds.size, CGSize(width: 40, height: 40))
    // 工具栏和候选栏保持全宽。
    let strip = try view("candidateStrip", in: controller)
    XCTAssertEqual(strip.convert(strip.bounds, to: controller.view).width, available, accuracy: 0.5)
    // 中间字母行的半键缩进按变窄后的按键宽度计算。
    let middle = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 A" }?.superview as? UIStackView)
    XCTAssertEqual(middle.layoutMargins.left,
                   KeyboardOneHandLayout.keysWidth(available: available, mode: .right) * KeyboardGeometry(keySpacing: 6, rowSpacing: 7).letterInsetRatio,
                   accuracy: 0.5)

    swap.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .left)
    XCTAssertEqual(swap.icon, .oneHandSwapRight)
    let moved = gutter.convert(gutter.bounds, to: controller.view)
    XCTAssertEqual(moved.maxX, controller.view.bounds.width - 3, accuracy: 0.5)
    XCTAssertEqual(letterQ.convert(letterQ.bounds, to: controller.view).minX, 3, accuracy: 0.5, "the keys moved to the left edge")
    XCTAssertLessThanOrEqual(try frame("letterDeleteKey", in: controller).maxX, moved.minX)

    try control("oneHandExit", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .off)
    XCTAssertTrue(gutter.isHidden)
    let restored = try frame("spaceKey", in: controller)
    XCTAssertEqual(restored.minX, fullSpace.minX, accuracy: 0.5)
    XCTAssertEqual(restored.width, fullSpace.width, accuracy: 0.5)
  }

  /// 长按卡片会换边，从关闭直接切到左侧，与 Android 的长按一致。
  func testHoldingTheTileSwapsTheSide() throws {
    let controller = makeController()
    _ = try openMenu(in: controller)
    let tile = try XCTUnwrap(try control("moreCard-单手模式", in: controller) as? KeyboardFunctionTileView)
    try XCTUnwrap(tile.tool.longPress)()
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .left)
    try XCTUnwrap((try control("moreCard-单手模式", in: controller) as? KeyboardFunctionTileView)?.tool.longPress)()
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
  }

  /// 模式在新建键盘后依然保持，覆盖上来的面板会把侧栏和按键一起遮住。
  func testTheStoredModeComesBackAndPanelsCoverTheColumn() throws {
    KeyboardLayoutPreference.oneHanded = .left
    let controller = makeController()
    let gutter = try view("oneHandGutter", in: controller)
    XCTAssertFalse(gutter.isHidden)
    XCTAssertEqual(gutter.convert(gutter.bounds, to: controller.view).maxX, controller.view.bounds.width - 3, accuracy: 0.5)
    let menu = try openMenu(in: controller)
    XCTAssertEqual(menu.convert(menu.bounds, to: controller.view).width, controller.view.bounds.width - 6, accuracy: 0.5,
                   "panels keep the full width")
    var node: UIView? = gutter
    while let current = node, current.alpha == 1 { node = current.superview }
    XCTAssertNotNil(node, "the column is covered with the keys")
  }

  /// 全宽的 iPad 键盘不绘制侧栏，并把卡片置灰，存储的值保持不变。
  func testTheTabletKeyboardIgnoresIt() throws {
    KeyboardLayoutPreference.oneHanded = .right
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = .regular
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 820, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    XCTAssertTrue(try view("oneHandGutter", in: controller).isHidden)
    _ = try openMenu(in: controller)
    let tile = try control("moreCard-单手模式", in: controller)
    XCTAssertFalse(tile.isEnabled)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
  }

  private func makeController() -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func openMenu(in controller: KeyboardViewController) throws -> KeyboardMorePickerView {
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    return try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" } as? KeyboardMorePickerView)
  }

  private func frame(_ identifier: String, in controller: KeyboardViewController) throws -> CGRect {
    let key = try view(identifier, in: controller)
    return KeyAreaStackView.layoutFrame(of: key, in: controller.view)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  private func view(_ identifier: String, in controller: KeyboardViewController) throws -> UIView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier },
                  "No view with accessibility identifier \(identifier).")
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(try view(identifier, in: controller) as? UIButton)
  }

  private func control(_ identifier: String, in controller: KeyboardViewController) throws -> UIControl {
    try XCTUnwrap(try view(identifier, in: controller) as? UIControl)
  }
}
