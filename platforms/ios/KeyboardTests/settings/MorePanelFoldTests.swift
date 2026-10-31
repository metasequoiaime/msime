import UIKit
import XCTest

/// 功能菜单（「功能」）是分页网格，所以没有东西藏在折叠线以下：每个工具都在某一页上，顺序与 Android 的 `FunctionPanelModel` 一致，翻页能到达每一个。这些测试固定这个顺序、每页数量，以及那些所做的事面板本身显示不出来的工具。
@MainActor
final class MorePanelFoldTests: XCTestCase {
  private var savedDefaults: [String: Any] = [:]
  private var savedSharedPreferences: [String: Any]?
  /// 「完成」写入共享文档的触屏几何参数，测试后恢复原值，免得之后的键盘画得更高。
  private var savedGeometry: [String: Any] = [:]
  private let geometryKeys = ["touch_key_spacing_tenths", "touch_row_spacing_tenths", "touch_keyboard_height_adjustment"]
  private let defaultsKeys = [
    KeyboardPrivacyPreference.incognitoKey, KeyboardFeedbackPreference.soundKey, KeyboardFeedbackPreference.hapticsKey,
    KeyboardLayoutPreference.heightAdjustmentKey,
    ImeModeMemoryPreference.enabledKey, ImeModeMemoryPreference.lastChineseKey,
  ]

