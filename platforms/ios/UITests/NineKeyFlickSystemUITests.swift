import XCTest

/// 手势经系统扩展和真实 UIKit 触摸路径送入独立编辑器，不能用直接调用 tracker 代替。
@MainActor
extension KeyboardExtensionEditorUITests {
  func testNineKeyFlickDirectionsAndLongPressReachSystemEditor() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    app.launch()
    app.terminate()
    XCTAssertTrue(addKeyboardInSettingsIfNeeded(), "系统键盘必须登记且允许访问共享设置")

    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    addTeardownBlock { @MainActor in editor.terminate() }
    editor.launch()
    let field = editor.textFields["systemKeyboardEditor"]
    XCTAssertTrue(field.waitForExistence(timeout: 10))
    _ = try activeSystemKeyboard(in: editor, field: field)
    editor.buttons["schemeButton"].tap()
    let card = editor.buttons["schemeCard-nineKey"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    card.tap()
    let language = editor.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }

    var expected = ""
    func expectDocument() {
      let received = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", expected), object: field)
      XCTAssertEqual(XCTWaiter.wait(for: [received], timeout: 5), .completed,
                     "flick 只能提交方向字母，不能再带上起始键的点按或数字")
    }
    func flick(_ digit: Int, _ offset: CGVector) {
      let key = editor.buttons["nineKey\(digit)"]
      XCTAssertTrue(key.waitForExistence(timeout: 5))
      let start = key.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
      start.press(forDuration: 0.05, thenDragTo: start.withOffset(offset),
                  withVelocity: 400, thenHoldForDuration: 0.05)
    }
    let directions = [CGVector(dx: 0, dy: -36), CGVector(dx: -36, dy: 0), CGVector(dx: 36, dy: 0)]
    for (digit, letters) in [(2, "ABC"), (3, "DEF"), (4, "GHI"), (5, "JKL"),
                             (6, "MNO"), (7, "PQR"), (8, "TUV"), (9, "WXY")] {
      for (offset, letter) in zip(directions, letters) {
        flick(digit, offset)
        expected.append(letter)
        expectDocument()
      }
      flick(digit, CGVector(dx: 0, dy: 36))
      expectDocument()
    }
    for offset in directions + [CGVector(dx: 0, dy: 36)] {
      flick(1, offset)
      expectDocument()
    }
    keepScreenshot("九键四向flick-字母与无动作方向", in: editor)
    editor.buttons["nineKey2"].press(forDuration: 0.7)
    let digit = editor.buttons["nineKeyHoldOption-2"]
    XCTAssertTrue(digit.waitForExistence(timeout: 5), "原数字入口必须能从长按到达")
    XCTAssertTrue(digit.isHittable, "首行长按选项必须留在系统扩展可点击的范围内")
    digit.tap()
    expected += "2"
    expectDocument()
    keepScreenshot("九键四向flick-长按数字", in: editor)
  }

  func testNineKeyFlickThresholdSettingsAffectNextSystemTouch() throws {
    let app = XCUIApplication()
    app.launchArguments = ["-hasCompletedOnboarding", "YES"]
    var original: Double?
    func configure(_ threshold: Double) throws {
      app.launch()
      app.tabBars.buttons["首页"].tap()
      let link = app.buttons["keyboardLayoutLink"]
      for _ in 0..<8 where !link.isHittable { app.swipeUp() }
      XCTAssertTrue(link.isHittable)
      link.tap()
      let slider = app.sliders["appNineKeyFlickThreshold"]
      for _ in 0..<14 where !slider.isHittable {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.03, dy: 0.84))
        start.press(forDuration: 0.05, thenDragTo:
          app.coordinate(withNormalizedOffset: CGVector(dx: 0.03, dy: 0.55)))
      }
      XCTAssertTrue(slider.isHittable)
      if original == nil {
        let raw = try XCTUnwrap(slider.value as? String)
        original = try XCTUnwrap(Double(raw.replacingOccurrences(of: " pt", with: "")))
      }
      slider.adjust(toNormalizedSliderPosition: (threshold - 14) / 34)
      let saved = XCTNSPredicateExpectation(
        predicate: NSPredicate(format: "value == %@", "\(Int(threshold)) pt"), object: slider)
      XCTAssertEqual(XCTWaiter.wait(for: [saved], timeout: 5), .completed)
      app.terminate()
    }
    addTeardownBlock { @MainActor in
      if let original { try configure(original) }
      app.terminate()
    }
    try configure(48)
    XCTAssertTrue(addKeyboardInSettingsIfNeeded())
    let editor = XCUIApplication(bundleIdentifier: "app.msime.ios.keyboard-test-host")
    editor.launchArguments = ["--system-keyboard-editor"]
    addTeardownBlock { @MainActor in editor.terminate() }
    editor.launch()
    let field = editor.textFields["systemKeyboardEditor"]
    XCTAssertTrue(field.waitForExistence(timeout: 10))
    _ = try activeSystemKeyboard(in: editor, field: field)
    editor.buttons["schemeButton"].tap()
    let card = editor.buttons["schemeCard-nineKey"]
    XCTAssertTrue(card.waitForExistence(timeout: 5))
    card.tap()
    let language = editor.buttons["bottomLanguageKey"]
    if language.label == "切换到中文输入" { language.tap() }

    func flick(_ distance: CGFloat) {
      let start = editor.buttons["nineKey2"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
      start.press(forDuration: 0.05, thenDragTo: start.withOffset(CGVector(dx: 0, dy: -distance)),
                  withVelocity: 400, thenHoldForDuration: 0.05)
    }
    flick(20)
    XCTAssertTrue(editor.buttons["candidate-1"].waitForExistence(timeout: 5),
                  "48pt 阈值下 20pt 移动必须保留普通九键点按")
    XCTAssertEqual(editor.buttons["returnKey"].label, "确认")
    editor.buttons["nineKeyDelete"].tap()
    XCTAssertNotEqual(editor.buttons["returnKey"].label, "确认")
    flick(60)
    XCTAssertEqual(field.value as? String, "A", "超过 48pt 才直接提交字母")
    editor.terminate()

    try configure(14)
    editor.launch()
    XCTAssertTrue(field.waitForExistence(timeout: 10))
    _ = try activeSystemKeyboard(in: editor, field: field)
    XCTAssertTrue(editor.buttons["nineKey2"].waitForExistence(timeout: 5))
    flick(20)
    let received = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == 'A'"), object: field)
    XCTAssertEqual(XCTWaiter.wait(for: [received], timeout: 5), .completed,
                   "App Group 中的新阈值必须在下一次触摸生效")
    keepScreenshot("九键flick-设置阈值14生效", in: editor)
  }
}
