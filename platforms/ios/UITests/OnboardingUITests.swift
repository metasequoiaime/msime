import XCTest

final class OnboardingUITests: XCTestCase {
  @MainActor
  func testAppIconsAreAvailableFromMyTabAndPersist() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    func openIcons() {
      app.tabBars.buttons["我的"].tap()
      let loginAlert = app.alerts["账号与登录"]
      if loginAlert.waitForExistence(timeout: 5) { loginAlert.buttons["好"].tap() }
      app.buttons["accountAppIcon"].tap()
      XCTAssertTrue(app.buttons["appIcon_classic"].waitForExistence(timeout: 5))
    }
    openIcons()
    for style in ["classic", "forest", "sky", "dusk", "vermilion"] {
      XCTAssertTrue(app.buttons["appIcon_\(style)"].exists)
    }
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "App icon gallery"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)

    let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    func select(_ style: String) {
      let button = app.buttons["appIcon_\(style)"]
      if button.value as? String == "使用中" { return }
      if !button.isHittable { app.swipeUp() }
      button.tap()
      // iOS 26 exposes its icon notification in the host app's hierarchy.
      let confirmation = app.buttons.matching(NSPredicate(format: "label IN %@", ["OK", "好"])).firstMatch
      if confirmation.waitForExistence(timeout: 10) {
        XCTAssertFalse(app.alerts["暂时无法更换图标"].exists, app.alerts.debugDescription)
        confirmation.tap()
      } else if springboard.alerts.firstMatch.exists {
        springboard.alerts.buttons.firstMatch.tap()
      }
      let selected = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", "使用中"), object: button)
      XCTAssertEqual(XCTWaiter.wait(for: [selected], timeout: 10), .completed)
    }
    // One change only. A Simulator applies the first alternate icon it is given and then refuses
    // every later request, relaunching the app included, so asking for a second one would test the
    // Simulator rather than the app. A fresh CI runner always exercises a real change; a Simulator
    // reused locally may already hold this icon, in which case select returns and the assertion
    // below still describes what the system reports.
    select("sky")
    let selectedScreenshot = XCTAttachment(screenshot: app.screenshot())
    selectedScreenshot.name = "App icon selected"
    selectedScreenshot.lifetime = .deleteOnSuccess
    add(selectedScreenshot)
    app.terminate()
    app.launch()
    openIcons()
    XCTAssertEqual(app.buttons["appIcon_sky"].value as? String, "使用中")
  }

  @MainActor
  func testAccountEntryExplainsExplicitDataSharing() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    let account = app.tabBars.buttons["我的"]
    XCTAssertTrue(account.waitForExistence(timeout: 5))
    account.tap()
    // The page carries the tab's large title now, and its own rows are what say which page it is.
    XCTAssertTrue(account.isSelected)
    XCTAssertTrue(app.navigationBars["我的"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["accountAppIcon"].waitForExistence(timeout: 5))
    // 我的设计 was withdrawn from this page: the skin editor keeps one entry, on the skin page,
    // rather than the same destination under two tabs.
    XCTAssertTrue(app.buttons["accountAppIcon"].exists)
  }

  @MainActor
  func testKeyboardSpacingSettingsPersist() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // The home page is a lazy list, so a row below the fold does not exist until it is scrolled to.
    reachSettingsLink("keyboardLayoutLink", in: app)
    settingsEntry("keyboardLayoutLink", in: app).tap()
    let keys = app.sliders["appKeySpacingSlider"]
    let rows = app.sliders["appRowSpacingSlider"]
    XCTAssertTrue(keys.waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["layoutPreset_msime"].exists)
    // 语音入口已移到 语音输入 → 启动方式，所以这一页上无论标识符是什么，都没有与它相关的开关。
    XCTAssertFalse(app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", "语音入口")).firstMatch.exists)
    func position(_ slider: XCUIElement) throws -> CGFloat {
      let raw = try XCTUnwrap(slider.value as? String)
      let value = try XCTUnwrap(Double(raw.replacingOccurrences(of: "%", with: "")))
      if raw.contains("%") { return CGFloat(value / 100) }
      let minimum = slider.identifier == "appKeySpacingSlider" ? 3.0 : 4.0
      let maximum = slider.identifier == "appKeySpacingSlider" ? 6.0 : 10.0
      return CGFloat((value - minimum) / (maximum - minimum))
    }
    let originalKeys = try position(keys), originalRows = try position(rows)
    // 间距存在 app group 里，比 app 本身活得久，测试中途失败就会把本测试的值留给之后的每个测试（以及共用这个 group 的键盘单元测试）。即使 `continueAfterFailure = false` 终止了测试，teardown block 也会执行，`defer` 不保证这一点，所以还原放在那里；下面的正常路径仍会检查这些值确实恢复了。
    let restore = SpacingRestore()
    addTeardownBlock { @MainActor in
      guard !restore.done else { return }
      app.terminate()
      app.launch()
      self.reachSettingsLink("keyboardLayoutLink", in: app)
      self.settingsEntry("keyboardLayoutLink", in: app).tap()
      guard keys.waitForExistence(timeout: 5) else { return }
      keys.adjust(toNormalizedSliderPosition: originalKeys)
      rows.adjust(toNormalizedSliderPosition: originalRows)
    }
    keys.adjust(toNormalizedSliderPosition: originalKeys > 0.5 ? 0 : 1)
    rows.adjust(toNormalizedSliderPosition: originalRows > 0.5 ? 0 : 1)
    let changedKeys = try position(keys), changedRows = try position(rows)

    reachSettingsLink("keyboardLayoutLink", in: app)
    settingsEntry("keyboardLayoutLink", in: app).tap()
    XCTAssertTrue(keys.waitForExistence(timeout: 5))
    XCTAssertEqual(try position(keys), changedKeys, accuracy: 0.01)
    XCTAssertEqual(try position(rows), changedRows, accuracy: 0.01)
    keys.adjust(toNormalizedSliderPosition: originalKeys)
    rows.adjust(toNormalizedSliderPosition: originalRows)
    restore.done = true
  }

  /// 键盘页的「中文键盘」在全拼下是 26 键、14 键、9 键三选一：选 14 键即启用并切换，离开这一页再回来仍是 14 键，再切到 9 键、回到 26 键，每一步都存下来。
  @MainActor
  func testChineseKeyboardOffersTwentySixFourteenAndNineKeys() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    func openPicker() -> XCUIElement {
      reachSettingsLink("keyboardLayoutLink", in: app)
      settingsEntry("keyboardLayoutLink", in: app).tap()
      let picker = app.buttons["chineseKeyboardPicker"]
      XCTAssertTrue(picker.waitForExistence(timeout: 5), "the 中文键盘 row is missing: \(visible(app))")
      revealBelowKeyboardPreview(picker, in: app)
      return picker
    }
    var picker = openPicker()
    let original = picker.value as? String ?? ""
    guard ["26 键", "14 键", "9 键"].contains(original) else {
      throw XCTSkip("the selected scheme is not a pinyin layout, so the row offers no 14 keys: \(original)")
    }
    let identifiers = ["26 键": "chineseKeyboard26Key", "14 键": "chineseKeyboard14Key", "9 键": "chineseKeyboard9Key"]
    for title in ["14 键", "9 键", "26 键"] {
      picker.tap()
      for id in identifiers.values {
        XCTAssertTrue(app.buttons[id].waitForExistence(timeout: 5), "\(id) is not offered: \(visible(app))")
      }
      app.buttons[try XCTUnwrap(identifiers[title])].tap()
      XCTAssertTrue(wait(picker, until: "value == '\(title)'"), "the row still reads \(picker.value ?? "")")
      // 离开这一页再回来，读到的是存下的布局，而不是这一页上留着的值。
      picker = openPicker()
      XCTAssertEqual(picker.value as? String, title)
    }
    if original != "26 键" {
      picker.tap()
      app.buttons[try XCTUnwrap(identifiers[original])].tap()
    }
  }

  /// `testKeyboardSpacingSettingsPersist` 是否已经把间距还原，与它的 teardown block 共享，测试通过时就不必再重新启动 app。
  private final class SpacingRestore: @unchecked Sendable {
    var done = false
  }

  /// Scroll the 键盘 page's form until `element` is on screen. The keyboard preview above the form reads a vertical drag as a row-spacing change, so the drag runs near the left edge of the form's lower half, clear of the preview and of the slider tracks, which start further in.
  @MainActor
  private func revealBelowKeyboardPreview(_ element: XCUIElement, in app: XCUIApplication) {
    for _ in 0..<6 {
      if element.exists && element.isHittable { return }
      let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.84))
      start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.6)))
    }
    XCTAssertTrue(element.exists && element.isHittable, "\(element) never scrolled into view: \(visible(app))")
  }

  @MainActor
  func testAISkinGenerationSavesAndPreparesCommunityPublication() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-aiSkinPreview"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    app.buttons["openAISkinDesigner"].tap()
    XCTAssertTrue(app.navigationBars["AI 皮肤抽卡"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.textViews["aiSkinPrompt"].exists)
    XCTAssertTrue(app.buttons["generateAISkins"].isEnabled)
    let deck = XCTAttachment(screenshot: app.screenshot())
    deck.name = "AI 皮肤抽卡入口"; deck.lifetime = .deleteOnSuccess; add(deck)
    app.buttons["generateAISkins"].tap()
    let save = app.buttons["saveAISkin_AI 测试 1"]
    XCTAssertTrue(save.waitForExistence(timeout: 5))
    save.tap()
    XCTAssertEqual(save.label, "已保存")
    app.buttons["publishAISkin_AI 测试 1"].tap()
    XCTAssertTrue(app.navigationBars["发布皮肤"].waitForExistence(timeout: 5))
    XCTAssertTrue((app.textFields["皮肤名称（最多 32 字）"].value as? String)?.hasPrefix("AI 测试 1") == true)
    // 发布按钮位于设计预览下方惰性表单的末尾，要滚动到那里才会存在。
    let publish = app.buttons["confirmCommunitySkinPublication"]
    for _ in 0..<6 where !publish.exists { scrollList(up: true, in: app) }
    XCTAssertFalse(publish.isEnabled, "Publication requires explicit consent")
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = "AI 生成皮肤的发布预览"; shot.lifetime = .deleteOnSuccess; add(shot)
    app.buttons["取消"].tap()
  }

  @MainActor
  func testCommunityCategoriesPreviewAndPublisher() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-communityPreview"]
    app.launch()
    app.tabBars.buttons["社区"].tap()
    app.buttons["communityCategory-1"].tap()
    XCTAssertEqual(app.segmentedControls.count, 0)
    let first = app.buttons["communityResource-10000000-0000-4000-8000-000000000001"]
    XCTAssertTrue(first.waitForExistence(timeout: 5))
    let second = app.buttons["communityResource-10000000-0000-4000-8000-000000000002"]
    // 词库是叠在同一张卡片里的若干行，每行旁边有自己的 添加 胶囊按钮。
    XCTAssertLessThanOrEqual(first.frame.maxY, second.frame.minY + 1)
    XCTAssertTrue(app.buttons["communityResourceAdd-10000000-0000-4000-8000-000000000001"].exists)
    let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Community word packs"; shot.lifetime = .deleteOnSuccess; add(shot)
    first.tap()
    XCTAssertTrue(app.buttons["communityImportLocal"].waitForExistence(timeout: 5))
    app.navigationBars.buttons.firstMatch.tap()
    app.buttons["communityCategory-2"].tap()
    app.buttons["communityResource-10000000-0000-4000-8000-000000000003"].tap()
    XCTAssertTrue(app.buttons["添加到高情商回复键盘"].waitForExistence(timeout: 5))
    let detail = XCTAttachment(screenshot: app.screenshot()); detail.name = "Community reply preview"; detail.lifetime = .deleteOnSuccess; add(detail)
    app.navigationBars.buttons.firstMatch.tap()
    app.buttons["publishCommunityWork"].tap()
    XCTAssertTrue(app.navigationBars["发布回复模板"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textViews["communityPromptEditor"].exists)
    app.buttons["取消"].tap()
    app.buttons["communityCategory-1"].tap()
    app.buttons["publishCommunityWork"].tap()
    XCTAssertTrue(app.navigationBars["发布词库"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["添加到待发布词库"].exists)
    app.buttons["取消"].tap()
    app.buttons["communityCategory-0"].tap()
    app.buttons["publishCommunityWork"].tap()
    XCTAssertTrue(app.navigationBars["发布皮肤"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testSkinDownloadOpensTryoutAndRestores() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-communityPreview", "--keyboard-chat-ui-fixture"]
    app.launch()
    XCTAssertFalse(app.buttons["keyboardGuideLink"].exists)
    XCTAssertFalse(app.buttons["aboutSettingsLink"].exists)
    app.tabBars.buttons["社区"].tap()
    let card = app.buttons["communitySkinCard-20000000-0000-4000-8000-000000000001"]
    XCTAssertTrue(card.waitForExistence(timeout: 5)); card.tap()
    app.buttons["downloadCommunitySkin"].tap()
    XCTAssertTrue(app.navigationBars["试用键盘"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textFields["keyboardTryoutField"].exists)
    XCTAssertTrue(app.buttons["keepTrialSkin"].exists)
    let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Skin trial with undo"; shot.lifetime = .deleteOnSuccess; add(shot)
    app.buttons["restoreTrialSkin"].tap()
    XCTAssertTrue(app.navigationBars["皮肤详情"].waitForExistence(timeout: 5))
    app.tabBars.buttons["我的"].tap()
    for _ in 0..<5 {
      if app.buttons["aboutSettingsLink"].isHittable { break }
      app.swipeUp()
    }
    XCTAssertTrue(app.buttons["desktopDownloadLink"].exists)
    app.buttons["aboutSettingsLink"].tap()
    XCTAssertTrue(app.navigationBars["关于"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testKeyboardHomePrioritizesTryoutAndQuickAdjustments() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "--keyboard-chat-ui-fixture"]
    app.launch()
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].waitForExistence(timeout: 5))
    // 搜索胶囊和状态卡片（内含 试用键盘 和两个 去开启 链接）位于页面顶部，第一个设置分组在它们下方。
    XCTAssertTrue(app.descendants(matching: .any)["settingsSearchField"].exists)
    XCTAssertTrue(app.buttons["openKeyboardSettingsButton"].exists)
    XCTAssertTrue(app.buttons["openFullAccessSettingsButton"].exists)
    XCTAssertTrue(app.buttons["skinSettingsLink"].exists)
    XCTAssertFalse(app.buttons["keyboardSettingsLink"].exists)
    XCTAssertFalse(app.buttons["keyboardGuideLink"].exists)
    let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Keyboard home"; shot.lifetime = .deleteOnSuccess; add(shot)
    // 其余条目在页面更下方，所以查找前先逐个滚动到它们。
    for identifier in ["inputSettingsLink", "expressionSettingsLink", "dictionarySettingsLink", "keyboardLayoutLink"] {
      reachSettingsLink(identifier, in: app)
    }
    settingsEntry("expressionSettingsLink", in: app).tap()
    XCTAssertTrue(app.navigationBars["表达"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.switches["englishPunctuationToggle"].exists)
    openAISettings(app)
    XCTAssertTrue(app.navigationBars["AI 设置"].waitForExistence(timeout: 5))
    reachSettingsLink("keyboardTryoutLink", in: app)
    settingsEntry("keyboardTryoutLink", in: app).tap()
    XCTAssertTrue(app.navigationBars["试用键盘"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textFields["keyboardTryoutField"].exists)
    reachSettingsLink("inputSettingsLink", in: app)
    settingsEntry("inputSettingsLink", in: app).tap()
  }

  /// A launch that resets onboarding opens on the splash, which leaves by itself after 2.8 seconds; tapping it gets the test to the onboarding without spending that time.
  @MainActor
  private func skipSplash(in app: XCUIApplication) {
    let splash = app.buttons["splashView"]
    if splash.waitForExistence(timeout: 5) && splash.isHittable { splash.tap() }
  }

  @MainActor
  private func wait(_ element: XCUIElement, until predicate: String, timeout: TimeInterval = 5) -> Bool {
    let expectation = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: predicate), object: element)
    return XCTWaiter.wait(for: [expectation], timeout: timeout) == .completed
  }

  // Driving the switches back on would need the app to still be on a screen this test can reach,
  // which a failure part way through does not promise. A relaunch clears the stored set instead,
  // and an absent set means every scheme is visible.
  @MainActor
  private func restoreSchemeVisibility(in app: XCUIApplication) {
    app.terminate()
    app.launchArguments = ["--reset-input-schemes-for-ui-tests"]
    app.launch()
    app.terminate()
  }

  /// Put a settings entry under the finger before the caller taps it.
  ///
  /// The keyboard tab carries most of these entries directly, and a test that has walked into
  /// another tab or scrolled the page has to come back rather than tap whatever happens to hold
  /// that identifier now. The entries sit below the fold on the shorter devices, so the page is
  /// scrolled until one is hittable instead of assuming a fixed offset, in both directions -
  /// an entry can be scrolled off either edge.
  ///
  /// 语音输入 has its own row on the home page again, in the 键盘 / 语音输入 / 手写输入 group of the mobile design, so every entry is reached the same way.
  @MainActor
  private func reachSettingsLink(_ identifier: String, in app: XCUIApplication) {
    // Pop whatever the caller pushed before looking: these entries live on the tab's root, and a
    // loop that visits one settings page per turn is still standing on the previous one. Tapping
    // the selected tab does not pop a SwiftUI navigation stack, so the back button does it.
    for _ in 0..<4 {
      let back = app.navigationBars.buttons.element(boundBy: 0)
      guard back.exists, back.isHittable else { break }
      back.tap()
    }
    let settingsTab = app.tabBars.buttons["设置"]
    if settingsTab.exists && !settingsTab.isSelected { settingsTab.tap() }

    XCTAssertTrue(scrollIntoView({ settingsEntry(identifier, in: app) }, in: app),
                  "\(identifier) never became reachable: \(visible(app))")
  }

  /// 在当前页内滚动（不离开该页），直到条目不再被上下栏遮挡，然后点击它。皮肤 页以键盘缩略图网格开头，所以在手机上 AI 卡片和网格下方的链接都在首屏之外。
  @MainActor
  private func tapRevealed(_ identifier: String, in app: XCUIApplication) {
    XCTAssertTrue(scrollIntoView({ settingsEntry(identifier, in: app) }, in: app),
                  "\(identifier) never became reachable: \(visible(app))")
    settingsEntry(identifier, in: app).tap()
  }

  /// Scroll the current page, without leaving it, until the element is clear of the bars.
  @MainActor
  private func reveal(_ element: XCUIElement, in app: XCUIApplication) {
    XCTAssertTrue(scrollIntoView({ element }, in: app), "\(element) never became reachable: \(visible(app))")
  }

  /// Scroll by dragging a point that belongs to the list, not by swiping the screen.
  ///
  /// `app.swipeUp()` starts at the centre, and on 键盘设置 the centre is the keyboard preview, which reads a vertical drag as a row-spacing change: the form never moves and the setting the test is about to read gets edited on the way. Swiping the collection view has the same problem, because its frame includes the preview. So the drag starts low, below the preview and above the tab bar, where only the form is.
  @MainActor
  private func scrollList(up: Bool, in app: XCUIApplication) {
    let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: up ? 0.86 : 0.52))
    let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: up ? 0.52 : 0.86))
    start.press(forDuration: 0.05, thenDragTo: end)
  }

  /// Hittable is not enough: the collapsed glass navigation bar and the floating tab bar are translucent, so a row scrolled under either still reports hittable while a tap on it lands on the bar. Only a row clear of both counts.
  @MainActor
  private func clearOfBars(_ element: XCUIElement, in app: XCUIApplication) -> Bool {
    guard element.exists, element.isHittable else { return false }
    let bar = app.navigationBars.firstMatch
    if bar.exists, element.frame.minY < bar.frame.maxY { return false }
    let tabs = app.tabBars.firstMatch
    if tabs.exists, element.frame.maxY > tabs.frame.minY { return false }
    return true
  }

  /// Scroll toward the top first, then down, since an entry can be scrolled off either edge; `resolve` is asked again each turn because a lazy list only creates a row once it is near the screen.
  @MainActor
  private func scrollIntoView(_ resolve: () -> XCUIElement, in app: XCUIApplication) -> Bool {
    if clearOfBars(resolve(), in: app) { return true }
    for _ in 0..<4 {
      if clearOfBars(resolve(), in: app) { return true }
      scrollList(up: false, in: app)
    }
    for _ in 0..<10 {
      if clearOfBars(resolve(), in: app) { return true }
      scrollList(up: true, in: app)
    }
    return clearOfBars(resolve(), in: app)
  }

  /// The settings entry with this identifier, whichever element type it came through as. A Form row does not always come through as a button: the home page's cards do, the plain NavigationLink rows on 键盘设置 come through as cells.
  @MainActor
  private func settingsEntry(_ name: String, in app: XCUIApplication) -> XCUIElement {
    for candidate in [app.buttons[name], app.cells[name], app.otherElements[name],
                      app.staticTexts[name]] where candidate.exists {
      return candidate
    }
    return app.buttons[name]
  }

  /// Every identifier on screen, for a failure message that says where the test actually was
  /// rather than only what it wanted. Without it, "never became reachable" reads the same whether
  /// the entry moved, the page did not load, or the scroll never happened.
  @MainActor
  private func visible(_ app: XCUIApplication) -> String {
    app.debugDescription.split(separator: "\n")
      .filter { $0.contains("identifier: ") }
      .map { $0.trimmingCharacters(in: .whitespaces) }
      .joined(separator: " | ")
  }

  /// AI 设置 从 键盘 页的 更多 分组（AI 润色与回复）进入，位于键盘预览和各设置分组下方。
  @MainActor
  private func openAISettings(_ app: XCUIApplication) {
    reachSettingsLink("keyboardLayoutLink", in: app)
    settingsEntry("keyboardLayoutLink", in: app).tap()
    let entry = app.buttons["aiSettingsLink"]
    XCTAssertTrue(app.sliders["appKeySpacingSlider"].waitForExistence(timeout: 5))
    for _ in 0..<12 {
      if clearOfBars(entry, in: app) { break }
      let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.84))
      start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.5)))
    }
    XCTAssertTrue(clearOfBars(entry, in: app), "aiSettingsLink never scrolled into view: \(visible(app))")
    entry.tap()
  }

  @MainActor
  func testMainTabsKeepIndependentNavigation() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    XCTAssertTrue(app.tabBars.buttons["设置"].isSelected)
    app.buttons["inputSettingsLink"].tap()
    app.tabBars.buttons["社区"].tap()
    XCTAssertTrue(app.navigationBars["社区"].waitForExistence(timeout: 5))
    XCTAssertEqual(app.tabBars.buttons.count, 4)
    app.tabBars.buttons["统计"].tap()
    XCTAssertTrue(app.navigationBars["统计"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["statisticsTab-0"].exists)
    app.tabBars.buttons["我的"].tap()
    XCTAssertTrue(app.buttons["accountAppIcon"].waitForExistence(timeout: 5))
    app.tabBars.buttons["设置"].tap()
    XCTAssertTrue(app.navigationBars["输入"].exists)
    // 设置 正在显示时再次点击它会回到根页面，与 Android 的标签栏一致。
    app.tabBars.buttons["设置"].tap()
    XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["inputSettingsLink"].exists)
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "Independent bottom tabs"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
  }

  @MainActor
  func testSkinDiscoveryUsesCommunityTabAndReturnsToOrigin() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-communityPreview"]
    app.launch()
    app.tabBars.buttons["社区"].tap()
    app.buttons["communitySkinCard-20000000-0000-4000-8000-000000000001"].tap()
    XCTAssertTrue(app.navigationBars["皮肤详情"].waitForExistence(timeout: 5))
    app.tabBars.buttons["设置"].tap()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("skinCommunityLink", in: app)
    XCTAssertTrue(app.tabBars.buttons["社区"].isSelected)
    XCTAssertTrue(app.navigationBars["社区"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.navigationBars["皮肤详情"].exists)
    app.buttons["communityCategory-2"].tap()
    app.tabBars.buttons["设置"].tap()
    XCTAssertTrue(app.navigationBars["皮肤"].exists)
    tapRevealed("skinCommunityLink", in: app)
    XCTAssertTrue(app.buttons["communitySkinCard-20000000-0000-4000-8000-000000000001"].exists)
    app.tabBars.buttons["设置"].tap()
    app.navigationBars.buttons.firstMatch.tap()
    XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].exists)
    app.tabBars.buttons["我的"].tap()
    let replay = app.buttons["replayOnboardingLink"]
    for _ in 0..<6 { if replay.isHittable { break }; app.swipeUp() }
    replay.tap()
    XCTAssertTrue(app.buttons["skipOnboardingButton"].waitForExistence(timeout: 5))
    app.buttons["skipOnboardingButton"].tap()
    // Replaying onboarding returns to the tab it was started from.
    XCTAssertTrue(app.buttons["accountAppIcon"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.tabBars.buttons["我的"].isSelected)
  }

  @MainActor
  func testChatLoginIsFocusedAndCancelReturnsToTryout() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["keyboardTryoutLink"].tap()
    let login = app.buttons["登录使用 AI"]
    XCTAssertTrue(login.waitForExistence(timeout: 8))
    login.tap()
    // 登录弹窗没有导航栏：它自己的页头放着 登录水杉 和关闭按钮。
    XCTAssertTrue(app.descendants(matching: .any)["accountLoginSheet"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["登录水杉"].exists)
    XCTAssertFalse(app.buttons["accountLocalDesigns"].exists)
    XCTAssertFalse(app.buttons["aboutSettingsLink"].exists)
    app.buttons["accountLoginClose"].tap()
    XCTAssertTrue(app.navigationBars["试用键盘"].waitForExistence(timeout: 5))
    app.navigationBars.buttons.firstMatch.tap()
    XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].exists)
  }

  /// 用户点完「发送验证码」要切到邮箱 App 去看验证码。回来时登录面板必须还在，填过的邮箱也还在，否则又得重新获取一遍。
  @MainActor
  func testCodeLoginKeepsTargetAcrossBackgrounding() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["我的"].tap()
    let loginAlert = app.alerts["账号与登录"]
    if loginAlert.waitForExistence(timeout: 5) { loginAlert.buttons["好"].tap() }
    // 未登录时点资料卡打开登录面板，邮箱表单在面板里原地展开。
    let card = app.buttons["accountProfileCard"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    guard card.label == "未登录，点按登录" else { throw XCTSkip("模拟器上已有登录会话") }
    card.tap()
    let sheet = app.descendants(matching: .any)["accountLoginSheet"]
    XCTAssertTrue(sheet.waitForExistence(timeout: 5))
    let emailLogin = app.buttons["backendCodeLogin_email"]
    guard emailLogin.waitForExistence(timeout: 10) else { throw XCTSkip("账号服务没有提供邮箱登录") }
    emailLogin.tap()
    let target = app.textFields["backendCodeTarget"]
    XCTAssertTrue(target.waitForExistence(timeout: 5))
    target.tap(); target.typeText("tester@example.com")

    XCUIDevice.shared.press(.home)
    XCTAssertTrue(app.wait(for: .runningBackground, timeout: 5))
    app.activate()
    XCTAssertTrue(app.wait(for: .runningForeground, timeout: 5))

    XCTAssertTrue(sheet.waitForExistence(timeout: 5))
    XCTAssertEqual(app.textFields["backendCodeTarget"].value as? String, "tester@example.com")
  }

  /// 点「通过 Google 登录」要先向后端拿到 challenge，再由系统弹出 Google 的登录页。取消后回到登录面板，提示里不出现错误。
  @MainActor
  func testGoogleSignInOpensGoogleAndCancelReturnsToSheet() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["我的"].tap()
    let loginAlert = app.alerts["账号与登录"]
    if loginAlert.waitForExistence(timeout: 5) { loginAlert.buttons["好"].tap() }
    let card = app.buttons["accountProfileCard"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    guard card.label == "未登录，点按登录" else { throw XCTSkip("模拟器上已有登录会话") }
    card.tap()
    XCTAssertTrue(app.descendants(matching: .any)["accountLoginSheet"].waitForExistence(timeout: 5))
    let google = app.buttons["backendGoogleSignIn"]
    // 不装 pod 的构建没有 GoogleSignIn，账号服务也可能没开 Google 登录。
    guard google.waitForExistence(timeout: 10) else { throw XCTSkip("这个构建或账号服务不提供 Google 登录") }
    google.tap()

    // ASWebAuthenticationSession 先由系统询问是否允许打开 google.com。
    let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    let consent = springboard.alerts.firstMatch
    XCTAssertTrue(consent.waitForExistence(timeout: 15))
    let proceed = consent.buttons.matching(NSPredicate(format: "label IN %@", ["继续", "Continue"])).firstMatch
    XCTAssertTrue(proceed.exists)
    proceed.tap()
    let page = app.webViews.firstMatch
    XCTAssertTrue(page.waitForExistence(timeout: 20))
    XCTAssertTrue(page.staticTexts.firstMatch.waitForExistence(timeout: 30))
    // client ID 或回调 scheme 配错时，Google 在这一页显示「Error 400: invalid_request」之类的错误，而不是账号选择。
    let rejection = NSPredicate(format: "label CONTAINS[c] 'invalid_' OR label CONTAINS[c] 'Error 40' OR label CONTAINS '错误 40'")
    XCTAssertEqual(page.staticTexts.matching(rejection).count, 0)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Google sign-in page"
    screenshot.lifetime = .keepAlways
    add(screenshot)

    app.buttons.matching(NSPredicate(format: "label IN %@", ["取消", "Cancel"])).firstMatch.tap()
    XCTAssertTrue(google.waitForExistence(timeout: 10))
    XCTAssertFalse(app.staticTexts["accountLoginStatus"].exists)
  }

  @MainActor
  func testCancellingPublicationPreservesCommunitySearch() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-communityPreview"]
    app.launch()
    app.tabBars.buttons["社区"].tap()
    app.buttons["communityCategory-2"].tap()
    let search = app.textFields.firstMatch
    search.tap(); search.typeText("reply\n")
    app.buttons["publishCommunityWork"].tap()
    XCTAssertTrue(app.navigationBars["发布回复模板"].waitForExistence(timeout: 5))
    app.buttons["取消"].tap()
    XCTAssertTrue(app.navigationBars["社区"].waitForExistence(timeout: 5))
    XCTAssertEqual(search.value as? String, "reply")
    XCTAssertTrue(app.buttons["communityCategory-2"].isSelected)
  }

  @MainActor
  func testBrandedLaunchScreenResource() {
    let app = XCUIApplication()
    app.launchArguments = ["-launchScreenPreview"]
    app.launch()
    XCTAssertTrue(app.staticTexts["水杉输入法"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["让输入更自然"].exists)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "System launch storyboard"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
  }

  @MainActor
  func testWelcomePrimaryButtonsRespondAcrossTheirVisibleBackground() {
    let app = XCUIApplication()
    app.launchArguments = ["--reset-onboarding-for-ui-tests"]
    app.launch()
    skipSplash(in: app)
    let next = app.buttons["nextOnboardingButton"]
    XCTAssertTrue(next.waitForExistence(timeout: 5))
    // The first step is adding the keyboard, so its settings button is there from the start.
    let settings = app.buttons["openKeyboardSettingsButton"]
    XCTAssertTrue(settings.waitForExistence(timeout: 5))
    settings.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
    let systemSettings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")
    XCTAssertTrue(systemSettings.wait(for: .runningForeground, timeout: 5))
    app.activate()
    // Tap the colored background, well outside the centered text.
    next.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.5)).tap()
    XCTAssertTrue(app.buttons["welcomeScheme_nineKey"].waitForExistence(timeout: 5))
    next.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
    XCTAssertTrue(app.buttons["welcomeTryoutLink"].waitForExistence(timeout: 5))
    next.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85)).tap()
    let finish = app.buttons["finishOnboardingButton"]
    XCTAssertTrue(finish.waitForExistence(timeout: 5))
    finish.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.5)).tap()
    XCTAssertTrue(app.tabBars.buttons["设置"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testReplayedWelcomeStartRespondsOutsideText() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["我的"].tap()
    let loginAlert = app.alerts["账号与登录"]
    if loginAlert.waitForExistence(timeout: 5) { loginAlert.buttons["好"].tap() }
    let replay = app.buttons["replayOnboardingLink"]
    for _ in 0..<8 {
      if replay.isHittable { break }
      app.swipeUp()
    }
    replay.tap()
    let next = app.buttons["nextOnboardingButton"]
    XCTAssertTrue(next.waitForExistence(timeout: 5))
    XCTAssertEqual(app.staticTexts["onboardingProgress"].label, "1 / 4")
    next.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
    XCTAssertTrue(app.buttons["welcomeScheme_nineKey"].waitForExistence(timeout: 5))
    app.buttons["skipOnboardingButton"].tap()
    XCTAssertTrue(app.tabBars.buttons["我的"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testWelcomeFlowSelectsSchemeAndCompletesOnce() {
    let app = XCUIApplication()
    app.launchArguments = ["--reset-onboarding-for-ui-tests"]
    app.launch()
    // The splash plays before the first onboarding and moves on by itself.
    XCTAssertTrue(app.buttons["splashView"].waitForExistence(timeout: 5))
    let title = app.staticTexts["onboardingTitle"]
    XCTAssertTrue(title.waitForExistence(timeout: 10))
    XCTAssertEqual(title.label, "把水杉加进键盘")
    XCTAssertEqual(app.staticTexts["onboardingProgress"].label, "1 / 4")
    let welcome = XCTAttachment(screenshot: app.screenshot())
    welcome.name = "Welcome onboarding"
    welcome.lifetime = .deleteOnSuccess
    add(welcome)
    XCTAssertTrue(app.buttons["openKeyboardSettingsButton"].exists)
    app.buttons["nextOnboardingButton"].coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    app.buttons["welcomeScheme_nineKey"].tap()
    XCTAssertEqual(app.buttons["welcomeScheme_nineKey"].value as? String, "已选择")
    app.buttons["nextOnboardingButton"].coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    XCTAssertTrue(app.buttons["welcomeTryoutLink"].waitForExistence(timeout: 5))
    app.buttons["nextOnboardingButton"].coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.5)).tap()
    XCTAssertEqual(app.staticTexts["onboardingProgress"].label, "4 / 4")
    app.buttons["finishOnboardingButton"].coordinate(withNormalizedOffset: CGVector(dx: 0.1, dy: 0.5)).tap()
    XCTAssertTrue(app.tabBars.buttons["设置"].waitForExistence(timeout: 5))
    app.terminate()
    app.launchArguments = []
    app.launch()
    XCTAssertTrue(app.tabBars.buttons["设置"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["nextOnboardingButton"].exists)
    app.buttons["inputSettingsLink"].tap()
    XCTAssertTrue(openSchemeSheet(.mandarin, in: app))
    XCTAssertTrue(app.buttons["inputScheme_nineKey"].isSelected)
    app.buttons["designOptionSheetCancel"].tap()
  }

  /// 输入 页 语言与方案 卡片里的各语言，以其行标识符的后缀表示。
  private enum InputLanguageRow: String {
    case mandarin, japanese
  }

  /// 在 输入 页打开某个语言的选项弹窗，该语言的每个方案都是一个 `inputScheme_<raw>` 选项；双拼 和 五笔 会再打开第二层弹窗。
  @MainActor
  private func openSchemeSheet(_ language: InputLanguageRow, in app: XCUIApplication) -> Bool {
    let row = app.buttons["inputLanguage_\(language.rawValue)"]
    guard row.waitForExistence(timeout: 5) else { return false }
    row.tap()
    return app.buttons["designOptionSheetCancel"].waitForExistence(timeout: 5)
  }

  @MainActor
  func testReplyKeyboardPastesChoosesStyleAndInserts() {
    let app = XCUIApplication()
    app.launchArguments = ["-keyboardReplyPreview"]
    app.launch()
    XCTAssertTrue(app.buttons["replyPaste"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["replyStyle_8"].isHittable)
    let grid = XCTAttachment(screenshot: app.screenshot())
    grid.name = "Reply keyboard style grid"
    grid.lifetime = .deleteOnSuccess
    add(grid)
    app.buttons["replyPaste"].tap()
    XCTAssertTrue(app.buttons["replySource"].label.contains("你睡了吗"))
    app.buttons["replyStyle_7"].tap()
    let candidate = app.buttons["replyCandidate"]
    XCTAssertTrue(candidate.waitForExistence(timeout: 5))
    XCTAssertTrue(candidate.label.contains("还没呢"))
    candidate.tap()
    XCTAssertTrue(app.buttons["replyStyle_0"].exists)
    XCTAssertTrue(app.staticTexts["replyStatus"].label.contains("已插入"))
    app.segmentedControls["replyMode"].buttons["帮润色"].tap()
    app.buttons["replyClear"].tap()
    XCTAssertTrue(app.buttons["replySource"].label.contains("粘贴"))
  }

  @MainActor
  func testKeyboardAICompactLargeTextKeepsControlsReachable() {
    let app = XCUIApplication()
    app.launchArguments = ["-keyboardAIPreview", "-keyboardCompactPreview", "-keyboardLargeType", "-keyboardLongPreview"]
    app.launch()
    let send = app.buttons["keyboardAISend"]
    let close = app.buttons["keyboardServiceClose"]
    XCTAssertTrue(send.waitForExistence(timeout: 5))
    XCTAssertTrue(send.isHittable)
    XCTAssertTrue(close.isHittable)
    XCTAssertGreaterThanOrEqual(send.frame.height, 44)
    XCTAssertGreaterThanOrEqual(close.frame.height, 44)
    XCTAssertLessThanOrEqual(send.frame.maxY - close.frame.minY, 216.5)
    let scroll = app.scrollViews["keyboardAIScroll"]
    XCTAssertGreaterThan(scroll.frame.height, 60)
    // Error notifications must scroll back into view, including the same error on retry.
    for _ in 0..<2 {
      scroll.swipeUp()
      send.tap()
      let error = app.staticTexts["keyboardAIStatus"]
      XCTAssertTrue(error.waitForExistence(timeout: 5))
      XCTAssertGreaterThanOrEqual(error.frame.minY, scroll.frame.minY - 1)
      XCTAssertLessThan(error.frame.minY, send.frame.minY)
    }
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Keyboard AI compact accessibility text"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
  }

  @MainActor
  func testVoiceResultIsExplicitlyTransferredAndClaimedOnce() {
    let app = XCUIApplication()
    let isolation = ["-voiceHandoffTestID", UUID().uuidString]
    app.launchArguments = isolation + ["-hasCompletedOnboarding", "YES", "-voiceResultFixture"]
    app.launch()
    reachSettingsLink("voiceSettingsLink", in: app)
    settingsEntry("voiceSettingsLink", in: app).tap()
    // 语音输入 是简短的设计页；录音和 发送到键盘 仍留在 识别服务 下的服务页上。
    XCTAssertTrue(app.navigationBars["语音输入"].waitForExistence(timeout: 5))
    app.buttons["voiceServiceLink"].tap()
    XCTAssertTrue(app.navigationBars["语音设置"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.staticTexts["等待键盘插入"].exists)
    for _ in 0..<8 {
      if app.buttons["sendVoiceToKeyboard"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["sendVoiceToKeyboard"].tap()
    XCTAssertTrue(app.staticTexts["等待键盘插入"].waitForExistence(timeout: 5))
    app.terminate()
    app.launchArguments = isolation + ["-keyboardVoicePreview"]
    app.launch()
    XCTAssertTrue(app.staticTexts["语音交接测试。"].waitForExistence(timeout: 5))
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Keyboard voice result preview"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    XCTAssertGreaterThanOrEqual(app.buttons["keyboardVoiceInsert"].frame.height, 44)
    app.buttons["keyboardVoiceInsert"].tap()
    XCTAssertTrue(app.staticTexts["插入验证：语音交接测试。"].waitForExistence(timeout: 5))
    app.terminate()
    app.launch()
    XCTAssertTrue(app.staticTexts["请在水杉 App 的“语音设置”中录音识别，点击“发送到键盘”，再返回这里插入。"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["keyboardVoiceInsert"].exists)
  }

  @MainActor
  func testKeyboardAIShowsSelectedTextAndRejectsStaleInput() {
    let app = XCUIApplication()
    app.launchArguments = ["-keyboardAIPreview"]
    app.launch()
    XCTAssertTrue(app.staticTexts["这是一段待润色的测试文字。只有点击发送才会请求服务。"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.staticTexts["输入位置或 AI 配置已变化，请关闭后重试。"].exists)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Keyboard AI narrow panel"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    XCTAssertGreaterThanOrEqual(app.buttons["keyboardAISend"].frame.height, 44)
    app.buttons["keyboardAISend"].tap()
    XCTAssertTrue(app.staticTexts["输入位置或 AI 配置已变化，请关闭后重试。"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["keyboardAIInsert"].exists)
  }

  @MainActor
  func testKeyboardAIOptInCanBeSavedAndRevoked() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-service.ai.provider", "custom",
      "-service.ai.endpoint", "https://keyboard-ai-fixture.invalid/v1/chat/completions",
      "-service.ai.model", "fixture"]
    func openAI() {
      openAISettings(app)
      for _ in 0..<6 {
        if app.switches["keyboardAIEnabled"].isHittable { break }
        app.swipeUp()
      }
    }
    app.launch()
    openAI()
    let toggle = app.switches["keyboardAIEnabled"]
    if toggle.value as? String == "1" { toggle.switches.firstMatch.tap() }
    toggle.switches.firstMatch.tap()
    for _ in 0..<6 {
      if app.buttons["saveServiceConfiguration"].isHittable { break }
      app.swipeDown()
    }
    app.buttons["saveServiceConfiguration"].tap()
    app.terminate()
    app.launch()
    openAI()
    XCTAssertEqual(toggle.value as? String, "1")
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Keyboard AI settings enabled"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    toggle.switches.firstMatch.tap()
    app.terminate()
    app.launch()
    openAI()
    XCTAssertEqual(toggle.value as? String, "0")
  }

  @MainActor
  func testPersonalDictionaryValidatesBeforeQueueing() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-personalDictionaryTestID", UUID().uuidString]
    app.launch()
    reachSettingsLink("dictionarySettingsLink", in: app)
    app.buttons["dictionarySettingsLink"].tap()
    app.buttons["personalDictionaryLink"].tap()
    app.buttons["importPersonalDictionary"].tap()
    XCTAssertTrue(app.buttons["choosePersonalDictionaryFile"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["confirmPersonalDictionaryImport"].isEnabled)
    let importScreen = XCTAttachment(screenshot: app.screenshot())
    importScreen.name = "Personal dictionary import"
    importScreen.lifetime = .deleteOnSuccess
    add(importScreen)
    app.buttons["取消"].tap()
    app.buttons["addPersonalWord"].tap()
    app.textFields["personalWordValue"].tap()
    app.textFields["personalWordValue"].typeText("你好")
    app.textFields["personalWordCode"].tap()
    app.textFields["personalWordCode"].typeText("nihao")
    app.buttons["savePersonalWord"].tap()
    XCTAssertTrue(app.staticTexts["请填写完整拼音，用空格或英文单引号分隔音节，例如 ni hao。"].waitForExistence(timeout: 5))
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Personal word editor validation"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    app.textFields["personalWordCode"].typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 5) + "ni hao")
    app.buttons["savePersonalWord"].tap()
    XCTAssertTrue(app.staticTexts["1 项等待键盘同步"].waitForExistence(timeout: 5))
    app.terminate()
    app.launch()
    reachSettingsLink("dictionarySettingsLink", in: app)
    app.buttons["dictionarySettingsLink"].tap()
    app.buttons["personalDictionaryLink"].tap()
    XCTAssertTrue(app.staticTexts["1 项等待键盘同步"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.staticTexts["等待同步 · 保存"].exists)
    let pending = XCTAttachment(screenshot: app.screenshot())
    pending.name = "Personal dictionary pending confirmation"
    pending.lifetime = .deleteOnSuccess
    add(pending)
  }

  @MainActor
  func testCancelledSkinGenerationDoesNotShowLateResult() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-aiSkinPreview", "-skinGenerationSlowFixture"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    app.buttons["openAISkinDesigner"].tap()
    XCTAssertTrue(app.buttons["generateAISkins"].waitForExistence(timeout: 5))
    app.buttons["generateAISkins"].tap()
    XCTAssertTrue(app.buttons["取消"].waitForExistence(timeout: 3))
    app.buttons["取消"].tap()
    let late = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == true"), object: app.buttons["saveAISkin_AI 测试 1"])
    late.isInverted = true
    XCTAssertEqual(XCTWaiter.wait(for: [late], timeout: 6), .completed)
    XCTAssertTrue(app.buttons["generateAISkins"].isEnabled)
  }

  @MainActor
  func testCuratedSkinCollectionShowsFullPreviewsAndUndo() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    app.buttons["skinEditorTemplates"].tap()
    let gallery = app.collectionViews.firstMatch.exists ? app.collectionViews.firstMatch : app.tables.firstMatch
    // Three of the nine curated designs. Walking all of them cost four minutes and asserted the
    // same three things each time; what every design contains is checked in KeyboardSkinTests,
    // which reads them directly instead of driving a Simulator. The first is above the fold and the
    // last two need scrolling, so the gallery is still exercised in both states.
    for (index, name) in ["苔庭晨雾", "冰川薄荷", "奶咖手账"].enumerated() {
      let template = app.buttons["skinTemplate_" + name]
      for _ in 0..<6 { if template.isHittable { break }; gallery.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.8)).press(forDuration: 0.05, thenDragTo: gallery.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25))) }
      XCTAssertTrue(template.isHittable, name)
      template.tap()
      app.segmentedControls["skinEditorPreviewLayout"].buttons[index % 2 == 0 ? "26 键" : "9 键"].tap()
      XCTAssertTrue(app.buttons["undoSkinDesign"].isEnabled)
    }
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = "Curated skin gallery"; shot.lifetime = .deleteOnSuccess; add(shot)
    for _ in 0..<8 {
      if !app.buttons["undoSkinDesign"].isEnabled { break }
      app.buttons["undoSkinDesign"].tap()
    }
    XCTAssertFalse(app.buttons["undoSkinDesign"].isEnabled)
  }

  @MainActor
  func testCustomSkinDesignPersistsAndApplies() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    func openEditor() {
      app.buttons["skinSettingsLink"].tap()
      tapRevealed("customSkinEditorLink", in: app)
      app.buttons["skinEditorTab_按键"].tap()
    }
    app.launch()
    openEditor()
    let radius = app.sliders["customSkinCornerRadius"]
    func revealRadius() {
      let controls = app.descendants(matching: .any)["skinEditorControls"].firstMatch
      for _ in 0..<12 {
        let top = app.buttons["skinEditorTab_按键"].frame.maxY + 28
        let bottom = app.buttons["applyCustomSkin"].frame.minY - 28
        if radius.exists && radius.isHittable && radius.frame.minY > top && radius.frame.maxY < bottom { return }
        let down = radius.exists && radius.frame.midY < top
        controls.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
          .press(forDuration: 0.05, thenDragTo: controls.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: down ? 0.7 : 0.3)))
      }
    }
    revealRadius()
    XCTAssertTrue(radius.isHittable)
    // XCTest's normalized drag is approximate, and Slider.value can be a
    // percentage on one runtime and a domain value on another. Verify the
    // actual displayed setting changes, then survives a process restart.
    let radiusLabel = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "圆角 · ")).firstMatch
    XCTAssertTrue(radiusLabel.exists)
    let before = radiusLabel.label
    let initial = Int(before.replacingOccurrences(of: "圆角 · ", with: "")) ?? 0
    radius.adjust(toNormalizedSliderPosition: initial < 10 ? 0.9 : 0.1)
    let changed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label != %@", before), object: radiusLabel)
    XCTAssertEqual(XCTWaiter.wait(for: [changed], timeout: 5), .completed)
    let edited = radiusLabel.label
    let editedRadius = Int(edited.replacingOccurrences(of: "圆角 · ", with: ""))
    XCTAssertNotNil(editedRadius)
    XCTAssertTrue((0...20).contains(editedRadius ?? -1))
    app.terminate()
    app.launch()
    openEditor()
    revealRadius()
    XCTAssertEqual(radiusLabel.label, edited, "The edited corner radius must survive restarting the app")
    for _ in 0..<5 {
      if app.buttons["applyCustomSkin"].isHittable { break }
      app.descendants(matching: .any)["skinEditorControls"].firstMatch.swipeDown()
    }
    app.buttons["applyCustomSkin"].tap()
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "Custom skin editor"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
    XCTAssertTrue(app.buttons["applyCustomSkin"].label.contains("正在使用"))
    // Reset lives in the editor tools menu, independent of the selected category.
    app.buttons["skinEditorTools"].tap()
    app.buttons["重置我的皮肤"].tap()
    app.buttons["重置"].tap()
    app.buttons["skinEditorTab_按键"].tap()
    revealRadius()
    XCTAssertEqual(radiusLabel.label, "圆角 · 8")
  }

  @MainActor
  func testCustomSkinTemplatesLibraryAndUndo() {
    let app = XCUIApplication()
    // This case saves a skin and deletes it at the end, so a run that fails in between leaves one
    // behind. The library holds twelve and lives in the app group, which uninstalling the app does
    // not clear, so without this the suite eventually wedges itself.
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "--reset-custom-skins-for-ui-tests"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    app.buttons["skinEditorTemplates"].tap()
    app.buttons["skinTemplate_紫夜星光"].tap()
    XCTAssertTrue(app.buttons["undoSkinDesign"].isEnabled)
    app.buttons["saveCustomSkin"].tap()
    let name = "夜色测试-" + String(UUID().uuidString.prefix(4))
    let field = app.textFields["customSkinName"]
    field.tap()
    if let current = field.value as? String { field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: current.count)) }
    field.typeText(name)
    app.buttons["confirmSaveCustomSkin"].tap()
    XCTAssertTrue(app.buttons["savedSkin_" + name].waitForExistence(timeout: 5))
    app.buttons["skinEditorTools"].tap(); app.buttons["设计模板"].tap()
    app.buttons["skinTemplate_水杉留白"].tap()
    app.buttons["undoSkinDesign"].tap()
    // The editor's own gradient state was asserted here by scrolling the control into view a second
    // time. What a design renders is covered by KeyboardSkinTests without a Simulator; the reload
    // below still reads the switch, which is the part this case is about.
    app.terminate()
    // The relaunch is what checks the skin survived it, so it must not carry the reset argument --
    // launchArguments persist across launch() on the same XCUIApplication.
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    app.buttons["skinEditorTools"].tap(); app.buttons["我的皮肤"].tap()
    XCTAssertTrue(app.buttons["savedSkin_" + name].exists)
    app.buttons["skinEditorTools"].tap(); app.buttons["设计模板"].tap()
    app.buttons["skinTemplate_水杉留白"].tap()
    app.buttons["skinEditorTools"].tap(); app.buttons["我的皮肤"].tap()
    app.buttons["管理" + name].tap()
    app.buttons["用当前设计更新"].tap()
    app.buttons["更新已保存的皮肤"].tap()
    app.buttons["skinEditorTools"].tap(); app.buttons["设计模板"].tap()
    app.buttons["skinTemplate_紫夜星光"].tap()
    app.buttons["skinEditorTools"].tap(); app.buttons["我的皮肤"].tap()
    app.buttons["savedSkin_" + name].tap()
    app.buttons["skinEditorTab_背景"].tap()
    for _ in 0..<12 {
      if app.switches["customSkinGradient"].exists && app.switches["customSkinGradient"].isHittable { break }
      let controls = app.collectionViews.firstMatch
      controls.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.75)).press(forDuration: 0.05, thenDragTo: controls.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.45)))
    }
    XCTAssertEqual(app.switches["customSkinGradient"].value as? String, "0")
    app.buttons["skinEditorTools"].tap(); app.buttons["我的皮肤"].tap()
    app.buttons["管理" + name].tap()
    // 删除 appears twice: once in the sheet 管理 opens and once to confirm. Tapping straight through
    // races the sheet's presentation, which is what made this the one case that failed at random.
    let remove = app.buttons["删除"]
    XCTAssertTrue(remove.waitForExistence(timeout: 5))
    remove.tap()
    let confirm = app.buttons["删除"]
    XCTAssertTrue(confirm.waitForExistence(timeout: 5))
    confirm.tap()
    XCTAssertFalse(app.buttons["savedSkin_" + name].waitForExistence(timeout: 2))
  }

  @MainActor
  func testSkinEditorKeepsFullPreviewBelowMaterialGrid() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    let preview = app.otherElements["fullKeyboardSkinPreview"]
    XCTAssertTrue(preview.waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["skinBackgroundPreset_0"].isHittable)
    XCTAssertTrue(app.buttons["saveCustomSkin"].isHittable)
    let frame = preview.frame
    XCTAssertEqual(frame.height / frame.width, 260.0 / 390.0, accuracy: 0.01)
    XCTAssertGreaterThan(frame.width, app.frame.width * 0.9)
    XCTAssertGreaterThan(frame.minY, app.buttons["skinEditorTab_背景"].frame.maxY)
    app.buttons["skinBackgroundPreset_5"].tap()
    XCTAssertEqual(preview.frame.minY, frame.minY, accuracy: 1)
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "Reference skin editor with pinned keyboard"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
    for tab in ["按键", "文本", "音效", "背景"] {
      app.buttons["skinEditorTab_" + tab].tap()
      XCTAssertEqual(preview.frame.minY, frame.minY, accuracy: 1)
      XCTAssertTrue(preview.isHittable)
    }
    app.segmentedControls["skinEditorPreviewLayout"].buttons["9 键"].tap()
    XCTAssertTrue(preview.label.contains("9 键"))
    XCTAssertEqual(preview.frame.height, frame.height, accuracy: 1)
  }

  @MainActor
  func testSkinEditorLandscapePreservesFullKeyboardAspectRatio() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    tapRevealed("customSkinEditorLink", in: app)
    XCUIDevice.shared.orientation = .landscapeLeft
    defer { XCUIDevice.shared.orientation = .portrait }
    let landscape = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
      let frame = app.windows.firstMatch.frame
      return frame.width > frame.height
    }, object: nil)
    XCTAssertEqual(XCTWaiter.wait(for: [landscape], timeout: 15), .completed)
    let preview = app.otherElements["fullKeyboardSkinPreview"]
    for title in ["26 键", "9 键"] {
      app.segmentedControls["skinEditorPreviewLayout"].buttons[title].tap()
      XCTAssertEqual(preview.frame.height / preview.frame.width, 260.0 / 390.0, accuracy: 0.01)
      XCTAssertLessThanOrEqual(preview.frame.maxY, app.frame.maxY)
      XCTAssertTrue(app.buttons["applyCustomSkin"].isHittable)
    }
    let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Landscape proportional preview"; shot.lifetime = .deleteOnSuccess; add(shot)
  }

  @MainActor
  func testHapticStrengthPreviewAndPersistence() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    // 按键反馈从 输入 移到了 键盘 页的 按键反馈 分组，位于键盘预览下方。
    func openFeedback() {
      reachSettingsLink("keyboardLayoutLink", in: app)
      settingsEntry("keyboardLayoutLink", in: app).tap()
      XCTAssertTrue(app.sliders["appKeySpacingSlider"].waitForExistence(timeout: 5))
      revealBelowKeyboardPreview(app.switches["keyboardHapticsToggle"], in: app)
    }
    app.launch()
    openFeedback()
    let toggle = app.switches["keyboardHapticsToggle"]
    let initiallyEnabled = toggle.value as? String == "1"
    if !initiallyEnabled { toggle.switches.firstMatch.tap() }
    // 振动强度 是选择行：它的值就是强度，点击会打开选项弹窗。
    let picker = app.buttons["keyboardHapticStrengthPicker"]
    let appeared = picker.waitForExistence(timeout: 5)
    if !appeared {
      let failure = XCTAttachment(screenshot: app.screenshot())
      failure.lifetime = .deleteOnSuccess
      add(failure)
    }
    XCTAssertTrue(appeared)
    revealBelowKeyboardPreview(picker, in: app)
    let previous = picker.value as? String ?? "中"
    picker.tap()
    XCTAssertTrue(app.buttons["designOptionSheetCancel"].waitForExistence(timeout: 5))
    app.buttons["强"].tap()
    XCTAssertTrue(wait(picker, until: "value == '强'"))
    revealBelowKeyboardPreview(app.buttons["previewKeyboardHaptics"], in: app)
    app.buttons["previewKeyboardHaptics"].tap()
    app.terminate()
    app.launch()
    openFeedback()
    revealBelowKeyboardPreview(picker, in: app)
    XCTAssertEqual(picker.value as? String, "强")
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "Haptic strength settings"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
    picker.tap()
    XCTAssertTrue(app.buttons["designOptionSheetCancel"].waitForExistence(timeout: 5))
    app.buttons[previous].tap()
    XCTAssertTrue(wait(picker, until: "value == '\(previous)'"))
    if !initiallyEnabled { toggle.switches.firstMatch.tap() }
  }

  override func setUpWithError() throws {
    continueAfterFailure = false
  }

  @MainActor
  private func selectProvider(_ name: String, picker: String, app: XCUIApplication) {
    for _ in 0..<4 {
      if app.buttons[picker].isHittable { break }
      app.swipeDown()
    }
    app.buttons[picker].tap()
    let search = app.searchFields.firstMatch
    XCTAssertTrue(search.waitForExistence(timeout: 5))
    search.tap()
    search.typeText(name)
    app.buttons[name].tap()
  }

  @MainActor
  func testStatisticsChartsAndDaySelection() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["统计"].tap()
    // 概览 / 习惯 / 按键 / 成就，每个分段都是设计稿圆角分段控件里的一个按钮。
    let overview = app.buttons["statisticsTab-0"]
    XCTAssertTrue(overview.waitForExistence(timeout: 5))
    XCTAssertTrue(overview.isSelected)
    for index in 1...3 {
      let tab = app.buttons["statisticsTab-\(index)"]
      tab.tap()
      XCTAssertTrue(wait(tab, until: "isSelected == true"))
    }
    let top = XCTAttachment(screenshot: app.screenshot())
    top.name = "统计成就"
    top.lifetime = .deleteOnSuccess
    add(top)
    // 按键分段让热力图在 26 键和九键键盘之间切换。
    app.buttons["statisticsTab-2"].tap()
    let nineKey = app.buttons["statisticsKeyLayout-1"]
    if nineKey.waitForExistence(timeout: 3) {
      nineKey.tap()
      XCTAssertTrue(wait(nineKey, until: "isSelected == true"))
    }
    // 逐日明细表移到了菜单里的 按日明细 后面。
    app.buttons["statisticsMenu"].tap()
    app.buttons["typingDailyDetailsMenu"].tap()
    XCTAssertTrue(app.navigationBars["按日明细"].waitForExistence(timeout: 5))
    let detail = XCTAttachment(screenshot: app.screenshot())
    detail.name = "统计按日明细"
    detail.lifetime = .deleteOnSuccess
    add(detail)
    app.navigationBars.buttons.element(boundBy: 0).tap()
    XCTAssertTrue(app.navigationBars["统计"].waitForExistence(timeout: 5))
  }

  /// 统计默认关闭，关闭时键盘不写入。页面只用「记录已关闭」说明原因，不能同时让人去开完全访问。
  @MainActor
  func testStatisticsOffShowsOnlyTheRecordingNotice() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["统计"].tap()
    XCTAssertTrue(app.buttons["statisticsTab-0"].waitForExistence(timeout: 5))
    let enable = app.buttons["typingStatisticsEnableNotice"]
    // 模拟器上可能有别的运行打开过记录：先从菜单关掉，结束时再恢复。
    let wasEnabled = !enable.waitForExistence(timeout: 5)
    if wasEnabled {
      app.buttons["statisticsMenu"].tap()
      app.buttons["typingStatisticsEnabled"].tap()
      XCTAssertTrue(enable.waitForExistence(timeout: 5))
    }
    XCTAssertFalse(app.staticTexts["统计没有数据"].exists)
    if wasEnabled {
      enable.tap()
      XCTAssertTrue(wait(enable, until: "exists == false"))
    }
  }

  /// 在 app 里开启记录就会写出统计文件，而没有完全访问的键盘不记录，页面停在计数为零。这时的提示要先让人确认「允许完全访问」。
  @MainActor
  func testEnablingRecordingWithZeroCountNamesFullAccess() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.tabBars.buttons["统计"].tap()
    XCTAssertTrue(app.buttons["statisticsTab-0"].waitForExistence(timeout: 5))
    let enable = app.buttons["typingStatisticsEnableNotice"]
    // 模拟器上可能有别的运行打开过记录：先从菜单关掉，结束时恢复成原来的状态。
    let wasEnabled = !enable.waitForExistence(timeout: 5)
    if wasEnabled {
      app.buttons["statisticsMenu"].tap()
      app.buttons["typingStatisticsEnabled"].tap()
      XCTAssertTrue(enable.waitForExistence(timeout: 5))
    }
    enable.tap()
    XCTAssertTrue(wait(enable, until: "exists == false"))
    defer {
      if !wasEnabled {
        app.buttons["statisticsMenu"].tap()
        app.buttons["typingStatisticsEnabled"].tap()
        XCTAssertTrue(enable.waitForExistence(timeout: 5))
      }
    }
    guard app.staticTexts["统计没有数据"].waitForExistence(timeout: 5) else {
      throw XCTSkip("这台模拟器上已有统计计数，到不了计数为零的提示。")
    }
    let fullAccess = app.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", "允许完全访问")).firstMatch
    XCTAssertTrue(fullAccess.exists)
    XCTAssertTrue(app.buttons["前往系统设置"].exists)
  }

  @MainActor
  func testFetchingModelsRequiresKeyButNotModel() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-service.ai.provider", "custom",
      "-service.ai.endpoint", "https://catalog-no-key.invalid/v1/chat/completions", "-service.ai.model", ""]
    app.launch()
    openAISettings(app)
    let fetch = app.buttons["fetchServiceModels"]
    XCTAssertTrue(fetch.waitForExistence(timeout: 5))
    fetch.tap()
    XCTAssertTrue(app.staticTexts["请先填写 API Key，或使用已保存的密钥。"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textFields["serviceModel"].exists)
  }

  /// 表达 → 常用语 写入的正是键盘 常用语 面板所列的存储：在这里添加的短语会显示在页面上，滑动删除后又会被移除，所以测试结束时存储与开始时一致。
  @MainActor
  func testCommonPhrasesCanBeAddedAndRemoved() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    reachSettingsLink("expressionSettingsLink", in: app)
    settingsEntry("expressionSettingsLink", in: app).tap()
    XCTAssertTrue(app.navigationBars["表达"].waitForExistence(timeout: 5))
    tapRevealed("commonPhrasesSettingsLink", in: app)
    XCTAssertTrue(app.navigationBars["常用语"].waitForExistence(timeout: 5))
    // 数字不受自动大写和自动纠错影响，所以无论输入框把单词改成什么样，都能凭数字找到这一行。
    let mark = String(Int.random(in: 10_000_000...99_999_999))
    let phrase = "phrase \(mark)"
    let add = app.buttons["addCommonPhrase"]
    XCTAssertTrue(add.waitForExistence(timeout: 5))
    add.tap()
    // 纵向 TextField 在某些系统版本上以 text view 进入无障碍树，在另一些版本上则是 text field。
    let field = app.descendants(matching: .any).matching(identifier: "commonPhraseText").firstMatch
    XCTAssertTrue(field.waitForExistence(timeout: 5))
    field.tap()
    field.typeText(phrase)
    app.buttons["saveCommonPhrase"].tap()
    let row = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", mark)).firstMatch
    XCTAssertTrue(row.waitForExistence(timeout: 5), visible(app))
    row.swipeLeft()
    app.buttons["删除"].firstMatch.tap()
    let confirm = app.alerts.buttons["删除"]
    XCTAssertTrue(confirm.waitForExistence(timeout: 5))
    confirm.tap()
    XCTAssertTrue(row.waitForNonExistence(timeout: 5))
  }

  @MainActor
  func testSkinPageShowsKeyboardTilesAndItsSubpages() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    XCTAssertTrue(app.navigationBars["皮肤"].waitForExistence(timeout: 5))
    // 每个目录主题都是一张键盘缩略图卡片；完整的 26 / 9 键预览在皮肤编辑器里（`testSkinEditorKeepsFullPreviewBelowMaterialGrid`）。
    XCTAssertTrue(app.buttons["skin_system"].waitForExistence(timeout: 5))
    // 本页自己没有 26 / 9 键预览切换，无论用什么标识符：完整预览在皮肤编辑器里。
    XCTAssertFalse(app.descendants(matching: .any).matching(NSPredicate(format: "label == %@ OR label == %@", "26 键", "9 键")).firstMatch.exists)
    let light = XCTAttachment(screenshot: app.screenshot())
    light.name = "皮肤网格"
    light.lifetime = .deleteOnSuccess
    add(light)
    // 网格以 AI 卡片结尾，点开是一句话设计器。
    tapRevealed("aiSkinDesignTile", in: app)
    XCTAssertTrue(app.navigationBars["AI 设计皮肤"].waitForExistence(timeout: 5))
    app.navigationBars.buttons.element(boundBy: 0).tap()
    // 外观选择器移到了网格下方的独立页面。
    tapRevealed("skinAppearanceLink", in: app)
    XCTAssertTrue(app.navigationBars["明暗与候选颜色"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.descendants(matching: .any)["globalTheme"].exists)
    XCTAssertTrue(app.descendants(matching: .any)["keyboardTheme"].exists)
  }

  @MainActor
  func testAIProviderSelectionAutofillsAndClearsUnsavedKey() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-service.ai.provider", "custom",
                           "-service.ai.endpoint", "", "-service.ai.model", ""]
    app.launch()
    openAISettings(app)
    let token = app.secureTextFields["serviceToken"]
    token.tap()
    token.typeText("provider-switch-fixture")
    app.buttons["serviceDismissKeyboard"].tap()
    selectProvider("DeepSeek", picker: "aiProviderPicker", app: app)
    XCTAssertEqual(app.textFields["serviceEndpoint"].value as? String, "https://api.deepseek.com/chat/completions")
    XCTAssertEqual(app.buttons["serviceModelPicker"].value as? String, "deepseek-v4-flash")
    XCTAssertFalse(app.textFields["serviceModel"].exists)
    app.buttons["serviceModelPicker"].tap()
    app.buttons["deepseek-v4-pro"].tap()
    XCTAssertEqual(app.buttons["serviceModelPicker"].value as? String, "deepseek-v4-pro")
    app.buttons["serviceModelPicker"].tap()
    app.buttons["自定义模型…"].tap()
    XCTAssertTrue(app.textFields["serviceModel"].exists)
    app.buttons["serviceModelPicker"].tap()
    app.buttons["deepseek-v4-flash"].tap()
    XCTAssertFalse(app.textFields["serviceModel"].exists)

    XCTAssertEqual(token.value as? String, token.placeholderValue)
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "AI 服务商预设"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
    selectProvider("Google · Gemini", picker: "aiProviderPicker", app: app)
    XCTAssertEqual(app.textFields["serviceEndpoint"].value as? String,
                   "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions")
    selectProvider("EveryAPI", picker: "aiProviderPicker", app: app)
    XCTAssertEqual(app.textFields["serviceEndpoint"].value as? String, "https://api.everyapi.ai/v1/chat/completions")
    XCTAssertEqual(app.buttons["serviceModelPicker"].value as? String, "deepseek-v4-flash")
    XCTAssertTrue(app.images["everyAPIProviderLogo"].exists)
    selectProvider("自定义", picker: "aiProviderPicker", app: app)
    XCTAssertTrue(app.textFields["serviceEndpoint"].isEnabled)
    XCTAssertEqual(app.textFields["serviceEndpoint"].value as? String, app.textFields["serviceEndpoint"].placeholderValue)
  }

  @MainActor
  func testKeyboardChatSendsAndDisplaysReply() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "--keyboard-chat-ui-fixture"]
    app.launch()
    for _ in 0..<6 {
      if app.buttons["keyboardTryoutLink"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["keyboardTryoutLink"].tap()
    XCTAssertTrue(app.buttons["keyboardChatModelPicker"].waitForExistence(timeout: 5))
    let input = app.textFields["keyboardTryoutField"]
    input.tap(); input.typeText("hello")
    app.buttons["keyboardChatSend"].tap()
    XCTAssertTrue(app.staticTexts["已收到：hello"].waitForExistence(timeout: 5))
    XCTAssertEqual(input.value as? String, input.placeholderValue)
    app.buttons["dismissKeyboardButton"].tap()
    let image = XCTAttachment(screenshot: app.screenshot())
    image.name = "Keyboard chat with model selection"
    image.lifetime = .deleteOnSuccess
    add(image)
  }

  @MainActor
  func testFuzzyPreferencesAndInformationPages() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // 模糊音 是 输入 页上的一个开关，只有打开时才会出现 模糊音规则 行。
    let fuzzy = app.switches["fuzzyPinyinToggle"]
    func openRules() {
      app.buttons["inputSettingsLink"].tap()
      reveal(fuzzy, in: app)
      if fuzzy.value as? String == "0" { fuzzy.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap() }
      XCTAssertTrue(wait(fuzzy, until: "value == '1'"))
      tapRevealed("fuzzyPinyinSettingsLink", in: app)
    }
    openRules()
    XCTAssertTrue(app.staticTexts["fuzzyPinyinAvailability"].waitForExistence(timeout: 5))
    let enabled = app.switches["fuzzyPinyinEnabled"]
    if enabled.value as? String == "0" { enabled.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap() }
    let rule = app.switches["fuzzyPinyinRule_z-zh"]
    if rule.value as? String == "0" { rule.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap() }
    XCTAssertEqual(enabled.value as? String, "1")
    XCTAssertEqual(rule.value as? String, "1")
    app.terminate()
    app.launch()
    openRules()
    XCTAssertEqual(enabled.value as? String, "1")
    XCTAssertEqual(rule.value as? String, "1")
    rule.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    enabled.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    app.navigationBars.buttons.element(boundBy: 0).tap()
    app.navigationBars.buttons.element(boundBy: 0).tap()
    app.tabBars.buttons["我的"].tap()
    for _ in 0..<6 {
      if app.buttons["desktopDownloadLink"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["desktopDownloadLink"].tap()
    // 同一个页面列出所有平台，每个平台有自己的 获取 胶囊按钮，下方是可复制的下载链接。
    XCTAssertTrue(app.navigationBars["其他平台下载"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["copyDesktopDownloadLink"].exists)
    for platform in ["macOS", "Windows", "Linux"] {
      XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", platform)).firstMatch.exists, platform)
    }
    // SwiftUI 的 `Link` 以链接而不是按钮的形式出现，所以只查找按钮永远找不到它。
    reveal(app.descendants(matching: .any)["desktopReleaseLink"], in: app)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Desktop download guide"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    app.navigationBars.buttons.element(boundBy: 0).tap()
    // 关于 closes the page, below the rows the download entry sits among.
    for _ in 0..<6 {
      if app.buttons["aboutSettingsLink"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["aboutSettingsLink"].tap()
    XCTAssertTrue(app.staticTexts["aboutAppVersion"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.navigationBars["关于"].exists)
  }

  @MainActor
  func testHandwritingCanBeEnabledSelectedAndDisabled() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // This one ends with handwriting hidden on purpose, which is still a scheme later tests cannot
    // select until the stored set is cleared.
    defer { restoreSchemeVisibility(in: app) }
    // 在键盘中显示手写和选用手写，现在都在 手写输入 页上。
    reachSettingsLink("handwritingSettingsLink", in: app)
    settingsEntry("handwritingSettingsLink", in: app).tap()
    let enabled = app.switches["handwritingEnabledToggle"]
    reveal(enabled, in: app)
    if enabled.value as? String == "0" { enabled.switches.firstMatch.tap() }
    XCTAssertTrue(wait(enabled, until: "value == '1'"))
    let select = app.buttons["handwritingSelectButton"]
    select.tap()
    XCTAssertTrue(wait(select, until: "value == '已选择'"))
    app.terminate(); app.launch()
    // The 输入 row on the home page shows the current scheme as its value.
    reachSettingsLink("inputSettingsLink", in: app)
    XCTAssertEqual(settingsEntry("inputSettingsLink", in: app).value as? String, "手写")
    reachSettingsLink("handwritingSettingsLink", in: app)
    settingsEntry("handwritingSettingsLink", in: app).tap()
    reveal(enabled, in: app)
    enabled.switches.firstMatch.tap()
    XCTAssertTrue(wait(enabled, until: "value == '0'"))
    XCTAssertFalse(select.isEnabled)
  }

  @MainActor
  func testInputLanguageRemovalPersistsAndFallsBack() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // 被移除的语言存在 app group 里，比这个 bundle 活得久，而本测试留下的隐藏方案会让 `InputSchemePreference` 把之后对它的每次指定都降级。即使下面的断言失败也要恢复可见性，否则键盘单元测试会继承一份残缺的方案列表。
    defer { restoreSchemeVisibility(in: app) }
    app.buttons["inputSettingsLink"].tap()
    let japanese = app.buttons["inputLanguage_japanese"]
    if !japanese.waitForExistence(timeout: 3) {
      // 中途停止的运行可能留下 日语 已被移除的状态；添加语言 会把它加回来。
      app.buttons["addInputLanguageButton"].tap()
      app.buttons["addInputLanguage_japanese"].tap()
    }
    XCTAssertTrue(openSchemeSheet(.japanese, in: app))
    app.buttons["inputScheme_japanese"].tap()
    XCTAssertTrue(wait(japanese, until: "value == '日语 26 键'"), japanese.debugDescription)
    // 移除键盘当前所用的语言会让它回到 全拼。
    XCTAssertTrue(openSchemeSheet(.japanese, in: app))
    app.buttons["removeInputLanguage_japanese"].tap()
    let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: japanese)
    XCTAssertEqual(XCTWaiter.wait(for: [gone], timeout: 5), .completed)
    XCTAssertEqual(app.buttons["inputLanguage_mandarin"].value as? String, "全拼")
    app.terminate()
    app.launch()
    app.buttons["inputSettingsLink"].tap()
    XCTAssertTrue(app.buttons["inputLanguage_mandarin"].waitForExistence(timeout: 5))
    XCTAssertFalse(japanese.exists)
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Input languages after removal"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    // 添加语言 会再次列出它，添加后启用该语言，但不会把键盘切换过去。
    app.buttons["addInputLanguageButton"].tap()
    app.buttons["addInputLanguage_japanese"].tap()
    XCTAssertTrue(japanese.waitForExistence(timeout: 5))
    XCTAssertEqual(app.buttons["inputLanguage_mandarin"].value as? String, "全拼")
  }

  @MainActor
  func testSettingsPersistAndExposeGuideAndTryout() {
    let app = XCUIApplication()
    app.launchArguments = ["--reset-onboarding-for-ui-tests"]
    app.launch()
    skipSplash(in: app)
    let finish = app.buttons["skipOnboardingButton"]
    XCTAssertTrue(finish.waitForExistence(timeout: 10))
    if !finish.isHittable { app.swipeUp() }
    finish.tap()
    app.launchArguments = ["-service.ai.endpoint", "", "-service.ai.model", ""]

    // Onboarding hands over to the 设置 home page.
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].waitForExistence(timeout: 10))
    app.buttons["inputSettingsLink"].tap()
    // 方案是 普通话 弹窗里的选项；双拼方案有多个时，它们位于 双拼 › 之后。
    XCTAssertTrue(openSchemeSheet(.mandarin, in: app))
    XCTAssertTrue(app.buttons["inputScheme_quanpin"].exists)
    XCTAssertTrue(app.buttons["inputScheme_shuangpin"].exists || app.buttons["inputSchemeGroup_shuangpin"].exists)
    let nineKey = app.buttons["inputScheme_nineKey"]
    XCTAssertTrue(nineKey.exists)
    nineKey.tap()
    XCTAssertTrue(wait(app.buttons["designOptionSheetCancel"], until: "exists == false"))
    app.terminate()
    app.launch()
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].waitForExistence(timeout: 5))
    app.buttons["inputSettingsLink"].tap()
    XCTAssertTrue(openSchemeSheet(.mandarin, in: app))
    XCTAssertTrue(app.buttons["inputScheme_nineKey"].isSelected)
    app.buttons["designOptionSheetCancel"].tap()

    // 中文字符集 是选择行，它的弹窗提供 简体 和 繁体。
    let outputPicker = app.buttons["chineseOutputPicker"]
    reveal(outputPicker, in: app)
    outputPicker.tap()
    XCTAssertTrue(app.buttons["designOptionSheetCancel"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["简体"].exists)
    XCTAssertTrue(app.buttons["繁体"].exists)
    app.buttons["designOptionSheetCancel"].tap()

    app.navigationBars.buttons.element(boundBy: 0).tap()
    for (identifier, title) in [
      ("skinSettingsLink", "皮肤"), ("expressionSettingsLink", "表达"), ("dictionarySettingsLink", "词库"),
      ("voiceSettingsLink", "语音输入"),
    ] {
      reachSettingsLink(identifier, in: app)
      let link = settingsEntry(identifier, in: app)
      link.tap()
      XCTAssertTrue(app.navigationBars[title].waitForExistence(timeout: 5))
      if identifier == "skinSettingsLink" {
        reveal(app.buttons["skin_night"], in: app)
        app.buttons["skin_night"].tap()
        XCTAssertEqual(app.buttons["skin_night"].value as? String, "已选择")
      }
      if identifier == "voiceSettingsLink" {
        XCTAssertTrue(app.buttons["voiceLanguagePicker"].exists)
        XCTAssertTrue(app.buttons["voiceServiceLink"].exists)
      }
      let attachment = XCTAttachment(screenshot: app.screenshot())
      attachment.name = title
      attachment.lifetime = .deleteOnSuccess
      add(attachment)
      app.navigationBars.buttons.element(boundBy: 0).tap()
    }
    // AI 设置 现在位于 键盘 → 更多 下。
    openAISettings(app)
    XCTAssertTrue(app.navigationBars["AI 设置"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textFields["serviceEndpoint"].exists)
    XCTAssertTrue(app.secureTextFields["serviceToken"].exists)
    let endpoint = app.textFields["serviceEndpoint"]
    endpoint.tap()
    endpoint.typeText("https://msime-ui-tests.invalid/v1/chat/completions")
    app.textFields["serviceModel"].tap()
    app.textFields["serviceModel"].typeText("fixture")
    app.buttons["serviceDismissKeyboard"].tap()
    // 模型输入框在密钥框下面，收起键盘后列表还在往回滚，这时点下去会落在移动中的密钥框之外；等键盘收起、列表停稳后再点，没拿到焦点就再点一次。
    let token = app.secureTextFields["serviceToken"]
    XCTAssertTrue(app.keyboards.element.waitForNonExistence(timeout: 5))
    token.tap()
    if !wait(token, until: "hasKeyboardFocus == true", timeout: 2) { token.tap() }
    token.typeText("msime-ui-fixture")
    app.buttons["serviceDismissKeyboard"].tap()
    app.buttons["saveServiceConfiguration"].tap()
    for _ in 0..<3 {
      if app.staticTexts["配置已保存"].exists { break }
      app.swipeUp()
    }
    XCTAssertTrue(app.staticTexts["配置已保存"].waitForExistence(timeout: 5))
    let deleteKey = app.buttons["删除此服务的密钥"]
    for _ in 0..<3 {
      if deleteKey.isHittable { break }
      app.swipeDown()
    }
    deleteKey.tap()
    for _ in 0..<3 {
      if app.staticTexts["已删除此服务的密钥"].exists { break }
      app.swipeUp()
    }
    XCTAssertTrue(app.staticTexts["已删除此服务的密钥"].waitForExistence(timeout: 5))
    // 遍历结束时导航栈停在 AI 页、根页面已向下滚动，而 试用键盘 在顶部的状态卡片里，所以无论哪个方向都要把它滚回视野内（并避开玻璃栏），而不是继续向下滑。
    reachSettingsLink("keyboardTryoutLink", in: app)
    let tryoutLink = settingsEntry("keyboardTryoutLink", in: app)
    let overview = XCTAttachment(screenshot: app.screenshot())
    overview.name = "设置分类"
    overview.lifetime = .deleteOnSuccess
    add(overview)
    tryoutLink.tap()
    let tryoutField = app.textFields["keyboardTryoutField"]
    XCTAssertTrue(tryoutField.waitForExistence(timeout: 10))
    // This screen focuses the field on appearance. Avoid tapping an already focused field
    // while the keyboard is animating: XCTest can hit a keyboard key instead of the field.
    let focused = XCTNSPredicateExpectation(
      predicate: NSPredicate(format: "hasKeyboardFocus == true"), object: tryoutField)
    guard XCTWaiter.wait(for: [focused], timeout: 10) == .completed else {
      XCTFail("The tryout field did not receive focus on appearance")
      return
    }
    tryoutField.typeText("test")
    // A loaded runner reports the field's value while it is still catching up with the keystrokes,
    // which reads as a prefix of the typed text rather than a lost character.
    XCTAssertTrue(wait(tryoutField, until: "value == 'test'"))

    // The field had no way to put the keyboard away without leaving the app.
    let dismissButton = app.buttons["dismissKeyboardButton"]
    XCTAssertTrue(dismissButton.waitForExistence(timeout: 5))
    dismissButton.tap()
    let unfocused = expectation(
      for: NSPredicate(format: "hasKeyboardFocus == false"), evaluatedWith: tryoutField)
    wait(for: [unfocused], timeout: 10)
    XCTAssertEqual(tryoutField.value as? String, "test")
    app.navigationBars.buttons.element(boundBy: 0).tap()
    XCTAssertFalse(app.buttons["keyboardGuideLink"].exists)
    // 去开启 在页面顶部的状态卡片里，遍历结束时页面已滚到它下方，所以要把它滚回视野内。
    reachSettingsLink("openKeyboardSettingsButton", in: app)
    XCTAssertTrue(app.buttons["openKeyboardSettingsButton"].exists)
  }
}
