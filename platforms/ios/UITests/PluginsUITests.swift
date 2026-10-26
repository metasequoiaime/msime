import XCTest

/// 「插件」页走一遍用户会走的路：新建短语表、在详情里启用、回到列表看标记，再看社区插件；每一步留一张截图给评审看。经 `msime://settings/plugins` 直接打开插件页。
@MainActor
final class PluginsUITests: XCTestCase {
  override func setUp() {
    super.setUp()
    continueAfterFailure = false
  }

  func testCreatingAPhraseTableEnablingItAndBrowsingTheCommunity() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    PluginsUITestFlow.openPluginsPage(app)
    PluginsUITestFlow.keep("插件-我的插件", app, in: self)

    let name = "合成短语\(Int.random(in: 100...999))"
    PluginsUITestFlow.create(in: app, kind: "phrase_table", name: name, fields: [
      ("第 1 条短语的编码", "zqdz"), ("第 1 条短语的文字", "合成地址"),
    ], screenshot: { PluginsUITestFlow.keep("插件-新建短语表", app, in: self) })
    XCTAssertTrue(app.staticTexts["pluginSavedNote"].waitForExistence(timeout: 15), "保存后打开详情并说明还差哪一步")
    PluginsUITestFlow.keep("插件-短语表详情", app, in: self)

    let enable = app.switches["pluginTableEnabled"]
    XCTAssertTrue(enable.waitForExistence(timeout: 5))
    PluginsUITestFlow.flip(enable, in: app)
    XCTAssertEqual(enable.value as? String, "1")
    app.navigationBars.buttons.element(boundBy: 0).tap()
    let marker = app.staticTexts["已启用 · 第 1 位"]
    XCTAssertTrue(marker.waitForExistence(timeout: 5) || app.staticTexts["已启用 · K 模式未开"].exists)
    PluginsUITestFlow.keep("插件-已启用的短语表", app, in: self)

    app.buttons["pluginsTab-1"].tap()
    XCTAssertTrue(app.buttons["publishCommunityPlugin"].waitForExistence(timeout: 10))
    // 等列表读完（或报出网络错误），再截图。
    _ = app.buttons["communityPluginKind-all"].waitForExistence(timeout: 10)
    sleep(3)
    PluginsUITestFlow.keep("插件-社区插件", app, in: self)
    let alert = app.alerts.firstMatch
    if alert.exists { alert.buttons.firstMatch.tap() }

    // 删掉这次做的包，不把它留给后面的用例。
    app.buttons["pluginsTab-0"].tap()
    PluginsUITestFlow.remove(name, in: app)
  }
}

/// 插件页上的几步，键盘的界面用例也用它在插件页里做包。
@MainActor
enum PluginsUITestFlow {
  static func keep(_ name: String, _ app: XCUIApplication, in test: XCTestCase) {
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = name
    shot.lifetime = .keepAlways
    test.add(shot)
  }

  static func openPluginsPage(_ app: XCUIApplication) {
    app.open(URL(string: "msime://settings/plugins")!)
    XCTAssertTrue(app.navigationBars["插件"].waitForExistence(timeout: 20), "深链没有打开插件页")
  }

  /// 点开关本身：点在行中间的说明文字上不会拨动 SwiftUI 的 Toggle。
  static func flip(_ toggle: XCUIElement, in app: XCUIApplication) {
    reveal(toggle, in: app).coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5)).tap()
  }

  /// 在「新建插件」里选一种类型，填名字和 `fields`（按无障碍标签找输入框），保存。`textViews` 是多行的符号框。
  static func create(in app: XCUIApplication, kind: String, name: String, fields: [(String, String)],
                     textViews: [(String, String)] = [], screenshot: () -> Void = {}) {
    reveal(app.buttons["pluginsCreate"], in: app).tap()
    let kindButton = app.buttons["pluginCreate.\(kind)"]
    XCTAssertTrue(kindButton.waitForExistence(timeout: 10))
    kindButton.tap()
    type(name, into: app.textFields["pluginEditorName"], in: app)
    for (label, text) in fields { type(text, into: app.textFields[label], in: app) }
    for (label, text) in textViews { type(text, into: app.textViews[label], in: app) }
    screenshot()
    let save = reveal(app.buttons["pluginEditorSave"], in: app)
    XCTAssertTrue(save.isEnabled, "表单应当可以保存：\(app.staticTexts["pluginEditorIssue"].label)")
    save.tap()
  }

  /// 打开详情，删除，确认。
  static func remove(_ name: String, in app: XCUIApplication) {
    let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", name)).firstMatch
    guard row.waitForExistence(timeout: 5) else { return }
    reveal(row, in: app).tap()
    reveal(app.buttons["pluginRemove"], in: app).tap()
    // 详情里的按钮读作「删除<名字>」，确认框里的才是「删除」。
    let confirm = app.buttons["删除"].firstMatch
    if confirm.waitForExistence(timeout: 5) { confirm.tap() }
  }

  static func type(_ text: String, into field: XCUIElement, in app: XCUIApplication) {
    reveal(field, in: app).tap()
    field.typeText(text)
  }

  /// 在当前表单里上下滚，直到元素露出来。拖动从屏幕上部起手：下半部可能是键盘或多行文本框，在那里拖动滚不动表单；编辑器的表单一滚就收起键盘。
  @discardableResult
  static func reveal(_ element: XCUIElement, in app: XCUIApplication) -> XCUIElement {
    _ = element.waitForExistence(timeout: 5)
    func drag(_ from: CGFloat, _ to: CGFloat) {
      app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: from))
        .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: to)))
    }
    if !(element.exists && element.isHittable) { drag(0.2, 0.3) }
    for _ in 0..<10 where !(element.exists && element.isHittable) { drag(0.45, 0.15) }
    for _ in 0..<10 where !(element.exists && element.isHittable) { drag(0.2, 0.5) }
    return element
  }
}
