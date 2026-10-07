import XCTest
import UIKit

/// iPhone 与 iPad 键盘的区分。
///
/// iPad 不等于平板键盘:浮动键盘和窄窗口是 compact 宽度,系统键盘在那里也画手机布局,这个键盘要跟着同一条规则走。
final class KeyboardFormFactorTests: XCTestCase {
  func testOnlyARegularWidthIPadGetsTheTabletKeyboard() {
    XCTAssertEqual(KeyboardFormFactor.resolve(idiom: .pad, horizontalSizeClass: .regular), .tablet)
    XCTAssertEqual(KeyboardFormFactor.resolve(idiom: .pad, horizontalSizeClass: .unspecified), .tablet)
    XCTAssertEqual(KeyboardFormFactor.resolve(idiom: .pad, horizontalSizeClass: .compact), .phone)
    XCTAssertEqual(KeyboardFormFactor.resolve(idiom: .phone, horizontalSizeClass: .regular), .phone)
    XCTAssertEqual(KeyboardFormFactor.resolve(idiom: .phone, horizontalSizeClass: .compact), .phone)
  }

  /// 手机高度保持原值;平板竖屏按设计稿的 54pt 键高定(见 testDesignKeyHeights),横屏更高,与系统键盘一致。
  func testHeights() {
    XCTAssertEqual(KeyboardFormFactor.phone.baseHeight(landscape: false, handwriting: false), 260)
    XCTAssertEqual(KeyboardFormFactor.phone.baseHeight(landscape: false, handwriting: true), 260)
    XCTAssertEqual(KeyboardFormFactor.phone.baseHeight(landscape: true, handwriting: false), 216)
    XCTAssertEqual(KeyboardFormFactor.phone.baseHeight(landscape: true, handwriting: true), 240)
    let portrait = KeyboardFormFactor.tablet.baseHeight(landscape: false, handwriting: false)
    let landscape = KeyboardFormFactor.tablet.baseHeight(landscape: true, handwriting: false)
    XCTAssertGreaterThan(portrait, 260)
    XCTAssertGreaterThan(landscape, portrait)
    // 数字行是整整一排键，平板加一排的高度而不是压扁字母；手机不带数字行。
    XCTAssertGreaterThan(KeyboardFormFactor.tablet.baseHeight(landscape: false, handwriting: false, numberRow: true), portrait + 40)
    XCTAssertGreaterThan(KeyboardFormFactor.tablet.baseHeight(landscape: true, handwriting: false, numberRow: true), landscape + 40)
    XCTAssertTrue(KeyboardFormFactor.tablet.canShowFullKeys)
    XCTAssertFalse(KeyboardFormFactor.phone.canShowFullKeys)
  }

  /// The drawn key heights at the default row spacing. The iPad keys are the design's 54pt (dc.html L1559 `keyH`), with or without the digit row. The phone keeps its 260pt keyboard, which leaves its keys above the 44pt touch target the layout tests hold them to, rather than the design's 42pt.
  @MainActor
  func testDesignKeyHeights() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let keys = [KeyboardLayoutPreference.rowSpacingKey, KeyboardLayoutPreference.heightAdjustmentKey, KeyboardLayoutPreference.tabletFullKeysKey]
    let stored = keys.map { defaults.object(forKey: $0) }
    defer { for (key, value) in zip(keys, stored) { defaults.set(value, forKey: key) } }
    keys.forEach { defaults.removeObject(forKey: $0) }
    KeyboardLayoutPreference.rowSpacing = 7

