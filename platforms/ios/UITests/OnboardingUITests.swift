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
    let voice = app.switches["appVoiceShortcutSwitch"]
    func position(_ slider: XCUIElement) throws -> CGFloat {
      let raw = try XCTUnwrap(slider.value as? String)
      let value = try XCTUnwrap(Double(raw.replacingOccurrences(of: "%", with: "")))
      if raw.contains("%") { return CGFloat(value / 100) }
      let minimum = slider.identifier == "appKeySpacingSlider" ? 3.0 : 4.0
      let maximum = slider.identifier == "appKeySpacingSlider" ? 6.0 : 10.0
      return CGFloat((value - minimum) / (maximum - minimum))
    }
    // The sliders sit at the top of the form and the voice switch below the fold, where the lazy form has not built it yet, so each visit handles the sliders first and then scrolls down to the switch.
    let originalKeys = try position(keys), originalRows = try position(rows)
    // The spacing and the switch live in the app group and outlive the app, so a failure part way through would hand every later test (and the keyboard unit tests sharing the group) this test's values. A teardown block runs even when `continueAfterFailure = false` stops the test, which a `defer` does not promise, so the restore lives there; the normal path below still checks that the values go back.
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
      if let originalVoice = restore.voice {
        self.revealBelowKeyboardPreview(voice, in: app)
        if voice.value as? String != originalVoice { voice.switches.firstMatch.tap() }
      }
    }
    keys.adjust(toNormalizedSliderPosition: originalKeys > 0.5 ? 0 : 1)
    rows.adjust(toNormalizedSliderPosition: originalRows > 0.5 ? 0 : 1)
    let changedKeys = try position(keys), changedRows = try position(rows)
    revealBelowKeyboardPreview(voice, in: app)
    let originalVoice = voice.value as? String
    restore.voice = originalVoice
    voice.switches.firstMatch.tap()
    let changedVoice = voice.value as? String
    XCTAssertNotEqual(changedVoice, originalVoice)

    reachSettingsLink("keyboardLayoutLink", in: app)
    settingsEntry("keyboardLayoutLink", in: app).tap()
    XCTAssertTrue(keys.waitForExistence(timeout: 5))
    XCTAssertEqual(try position(keys), changedKeys, accuracy: 0.01)
    XCTAssertEqual(try position(rows), changedRows, accuracy: 0.01)
    keys.adjust(toNormalizedSliderPosition: originalKeys)
    rows.adjust(toNormalizedSliderPosition: originalRows)
    revealBelowKeyboardPreview(voice, in: app)
    XCTAssertEqual(voice.value as? String, changedVoice)
    voice.switches.firstMatch.tap()
    XCTAssertEqual(voice.value as? String, originalVoice)
    restore.done = true
  }

  /// What testKeyboardSpacingSettingsPersist has to put back, shared with its teardown block: the switch's first value is only known once the test has scrolled to it, and `done` skips the relaunch when the test restored everything itself.
  private final class SpacingRestore: @unchecked Sendable {
    var voice: String?
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
    XCTAssertFalse(app.buttons["confirmCommunitySkinPublication"].isEnabled, "Publication requires explicit consent")
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
    XCTAssertEqual(first.frame.minY, second.frame.minY, accuracy: 2)
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
    XCTAssertTrue(app.navigationBars["关于水杉"].waitForExistence(timeout: 5))
  }

  @MainActor
  func testKeyboardHomePrioritizesTryoutAndQuickAdjustments() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "--keyboard-chat-ui-fixture"]
    app.launch()
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].waitForExistence(timeout: 5))
    // The status card and the try-out row lead the page, with the first settings group under them.
    XCTAssertTrue(app.buttons["openKeyboardSettingsButton"].exists)
    XCTAssertTrue(app.buttons["skinSettingsLink"].exists)
    XCTAssertFalse(app.buttons["keyboardSettingsLink"].exists)
    XCTAssertFalse(app.buttons["keyboardGuideLink"].exists)
    let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Keyboard home"; shot.lifetime = .deleteOnSuccess; add(shot)
    // The rest sit further down a lazy list, so each is scrolled to before it is looked for.
    for identifier in ["inputSettingsLink", "dictionarySettingsLink", "keyboardLayoutLink", "aiSettingsLink"] {
      reachSettingsLink(identifier, in: app)
    }
    settingsEntry("aiSettingsLink", in: app).tap()
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

  /// Scroll the current page, without leaving it, until the entry is clear of the bars, then tap it. The 主题 page leads with the candidate preview and the theme grid, so its links and preview controls sit below the fold on a phone.
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

  @MainActor
  private func openKeyboardSettingsIfNeeded(_ app: XCUIApplication) {
    let entry = app.buttons["aiSettingsLink"]
    guard entry.exists else { return }
    for _ in 0..<4 { if entry.isHittable { break }; app.swipeUp() }
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
    XCTAssertTrue(app.segmentedControls["statisticsTab"].exists)
    app.tabBars.buttons["我的"].tap()
    XCTAssertTrue(app.buttons["accountAppIcon"].waitForExistence(timeout: 5))
    app.tabBars.buttons["设置"].tap()
    XCTAssertTrue(app.navigationBars["输入"].exists)
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
    XCTAssertTrue(app.navigationBars["主题"].exists)
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
    XCTAssertTrue(app.navigationBars["登录水杉"].waitForExistence(timeout: 5))
    XCTAssertFalse(app.buttons["accountLocalDesigns"].exists)
    XCTAssertFalse(app.buttons["aboutSettingsLink"].exists)
    app.navigationBars["登录水杉"].buttons["取消"].tap()
    XCTAssertTrue(app.navigationBars["试用键盘"].waitForExistence(timeout: 5))
    app.navigationBars.buttons.firstMatch.tap()
    XCTAssertTrue(app.navigationBars["设置"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].exists)
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
    XCTAssertEqual(app.buttons["inputScheme_nineKey"].value as? String, "已选择")
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
      openKeyboardSettingsIfNeeded(app)
    app.buttons["aiSettingsLink"].tap()
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
    openKeyboardSettingsIfNeeded(app)
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
    func openFeedback() {
      app.buttons["inputSettingsLink"].tap()
      for _ in 0..<5 {
        if app.switches["keyboardHapticsToggle"].isHittable { break }
        app.swipeUp()
      }
    }
    app.launch()
    openFeedback()
    let toggle = app.switches["keyboardHapticsToggle"]
    let initiallyEnabled = toggle.value as? String == "1"
    if !initiallyEnabled { toggle.switches.firstMatch.tap() }
    app.swipeUp()
    let picker = app.segmentedControls["keyboardHapticStrengthPicker"]
    let appeared = picker.waitForExistence(timeout: 5)
    if !appeared {
      let failure = XCTAttachment(screenshot: app.screenshot())
      failure.lifetime = .deleteOnSuccess
      add(failure)
    }
    XCTAssertTrue(appeared)
    let previous = picker.buttons.allElementsBoundByIndex.first { $0.isSelected }?.label ?? "中"
    picker.buttons["强"].tap()
    app.buttons["previewKeyboardHaptics"].tap()
    app.terminate()
    app.launch()
    openFeedback()
    app.swipeUp()
    XCTAssertTrue(picker.buttons["强"].isSelected)
    let attachment = XCTAttachment(screenshot: app.screenshot())
    attachment.name = "Haptic strength settings"
    attachment.lifetime = .deleteOnSuccess
    add(attachment)
    picker.buttons[previous].tap()
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
    // The period switch is gone: each statistic now has its own tab, drawn as the shape its own
    // question wants rather than four copies of one bar.
    let tabs = app.segmentedControls["statisticsTab"]
    XCTAssertTrue(tabs.waitForExistence(timeout: 5))
    for title in ["类型", "模式", "方案", "趋势"] { tabs.buttons[title].tap() }
    let day = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "statisticsDay_")).firstMatch
    XCTAssertTrue(day.waitForExistence(timeout: 3), app.debugDescription)
    day.tap()
    XCTAssertTrue(app.buttons["返回累计"].waitForExistence(timeout: 3))
    app.buttons["返回累计"].tap()
    let top = XCTAttachment(screenshot: app.screenshot())
    top.name = "统计趋势与字符分布"
    top.lifetime = .deleteOnSuccess
    add(top)
    app.swipeUp()
    app.swipeUp()
    let detail = XCTAttachment(screenshot: app.screenshot())
    detail.name = "统计语言与输入方案"
    detail.lifetime = .deleteOnSuccess
    add(detail)
  }

  @MainActor
  func testFetchingModelsRequiresKeyButNotModel() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-service.ai.provider", "custom",
      "-service.ai.endpoint", "https://catalog-no-key.invalid/v1/chat/completions", "-service.ai.model", ""]
    app.launch()
    openKeyboardSettingsIfNeeded(app)
    app.buttons["aiSettingsLink"].tap()
    let fetch = app.buttons["fetchServiceModels"]
    XCTAssertTrue(fetch.waitForExistence(timeout: 5))
    fetch.tap()
    XCTAssertTrue(app.staticTexts["请先填写 API Key，或使用已保存的密钥。"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.textFields["serviceModel"].exists)
  }

  @MainActor
  func testSkinShowsFullKeyboardInBothLayoutsAndAppearances() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.buttons["skinSettingsLink"].tap()
    XCTAssertTrue(app.navigationBars["主题"].waitForExistence(timeout: 5))
    // The full preview sits below the candidate preview, the theme grid and 自定义主题 (dc.html L769-783), so it is scrolled to rather than expected on the first screen.
    let layout = app.segmentedControls["skinPreviewLayout"]
    reveal(layout, in: app)
    layout.buttons["26 键"].tap()
    let preview = app.otherElements["fullKeyboardSkinPreview"]
    XCTAssertTrue(preview.exists)
    XCTAssertTrue(preview.label.contains("26 键"))
    let light = XCTAttachment(screenshot: app.screenshot())
    light.name = "完整 26 键皮肤预览"
    light.lifetime = .deleteOnSuccess
    add(light)
    layout.buttons["9 键"].tap()
    reveal(app.switches["skinPreviewDark"], in: app)
    app.switches["skinPreviewDark"].coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
    XCTAssertEqual(app.switches["skinPreviewDark"].value as? String, "1")
    XCTAssertTrue(preview.label.contains("9 键"))
    let dark = XCTAttachment(screenshot: app.screenshot())
    dark.name = "完整 9 键深色皮肤预览"
    dark.lifetime = .deleteOnSuccess
    add(dark)
  }

  @MainActor
  func testAIProviderSelectionAutofillsAndClearsUnsavedKey() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES", "-service.ai.provider", "custom",
                           "-service.ai.endpoint", "", "-service.ai.model", ""]
    app.launch()
    openKeyboardSettingsIfNeeded(app)
    app.buttons["aiSettingsLink"].tap()
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
    app.buttons["inputSettingsLink"].tap()
    for _ in 0..<5 {
      if app.buttons["fuzzyPinyinSettingsLink"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["fuzzyPinyinSettingsLink"].tap()
    XCTAssertTrue(app.staticTexts["fuzzyPinyinAvailability"].exists)
    let enabled = app.switches["fuzzyPinyinEnabled"]
    if enabled.value as? String == "0" { enabled.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap() }
    let rule = app.switches["fuzzyPinyinRule_z-zh"]
    if rule.value as? String == "0" { rule.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap() }
    XCTAssertEqual(enabled.value as? String, "1")
    XCTAssertEqual(rule.value as? String, "1")
    app.terminate()
    app.launch()
    app.buttons["inputSettingsLink"].tap()
    for _ in 0..<5 {
      if app.buttons["fuzzyPinyinSettingsLink"].isHittable { break }
      app.swipeUp()
    }
    app.buttons["fuzzyPinyinSettingsLink"].tap()
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
    for platform in ["macOS", "Windows", "Linux"] {
      app.segmentedControls["desktopPlatformPicker"].buttons[platform].tap()
      XCTAssertTrue(app.staticTexts[platform + " 安装指南"].exists)
      // A SwiftUI Link surfaces as a link, not a button, so asking only for buttons never finds it.
      XCTAssertTrue(app.descendants(matching: .any)["desktopReleaseLink"].exists)
    }
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
    XCTAssertTrue(app.staticTexts["aboutAppVersion"].exists)
    XCTAssertTrue(app.navigationBars["关于水杉"].exists)
  }

  @MainActor
  func testHandwritingCanBeEnabledSelectedAndDisabled() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // This one ends with handwriting hidden on purpose, which is still a scheme later tests cannot
    // select until the stored set is cleared.
    defer { restoreSchemeVisibility(in: app) }
    app.buttons["inputSettingsLink"].tap()
    let enabled = app.switches["enabledInputScheme_handwriting"]
    for _ in 0..<8 { if enabled.isHittable { break }; app.swipeUp() }
    XCTAssertTrue(enabled.isHittable)
    if enabled.value as? String == "0" { enabled.tap() }
    let scheme = app.buttons["inputScheme_handwriting"]
    scheme.tap()
    XCTAssertEqual(scheme.value as? String, "已选择")
    app.terminate(); app.launch()
    // The 输入 row on the home page shows the current scheme as its value.
    let input = app.buttons["inputSettingsLink"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    XCTAssertEqual(input.value as? String, "手写")
    input.tap()
    for _ in 0..<8 { if enabled.isHittable { break }; app.swipeUp() }
    enabled.tap()
    XCTAssertFalse(scheme.isEnabled)
  }

  @MainActor
  func testInputSchemeVisibilityPersistsAndFallsBack() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    // Hidden schemes live in the app group and outlive this bundle, and a scheme this test leaves
    // hidden makes InputSchemePreference downgrade every later assignment of it. Restore visibility
    // even when an assertion below fails, or the keyboard unit tests inherit a crippled scheme list.
    defer { restoreSchemeVisibility(in: app) }
    app.buttons["inputSettingsLink"].tap()
    let full = app.switches["enabledInputScheme_quanpin"]
    let nine = app.switches["enabledInputScheme_nineKey"]
    if full.value as? String == "0" { full.tap() }
    if nine.value as? String == "0" { nine.tap() }
    app.buttons["inputScheme_nineKey"].tap()
    nine.tap()
    // Toggling visibility rebuilds the scheme list, so wait for the switch to report its new
    // state instead of reading it while SwiftUI is still applying the change.
    XCTAssertTrue(wait(nine, until: "value == '0'"))
    XCTAssertEqual(app.buttons["inputScheme_quanpin"].value as? String, "已选择")
    app.terminate()
    app.launch()
    app.buttons["inputSettingsLink"].tap()
    XCTAssertEqual(nine.value as? String, "0")
    // The switch reports its stored value before SwiftUI has rebuilt the scheme list below it, so
    // the button is briefly still enabled after a relaunch.
    XCTAssertTrue(wait(app.buttons["inputScheme_nineKey"], until: "isEnabled == false"))
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = "Input scheme visibility settings"
    screenshot.lifetime = .deleteOnSuccess
    add(screenshot)
    nine.tap()
    // Same two steps as the hide above: the switch commits first and the scheme list is rebuilt
    // from it, so waiting on the button alone races a rebuild that has not been asked for yet.
    // Toggling back also follows a screenshot, which leaves the app busy for a moment longer.
    XCTAssertTrue(wait(nine, until: "value == '1'"))
    XCTAssertTrue(wait(app.buttons["inputScheme_nineKey"], until: "isEnabled == true", timeout: 15))
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
    XCTAssertTrue(app.buttons["inputScheme_quanpin"].exists)
    XCTAssertTrue(app.buttons["inputScheme_shuangpin"].exists)
    let nineKey = app.buttons["inputScheme_nineKey"]
    XCTAssertTrue(nineKey.exists)
    nineKey.tap()
    XCTAssertEqual(nineKey.value as? String, "已选择")
    app.terminate()
    app.launch()
    XCTAssertTrue(app.buttons["keyboardTryoutLink"].waitForExistence(timeout: 5))
    app.buttons["inputSettingsLink"].tap()
    XCTAssertEqual(app.buttons["inputScheme_nineKey"].value as? String, "已选择")

    let outputPicker = app.segmentedControls["chineseOutputPicker"]
    for _ in 0..<6 {
      if outputPicker.isHittable { break }
      app.swipeUp()
    }
    XCTAssertTrue(outputPicker.exists)
    XCTAssertTrue(outputPicker.buttons["简体"].exists)
    XCTAssertTrue(outputPicker.buttons["繁体"].exists)

    app.navigationBars.buttons.element(boundBy: 0).tap()
    for (identifier, title) in [
      ("skinSettingsLink", "主题"), ("dictionarySettingsLink", "词库"),
      ("aiSettingsLink", "AI 设置"), ("voiceSettingsLink", "语音设置"),
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
      if identifier == "aiSettingsLink" || identifier == "voiceSettingsLink" {
        XCTAssertTrue(app.textFields["serviceEndpoint"].exists)
        XCTAssertTrue(app.secureTextFields["serviceToken"].exists)
        if identifier == "aiSettingsLink" {
          let endpoint = app.textFields["serviceEndpoint"]
          endpoint.tap()
          endpoint.typeText("https://msime-ui-tests.invalid/v1/chat/completions")
          app.textFields["serviceModel"].tap()
          app.textFields["serviceModel"].typeText("fixture")
          app.buttons["serviceDismissKeyboard"].tap()
          app.secureTextFields["serviceToken"].tap()
          app.secureTextFields["serviceToken"].typeText("msime-ui-fixture")
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
        }
      }
      let attachment = XCTAttachment(screenshot: app.screenshot())
      attachment.name = title
      attachment.lifetime = .deleteOnSuccess
      add(attachment)
      app.navigationBars.buttons.element(boundBy: 0).tap()
    }
    // The loop leaves the list scrolled down to 语音输入, and 试用键盘 sits at the top under the status card, so it is scrolled back into view (and clear of the glass bars) in either direction rather than swiped further down.
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
    // 去开启 lives in the status card at the top of the lazy list, which the page left scrolled below it, so the row only exists once it is scrolled back into view.
    reachSettingsLink("openKeyboardSettingsButton", in: app)
    XCTAssertTrue(app.buttons["openKeyboardSettingsButton"].exists)
  }
}
