import XCTest

/// 从原生词库入口新建两个合成分组，核对多选启停、删除确认及返回后的持久状态。
@MainActor
final class DictionaryGroupsUITests: XCTestCase {
  override func setUp() {
    super.setUp()
    continueAfterFailure = false
  }

  func testBatchManagementKeepsSelectionAndConfirmsDeletion() {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.open(URL(string: "msime://settings/lexicon")!)
    let entry = app.buttons["dictionaryCollectionsLink"]
    XCTAssertTrue(entry.waitForExistence(timeout: 20))
    entry.tap()
    let actions = app.buttons["dictionaryGroupsActions"]
    XCTAssertTrue(actions.waitForExistence(timeout: 10))
    let suffix = String(UUID().uuidString.prefix(4))
    let names = ["合成分组甲-\(suffix)", "合成分组乙-\(suffix)"]
    for name in names {
      let ready = XCTNSPredicateExpectation(predicate: NSPredicate(format: "enabled == true AND hittable == true"), object: actions)
      XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 30), .completed)
      actions.tap()
      let create = app.buttons["新建分组"]
      XCTAssertTrue(create.waitForExistence(timeout: 5))
      create.tap()
      let alert = app.alerts["新建分组"]
      XCTAssertTrue(alert.waitForExistence(timeout: 5))
      alert.textFields.firstMatch.tap()
      alert.textFields.firstMatch.typeText(name)
      alert.buttons["保存"].tap()
      XCTAssertTrue(app.staticTexts[name].waitForExistence(timeout: 10))
    }
    for name in names { app.buttons["选择「\(name)」"].tap() }
    app.buttons["停用所选（2）"].tap()
    for name in names {
      let row = app.cells.containing(.staticText, identifier: name)
      XCTAssertTrue(row.staticTexts["0 条 · 已停用"].waitForExistence(timeout: 10))
    }
    let shot = XCTAttachment(screenshot: app.screenshot())
    shot.name = "词库分组-多选停用"
    shot.lifetime = .keepAlways
    add(shot)
    app.navigationBars.buttons.element(boundBy: 0).tap()
    entry.tap()
    for name in names {
      let row = app.cells.containing(.staticText, identifier: name)
      XCTAssertTrue(row.staticTexts["0 条 · 已停用"].waitForExistence(timeout: 10))
    }
    for name in names { app.buttons["选择「\(name)」"].tap() }
    app.buttons["启用所选（2）"].tap()
    for name in names {
      let row = app.cells.containing(.staticText, identifier: name)
      XCTAssertTrue(row.staticTexts["0 条 · 已启用"].waitForExistence(timeout: 10))
    }
    app.buttons["删除所选（2）"].tap()
    XCTAssertTrue(app.buttons["删除"].waitForExistence(timeout: 5))
    if app.buttons["取消"].exists { app.buttons["取消"].tap() }
    else { app.otherElements["PopoverDismissRegion"].tap() }
    XCTAssertTrue(app.buttons["删除"].waitForNonExistence(timeout: 5))
    for name in names { XCTAssertTrue(app.staticTexts[name].exists) }
    app.buttons["删除所选（2）"].tap()
    XCTAssertTrue(app.buttons["删除"].waitForExistence(timeout: 5))
    app.buttons["删除"].tap()
    for name in names { XCTAssertTrue(app.staticTexts[name].waitForNonExistence(timeout: 10)) }
  }
}