    func letterHeight(tablet: Bool, width: CGFloat, height: CGFloat) throws -> CGFloat {
      let controller = KeyboardViewController()
      if tablet {
        controller.traitOverrides.userInterfaceIdiom = .pad
        controller.traitOverrides.horizontalSizeClass = .regular
      }
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: height + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      return try key("letterDeleteKey", in: controller).bounds.height
    }
    let phone = try letterHeight(tablet: false, width: 393, height: KeyboardFormFactor.phone.baseHeight(landscape: false, handwriting: false))
    XCTAssertGreaterThanOrEqual(phone, 44)
    let withRow = try letterHeight(tablet: true, width: 820, height: KeyboardFormFactor.tablet.baseHeight(landscape: false, handwriting: false, numberRow: true))
    XCTAssertEqual(withRow, 54, accuracy: 0.5)
    KeyboardLayoutPreference.tabletFullKeys = false
    let plain = try letterHeight(tablet: true, width: 820, height: KeyboardFormFactor.tablet.baseHeight(landscape: false, handwriting: false))
    XCTAssertEqual(plain, 54, accuracy: 0.5)
  }

  /// iPad 全尺寸键盘有数字行和 Tab 键，可以在设置里关掉；手机（以及 iPad 的窄键盘）始终没有。
  func testTabletCarriesTheDigitRowAndTabUnlessTurnedOff() throws {
    let stored = KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.tabletFullKeysKey)
    defer { KeyboardLayoutPreference.defaults.set(stored, forKey: KeyboardLayoutPreference.tabletFullKeysKey) }
    KeyboardLayoutPreference.defaults.removeObject(forKey: KeyboardLayoutPreference.tabletFullKeysKey)
    XCTAssertTrue(KeyboardLayoutPreference.tabletFullKeys, "on by default")

    let phone = KeyboardViewController()
    phone.loadViewIfNeeded()
    phone.view.frame = CGRect(x: 0, y: 0, width: 440, height: 292)
    phone.view.layoutIfNeeded()
    XCTAssertTrue(try view("numberRow", in: phone).isHidden)
    XCTAssertTrue(try key("tabKey", in: phone).isHidden)

    let tablet = tabletController()
    let numberRow = try view("numberRow", in: tablet)
    XCTAssertFalse(numberRow.isHidden)
    XCTAssertFalse(try key("tabKey", in: tablet).isHidden)
    XCTAssertEqual(try key("numberRowKey0", in: tablet).configuration?.title, "0")
    let tab = try key("tabKey", in: tablet)
    let firstRow = try XCTUnwrap(tab.superview as? UIStackView)
    XCTAssertEqual(firstRow.arrangedSubviews.first, tab, "Tab comes before Q")
    XCTAssertGreaterThan(tab.bounds.width, firstRow.arrangedSubviews[1].bounds.width * 1.3)
    let withRow = try XCTUnwrap(tablet.view.constraints.first { $0.identifier == "keyboardHeight" }).constant

    KeyboardLayoutPreference.tabletFullKeys = false
    let plain = tabletController()
    XCTAssertTrue(try view("numberRow", in: plain).isHidden)
    XCTAssertTrue(try key("tabKey", in: plain).isHidden)
    let withoutRow = try XCTUnwrap(plain.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    XCTAssertGreaterThan(withRow, withoutRow)
  }

  /// Tab 在组字时对应桌面端的 `navigation.tab` 翻页，默认开。
  func testTabFollowsTheSharedPagingSwitch() {
    XCTAssertTrue(KeyboardViewController.tabShowsMoreCandidates(nil))
    XCTAssertTrue(KeyboardViewController.tabShowsMoreCandidates(["navigation": ["tab": true]]))
    XCTAssertFalse(KeyboardViewController.tabShowsMoreCandidates(["navigation": ["tab": false]]))
  }

  /// The App's iPad switch writes `navigation.tab` and leaves the other paging keys where they were.
  func testTheIPadSwitchWritesOnlyTheTabPagingKey() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-tab-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let before = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)?["navigation"] as? [String: Any])

    XCTAssertTrue(KeyboardLayoutPreference.saveTabShowsMoreCandidates(false, stateRoot: state))
    let stored = MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state)
    XCTAssertFalse(KeyboardLayoutPreference.tabShowsMoreCandidates(stored))
    let after = try XCTUnwrap(stored?["navigation"] as? [String: Any])
    XCTAssertEqual(Set(after.keys), Set(before.keys))
    for (name, value) in before where name != "tab" {
      XCTAssertEqual(after[name] as? NSObject, value as? NSObject, name)
    }
  }

  // MARK: - 横屏分离式键盘

  /// 只有平板形态（regular 宽度的 iPad）横屏且开关打开时才分；手机、iPad 的浮动键盘和窄窗口（compact 宽度）以及竖屏都不分。
  func testSplitKeyboardAppliesOnlyToALandscapeTabletWithTheSwitchOn() {
    for idiom in [UIUserInterfaceIdiom.pad, .phone] {
      for sizeClass in [UIUserInterfaceSizeClass.regular, .compact, .unspecified] {
        for landscape in [true, false] {
          for enabled in [true, false] {
            let formFactor = KeyboardFormFactor.resolve(idiom: idiom, horizontalSizeClass: sizeClass)
            let expected = idiom == .pad && sizeClass != .compact && landscape && enabled
            XCTAssertEqual(
              KeyboardSplitLayout.isActive(formFactor: formFactor, landscape: landscape, enabled: enabled), expected,
              "idiom \(idiom.rawValue) size \(sizeClass.rawValue) landscape \(landscape) enabled \(enabled)")
          }
        }
      }
    }
    // 手机和竖屏不去读开关。
    XCTAssertFalse(KeyboardSplitLayout.isActive(formFactor: .phone, landscape: true, enabled: { XCTFail("read"); return true }()))
    XCTAssertFalse(KeyboardSplitLayout.isActive(formFactor: .tablet, landscape: false, enabled: { XCTFail("read"); return true }()))
  }

  /// 26 键字母方案分，九键、笔画、假名九键、注音大千和手写不分；英文和本地输入模式画的是字母，照样分。
  func testOnlyTheLetterLayoutsSplit() {
    let unsplit: Set<ChineseInputScheme> = [.nineKey, .stroke, .japaneseNineKey, .zhuyin, .handwriting]
    for scheme in ChineseInputScheme.allCases {
      XCTAssertEqual(KeyboardSplitLayout.splitsLayout(scheme: scheme, chinese: true, localMode: false),
                     !unsplit.contains(scheme), scheme.rawValue)
      XCTAssertTrue(KeyboardSplitLayout.splitsLayout(scheme: scheme, chinese: false, localMode: false), scheme.rawValue)
      XCTAssertTrue(KeyboardSplitLayout.splitsLayout(scheme: scheme, chinese: true, localMode: true), scheme.rawValue)
    }
  }

  /// 每排从中间分，奇数时左半多一个：qwert | yuiop、asdfg | hjkl(;)、zxcvb | nm，。、12345 | 67890。中缝视图减去两侧的键距，两半之间正好空出四分之一。
  func testSplitPointAndGapWidth() {
    XCTAssertEqual(KeyboardSplitLayout.leftKeyCount(10), 5)
    XCTAssertEqual(KeyboardSplitLayout.leftKeyCount(9), 5)
    XCTAssertEqual(KeyboardSplitLayout.leftKeyCount(7), 4)
    XCTAssertEqual(KeyboardSplitLayout.leftKeyCount(1), 1)
    XCTAssertEqual(KeyboardSplitLayout.leftKeyCount(0), 0)
    XCTAssertEqual(KeyboardSplitLayout.gapRatio, 0.25)
    XCTAssertEqual(KeyboardSplitLayout.gapViewWidth(rowWidth: 1184, keySpacing: 6), 284)
    XCTAssertEqual(KeyboardSplitLayout.gapViewWidth(rowWidth: 10, keySpacing: 6), 0)
  }

  /// 开关默认关；App 那一页的「恢复默认」把它和间距、高度一起还原，键盘里间距面板的 `resetToDefaults` 不碰它。
  func testSplitSwitchDefaultsOffAndResetsWithTheAppPage() throws {
    let defaults = KeyboardLayoutPreference.defaults
    let stored = defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey)
    defer { defaults.set(stored, forKey: KeyboardLayoutPreference.tabletSplitKey) }
    defaults.removeObject(forKey: KeyboardLayoutPreference.tabletSplitKey)
    XCTAssertEqual(KeyboardLayoutPreference.tabletSplitKey, "keyboard.tablet.split")
    XCTAssertFalse(KeyboardLayoutPreference.tabletSplit, "off by default")

    KeyboardLayoutPreference.tabletSplit = true
    XCTAssertTrue(KeyboardLayoutPreference.tabletSplit)
    XCTAssertEqual(defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey) as? Bool, true)
    KeyboardLayoutPreference.resetToDefaults()
    XCTAssertTrue(KeyboardLayoutPreference.tabletSplit, "the keyboard's spacing panel does not show this switch")

    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-split-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    let geometry = [KeyboardLayoutPreference.keySpacingKey, KeyboardLayoutPreference.rowSpacingKey,
                    KeyboardLayoutPreference.heightAdjustmentKey, KeyboardLayoutPreference.voiceShortcutKey]
    let storedGeometry = geometry.map { defaults.object(forKey: $0) }
    defer { for (key, value) in zip(geometry, storedGeometry) { defaults.set(value, forKey: key) } }
    XCTAssertTrue(KeyboardLayoutPreference.resetGeometry(stateRoot: state))
    XCTAssertNil(defaults.object(forKey: KeyboardLayoutPreference.tabletSplitKey))
    XCTAssertFalse(KeyboardLayoutPreference.tabletSplit)
  }

  /// iPad 横屏打开开关后，数字行、三排字母和底部那一排都从中间分开，两半之间空出键区宽度的四分之一；右半多一个同样宽的空格，键盘高度不变。
  @MainActor
  func testLandscapeTabletSplitsEveryRowAroundAQuarterWideGap() throws {
    try withSplitPreferences(scheme: .quanpin) {
      KeyboardLayoutPreference.tabletSplit = false
      let whole = splitTestController()
      let wholeHeight = try XCTUnwrap(whole.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
      XCTAssertTrue(visibleGaps(in: whole).isEmpty)
      XCTAssertTrue(try key("splitSpaceKey", in: whole).isHidden)

      KeyboardLayoutPreference.tabletSplit = true
      let split = splitTestController()
      let rowWidth = split.view.bounds.width - 10
      let gap = rowWidth * KeyboardSplitLayout.gapRatio
      XCTAssertEqual(visibleGaps(in: split).count, 5, "number row, three letter rows and the bottom row")
      for (left, right) in [("t", "y"), ("g", "h"), ("b", "n")] {
        let leftFrame = try frame(letter(left, in: split), in: split)
        let rightFrame = try frame(letter(right, in: split), in: split)
        XCTAssertEqual(rightFrame.minX - leftFrame.maxX, gap, accuracy: 1, "\(left) | \(right)")
      }
      let five = try frame(key("numberRowKey5", in: split), in: split)
      let six = try frame(key("numberRowKey6", in: split), in: split)
      XCTAssertEqual(six.minX - five.maxX, gap, accuracy: 1)
      // 逗号句号、删除留在右半外沿，Shift 和 Tab 留在左半外沿。
      XCTAssertGreaterThan(try frame(key("letterRowCommaKey", in: split), in: split).minX, five.maxX + gap)
      XCTAssertLessThan(try frame(key("shiftButton", in: split), in: split).maxX, five.maxX)
      XCTAssertLessThan(try frame(key("tabKey", in: split), in: split).maxX, five.maxX)

      let space = try frame(key("spaceKey", in: split), in: split)
      let splitSpace = try key("splitSpaceKey", in: split)
      XCTAssertFalse(splitSpace.isHidden)
      let splitSpaceFrame = frame(splitSpace, in: split)
      XCTAssertEqual(splitSpaceFrame.minX - space.maxX, gap, accuracy: 1)
      XCTAssertEqual(splitSpaceFrame.width, space.width, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(space.width, 44)
      XCTAssertEqual(splitSpace.accessibilityLabel, "空格")

      let splitHeight = try XCTUnwrap(split.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
      XCTAssertEqual(splitHeight, wholeHeight, "the keyboard keeps its height")
    }
  }

  /// 竖屏、手机、iPad 的浮动键盘（compact 宽度）都不分，开关开着也一样。
  @MainActor
  func testPortraitPhonesAndFloatingKeyboardsStayWhole() throws {
    try withSplitPreferences(scheme: .quanpin) {
      KeyboardLayoutPreference.tabletSplit = true
      for (idiom, sizeClass, landscape) in [
        (UIUserInterfaceIdiom.pad, UIUserInterfaceSizeClass.regular, false),
        (.pad, .compact, true),
        (.phone, .compact, true),
      ] {
        let controller = splitTestController(idiom: idiom, horizontalSizeClass: sizeClass, landscape: landscape)
        XCTAssertTrue(visibleGaps(in: controller).isEmpty, "\(idiom.rawValue) \(sizeClass.rawValue) \(landscape)")
        XCTAssertTrue(try key("splitSpaceKey", in: controller).isHidden)
      }
    }
  }

  /// 九键、笔画、手写、注音大千和假名九键横屏也不分。注音和笔画要有各自的语言词库才会出现，没有暂存词库的运行跳过这两个。
  @MainActor
  func testNonLetterLayoutsStayWhole() throws {
    for scheme in [ChineseInputScheme.nineKey, .stroke, .handwriting, .zhuyin, .japaneseNineKey] {
      try withSplitPreferences(scheme: scheme) {
        guard InputSchemePreference.scheme == scheme else { return }
        KeyboardLayoutPreference.tabletSplit = true
        let controller = splitTestController()
        XCTAssertTrue(visibleGaps(in: controller).isEmpty, scheme.rawValue)
        XCTAssertTrue(try key("splitSpaceKey", in: controller).isHidden, scheme.rawValue)
      }
    }
  }

  /// 旋转、开关在键盘显示时改变、切到 123 符号页、关掉数字行、微软双拼多出 `;`：分离状态都跟着当前情况走。
  @MainActor
  func testSplitFollowsRotationTheSwitchAndTheLayer() throws {
    try withSplitPreferences(scheme: .microsoft) {
      KeyboardLayoutPreference.tabletSplit = true
      let controller = splitTestController()
      let gap = (controller.view.bounds.width - 10) * KeyboardSplitLayout.gapRatio
      XCTAssertFalse(try key("splitSpaceKey", in: controller).isHidden)
      // 微软双拼的 `;` 归右半，g | h 的位置不变。
      let semicolon = try key("microsoftFinalKey", in: controller)
      XCTAssertFalse(semicolon.isHidden)
      let g = try frame(letter("g", in: controller), in: controller)
      XCTAssertEqual(try frame(letter("h", in: controller), in: controller).minX - g.maxX, gap, accuracy: 1)
      XCTAssertEqual(frame(semicolon, in: controller).width, try frame(letter("h", in: controller), in: controller).width, accuracy: 0.5)

      // 转到竖屏：iPad 旋转时 size class 不变，靠布局时重新核对。
      controller.traitOverrides.verticalSizeClass = .regular
      relayout(controller)
      XCTAssertTrue(visibleGaps(in: controller).isEmpty)
      XCTAssertTrue(try key("splitSpaceKey", in: controller).isHidden)
      controller.traitOverrides.verticalSizeClass = .compact
      relayout(controller)
      XCTAssertEqual(visibleGaps(in: controller).count, 5)

      // 开关在键盘显示时被关掉，下一次布局就收起中缝。
      KeyboardLayoutPreference.tabletSplit = false
      relayout(controller)
      XCTAssertTrue(visibleGaps(in: controller).isEmpty)
      KeyboardLayoutPreference.tabletSplit = true
      relayout(controller)
      XCTAssertEqual(visibleGaps(in: controller).count, 5)

      // 123 符号页同样分开：数字 12345 | 67890。
      try key("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(visibleGaps(in: controller).count, 4, "three symbol rows and the bottom row")
      let five = try frame(titled("5", in: controller), in: controller)
      let six = try frame(titled("6", in: controller), in: controller)
      XCTAssertEqual(six.minX - five.maxX, gap, accuracy: 1)
      try key("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()

      // 关掉数字行与 Tab 键后字母照样分开。
      KeyboardLayoutPreference.tabletFullKeys = false
      let plain = splitTestController()
      XCTAssertTrue(try view("numberRow", in: plain).isHidden)
      XCTAssertEqual(visibleGaps(in: plain).count, 4, "three letter rows and the bottom row")
      let t = try frame(letter("t", in: plain), in: plain)
      XCTAssertEqual(try frame(letter("y", in: plain), in: plain).minX - t.maxX, gap, accuracy: 1)
    }
  }

  @MainActor
  private func withSplitPreferences(scheme: ChineseInputScheme, _ body: () throws -> Void) throws {
    enableAllInputSchemes()
    let defaults = KeyboardLayoutPreference.defaults
    let keys = [KeyboardLayoutPreference.tabletSplitKey, KeyboardLayoutPreference.tabletFullKeysKey,
                KeyboardLayoutPreference.keySpacingKey, KeyboardLayoutPreference.rowSpacingKey]
    let stored = keys.map { defaults.object(forKey: $0) }
    let previousScheme = InputSchemePreference.scheme
    defer {
      for (key, value) in zip(keys, stored) { defaults.set(value, forKey: key) }
      InputSchemePreference.scheme = previousScheme
    }
    keys.forEach { defaults.removeObject(forKey: $0) }
    InputSchemePreference.scheme = scheme
    try body()
  }

  /// 测试里的控制器没有窗口场景，横屏用 compact 的竖直 size class 模拟（见 `KeyboardViewController.isLandscape`）。
  @MainActor
  private func splitTestController(
    idiom: UIUserInterfaceIdiom = .pad, horizontalSizeClass: UIUserInterfaceSizeClass = .regular, landscape: Bool = true
  ) -> KeyboardViewController {
    let controller = KeyboardViewController()
    controller.traitOverrides.userInterfaceIdiom = idiom
    controller.traitOverrides.horizontalSizeClass = horizontalSizeClass
    controller.traitOverrides.verticalSizeClass = landscape ? .compact : .regular
    controller.loadViewIfNeeded()
    let width: CGFloat = idiom == .phone ? 852 : (landscape ? 1194 : 834)
    let height = KeyboardFormFactor.tablet.baseHeight(landscape: true, handwriting: false, numberRow: true)
    controller.view.frame = CGRect(x: 0, y: 0, width: width, height: height + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  private func relayout(_ controller: KeyboardViewController) {
    controller.view.setNeedsLayout()
    controller.view.layoutIfNeeded()
    controller.view.layoutIfNeeded()
  }

  /// 正在显示的中缝：自己和所在的那一排都没有隐藏。
  private func visibleGaps(in controller: KeyboardViewController) -> [UIView] {
    descendants(controller.view).filter {
      $0.accessibilityIdentifier == "splitKeyboardGap" && !$0.isHidden && $0.superview?.isHidden == false
    }
  }

  private func letter(_ lowercase: String, in controller: KeyboardViewController) throws -> UIButton {
    try titled(lowercase, in: controller)
  }

  /// 键面是 `title`（不分大小写）而且整条父链都没隐藏的那个键。
  private func titled(_ title: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { view in
      guard let button = view as? UIButton, button.configuration?.title?.lowercased() == title else { return false }
      var node: UIView? = button
      while let current = node, current !== controller.view {
        if current.isHidden { return false }
        node = current.superview
      }
      return true
    } as? UIButton, "no visible key \(title)")
  }

  private func frame(_ view: UIView, in controller: KeyboardViewController) -> CGRect {
    view.convert(view.bounds, to: controller.view)
  }

  private func tabletController() -> KeyboardViewController {
    let tablet = KeyboardViewController()
    tablet.traitOverrides.userInterfaceIdiom = .pad
    tablet.traitOverrides.horizontalSizeClass = .regular
    tablet.loadViewIfNeeded()
    tablet.view.frame = CGRect(x: 0, y: 0, width: 1032, height: 440)
    tablet.view.layoutIfNeeded()
    return tablet
  }

  private func view(_ identifier: String, in controller: KeyboardViewController) throws -> UIView {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier })
  }

  /// 手机上第三排没有逗号句号;平板上有,并且中文模式下键面是中文标点。
  func testTabletLetterRowCarriesCommaAndFullStop() throws {
    let phone = KeyboardViewController()
    phone.loadViewIfNeeded()
    phone.view.frame = CGRect(x: 0, y: 0, width: 440, height: 292)
    phone.view.layoutIfNeeded()
    XCTAssertTrue(try key("letterRowCommaKey", in: phone).isHidden)
    XCTAssertTrue(try key("letterRowPeriodKey", in: phone).isHidden)

    let tablet = KeyboardViewController()
    tablet.traitOverrides.userInterfaceIdiom = .pad
    tablet.traitOverrides.horizontalSizeClass = .regular
    tablet.loadViewIfNeeded()
    tablet.view.frame = CGRect(x: 0, y: 0, width: 1032, height: 380)
    tablet.view.layoutIfNeeded()
    let comma = try key("letterRowCommaKey", in: tablet)
    XCTAssertFalse(comma.isHidden)
    XCTAssertFalse(try key("letterRowPeriodKey", in: tablet).isHidden)
    XCTAssertEqual(comma.configuration?.title, "，")

    let phoneHeight = try XCTUnwrap(phone.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    let tabletHeight = try XCTUnwrap(tablet.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    XCTAssertGreaterThan(tabletHeight, phoneHeight)
  }

  private func key(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == identifier } as? UIButton)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }
}