  override func setUp() {
    super.setUp()
    for key in defaultsKeys {
      savedDefaults[key] = KeyboardFeedbackPreference.defaults.object(forKey: key)
    }
    // 功能菜单的可用性取决于新键盘初始化时的中英文模式；测试不能继承模拟器上一次运行留下的英文状态。
    KeyboardFeedbackPreference.defaults.set(true, forKey: ImeModeMemoryPreference.enabledKey)
    KeyboardFeedbackPreference.defaults.set(true, forKey: ImeModeMemoryPreference.lastChineseKey)
    let document = MetasequoiaInputSessionBridge.loadSharedPreferences() ?? [:]
    savedSharedPreferences = MetasequoiaInputSessionBridge.loadSharedPreferences()
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
    if let savedSharedPreferences {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in document = savedSharedPreferences }
    }
    savedSharedPreferences = nil
    super.tearDown()
  }

  /// 先是设计稿里的各项，再是在别处没有位置的 iOS 工具；振动相关的项只在有 Taptic Engine 的设备上出现。
  private var expectedTitles: [String] {
    let haptics = KeyboardFeedbackPreference.hapticsAvailable
    var titles: [String] = ["全角", "中文标点", "模糊音", "繁体", "手写", "词库", "键盘高度", "设置", "按键音"]
    if haptics { titles.append("振动") }
    // 工具栏不放语音按钮，「语音输入」一直在菜单里。
    titles += ["单手模式", "隐私模式", "反馈", "关于", "AI 润色", "高情商回复", "本地输入", "语音输入"]
    if (try? VoiceTextHandoffStore().read()) != nil {
      titles.append("插入语音文字")
    }
    if haptics { titles.append("振动强度 \(KeyboardFeedbackPreference.hapticStrength.title)") }
    titles += ["表情", "宣传", "剪贴板历史", "清除候选缓存"]
    return titles
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
    XCTAssertEqual(panel.pageCount, (expectedTitles.count + 11) / 12)
    for index in 0..<panel.pageCount {
      XCTAssertEqual(tiles.filter { page(of: $0, in: scroll) == index }.count, min(12, expectedTitles.count - index * 12),
                     "page \(index + 1) holds 4 × 3 tiles")
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

  /// 三端共用的选择面板排布规则（Android `PagedTileGrid.pickerGrid`、HarmonyOS `FunctionPanelPolicy.pickerGrid` 用同一组向量）：高度够时满尺寸，不够时等比缩小，三行缩到 44pt 以下才改排两行，两行最多缩到 44pt。
  func testPickerLayoutFollowsTheSharedRule() {
    func check(_ preferred: Int, _ height: CGFloat, rows: Int, scale: CGFloat, line: UInt = #line) {
      let layout = KeyboardPagedGridView.pickerLayout(preferredRows: preferred, height: height, rowHeight: 52)
      XCTAssertEqual(layout.rows, rows, "preferred \(preferred), H \(height)", line: line)
      XCTAssertEqual(layout.scale, scale, accuracy: 0.0001, "preferred \(preferred), H \(height)", line: line)
      XCTAssertEqual(layout.rowHeight, 52 * scale, accuracy: 0.0001, line: line)
      XCTAssertEqual(layout.rowGap, KeyboardPagedGridView.pickerRowGap(rows: rows) * scale, accuracy: 0.0001, line: line)
    }
    check(3, 200, rows: 3, scale: 1)
    check(3, 180, rows: 3, scale: 1)
    check(3, 162, rows: 3, scale: 0.9)
    check(3, 152.4, rows: 3, scale: 152.4 / 180)
    check(3, 152, rows: 2, scale: 1)
    check(3, 100, rows: 2, scale: 44 / 52)
    check(2, 200, rows: 2, scale: 1)
    check(2, 110, rows: 2, scale: 110 / 120)
    // 正好缩到 44pt 的三行仍排三行。
    check(3, 180 * 44 / 52, rows: 3, scale: 44 / 52)
    XCTAssertEqual(KeyboardPagedGridView.pickerMinTileHeight, 44)
    XCTAssertEqual(KeyboardPagedGridView.pickerRowGap(rows: 3), 12)
    XCTAssertEqual(KeyboardPagedGridView.pickerRowGap(rows: 2), 16)
    // 行高本来就不超过 44pt 时两行不缩，也不放大。
    let small = KeyboardPagedGridView.pickerLayout(preferredRows: 2, height: 50, rowHeight: 40)
    XCTAssertEqual(small.rows, 2)
    XCTAssertEqual(small.scale, 1)
  }

  /// 面板装在键区里（`installPanel`：顶栏底边到键盘根视图底边，26 键竖屏再减去 10pt 的补偿留白），高度是顶栏下的间隔加按键区再加高度调整。两种选择面板都按这个真实高度排布：格子各行放得进滚动区、不能上下拖动，所有格子一样高，页码圆点不出面板。
  ///
  /// 最矮的手机竖屏是 26 键、行距 4pt、高度调到 -12pt：4 + 220 - 12 - 10 = 202pt，减去上下边距 14pt 和圆点 14pt 后格子只有 174pt，比三行满尺寸的 180pt 矮，于是三行等比缩到约 50.3pt；九键同样设置下是 212pt，放得下满尺寸。手机横屏最矮是 140pt，两行缩到 112 / 120。
  func testPickerPanelsFitTheRealKeyAreaHeight() throws {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    func panelHeight(_ form: KeyboardFormFactor, landscape: Bool, letterKeys: Bool, rowSpacing: CGFloat, adjustment: CGFloat) -> CGFloat {
      form.topRowGap
        + KeyboardHeightPercent.keyBlockHeight(keyHeight: form.keyHeight(landscape: landscape, handwriting: false), rows: 4, rowSpacing: rowSpacing)
        + adjustment - (form == .phone && letterKeys && !landscape ? KeyboardFormFactor.phoneLetterVerticalPadding : 0)
    }
    let shortest = KeyboardHeightPercent.adjustmentRange.lowerBound
    let cases: [(form: KeyboardFormFactor, landscape: Bool, width: CGFloat, height: CGFloat, rows: Int, scaled: Bool)] = [
      (.phone, false, 375, panelHeight(.phone, landscape: false, letterKeys: true, rowSpacing: 4, adjustment: shortest), 3, true),
      (.phone, false, 375, panelHeight(.phone, landscape: false, letterKeys: false, rowSpacing: 4, adjustment: shortest), 3, false),
      (.phone, false, 375, panelHeight(.phone, landscape: false, letterKeys: true, rowSpacing: 7, adjustment: 0), 3, false),
      (.phone, true, 812, panelHeight(.phone, landscape: true, letterKeys: true, rowSpacing: 4, adjustment: shortest), 2, true),
      (.phone, true, 812, panelHeight(.phone, landscape: true, letterKeys: true, rowSpacing: 7, adjustment: 0), 2, false),
      (.tablet, false, 820, panelHeight(.tablet, landscape: false, letterKeys: true, rowSpacing: 4, adjustment: shortest), 3, false),
      // 比任何真实键盘都矮：三行要缩到 44pt 以下，改排满尺寸的两行。
      (.phone, false, 375, 150, 2, false),
    ]
    XCTAssertEqual(cases[0].height, 202)
    XCTAssertEqual(cases[3].height, 140)
    for item in cases {
      for build in [{ KeyboardMorePickerView(tools: tools, formFactor: item.form, landscape: item.landscape) as UIView },
                    { KeyboardSchemePickerView(selected: .quanpin, showsHeader: false, formFactor: item.form, landscape: item.landscape,
                                               onSelect: { _ in }, onClose: {}) as UIView }] {
        let panel = build()
        let label = "\(type(of: panel)) \(item.form) landscape \(item.landscape) in \(item.height)pt"
        panel.frame = CGRect(x: 0, y: 0, width: item.width, height: item.height)
        panel.layoutIfNeeded()
        let grid = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagedGridView }.first)
        let scroll = try XCTUnwrap(descendants(grid).compactMap { $0 as? UIScrollView }.first)
        XCTAssertEqual(grid.rows, item.rows, label)
        let layout = KeyboardPagedGridView.pickerLayout(
          preferredRows: KeyboardPagedGridView.pickerRows(formFactor: item.form, landscape: item.landscape),
          height: scroll.bounds.height, rowHeight: 52)
        XCTAssertEqual(layout.scale < 1, item.scaled, label)
        XCTAssertLessThanOrEqual(scroll.contentSize.height, scroll.bounds.height + 0.5, "no vertical scrolling: \(label)")
        let tiles = descendants(grid).filter { $0 is KeyboardFunctionTileView || $0 is KeyboardSchemeTileView }
        XCTAssertFalse(tiles.isEmpty, label)
        for tile in tiles {
          XCTAssertEqual(tile.bounds.height, layout.rowHeight, accuracy: 0.5, label)
          let frame = tile.convert(tile.bounds, to: scroll)
          XCTAssertGreaterThanOrEqual(frame.minY, -0.5, label)
          XCTAssertLessThanOrEqual(frame.maxY, scroll.bounds.height + 0.5, label)
          // 键面跟着格子缩小，文字不出格子。
          for text in descendants(tile).compactMap({ $0 as? UILabel }) where text.text?.isEmpty == false && !text.isHidden {
            XCTAssertLessThanOrEqual(text.frame.maxY, tile.bounds.height + 1, label)
          }
        }
        let dots = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagerDotsView }.first)
        XCTAssertLessThanOrEqual(dots.convert(dots.bounds, to: panel).maxY, item.height + 0.5, label)
        XCTAssertEqual(dots.count, grid.pageCount, label)
      }
    }
  }

  /// 缩小的图块整个键面按同一比例缩小：字号跟着格子高度缩放，触摸区域仍是整个格子，无障碍信息不变。
  func testFunctionTileScalesItsFaceWithTheCell() throws {
    let tool = KeyboardTool(id: "fullwidth", title: "全角", face: .glyph("全"), isToggle: true, selected: true) {}
    let tile = KeyboardFunctionTileView(tool: tool, skin: KeyboardTheme.current)
    tile.frame = CGRect(x: 0, y: 0, width: 80, height: 52)
    tile.layoutIfNeeded()
    let title = try XCTUnwrap(tile.subviews.compactMap { $0 as? UILabel }.first { $0.text == "全角" })
    XCTAssertEqual(title.font.pointSize, 12.5, accuracy: 0.01)
    let label = tile.accessibilityLabel, value = tile.accessibilityValue, traits = tile.accessibilityTraits
    tile.frame = CGRect(x: 0, y: 0, width: 80, height: 44)
    tile.layoutIfNeeded()
    XCTAssertEqual(title.font.pointSize, 12.5 * 44 / 52, accuracy: 0.01)
    XCTAssertLessThanOrEqual(title.frame.maxY, 45)
    XCTAssertEqual(tile.accessibilityLabel, label)
    XCTAssertEqual(tile.accessibilityValue, value)
    XCTAssertEqual(tile.accessibilityTraits, traits)
    XCTAssertTrue(tile.point(inside: CGPoint(x: 2, y: 42), with: nil), "the whole cell stays the touch target")
  }

  /// 高度变化让行数改变时网格重新分页，停在含有原来那一页第一个图块的页上，页码圆点跟着换数量。
  func testPickerGridRepagesWhenTheRowCountChanges() throws {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    let panel = KeyboardMorePickerView(tools: tools, formFactor: .phone)
    panel.frame = CGRect(x: 0, y: 0, width: 375, height: 230)
    panel.layoutIfNeeded()
    let grid = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagedGridView }.first)
    let dots = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagerDotsView }.first)
    XCTAssertEqual(grid.rows, 3)
    XCTAssertEqual(panel.pageCount, 2)
    grid.scrollToPage(1, animated: false)

    // 第 2 页从第 13 个图块开始；两行每页 8 个时它在第 2 页。
    panel.frame.size.height = 150
    panel.layoutIfNeeded()
    XCTAssertEqual(grid.rows, 2)
    XCTAssertEqual(panel.pageCount, 3)
    XCTAssertEqual(dots.count, 3)
    XCTAssertEqual(panel.currentPage, 1)
    let scroll = try XCTUnwrap(descendants(grid).compactMap { $0 as? UIScrollView }.first)
    XCTAssertEqual(scroll.contentOffset.x, scroll.bounds.width, accuracy: 0.5)

    // 两行的第 3 页从第 17 个图块开始；回到三行时它在第 2 页。
    grid.scrollToPage(2, animated: false)
    panel.frame.size.height = 230
    panel.layoutIfNeeded()
    XCTAssertEqual(grid.rows, 3)
    XCTAssertEqual(panel.pageCount, 2)
    XCTAssertEqual(dots.count, 2)
    XCTAssertEqual(panel.currentPage, 1)
    for tile in descendants(grid).compactMap({ $0 as? KeyboardFunctionTileView }) {
      XCTAssertEqual(tile.bounds.height, KeyboardFunctionTileView.height, accuracy: 0.5)
    }
  }

  /// iPad 键盘每页显示 6 × 3 个图块，手机竖屏 4 × 3，手机横屏放不下三行，仍是 4 × 2。
  func testTheTabletMenuHoldsEighteenTilesAPage() {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    let tablet = KeyboardMorePickerView(tools: tools, formFactor: .tablet)
    tablet.frame = CGRect(x: 0, y: 0, width: 820, height: 237)
    tablet.layoutIfNeeded()
    XCTAssertEqual(tablet.pageCount, 2)
    let phone = KeyboardMorePickerView(tools: tools, formFactor: .phone)
    phone.frame = CGRect(x: 0, y: 0, width: 380, height: 201)
    phone.layoutIfNeeded()
    XCTAssertEqual(phone.pageCount, 2)
    let grid = descendants(tablet).compactMap { $0 as? KeyboardPagedGridView }.first
    XCTAssertEqual(grid?.tilesPerPage, 18)
    let landscape = KeyboardMorePickerView(tools: tools, formFactor: .phone, landscape: true)
    XCTAssertEqual(descendants(landscape).compactMap { $0 as? KeyboardPagedGridView }.first?.tilesPerPage, 8)
    XCTAssertEqual(KeyboardPagedGridView.pickerRows(formFactor: .tablet, landscape: true), 3)
  }

  /// 翻页交给滚动视图自己的分页：分页打开，自带的 pan 开着、没有另装的拖动手势，从图块上开始的拖动能取消图块的触摸。
  func testPagedGridUsesSystemPagingFromTiles() throws {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    let panel = KeyboardMorePickerView(tools: tools, formFactor: .phone)
    panel.frame = CGRect(x: 0, y: 0, width: 380, height: 201)
    panel.layoutIfNeeded()
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    let tile = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardFunctionTileView }.first)
    XCTAssertTrue(scroll.isPagingEnabled)
    XCTAssertTrue(scroll.panGestureRecognizer.isEnabled)
    XCTAssertEqual(scroll.gestureRecognizers?.filter { $0 is UIPanGestureRecognizer }.count, 1, "only the scroll view's own pan")
    XCTAssertFalse(scroll.delaysContentTouches)
    XCTAssertTrue(scroll.canCancelContentTouches)
    XCTAssertTrue(scroll.touchesShouldCancel(in: tile))
  }

  /// 反向拖动或布局中断减速时，分页网格必须回到最近的整页，避免下一次手势从半页继续。
  func testPagedGridSettlesAfterInterruptedDrag() throws {
    let tools = (0..<20).map { index in KeyboardTool(id: "tool\(index)", title: "工具\(index)", face: .glyph("\(index)")) {} }
    let panel = KeyboardMorePickerView(tools: tools, formFactor: .phone)
    panel.frame = CGRect(x: 0, y: 0, width: 380, height: 201)
    panel.layoutIfNeeded()
    let grid = try XCTUnwrap(descendants(panel).compactMap { $0 as? KeyboardPagedGridView }.first)
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    let width = scroll.bounds.width
    grid.scrollToPage(1, animated: false)
    scroll.setContentOffset(CGPoint(x: width * 0.2, y: 0), animated: false)
    grid.scrollViewDidEndDragging(scroll, willDecelerate: false)
    XCTAssertEqual(scroll.contentOffset.x, 0, accuracy: 0.5)
    scroll.setContentOffset(CGPoint(x: width * 0.8, y: 0), animated: false)
    grid.scrollViewDidEndDragging(scroll, willDecelerate: false)
    XCTAssertEqual(scroll.contentOffset.x, width, accuracy: 0.5)
    XCTAssertEqual(grid.currentPage, 1)
  }

  /// Windows 上的 Ctrl+Shift+Alt+C 组合键在 iOS 上没有键可按，所以菜单把它放在最后一页；点这个图块会关闭菜单并提示已生效。
  func testTheClearCacheCardClearsAndSaysSo() throws {
    let controller = makeController(width: 390)
    let panel = try openMenu(in: controller)
    let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
    let card = try tile("moreCard-清除候选缓存", in: controller)
    XCTAssertEqual(page(of: card, in: scroll), panel.pageCount - 1, "清除候选缓存 is on the last page")
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
    _ = InputSchemePreference.update { selection in
      selection.enabled = ChineseInputScheme.allCases
      selection.scheme = .quanpin
    }
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
    let keyBlock = KeyboardViewController.keyBlockHeight(
      keyHeight: KeyboardHeightPercent.portraitKeyHeight(tablet: false), rows: 4, rowSpacing: CGFloat(KeyboardLayoutPreference.rowSpacing))
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

  /// 调节条开着时按键照常可用，所以按键打开的面板（这里是底行的符号键）不会结束调整、也不会丢掉预览的高度；与 Android 一样，只有「取消」「完成」或收起键盘才结束，收起键盘按「取消」处理。
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
    let hasPicker = { self.descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSymbolPanel" } }
    let symbols = try button("symbolPanelKey", in: controller)
    XCTAssertFalse(symbols.isHidden)
    symbols.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(hasPicker())
    XCTAssertTrue(hasBar(), "a panel opened from the keys leaves the bar in place")
    XCTAssertEqual(heightConstraint.constant, previewed, accuracy: 0.5, "and keeps the previewed height")
    try control("closeSymbolPanel", in: controller.view).sendActions(for: .primaryActionTriggered)
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
    let keyBlock = KeyboardViewController.keyBlockHeight(keyHeight: 52, rows: 4, rowSpacing: 7)
    XCTAssertEqual(keyBlock, 229, "四排 52pt 手机按键与三个 7pt 行间距")
    XCTAssertEqual(KeyboardHeightPercent.portraitKeyBlockHeight(tablet: false, numberRow: true, rowSpacing: 7), keyBlock,
                   "a phone has no digit row")
    XCTAssertEqual(KeyboardHeightPercent.portraitKeyBlockHeight(tablet: true, numberRow: true, rowSpacing: 7), 5 * 54 + 4 * 7)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: 0, keyBlock: keyBlock), 100)
    XCTAssertEqual(KeyboardViewController.heightAdjustment(percent: 100, keyBlock: keyBlock), 0)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: -12, keyBlock: keyBlock), 95)
    XCTAssertEqual(KeyboardViewController.heightPercent(adjustment: 48, keyBlock: keyBlock), 121)
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
