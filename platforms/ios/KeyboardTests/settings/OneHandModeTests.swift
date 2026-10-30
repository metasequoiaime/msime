import UIKit
import XCTest

/// 单手模式（dc.html `oneHand`），行为与 Android 一致：功能菜单卡片在关闭和右侧之间切换，长按换边；侧栏的按钮负责换边和退出；值存在 App Group 里。手机键盘整行变成「侧栏加按键」；全宽的 iPad 键盘把这一块缩到手机大小贴到一侧，画横屏分离式键盘时让位，空出来的一侧可以放侧边数字键盘（#10203）。
@MainActor
final class OneHandModeTests: XCTestCase {
  private var stored: Any?
  private var storedDockedTablet: Any?
  private var storedScheme = InputSchemePreference.scheme

  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.oneHandedKey)
    storedDockedTablet = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.oneHandedDockedTabletKey)
    storedScheme = InputSchemePreference.scheme
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.oneHandedKey)
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.oneHandedDockedTabletKey)
    InputSchemePreference.scheme = .quanpin
    // 用例改的是模拟器共用的那份文档和 App Group：侧边面板、iPad 数字行和分离式键盘的开关结束后都放回原样，用例开始时一律关着。
    let panel = MetasequoiaInputSessionBridge.loadSharedPreferences()?[TouchOneHandedSidePanel.documentKey]
    let defaults = KeyboardLayoutPreference.defaults
    let fullKeys = defaults.object(forKey: KeyboardLayoutPreference.tabletFullKeysKey)
    let split = defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey)
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[TouchOneHandedSidePanel.documentKey] = panel }
      defaults.set(fullKeys, forKey: KeyboardLayoutPreference.tabletFullKeysKey)
      defaults.set(split, forKey: KeyboardLayoutPreference.tabletSplitKey)
    }
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.off))
    KeyboardLayoutPreference.tabletFullKeys = false
    KeyboardLayoutPreference.tabletSplit = false
  }

  override func tearDown() {
    if let stored { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.oneHandedKey) }
    else { KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.oneHandedKey) }
    KeyboardLayoutPreference.defaults.set(storedDockedTablet, forKey: KeyboardLayoutPreference.oneHandedDockedTabletKey)
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
    XCTAssertEqual(KeyboardOneHandLayout.effective(.left, formFactor: .tablet), .left)
    XCTAssertEqual(KeyboardOneHandLayout.effective(.left, formFactor: .tablet, split: true), .off, "the split keyboard takes precedence")
    XCTAssertEqual(KeyboardOneHandLayout.effective(.right, formFactor: .phone, split: true), .off, "so does the iPhone landscape split")
    XCTAssertTrue(KeyboardOneHandLayout.gutterLeads(.right))
    XCTAssertFalse(KeyboardOneHandLayout.gutterLeads(.left))
  }

  /// iPad 单手的一块与鸿蒙同一个公式：按键 360、侧栏占块宽 15%、间距 6，round(366 / 0.85) = 431；空出来的宽度（整行减块再减间距）不到 150 时不放侧边数字键盘，最宽 300。手机没有侧边数字键盘。
  func testTheTabletBlockAndTheSidePadWidths() {
    XCTAssertEqual(KeyboardOneHandLayout.blockWidth(available: 728, formFactor: .tablet), 431)
    XCTAssertEqual(KeyboardOneHandLayout.blockWidth(available: 390, formFactor: .tablet), 390, "a narrower window keeps the whole row")
    XCTAssertEqual(KeyboardOneHandLayout.blockWidth(available: 384, formFactor: .phone), 384)
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 728, mode: .right, formFactor: .tablet), 431 - 431 * 0.15 - 6, accuracy: 0.001)
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 728, mode: .off, formFactor: .tablet), 728)
    XCTAssertEqual(KeyboardOneHandLayout.keysWidth(available: 384, mode: .left, formFactor: .phone),
                   KeyboardOneHandLayout.keysWidth(available: 384, mode: .left))
    // iPad mini 竖屏 744pt 减去两侧 8pt 的内边距是 728，空出 291。
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 728, formFactor: .tablet), 291)
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 804, formFactor: .tablet), 300)
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 1178, formFactor: .tablet), 300)
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 587, formFactor: .tablet), 150)
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 586, formFactor: .tablet), 0, "below the minimum")
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 0, formFactor: .tablet), 0)
    XCTAssertEqual(KeyboardOneHandLayout.sidePadWidth(available: 1178, formFactor: .phone), 0)
  }

  /// 偏好的写法与 client-core 一致：缺键、不是字符串和不认识的值（以后的 `text_edit`）都读作空着；开关写 `number_pad`，关掉写 `off`，client-core 把 `off` 存成缺键。设置页的文字三端逐字相同。
  func testTheSidePanelPreference() throws {
    XCTAssertEqual(TouchOneHandedSidePanel.documentKey, "touch_one_handed_side_panel")
    XCTAssertEqual(TouchOneHandedSidePanel.allCases.map(\.rawValue), ["off", "number_pad"])
    XCTAssertEqual(TouchOneHandedSidePanel(in: nil), .off)
    XCTAssertEqual(TouchOneHandedSidePanel(in: [:]), .off)
    XCTAssertEqual(TouchOneHandedSidePanel(in: ["touch_one_handed_side_panel": "text_edit"]), .off)
    XCTAssertEqual(TouchOneHandedSidePanel(in: ["touch_one_handed_side_panel": true]), .off)
    XCTAssertEqual(TouchOneHandedSidePanel(in: ["touch_one_handed_side_panel": "number_pad"]), .numberPad)
    XCTAssertEqual(TouchOneHandedSidePanelPreference.title, "大屏单手时侧边显示数字键盘")
    XCTAssertEqual(TouchOneHandedSidePanelPreference.subtitle,
                   "单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。")

    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: state, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertNil(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["touch_one_handed_side_panel"],
                 "the default document does not carry it")
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad, stateRoot: state))
    XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["touch_one_handed_side_panel"] as? String,
                   "number_pad")
    XCTAssertEqual(TouchOneHandedSidePanelPreference.load(stateRoot: state), .numberPad)
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.off, stateRoot: state))
    XCTAssertNil(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["touch_one_handed_side_panel"],
                 "off is stored as a missing key")
    XCTAssertEqual(TouchOneHandedSidePanelPreference.load(stateRoot: state), .off)
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

  /// 全宽的 iPad 键盘也画单手模式，卡片不再置灰：侧栏和按键是手机大小的一块（按键约 360pt），贴到存下的那一侧，另一侧是一块不接触摸的空白。键面换成手机的：没有 Tab、iPad 数字行、iPad 的 ⌫ 和第二个 ⇧，底行是手机底行；键盘高度仍按 iPad。退出后回到 iPad 键面。
  func testTheTabletKeyboardDocksAPhoneSizedBlock() throws {
    let twoHanded = tablet(width: 820)
    let tabletHeight = try requestedHeight(twoHanded)
    XCTAssertFalse(shown(try view("letterDeleteKey", in: twoHanded)), "the iPad face has its delete key on the first row")
    XCTAssertTrue(shown(try view("tabletDeleteKey", in: twoHanded)))
    XCTAssertEqual(try letterQ(in: twoHanded).titleLabel?.font.pointSize ?? 0, 20, accuracy: 0.01, "the iPad letter face")
    _ = try openMenu(in: twoHanded)
    let tile = try control("moreCard-单手模式", in: twoHanded)
    XCTAssertTrue(tile.isEnabled, "the tile is no longer dimmed on the iPad")
    tile.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
    try button("moreShortcut", in: twoHanded).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(twoHanded)

    for controller in [twoHanded, tablet(width: 820)] {
      let width = controller.view.bounds.width
      let available = width - 16
      let gutter = try view("oneHandGutter", in: controller)
      XCTAssertTrue(shown(gutter))
      let column = gutter.convert(gutter.bounds, to: controller.view)
      XCTAssertEqual(column.width, 431 * KeyboardOneHandLayout.gutterRatio, accuracy: 0.5)
      XCTAssertEqual(column.minX, width - 8 - 431, accuracy: 0.5, "the block is docked to the right edge")
      let delete = try frame("letterDeleteKey", in: controller)
      XCTAssertEqual(delete.maxX, width - 8, accuracy: 0.5)
      let q = try letterQ(in: controller)
      XCTAssertEqual(q.convert(q.bounds, to: controller.view).minX, column.maxX + KeyboardOneHandLayout.gap, accuracy: 0.5)
      let spacer = try view("oneHandSideSpacer", in: controller)
      XCTAssertTrue(shown(spacer))
      XCTAssertFalse(spacer.isAccessibilityElement)
      let blank = spacer.convert(spacer.bounds, to: controller.view)
      XCTAssertEqual(blank.minX, 8, accuracy: 0.5)
      XCTAssertEqual(blank.width, available - 431 - KeyboardOneHandLayout.gap, accuracy: 0.5)
      XCTAssertFalse(shown(try view("oneHandSidePad", in: controller)), "the side pad is off by default")
      // 空白不接触摸，也不把触摸交给旁边的键。
      let area = try keyArea(controller)
      XCTAssertTrue(area.hitTest(controller.view.convert(CGPoint(x: blank.midX, y: blank.midY), to: area), with: nil) === spacer)
      // 手机键面：没有 Tab 和 iPad 的 ⌫、第二个 ⇧，底行有手机的逗号键，没有 iPad 底行的 ⌄；字母字号是手机的。
      XCTAssertTrue(shown(try view("letterDeleteKey", in: controller)))
      XCTAssertFalse(shown(try view("tabletDeleteKey", in: controller)))
      XCTAssertFalse(shown(try view("rightShiftButton", in: controller)))
      XCTAssertFalse(shown(try view("tabKey", in: controller)))
      XCTAssertFalse(shown(try view("dismissKeyboardKey", in: controller)))
      XCTAssertTrue(shown(try view("quickPunctuationKey", in: controller)))
      XCTAssertEqual(q.titleLabel?.font.pointSize ?? 0, 22, accuracy: 0.01, "the phone letter face")
      XCTAssertEqual(try requestedHeight(controller), tabletHeight, accuracy: 0.5, "the keyboard keeps the iPad height")
    }

    try control("oneHandExit", in: twoHanded).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(twoHanded)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .off)
    XCTAssertFalse(shown(try view("oneHandGutter", in: twoHanded)))
    XCTAssertFalse(shown(try view("oneHandSideSpacer", in: twoHanded)))
    XCTAssertTrue(shown(try view("tabletDeleteKey", in: twoHanded)), "the iPad face comes back")
    XCTAssertEqual(try frame("tabletDeleteKey", in: twoHanded).maxX, 820 - 8, accuracy: 0.5)
  }

  /// 打开了「数字行与 Tab 键」时，iPad 单手不画 Tab，1 到 0 这一排照画：排数不变，键盘高度和双手时相同。
  func testTheTabletOneHandedFaceKeepsTheNumberRowButDropsTab() throws {
    KeyboardLayoutPreference.tabletFullKeys = true
    let full = tablet(width: 1032)
    XCTAssertTrue(shown(try view("tabKey", in: full)))
    XCTAssertTrue(shown(try view("numberRow", in: full)))
    let fullHeight = try requestedHeight(full)
    KeyboardLayoutPreference.oneHanded = .left
    let oneHanded = tablet(width: 1032)
    XCTAssertFalse(shown(try view("tabKey", in: oneHanded)))
    XCTAssertTrue(shown(try view("numberRow", in: oneHanded)), "the digits stay above the letters")
    XCTAssertEqual(try requestedHeight(oneHanded), fullHeight, accuracy: 0.5, "turning one-handed on keeps the keyboard height")
    XCTAssertEqual(try frame("letterDeleteKey", in: oneHanded).maxX,
                   8 + 431 - 431 * KeyboardOneHandLayout.gutterRatio - KeyboardOneHandLayout.gap, accuracy: 0.5,
                   "the keys sit at the left edge, the column beside them")
    // 数字和符号页用补充行保持同样的排数，切层时键盘不跳。
    try button("layoutToggleButton", in: oneHanded).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(oneHanded)
    XCTAssertEqual(try requestedHeight(oneHanded), fullHeight, accuracy: 0.5, "the symbol layer keeps the height")
  }

  /// 升级前在浮动键盘或窄窗口（手机形态）存下的单手模式不带到全宽键盘上：全宽键盘第一次画单手之前把它改成关闭，只做一次；浮动键盘照旧用它。用户之后自己打开的单手模式在全宽键盘上照画。
  func testAValueStoredBeforeTheUpgradeDoesNotShrinkTheDockedKeyboard() throws {
    let defaults = KeyboardLayoutPreference.defaults
    defaults.set("right", forKey: KeyboardLayoutPreference.oneHandedKey)
    let floating = tablet(width: 320, compact: true)
    XCTAssertTrue(shown(try view("oneHandGutter", in: floating)), "the floating keyboard keeps its one-handed mode")
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
    XCTAssertNil(defaults.object(forKey: KeyboardLayoutPreference.oneHandedDockedTabletKey))

    let docked = tablet(width: 820)
    XCTAssertFalse(shown(try view("oneHandGutter", in: docked)), "the docked keyboard stays two-handed")
    XCTAssertTrue(shown(try view("tabletDeleteKey", in: docked)))
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .off)
    XCTAssertTrue(defaults.bool(forKey: KeyboardLayoutPreference.oneHandedDockedTabletKey))
    _ = try openMenu(in: docked)
    XCTAssertEqual(try control("moreCard-单手模式", in: docked).accessibilityValue, "已关闭")

    // 只做一次：用户在全宽键盘上自己打开后，新建的键盘照画单手。
    try control("moreCard-单手模式", in: docked).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
    XCTAssertTrue(shown(try view("oneHandGutter", in: tablet(width: 820))))
  }

  /// 横屏分离式键盘画着时单手让位：不画侧栏，存的值不变；九键不分离，同一台 iPad 横屏照样画单手。
  func testTheSplitKeyboardTakesPrecedenceOverOneHanded() throws {
    KeyboardLayoutPreference.tabletSplit = true
    KeyboardLayoutPreference.oneHanded = .right
    let split = tablet(width: 1194, landscape: true)
    XCTAssertFalse(shown(try view("oneHandGutter", in: split)))
    XCTAssertTrue(shown(try view("tabletDeleteKey", in: split)), "the split keyboard keeps the iPad face")
    XCTAssertEqual(KeyboardLayoutPreference.oneHanded, .right)
    InputSchemePreference.scheme = .nineKey
    let nineKey = tablet(width: 1194, landscape: true)
    XCTAssertTrue(shown(try view("oneHandGutter", in: nineKey)), "the nine-key keyboard does not split")
  }

  /// iPad 的浮动键盘和窄窗口是手机形态，单手照手机的样子画：侧栏占整行 15%，没有空白，也没有侧边数字键盘。
  func testTheFloatingKeyboardStaysAPhoneKeyboard() throws {
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad))
    InputSchemePreference.scheme = .nineKey
    KeyboardLayoutPreference.oneHanded = .right
    let floating = tablet(width: 320, compact: true)
    let gutter = try view("oneHandGutter", in: floating)
    XCTAssertTrue(shown(gutter))
    XCTAssertEqual(gutter.bounds.width, (320 - 6) * KeyboardOneHandLayout.gutterRatio, accuracy: 0.5)
    XCTAssertFalse(shown(try view("oneHandSideSpacer", in: floating)))
    XCTAssertFalse(shown(try view("oneHandSidePad", in: floating)))
    let phone = makeController()
    XCTAssertFalse(shown(try view("oneHandSidePad", in: phone)), "phones have no side pad")
  }

  /// 偏好选了数字键盘、iPad 单手画九键一族的字母层时，空出来的一侧贴屏幕边放 3 × 4 拨号盘：前三行与九宫格逐行对齐，`. 0 ⌫` 与底行对齐；宽度是空出来的宽度，最宽 300，剩下的是空白。换边后面板跟到另一侧的屏幕边。
  func testTheSidePadFillsTheFreedSideBesideNineKeys() throws {
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad))
    KeyboardLayoutPreference.oneHanded = .right
    for scheme in [ChineseInputScheme.nineKey, .nineKeyShuangpin, .stroke] {
      InputSchemePreference.scheme = scheme
      // 没有暂存笔画词库时笔画不提供，镜像退回别的方案。
      if scheme == .stroke && InputSchemePreference.scheme != .stroke { continue }
      for (width, padWidth) in [(CGFloat(744), CGFloat(291)), (820, 300)] {
        let label = "\(scheme.rawValue) \(width)"
        let controller = tablet(width: width)
        let pad = try view("oneHandSidePad", in: controller)
        XCTAssertTrue(shown(pad), label)
        let padFrame = pad.convert(pad.bounds, to: controller.view)
        XCTAssertEqual(padFrame.minX, 8, accuracy: 0.5, label)
        XCTAssertEqual(padFrame.width, padWidth, accuracy: 0.5, label)
        let spacer = try view("oneHandSideSpacer", in: controller)
        XCTAssertEqual(spacer.bounds.width, width - 16 - 431 - KeyboardOneHandLayout.gap - padWidth, accuracy: 0.5, label)
        let faces = (try keys(in: pad, controller)).map { $0.configuration?.title ?? "" }
        XCTAssertEqual(faces, ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", ""], label)
        XCTAssertEqual(try button("sidePadDigit5", in: controller).accessibilityLabel, "数字 5")
        XCTAssertEqual(try button("sidePadDecimal", in: controller).accessibilityLabel, "小数点")
        XCTAssertEqual(try button("sidePadDelete", in: controller).accessibilityLabel, "删除")
        // 各行的上下边与主键区对齐：拨号盘前三行占九键外框那么高（符号栏从顶到底），最后一行对着底行。
        let frameBlock = try frame("nineKeySidebar", in: controller)
        XCTAssertEqual(try frame("sidePadDigit1", in: controller).minY, frameBlock.minY, accuracy: 0.5, label)
        XCTAssertEqual(try frame("sidePadDigit7", in: controller).maxY, frameBlock.maxY, accuracy: 0.5, label)
        if scheme != .stroke {
          XCTAssertEqual(try frame("sidePadDigit4", in: controller).minY, try frame("nineKey4", in: controller).minY, accuracy: 0.5, label)
          XCTAssertEqual(try frame("sidePadDigit4", in: controller).height, try frame("nineKey4", in: controller).height,
                         accuracy: 0.5, label)
        }
        let bottom = try frame("returnKey", in: controller)
        XCTAssertEqual(try frame("sidePadDecimal", in: controller).minY, bottom.minY, accuracy: 0.5, label)
        XCTAssertEqual(try frame("sidePadDelete", in: controller).maxY, bottom.maxY, accuracy: 0.5, label)
      }
    }
    InputSchemePreference.scheme = .nineKey
    let controller = tablet(width: 820)
    try control("oneHandSwap", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    let pad = try view("oneHandSidePad", in: controller)
    XCTAssertEqual(pad.convert(pad.bounds, to: controller.view).maxX, 820 - 8, accuracy: 0.5, "the pad follows to the other edge")
    XCTAssertLessThan(try frame("returnKey", in: controller).maxX, 8 + 431, "the keys are on the left")
  }

  /// 侧边数字键盘只陪着九键一族的字母层：切到 123 数字层、英文、26 键时不画，切回来又画；面板盖住按键时它和按键一起遮住；偏好关着时不画。
  func testTheSidePadOnlyShowsOnNineKeyLetters() throws {
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad))
    KeyboardLayoutPreference.oneHanded = .right
    InputSchemePreference.scheme = .nineKey
    let controller = tablet(width: 820)
    let pad = try view("oneHandSidePad", in: controller)
    XCTAssertTrue(shown(pad))
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(pad), "the 123 layer already is a number pad")
    XCTAssertTrue(shown(try view("oneHandSideSpacer", in: controller)), "the freed side stays blank")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(pad))
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertFalse(shown(pad), "English draws 26 keys")
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    try layoutAtRequestedHeight(controller)
    XCTAssertTrue(shown(pad))
    _ = try openMenu(in: controller)
    var node: UIView? = pad
    while let current = node, current.alpha == 1 { node = current.superview }
    XCTAssertNotNil(node, "a panel covers the side pad with the keys")

    for scheme in [ChineseInputScheme.quanpin, .fourteenKey, .japaneseNineKey] {
      InputSchemePreference.scheme = scheme
      XCTAssertFalse(shown(try view("oneHandSidePad", in: tablet(width: 820))), scheme.rawValue)
    }
    InputSchemePreference.scheme = .nineKey
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.off))
    XCTAssertFalse(shown(try view("oneHandSidePad", in: tablet(width: 820))), "the preference is off")
    // 太窄放不下 150 时也不画：iPad 窄窗口仍是 regular 宽度时。
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad))
    XCTAssertFalse(shown(try view("oneHandSidePad", in: tablet(width: 600))), "no room for the pad")
  }

  /// 侧边数字键盘的数字和小数点走数字层的字面上屏：正在组的拼音先原样上屏，组字结束；⌫ 与九键的 ⌫ 同一个动作，组字时删组字的最后一码。键与键之间的空隙交给最近的键。
  func testTheSidePadTypesLikeTheNumberLayer() throws {
    XCTAssertTrue(TouchOneHandedSidePanelPreference.save(.numberPad))
    KeyboardLayoutPreference.oneHanded = .right
    InputSchemePreference.scheme = .nineKey
    let controller = tablet(width: 820)
    for digit in [6, 4] { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
    let composing = try preedit(controller)
    XCTAssertFalse(composing.isEmpty, "typing 6 4 composes")
    let delete = try button("sidePadDelete", in: controller)
    delete.sendActions(for: .touchDown)
    delete.sendActions(for: .touchUpInside)
    let shorter = try preedit(controller)
    XCTAssertFalse(shorter.isEmpty)
    XCTAssertNotEqual(shorter, composing, "⌫ removes the last key of the composition")
    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try preedit(controller).isEmpty)
    try button("sidePadDigit5", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try preedit(controller), "", "the digit ends the composition, committing it as typed")
    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    try button("sidePadDecimal", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try preedit(controller), "")

    // 落在 2 和 3 之间、5 和 8 之间的触摸归最近的键。
    let area = try keyArea(controller)
    let two = try frame("sidePadDigit2", in: controller), three = try frame("sidePadDigit3", in: controller)
    let between = controller.view.convert(CGPoint(x: (two.maxX + three.minX) / 2 - 0.5, y: two.midY), to: area)
    XCTAssertTrue(area.hitTest(between, with: nil) === (try button("sidePadDigit2", in: controller)))
    let five = try frame("sidePadDigit5", in: controller), eight = try frame("sidePadDigit8", in: controller)
    let below = controller.view.convert(CGPoint(x: five.midX, y: (five.maxY + eight.minY) / 2 + 0.5), to: area)
    XCTAssertTrue(area.hitTest(below, with: nil) === (try button("sidePadDigit8", in: controller)))
  }

  /// iPad 键盘：全宽（regular）时是平板形态，`compact` 时是浮动键盘和窄窗口的手机形态；横屏用 compact 的竖直 size class 模拟（测试里的控制器没有窗口场景）。
  private func tablet(width: CGFloat, compact: Bool = false, landscape: Bool = false) -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = .pad
    controller.traitOverrides.horizontalSizeClass = compact ? .compact : .regular
    if landscape { controller.traitOverrides.verticalSizeClass = .compact }
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: width,
                                   height: KeyboardViewController.keyboardHeight(compact ? .phone : .tablet, landscape: landscape))
    controller.viewWillAppear(false)
    try? layoutAtRequestedHeight(controller)
    return controller
  }

  private func letterQ(in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" } as? UIButton)
  }

  private func requestedHeight(_ controller: KeyboardViewController) throws -> CGFloat {
    try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
  }

  /// 按键盘请求的高度排好，与 `KeyLayoutStyleTests` 相同。
  private func layoutAtRequestedHeight(_ controller: KeyboardViewController) throws {
    controller.view.frame.size.height = try requestedHeight(controller)
    controller.view.layoutIfNeeded()
  }

  /// 视图和它的每一层祖先都没有隐藏。
  private func shown(_ view: UIView) -> Bool {
    sequence(first: view, next: \.superview).allSatisfy { !$0.isHidden }
  }

  private func keyArea(_ controller: KeyboardViewController) throws -> KeyAreaStackView {
    try XCTUnwrap(descendants(controller.view).first { $0 is KeyAreaStackView } as? KeyAreaStackView)
  }

  /// `container` 里显示着的按键，从上到下、从左到右。
  private func keys(in container: UIView, _ controller: KeyboardViewController) throws -> [UIButton] {
    descendants(container).compactMap { $0 as? UIButton }.filter(shown).sorted {
      let a = KeyAreaStackView.layoutFrame(of: $0, in: controller.view), b = KeyAreaStackView.layoutFrame(of: $1, in: controller.view)
      return abs(a.minY - b.minY) > 0.5 ? a.minY < b.minY : a.minX < b.minX
    }
  }

  /// 组字区里还没上屏的拼写（ASCII 字母和数字）；没有组字时组字区读作「水杉输入法」，这里为空。
  private func preedit(_ controller: KeyboardViewController) throws -> String {
    (try button("preeditButton", in: controller).accessibilityValue ?? "").filter { $0.isASCII && ($0.isLetter || $0.isNumber) }
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
