import XCTest
import UIKit
import Darwin

@MainActor
final class NineKeyKeyboardTests: XCTestCase {
  // Claims every scheme so an assignment to InputSchemePreference.scheme is not downgraded to
  // whatever the app group was left holding. See InputSchemeTestSupport.
  private var savedKeyboardPreferences: [String: Any] = [:]
  private let preferenceKeys = [KeyboardLayoutPreference.key, KeyboardLayoutPreference.keySpacingKey,
    KeyboardLayoutPreference.rowSpacingKey, KeyboardLayoutPreference.heightAdjustmentKey,
    KeyboardLayoutPreference.voiceShortcutKey,
    KeyboardLayoutPreference.fullWidthInputKey]
  override func tearDown() {
    for key in preferenceKeys {
      if let value = savedKeyboardPreferences[key] { KeyboardLayoutPreference.defaults.set(value, forKey: key) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
    super.tearDown()
  }
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
    savedKeyboardPreferences = [:]
    for key in preferenceKeys {
      savedKeyboardPreferences[key] = KeyboardLayoutPreference.defaults.object(forKey: key)
      if key != KeyboardLayoutPreference.key { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
  }

  func testLayoutPresetsKeepKeysInBoundsAcrossBothKeyboards() throws {
    let previousLayout = KeyboardLayoutPreference.selected
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      KeyboardLayoutPreference.selected = previousLayout
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    for preset in KeyboardLayoutPreset.allCases {
      KeyboardLayoutPreference.selected = preset
      for scheme in [ChineseInputScheme.quanpin, .nineKey] {
        InputSchemePreference.scheme = scheme
        for width in [320.0, 414.0] {
          let controller = KeyboardViewController()
          controller.loadViewIfNeeded()
          controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
          controller.view.layoutIfNeeded()
          let space = try button("spaceKey", in: controller)
          let enter = try button("returnKey", in: controller)
          XCTAssertGreaterThanOrEqual(space.bounds.width, 43.5, "\(preset) / \(scheme) / \(width)")
          XCTAssertEqual(
            controller.view.bounds.height, 260 + KeyboardViewController.stripExtraHeight,
            accuracy: 0.5)
          XCTAssertLessThanOrEqual(enter.convert(enter.bounds, to: controller.view).maxX, width)
          let language = try button("bottomLanguageKey", in: controller)
          XCTAssertFalse(language.isHidden)
          if preset != .msime {
            XCTAssertGreaterThanOrEqual(language.convert(language.bounds, to: controller.view).minX,
              space.convert(space.bounds, to: controller.view).maxX)
          }
          if width == 414 {
            let renderer = UIGraphicsImageRenderer(bounds: controller.view.bounds)
            let screenshot = renderer.image { context in controller.view.layer.render(in: context.cgContext) }
            let attachment = XCTAttachment(image: screenshot)
            attachment.name = "Layout-\(preset.rawValue)-\(scheme.rawValue)"
            attachment.lifetime = .keepAlways
            add(attachment)
          }
        }
      }
    }
  }

  func testNineKeysCarryTheirLettersAndAHoldGesture() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)

    let expected = [2: "ABC", 3: "DEF", 4: "GHI", 5: "JKL", 6: "MNO", 7: "PQRS", 8: "TUV", 9: "WXYZ"]
    for (digit, letters) in expected {
      let key = try button("nineKey\(digit)", in: controller)
      XCTAssertEqual(key.configuration?.title, letters)
      XCTAssertEqual(key.tag, digit)
      XCTAssertEqual(
        (key.gestureRecognizers ?? []).compactMap { $0 as? UILongPressGestureRecognizer }.count, 1)
    }
    let split = try button("nineKey1", in: controller)
    XCTAssertEqual(split.configuration?.title, "分词")
    XCTAssertTrue((split.gestureRecognizers ?? []).compactMap { $0 as? UILongPressGestureRecognizer }.isEmpty)
  }

  func testNineKeyDigitLayerKeepsTheGridAndRestoresLetters() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)

    XCTAssertEqual(try button("nineKey2", in: controller).configuration?.title, "ABC")
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    for digit in 1...9 {
      XCTAssertEqual(try button("nineKey\(digit)", in: controller).configuration?.title, String(digit))
    }
    XCTAssertTrue(descendants(controller.view).filter {
      $0.accessibilityLabel == "符号 1" && !($0.superview?.isHidden ?? true)
    }.isEmpty)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertEqual(try button("nineKey2", in: controller).configuration?.title, "ABC")
  }

  func testLayoutPreferencePreservesActiveComposition() throws {
    let previousLayout = KeyboardLayoutPreference.selected
    let previousScheme = InputSchemePreference.scheme
    defer { KeyboardLayoutPreference.selected = previousLayout; InputSchemePreference.scheme = previousScheme }
    KeyboardLayoutPreference.selected = .msime
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    key.sendActions(for: .primaryActionTriggered)
    let before = try button("preeditButton", in: controller).configuration?.title
    KeyboardLayoutPreference.selected = .wechat
    controller.viewWillAppear(false)
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, before)
    XCTAssertFalse(try button("bottomLanguageKey", in: controller).isHidden)
  }

  func testTouchSchemeWritesCanonicalEngineAndPresentationMapping() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-scheme-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTouchKeyboardScheme(
      .microsoft, enabledSchemes: [.quanpin, .microsoft, .japaneseNineKey]))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["scheme"] as? String, "shuangpin")
    XCTAssertEqual(preferences["last_chinese_scheme"] as? String, "shuangpin")
    XCTAssertEqual(preferences["shuangpin_profile"] as? String, "microsoft")
    XCTAssertEqual(preferences["touch_keyboard_layout"] as? String, "twenty_six_key")
    let schemes = try XCTUnwrap(preferences["touch_keyboard_schemes"] as? [String: Any])
    XCTAssertEqual(schemes["enabled"] as? [String], ["quanpin", "microsoft", "japanese_nine_key"])
    XCTAssertEqual(schemes["selected"] as? String, "microsoft")
  }

  func testKeyboardThemeWritesCanonicalPreference() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-skin-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.updateTheme(GlobalThemePreference.selecting("night")))
    var preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "night")
    // A design picked on the keyboard moves to the custom theme over the theme it replaces.
    let design = CustomKeyboardSkin.templates[2].1
    XCTAssertTrue(bridge.updateTheme(try XCTUnwrap(GlobalThemePreference.applyingDesign(design))))
    preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "custom")
    XCTAssertEqual(GlobalThemePreference.base(in: preferences), "night")
    XCTAssertEqual(GlobalThemePreference.design(in: preferences), design.normalized)
    // An id the catalog does not list is refused and changes nothing.
    XCTAssertFalse(bridge.updateTheme(GlobalThemePreference.selecting("midnight")))
    XCTAssertEqual(try XCTUnwrap(bridge.sharedPreferences)["global_theme"] as? String, "custom")
  }

  func testTraditionalOutputWritesCanonicalPreference() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-traditional-output-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTraditionalChineseOutput(true))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["traditional_chinese_output"] as? Bool, true)
  }

  func testFuzzyPreferencesWaitForIdleAndSurviveSchemeRebuild() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-fuzzy-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setFuzzyPinyinRules(1))
    _ = bridge.handleCharacter("z")
    XCTAssertFalse(bridge.setFuzzyPinyinRules(0))
    _ = bridge.cancel()
    XCTAssertTrue(bridge.setFuzzyPinyinRules(0))
    XCTAssertTrue(bridge.setFuzzyPinyinRules(1))
    _ = bridge.switchToNineKey()
    _ = bridge.switch(toShuangpinProfile: "xiaohe")
    _ = bridge.handleCharacter("z")
    let view = bridge.handleCharacter("s")
    let rows = try XCTUnwrap(try bridge.allCandidates()["candidates"] as? [[String: Any]])
    XCTAssertTrue(rows.contains { $0["text"] as? String == "中" }, String(describing: view.candidates))
    XCTAssertFalse(bridge.setFuzzyPinyinRules(0))
    _ = bridge.cancel()
    XCTAssertTrue(bridge.setFuzzyPinyinRules(0))
  }

  func testThoughtfulReplySchemeShowsDedicatedKeyboardAndCanBeDisabled() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previousEnabled = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    let previous = InputSchemePreference.scheme
    defer {
      defaults.set(previousEnabled, forKey: InputSchemePreference.enabledSchemesKey)
      InputSchemePreference.scheme = previous
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .thoughtfulReply]
    InputSchemePreference.scheme = .thoughtfulReply
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "高情商回复")
    XCTAssertTrue(descendants(controller.view).contains { $0.accessibilityIdentifier == "replyKeyboard" })
    let reply = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "replyKeyboard" })
    let strip = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "candidateStrip" })
    controller.view.layoutIfNeeded()
    XCTAssertGreaterThanOrEqual(reply.convert(reply.bounds, to: controller.view).minY,
                                strip.convert(strip.bounds, to: controller.view).maxY - 0.5)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "replySchemes" })
    XCTAssertTrue(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
    InputSchemePreference.enabledSchemes = [.quanpin]
    controller.viewWillAppear(false)
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
    XCTAssertTrue(try button("layoutVoiceShortcut", in: controller).isHidden)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "replyKeyboard" })

    // Inserting a reply takes the panel away so the text it just wrote, and the backspace that
    // edits it, are reachable. The reply shortcut is what brings it back, so that path has to work
    // even when the panel is already gone.
    InputSchemePreference.enabledSchemes = [.quanpin, .thoughtfulReply]
    InputSchemePreference.scheme = .thoughtfulReply
    controller.viewWillAppear(false)
    try button("replyShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(descendants(controller.view).contains { $0.accessibilityIdentifier == "replyKeyboard" })
  }

  func testDisabledSchemesAreHiddenAndCurrentSchemeFallsBack() throws {
    let defaults = try XCTUnwrap(UserDefaults(suiteName: InputSchemePreference.appGroupIdentifier))
    let previousEnabled = defaults.object(forKey: InputSchemePreference.enabledSchemesKey)
    let previousScheme = InputSchemePreference.scheme
    defer {
      defaults.set(previousEnabled, forKey: InputSchemePreference.enabledSchemesKey)
      InputSchemePreference.scheme = previousScheme
    }
    defaults.removeObject(forKey: InputSchemePreference.enabledSchemesKey)
    XCTAssertEqual(InputSchemePreference.enabledSchemes, ChineseInputScheme.allCases.filter { !ChineseInputScheme.optInSchemes.contains($0) })
    InputSchemePreference.scheme = .japanese
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    InputSchemePreference.enabledSchemes = [.nineKey, .wubi]
    XCTAssertEqual(InputSchemePreference.scheme, .nineKey)
    controller.viewWillAppear(false)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    let cards = descendants(controller.view).compactMap(\.accessibilityIdentifier).filter { $0.hasPrefix("schemeCard-") }
    XCTAssertEqual(Set(cards), ["schemeCard-nineKey", "schemeCard-wubi"])
    try button("schemeCard-wubi", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
    InputSchemePreference.enabledSchemes = []
    XCTAssertEqual(InputSchemePreference.enabledSchemes, [.quanpin])
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
  }

  func testFieldLanguagePreferenceIsTemporaryAndRespectsManualChoice() {
    var context = KeyboardInputContext()
    let ordinary = UUID(), code = UUID(), url = UUID()
    XCTAssertNil(context.languageOverride(for: .default, document: ordinary, isChinese: true))
    XCTAssertEqual(context.languageOverride(for: .asciiCapable, document: code, isChinese: true), false)
    // A user explicitly switched to Chinese in this field; callbacks must not force English again.
    XCTAssertNil(context.languageOverride(for: .asciiCapable, document: code, isChinese: true))
    XCTAssertEqual(context.languageOverride(for: .URL, document: url, isChinese: true), false)
    XCTAssertEqual(context.languageOverride(for: .default, document: ordinary, isChinese: false), true)
    XCTAssertEqual(context.languageOverride(for: .emailAddress, document: code, isChinese: false), false)
    XCTAssertEqual(context.languageOverride(for: .default, document: ordinary, isChinese: false), false)
    XCTAssertNil(context.languageOverride(for: .webSearch, document: UUID(), isChinese: true))
    XCTAssertNil(context.languageOverride(for: .default, document: UUID(), isChinese: true))
  }

  func testLatinFieldsUseFullKeyboardAndRestoreNineKeyHeight() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      let ordinary = UUID()
      controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
      for type in [UIKeyboardType.asciiCapable, .emailAddress, .URL] {
        // Any preedit left after a missed focus callback must not enter the new field.
        try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
        let field = UUID()
        controller.applyInputContext(keyboardType: type, documentIdentifier: field)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
        XCTAssertTrue(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
        XCTAssertFalse(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" }).isHidden)
        let q = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 Q" } as? UIButton)
        XCTAssertFalse(try XCTUnwrap(q.superview).isHidden)
        XCTAssertEqual(q.bounds.height, try button("returnKey", in: controller).bounds.height, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(q.bounds.height, 44)
        if type != .asciiCapable { XCTAssertEqual(q.configuration?.title, "q") }
        XCTAssertEqual(try button("quickPunctuationKey", in: controller).configuration?.title, ",")
        q.sendActions(for: .primaryActionTriggered)
        XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "英文输入")
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        for symbol in ["@", "/", "_", "="] {
          let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "符号 \(symbol)" } as? UIButton)
          XCTAssertFalse(try XCTUnwrap(key.superview).isHidden)
          XCTAssertGreaterThan(key.bounds.width, 20)
        }
        controller.applyInputContext(keyboardType: type, documentIdentifier: field)
        controller.applyInputContext(keyboardType: .default, documentIdentifier: ordinary)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
        XCTAssertFalse(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden)
        XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")
        XCTAssertEqual(controller.view.bounds.height, 260 + KeyboardViewController.stripExtraHeight)
      }
    }
  }

  func testCursorDragCannotResumeAfterDocumentChangeOrCancellation() {
    var movement = SpaceCursorMovement()
    let first = UUID(), second = UUID()
    movement.begin(at: 0, document: first)
    XCTAssertEqual(movement.advance(to: 24, document: first), 2)
    XCTAssertEqual(movement.advance(to: 48, document: second), 0)
    XCTAssertFalse(movement.isActive)
    XCTAssertEqual(movement.advance(to: 60, document: first), 0)
    movement.begin(at: 0, document: second)
    movement.cancel()
    XCTAssertEqual(movement.advance(to: 24, document: second), 0)
    movement.begin(at: 0, document: second)
    XCTAssertEqual(movement.advance(to: .greatestFiniteMagnitude, document: second), 0)
    XCTAssertFalse(movement.isActive)
    movement.begin(at: .nan, document: first)
    XCTAssertFalse(movement.isActive)
  }

  func testSpaceCursorMovementAccumulatesDistanceAndReverses() throws {
    var movement = SpaceCursorMovement()
    let document = UUID()
    movement.begin(at: 10, document: document)
    XCTAssertEqual(movement.advance(to: 15, document: document), 0)
    XCTAssertEqual(movement.advance(to: 34, document: document), 2)
    XCTAssertEqual(movement.advance(to: 30, document: document), 0)
    XCTAssertEqual(movement.advance(to: 22, document: document), -1)
    movement.begin(at: -20, document: document)
    XCTAssertEqual(movement.advance(to: -31, document: document), 0)
    XCTAssertEqual(movement.advance(to: -44, document: document), -2)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let space = try button("spaceKey", in: controller)
    XCTAssertEqual(space.accessibilityCustomActions?.map(\.name), ["光标左移", "光标右移"])
    let pan = try XCTUnwrap(space.gestureRecognizers?.first { $0.name == "spaceCursorPan" } as? UIPanGestureRecognizer)
    XCTAssertTrue(pan.cancelsTouchesInView)
    XCTAssertEqual(pan.maximumNumberOfTouches, 1)
  }

  func testChangingHapticStrengthReplacesTheViewGenerator() throws {
    guard #available(iOS 17.5, *) else { throw XCTSkip("View-bound haptics require iOS 17.5") }
    let defaults = KeyboardFeedbackPreference.defaults
    let keys = [KeyboardFeedbackPreference.hapticsKey, KeyboardFeedbackPreference.strengthKey]
    let previous = keys.map { defaults.object(forKey: $0) }
    defer {
      for (key, value) in zip(keys, previous) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
      }
    }
    defaults.set(true, forKey: KeyboardFeedbackPreference.hapticsKey)
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    var last: UIImpactFeedbackGenerator?
    for strength in KeyboardHapticStrength.allCases {
      defaults.set(strength.rawValue, forKey: KeyboardFeedbackPreference.strengthKey)
      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      let generators = controller.view.interactions.compactMap { $0 as? UIImpactFeedbackGenerator }
      XCTAssertEqual(generators.count, 1)
      let current = try XCTUnwrap(generators.first)
      if let last { XCTAssertFalse(last === current) }
      last = current
    }
  }

  func testCandidateManagementMenuUsesEngineSupportedLayouts() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      for character in (scheme == .nineKey ? "64426" : "nihao") {
        if scheme == .nineKey {
          try button("nineKey\(character)", in: controller).sendActions(for: .primaryActionTriggered)
        } else {
          let key = try XCTUnwrap(descendants(controller.view).first {
            $0.accessibilityLabel == "字母 \(String(character).uppercased())"
          } as? UIButton)
          key.sendActions(for: .primaryActionTriggered)
        }
      }
      let candidate = try button("candidate-1", in: controller)
      XCTAssertTrue(candidate.menu?.children.first is UIDeferredMenuElement)
      // 你好 has two characters, so 以词定字 leads the menu while the shared `word_character.enabled` default is on. It is not pinned, so there is no 取消固定 and no slot is checked.
      XCTAssertEqual(controller.candidateMenuElements(at: 0).map(\.title),
                     ["以词定字", "优先显示", "固定排位", "删除词条…"])
      let slots = try XCTUnwrap((controller.candidateMenuElements(at: 0)[2] as? UIMenu)?.children as? [UIAction])
      XCTAssertEqual(slots.map(\.title), ["第 1 位", "第 2 位", "第 3 位", "第 4 位", "第 5 位"])
      XCTAssertTrue(slots.allSatisfy { $0.state == .off })
      XCTAssertEqual((controller.candidateMenuElements(at: 0).last as? UIMenu)?.children.first?.title,
                     "确认删除此词条")
    }
  }

  func testSkinCardsPreviewAndApplyWithoutChangingKeyboardHeight() throws {
    preserveSharedTheme()
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      try button("skinShortcut", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let picker = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSkinPicker" })
      XCTAssertEqual(picker.bounds.height, 260 + KeyboardViewController.stripExtraHeight)
      for id in GlobalThemeCatalog.ids {
        let card = try button("skinCard-\(id)", in: controller)
        XCTAssertGreaterThan(card.bounds.width, 140)
        let miniature = try XCTUnwrap(descendants(card).compactMap { $0 as? KeyboardSkinMiniature }.first)
        XCTAssertGreaterThan(miniature.bounds.height, 75)
        XCTAssertEqual(miniature.bounds.height / miniature.bounds.width, 0.6, accuracy: 0.01)
        XCTAssertEqual(card.bounds.height, miniature.bounds.height + 36, accuracy: 0.1)
      }
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Skin cards \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      try button("skinCard-night", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(GlobalThemePreference.selected, "night")
      XCTAssertEqual(MetasequoiaInputSessionBridge.loadSharedPreferences()?["global_theme"] as? String, "night")
      XCTAssertNil(picker.superview)
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 260 + KeyboardViewController.stripExtraHeight)
      try button("skinShortcut", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(try button("skinCard-night", in: controller).accessibilityValue, "已选中")
      try button("closeSkinPicker", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSkinPicker" })
    }
  }

  func testLegacyLayoutsMigrateIndependentSettings() {
    let defaults = KeyboardLayoutPreference.defaults
    for preset in KeyboardLayoutPreset.allCases {
      for key in [KeyboardLayoutPreference.keySpacingKey, KeyboardLayoutPreference.rowSpacingKey,
                  KeyboardLayoutPreference.heightAdjustmentKey, KeyboardLayoutPreference.voiceShortcutKey] {
        defaults.removeObject(forKey: key)
      }
      KeyboardLayoutPreference.selected = preset
      XCTAssertEqual(KeyboardLayoutPreference.keySpacing, preset.keySpacing)
      XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, preset.rowSpacing)
      XCTAssertEqual(KeyboardLayoutPreference.voiceShortcutEnabled, preset == .doubao)
      XCTAssertEqual(KeyboardLayoutPreference.geometry.sidebarRatio, 0.14)
      XCTAssertFalse(KeyboardLayoutPreference.geometry.showsFullKeyboardSymbols)
      KeyboardLayoutPreference.keySpacing = 5
      KeyboardLayoutPreference.voiceShortcutEnabled = !(preset == .doubao)
      XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 5)
      XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, preset.rowSpacing)
      XCTAssertEqual(KeyboardLayoutPreference.voiceShortcutEnabled, preset != .doubao)
      XCTAssertEqual(KeyboardLayoutPreference.selected, preset)
    }
    KeyboardLayoutPreference.keySpacing = 99
    KeyboardLayoutPreference.rowSpacing = -99
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 6)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 4)
  }

  func testKeyboardHeightFollowsTheSetting() throws {
    let previous = KeyboardLayoutPreference.heightAdjustment
    defer { KeyboardLayoutPreference.heightAdjustment = previous }

    func height(for adjustment: Double) -> CGFloat {
      KeyboardLayoutPreference.heightAdjustment = adjustment
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)
      controller.view.layoutIfNeeded()
      return controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant ?? 0
    }

    let standard = height(for: 0)
    XCTAssertGreaterThan(standard, 0)
    XCTAssertEqual(height(for: 24), standard + 24, accuracy: 0.5)
    XCTAssertEqual(height(for: -12), standard - 12, accuracy: 0.5)

    KeyboardLayoutPreference.heightAdjustment = 500
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 48)
    KeyboardLayoutPreference.heightAdjustment = -500
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, -12)
  }

  func testSymbolKeysShowThePunctuationTheyInsert() throws {
    XCTAssertEqual(KeyboardViewController.chineseSymbolFaces, [
      ",": "，", ".": "。", "?": "？", "!": "！", ";": "；", ":": "：",
      "(": "（", ")": "）", "[": "【", "]": "】", "\\": "、",
      "<": "《", ">": "》", "'": "‘", "\"": "“", "_": "——",
    ])
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()

    func face(_ label: String) -> UIButton? {
      descendants(controller.view).first { $0.accessibilityLabel == "符号 \(label)" } as? UIButton
    }

    for (ascii, chinese) in [("\\", "、"), (",", "，"), ("[", "【"), ("<", "《")] {
      XCTAssertNotNil(face(chinese), "中文模式下应显示 \(chinese)")
      XCTAssertNil(face(ascii), "中文模式下不该显示 \(ascii)")
    }

    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(face("\\"), "英文模式下应显示反斜杠本身")
    XCTAssertNil(face("、"))
  }

  func testSpacingChangesKeepDefaultKeyPlacementAndComposition() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    for scheme in [ChineseInputScheme.quanpin, .nineKey] {
      InputSchemePreference.scheme = scheme
      for width in [320.0, 440.0] {
        let controller = KeyboardViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: width, height: 292)
        for gap in [3.0, 6.0] {
          // The spacing used to come from sliders inside the keyboard and now comes from a drag on
          // the keys. A pan cannot be synthesised here, and what this checks is that the keys follow
          // the spacing, so it writes the preference and lets the keyboard lay out again.
          KeyboardLayoutPreference.keySpacing = gap
          KeyboardLayoutPreference.rowSpacing = gap == 3 ? 4 : 10
          controller.viewWillAppear(false)
          controller.view.layoutIfNeeded()
          XCTAssertEqual(KeyboardLayoutPreference.geometry.keySpacing, gap)
          let enter = try button("returnKey", in: controller)
          let space = try button("spaceKey", in: controller)
          XCTAssertGreaterThanOrEqual(space.bounds.width, 43.5)
          XCTAssertLessThanOrEqual(enter.convert(enter.bounds, to: controller.view).maxX, width)
          XCTAssertEqual(controller.view.bounds.height, 292)
          XCTAssertFalse(try button("bottomLanguageKey", in: controller).isHidden)
        }
      }
    }
  }

  func testKeyboardSettingsReplaceLayoutCardsAndVoiceIsIndependent() throws {
    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    KeyboardLayoutPreference.keySpacing = 5
    KeyboardLayoutPreference.rowSpacing = 8
    KeyboardLayoutPreference.voiceShortcutEnabled = false
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 440, height: 292)
    let letter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    letter.sendActions(for: .primaryActionTriggered)
    let preedit = try button("preeditButton", in: controller).configuration?.title
    try button("layoutShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier?.hasPrefix("layoutCard-") == true })
    let height = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardHeightGrip" })
    controller.view.layoutIfNeeded()
    let initialHeight = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    for _ in 0..<12 { height.accessibilityIncrement() }
    controller.view.layoutIfNeeded()
    let adjustedHeight = try XCTUnwrap(controller.view.constraints.first { $0.identifier == "keyboardHeight" }).constant
    XCTAssertEqual(adjustedHeight, initialHeight + 24, accuracy: 0.5)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 24)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, 5)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, 8)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 24)
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, preedit)
    try button("closeLayoutPicker", in: controller).sendActions(for: .primaryActionTriggered)
    // 语音入口的开关已经不在这条工具条上了 —— 它留在应用的键盘设置页。要守住的契约没变:这个偏好
    // 不被间距和高度牵连(上面三条刚查过),而且只有它决定顶栏那个语音按钮露不露面。
    KeyboardLayoutPreference.voiceShortcutEnabled = true
    controller.viewWillAppear(false)
    XCTAssertFalse(try button("layoutVoiceShortcut", in: controller).isHidden)
    KeyboardLayoutPreference.voiceShortcutEnabled = false
    controller.viewWillAppear(false)
    XCTAssertTrue(try button("layoutVoiceShortcut", in: controller).isHidden)
  }

  func testKeyboardSettingsResetRestoresIndependentDefaults() throws {
    KeyboardLayoutPreference.keySpacing = 5
    KeyboardLayoutPreference.rowSpacing = 8
    KeyboardLayoutPreference.heightAdjustment = 24
    KeyboardLayoutPreference.voiceShortcutEnabled = true
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 440, height: 292)
    try button("layoutShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    try button("resetKeyboardSettings", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(KeyboardLayoutPreference.keySpacing, KeyboardLayoutPreference.selected.keySpacing)
    XCTAssertEqual(KeyboardLayoutPreference.rowSpacing, KeyboardLayoutPreference.selected.rowSpacing)
    XCTAssertEqual(KeyboardLayoutPreference.heightAdjustment, 0)
    XCTAssertEqual(KeyboardLayoutPreference.voiceShortcutEnabled,
                   KeyboardLayoutPreference.selected == .doubao)
    for key in [KeyboardLayoutPreference.keySpacingKey, KeyboardLayoutPreference.rowSpacingKey,
                KeyboardLayoutPreference.heightAdjustmentKey, KeyboardLayoutPreference.voiceShortcutKey] {
      XCTAssertNil(KeyboardLayoutPreference.defaults.object(forKey: key), "\(key) 应被恢复默认")
    }
    XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardHeightGrip" })
  }

  func testTouchGeometryWritesAndResetsCanonicalPreferences() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-touch-geometry-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    XCTAssertTrue(bridge.setTouchKeyboardGeometry(
      keySpacing: 99, rowSpacing: -1, heightAdjustment: 99, voiceEnabled: true))
    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual((preferences["touch_key_spacing_tenths"] as? NSNumber)?.intValue, 60)
    XCTAssertEqual((preferences["touch_row_spacing_tenths"] as? NSNumber)?.intValue, 40)
    XCTAssertEqual((preferences["touch_keyboard_height_adjustment"] as? NSNumber)?.intValue, 48)
    XCTAssertEqual(preferences["touch_voice_shortcut"] as? Bool, true)

    XCTAssertTrue(bridge.resetTouchKeyboardGeometry())
    let reset = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertNil(reset["touch_key_spacing_tenths"])
    XCTAssertNil(reset["touch_row_spacing_tenths"])
    XCTAssertNil(reset["touch_keyboard_height_adjustment"])
    XCTAssertNil(reset["touch_voice_shortcut"])
  }

  func testBrandOpensCompactToolsAndUpdatesFeedbackState() throws {
    let previous = KeyboardFeedbackPreference.soundEnabled
    defer { KeyboardFeedbackPreference.defaults.set(previous, forKey: KeyboardFeedbackPreference.soundKey) }
    for width in [320.0, 414.0] {
      KeyboardFeedbackPreference.defaults.set(true, forKey: KeyboardFeedbackPreference.soundKey)
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" } as? UIStackView)
      let more = try button("moreShortcut", in: controller)
      XCTAssertTrue(toolbar.arrangedSubviews.first === more)
      // The optional 工具栏按钮 are arranged but hidden until pinned, so a default bar is still these.
      let visible = toolbar.arrangedSubviews.filter { !$0.isHidden }
      XCTAssertEqual(visible.compactMap(\.accessibilityIdentifier), [
        "moreShortcut", "layoutShortcut",
      ] + (KeyboardLayoutPreference.voiceShortcutEnabled ? ["layoutVoiceShortcut"] : []) + [
        "emojiShortcut", "skinShortcut", "schemeButton", "dismissShortcut",
      ])
      for item in visible {
        XCTAssertGreaterThanOrEqual(item.bounds.width, 42)
        XCTAssertLessThanOrEqual(item.frame.maxX, toolbar.bounds.width + 0.5)
      }
      XCTAssertNil(more.menu)
      XCTAssertNotNil(descendants(more).first { $0.accessibilityIdentifier == "keyboardBrandIcon" })
      // "Unboxed" is about what paints, not about a zero stroke: UIButton.Configuration.plain()
      // ships a 1pt stroke, so the shortcuts stay borderless through a clear stroke and fill.
      let schemeBackground = try XCTUnwrap(
        try button("schemeButton", in: controller).configuration?.background)
      XCTAssertEqual(schemeBackground.strokeColor?.cgColor.alpha, 0)
      XCTAssertEqual(schemeBackground.backgroundColor?.cgColor.alpha, 0)
      more.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardMorePicker" })
      XCTAssertEqual(panel.bounds.height, 260 + KeyboardViewController.stripExtraHeight)
      // 键盘设置 is not one of these any more: the switches behind it moved onto this page, which
      // is why the toggles below are read straight off the root rather than through a card. This
      // list kept asking for the card that level used to be.
      for title in ["表情", "剪贴板历史", "AI 润色", "语音结果", "本地输入"] {
        let card = try button("moreCard-" + title, in: controller)
        XCTAssertGreaterThan(card.bounds.width, 140)
        XCTAssertEqual(card.bounds.height, 48)
        XCTAssertEqual(card.configuration?.imagePlacement, .leading)
        XCTAssertLessThanOrEqual(card.convert(card.bounds, to: panel).maxY, panel.bounds.height)
      }
      let back = try button("closeMorePicker", in: controller)
      XCTAssertEqual(back.configuration?.title, "返回")
      XCTAssertEqual(back.accessibilityLabel, "返回键盘")
      XCTAssertGreaterThanOrEqual(back.bounds.height, 44)
      XCTAssertLessThan(back.frame.midX, panel.bounds.midX)

      try button("moreCard-本地输入", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      for title in ["返回工具", "日期时间", "Unicode 码点"] {
        let card = try button("moreCard-" + title, in: controller)
        XCTAssertGreaterThan(card.bounds.width, 140)
        XCTAssertEqual(card.bounds.height, 48)
      }
      try button("moreCard-返回工具", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      // The 设置 switches sit on the root page rather than behind a card of their own, so the page
      // is taller than the keyboard and the last row is reached by scrolling. That is the panel's
      // design - the group header says the switches are there, which is what the local input modes
      // lacked when they were stranded in the same scroll. So this asks that every switch is laid
      // out inside the scrollable content rather than that every one is above the fold; a card
      // that fell outside the content is one nothing can scroll to.
      let scroll = try XCTUnwrap(descendants(panel).compactMap { $0 as? UIScrollView }.first)
      for title in ["繁体输出", "按键音", "按键振动", "全角输入", "振动强度"] {
        let card = try button("moreCard-" + title, in: controller)
        XCTAssertEqual(card.bounds.height, 48)
        let frame = card.convert(card.bounds, to: scroll)
        XCTAssertGreaterThanOrEqual(frame.minY, -0.5, "\(title) is above the scrollable content")
        XCTAssertLessThanOrEqual(frame.maxY, scroll.contentSize.height + 0.5,
                                 "\(title) is below the scrollable content")
      }
      // The entry cards above them stay on the first screen: the page opens on the tools, and a
      // tool nobody scrolls to is a tool nobody finds.
      for title in ["表情", "剪贴板历史", "AI 润色", "语音结果", "本地输入"] {
        let card = try button("moreCard-" + title, in: controller)
        XCTAssertLessThanOrEqual(card.convert(card.bounds, to: panel).maxY, panel.bounds.height,
                                 "\(title) was pushed off the first screen")
      }
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Compact tools \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      XCTAssertEqual(try button("moreCard-按键音", in: controller).accessibilityValue, "已开启")
      try button("moreCard-按键音", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(KeyboardFeedbackPreference.soundEnabled)
      XCTAssertEqual(try button("moreCard-按键音", in: controller).accessibilityValue, "已关闭")
      try button("moreCard-AI 润色", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 260 + KeyboardViewController.stripExtraHeight)
    }
  }

  func testFullWidthInputConvertsOnlyDirectPrintableASCIIAndPreservesComposition() throws {
    XCTAssertEqual(FullWidthInputPolicy.output(" A!~9", enabled: true), "　Ａ！～９")
    XCTAssertEqual(FullWidthInputPolicy.output("中文，🙂\n", enabled: true), "中文，🙂\n")
    XCTAssertEqual(FullWidthInputPolicy.output(" A!~9", enabled: false), " A!~9")

    let previousScheme = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previousScheme }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let letter = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 N" } as? UIButton)
    letter.sendActions(for: .primaryActionTriggered)
    let preedit = try button("preeditButton", in: controller).configuration?.title
    try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("moreCard-全角输入", in: controller).accessibilityValue, "已关闭")
    try button("moreCard-全角输入", in: controller).sendActions(for: .primaryActionTriggered)
    // The card switches the running keyboard; the setting that outlives it is the shared `character_width`.
    XCTAssertNil(KeyboardLayoutPreference.defaults.object(forKey: KeyboardLayoutPreference.fullWidthInputKey))
    XCTAssertEqual(try button("moreCard-全角输入", in: controller).accessibilityValue, "已开启")
    XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, preedit)
  }

  func testSchemePickerUsesCurrentSkinPalette() throws {
    preserveSharedTheme()
    for id in ["system", "paper", "night"] {
      XCTAssertTrue(GlobalThemePreference.save(id), id)
      let skin = KeyboardTheme.current
      XCTAssertEqual(skin.id, id)
      for style in [UIUserInterfaceStyle.light, .dark] {
        let traits = UITraitCollection(userInterfaceStyle: style)
        let picker = KeyboardSchemePickerView(selected: .nineKey, onSelect: { _ in }, onClose: {})
        picker.overrideUserInterfaceStyle = style
        picker.frame = CGRect(x: 0, y: 0, width: 414, height: 292)
        picker.layoutIfNeeded()
        func assertColor(_ actual: UIColor?, _ expected: UIColor, file: StaticString = #filePath, line: UInt = #line) {
          XCTAssertEqual(actual?.resolvedColor(with: traits), expected.resolvedColor(with: traits), file: file, line: line)
        }
        let buttons = descendants(picker).compactMap { $0 as? UIButton }
        XCTAssertFalse(buttons.contains { $0.accessibilityIdentifier == "schemePickerKeyboardTab" })
        XCTAssertFalse(buttons.contains { $0.accessibilityIdentifier == "schemePickerThemeTab" })
        let back = try XCTUnwrap(buttons.first { $0.accessibilityIdentifier == "closeSchemePicker" })
        let selected = try XCTUnwrap(buttons.first { $0.accessibilityIdentifier == "schemeCard-nineKey" })
        assertColor(picker.backgroundColor, skin.background)
        assertColor(back.tintColor, skin.accent)
        // 0.12 is the source's fill for a selected scheme card; the picker was brought onto it and
        // this expectation was left on the old 0.10.
        assertColor(selected.backgroundColor, skin.accent.withAlphaComponent(0.12))
        let labels = selected.subviews.compactMap { $0 as? UILabel }.filter { $0.text?.isEmpty == false }
        XCTAssertEqual(labels.count, 3)
        for label in labels {
          assertColor(label.textColor, skin.accent)
        }
      }
    }
  }

  func testSchemeGridHasFourColumnsAtNarrowAndWideSizes() throws {
    for width in [320.0, 414.0, 812.0] {
      let picker = KeyboardSchemePickerView(selected: .nineKey, onSelect: { _ in }, onClose: {})
      picker.frame = CGRect(x: 0, y: 0, width: width, height: width > 500 ? 216 : 260)
      picker.layoutIfNeeded()
      let cards = descendants(picker).filter { $0.accessibilityIdentifier?.hasPrefix("schemeCard-") == true }
      XCTAssertGreaterThanOrEqual(cards.count, 4)
      let firstRow = cards.prefix(4).map { $0.convert($0.bounds, to: picker) }
      for frame in firstRow {
        XCTAssertEqual(frame.minY, firstRow[0].minY, accuracy: 0.1)
        XCTAssertEqual(frame.width, firstRow[0].width, accuracy: 0.5)
        XCTAssertGreaterThanOrEqual(frame.width, 60)
        XCTAssertGreaterThanOrEqual(frame.height, 44)
        XCTAssertGreaterThanOrEqual(frame.minX, 14)
        XCTAssertLessThanOrEqual(frame.maxX, width - 14)
      }
      XCTAssertEqual(Set(firstRow.map { $0.minX }).count, 4)
      XCTAssertTrue(descendants(picker).compactMap { $0 as? KeyboardSkinMiniature }.isEmpty)
    }
  }

  func testSchemeCardsSelectAndKeepKeyboardHeight() throws {
    let enabled = InputSchemePreference.enabledSchemes
    defer { InputSchemePreference.enabledSchemes = enabled }
    InputSchemePreference.enabledSchemes = ChineseInputScheme.allCases
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for width in [320.0, 414.0] {
      InputSchemePreference.scheme = .nineKey
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let picker = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSchemePicker" })
      XCTAssertEqual(picker.bounds.height, 260 + KeyboardViewController.stripExtraHeight)
      for scheme in ChineseInputScheme.allCases {
        let card = try button("schemeCard-\(scheme.rawValue)", in: controller)
        XCTAssertGreaterThanOrEqual(card.bounds.width, 60)
        XCTAssertGreaterThanOrEqual(card.bounds.height, 62)
      }
      let lowestCard = try ChineseInputScheme.allCases
        .map { scheme -> CGFloat in
          let card = try button("schemeCard-\(scheme.rawValue)", in: controller)
          return card.convert(card.bounds, to: picker).maxY
        }
        .max() ?? 0
      XCTAssertGreaterThan(lowestCard, picker.bounds.height - 80,
                           "The scheme cards leave an oversized blank area below")
      XCTAssertEqual(try button("schemeCard-nineKey", in: controller).accessibilityValue, "已选中")
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Input scheme cards \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      try button("schemeCard-quanpin", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertNil(picker.superview)
      XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
      XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, ChineseInputScheme.quanpin.title)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 260 + KeyboardViewController.stripExtraHeight)
      try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "已选中")
      try button("closeSchemePicker", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardSchemePicker" })
      XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
    }
  }

  func testSchemePickerSwitchesEnglishAndSettingsWithSeparateThemeEntry() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 260)
    controller.view.layoutIfNeeded()
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    try button("schemeEnglishCard", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("schemeEnglishCard", in: controller).accessibilityValue, "已选中")
    XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "")
    try button("schemeCard-quanpin", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "schemePickerThemeTab" })
    XCTAssertFalse(descendants(controller.view).contains { $0.accessibilityIdentifier == "schemePickerKeyboardTab" })
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
    XCTAssertEqual(try button("schemeCard-quanpin", in: controller).accessibilityValue, "已选中")
    try button("schemePickerSettings", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSchemePickerView })
    XCTAssertTrue(descendants(controller.view).contains { $0.accessibilityIdentifier == "keyboardMorePicker" })
    try button("closeMorePicker", in: controller).sendActions(for: .primaryActionTriggered)
    try button("skinShortcut", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
    try button("closeSkinPicker", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(descendants(controller.view).contains { $0 is KeyboardSkinPickerView })
  }

  func testVisibleInputViewReflectsSoundPreference() throws {
    let defaults = KeyboardFeedbackPreference.defaults
    let previous = defaults.object(forKey: KeyboardFeedbackPreference.soundKey)
    defer {
      if let previous { defaults.set(previous, forKey: KeyboardFeedbackPreference.soundKey) }
      else { defaults.removeObject(forKey: KeyboardFeedbackPreference.soundKey) }
    }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    let inputView = try XCTUnwrap(controller.inputView as? KeyboardInputView)
    XCTAssertTrue(controller.view === inputView)
    defaults.set(true, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertTrue(inputView.enableInputClicksWhenVisible)
    defaults.set(false, forKey: KeyboardFeedbackPreference.soundKey)
    XCTAssertFalse(inputView.enableInputClicksWhenVisible)
  }

  func testShiftIsDiscoverableAndSwitchesToEnglishCapitalization() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .quanpin
    defer { InputSchemePreference.scheme = previous }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()
    let shift = try button("shiftButton", in: controller)
    XCTAssertFalse(shift.isHidden)
    XCTAssertEqual(shift.accessibilityLabel, "切换到英文大写")
    XCTAssertEqual(try button("inputModeSwitchButton", in: controller).isHidden,
                   !controller.needsInputModeSwitchKey)
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "下一字母")
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "英文输入")
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "开启")
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(shift.accessibilityValue, "关闭")
  }

  func testShiftEntersHelpcodeWhileComposingShuangpin() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .shuangpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 390,
      height: 260 + KeyboardViewController.stripExtraHeight)

    func letterKey(_ letter: String) throws -> UIButton {
      try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityLabel == "字母 \(letter)" || $0.accessibilityLabel == "大写 \(letter)"
      } as? UIButton)
    }

    try letterKey("N").sendActions(for: .primaryActionTriggered)
    try letterKey("I").sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try button("preeditButton", in: controller).configuration?.title?.isEmpty ?? true)

    let shift = try button("shiftButton", in: controller)
    shift.sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try button("bottomLanguageKey", in: controller).accessibilityValue, "中文输入")
    XCTAssertEqual(try letterKey("H").accessibilityLabel, "大写 H")

    try letterKey("H").sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(try letterKey("H").accessibilityLabel, "字母 H")
    XCTAssertEqual(shift.accessibilityValue, "关闭")
  }

  func testPressFeedbackPreservesLayoutAndResetsAfterInterruption() {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 414, height: 260))
    let controller = UIViewController()
    window.rootViewController = controller
    window.isHidden = false
    defer { window.isHidden = true }
    let key = KeyboardKeyButton(frame: CGRect(x: 20, y: 20, width: 44, height: 48))
    controller.view.addSubview(key)
    let bounds = key.bounds
    let center = key.center
    for _ in 0..<10 {
      key.isHighlighted = true
      XCTAssertEqual(key.bounds, bounds)
      XCTAssertEqual(key.center, center)
      XCTAssertEqual(key.transform.isIdentity, UIAccessibility.isReduceMotionEnabled)
      key.isHighlighted = false
      XCTAssertTrue(key.transform.isIdentity)
    }
    key.isHighlighted = true
    key.isEnabled = false
    XCTAssertTrue(key.transform.isIdentity)
    key.isEnabled = true
    key.isHighlighted = false
    key.isHighlighted = true
    key.removeFromSuperview()
    XCTAssertTrue(key.transform.isIdentity)
    XCTAssertTrue(key.layer.animationKeys()?.isEmpty ?? true)
  }

  private func descendants(_ view: UIView) -> [UIView] {
    [view] + view.subviews.flatMap { descendants($0) }
  }

  /// Clear the whole composition the way the keyboard does it: holding delete past the repeat
  /// threshold wipes the composition instead of deleting one more character. There is no separate
  /// clear key to press -- the tests used to reach for a `nineKeyClear` that no keyboard has ever
  /// built, so they failed looking it up before asserting anything.
  private func clearComposition(in controller: KeyboardViewController) throws {
    let delete = try button("nineKeyDelete", in: controller)
    let begin = try XCTUnwrap(delete.actions(forTarget: controller, forControlEvent: .touchDown)?.first)
    controller.perform(NSSelectorFromString(begin))
    controller.perform(NSSelectorFromString("repeatBackspace"))
  }

  private func button(_ identifier: String, in controller: KeyboardViewController) throws -> UIButton {
    // Name the identifier: a bare "expected non-nil value of type UIButton" from a test that
    // looks up a dozen keys says nothing about which one went missing.
    try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == identifier
    } as? UIButton, "No button with accessibility identifier \(identifier).")
  }

  func testLetterFacesAreUppercaseUntilEnglishTakesOver() throws {
    // Chinese and Japanese romanization use uppercase faces as a visual convention. The engine
    // still receives lowercase input, and English alone lets Shift determine what is inserted.
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.quanpin, .japanese] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(
        x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()

      let a = try XCTUnwrap(
        descendants(controller.view).first { $0.accessibilityLabel == "字母 A" } as? UIButton)
      XCTAssertEqual(a.configuration?.title, "A", "\(scheme) should draw uppercase letter faces")
      XCTAssertEqual(a.accessibilityLabel, "字母 A", "The pinyin face is not an uppercase keystroke")

      a.sendActions(for: .primaryActionTriggered)
      XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidate-1" })

      try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(a.configuration?.title, "a", "English should return to lowercase")

      try button("shiftButton", in: controller).sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(a.configuration?.title, "A", "English Shift should uppercase the face")
      XCTAssertEqual(a.accessibilityLabel, "大写 A", "English Shift should be announced as uppercase")
    }
  }

  func testShortcutsYieldToCandidatesWithoutMovingKeys() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousScript = ChineseOutputPreference.usesTraditional
    // The chip text is the subject here -- no ordinal, no stray whitespace. A gloss adds a second
    // line to that text and would fail the assertion for a reason this test is not about.
    let previousGloss = CandidateGlossPreference.enabled
    InputSchemePreference.scheme = .nineKey
    ChineseOutputPreference.usesTraditional = false
    CandidateGlossPreference.enabled = false
    defer {
      InputSchemePreference.scheme = previousScheme
      ChineseOutputPreference.usesTraditional = previousScript
      CandidateGlossPreference.enabled = previousGloss
    }
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      let toolbar = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" })
      XCTAssertFalse(toolbar.isHidden)
      let brand = try XCTUnwrap(descendants(toolbar).first { $0.accessibilityIdentifier == "keyboardBrandIcon" } as? UIImageView)
      XCTAssertNotNil(brand.image)
      XCTAssertEqual(brand.bounds.width, 28, accuracy: 0.1)
      XCTAssertEqual(brand.bounds.height, 28, accuracy: 0.1)
      XCTAssertEqual(brand.image?.renderingMode, .alwaysTemplate)
      let brandSlot = try XCTUnwrap(brand.superview)
      XCTAssertGreaterThanOrEqual(brand.frame.minX, 6)
      XCTAssertGreaterThanOrEqual(brandSlot.bounds.width - brand.frame.maxX, 6)
      XCTAssertLessThan(brand.convert(brand.bounds, to: toolbar).maxX,
                        try button("schemeButton", in: controller).convert(try button("schemeButton", in: controller).bounds, to: toolbar).minX)
      for id in ["layoutShortcut", "schemeButton", "emojiShortcut",
                 "skinShortcut", "moreShortcut", "dismissShortcut"] {
        let control = try button(id, in: controller)
        XCTAssertGreaterThanOrEqual(control.bounds.width, 44)
        XCTAssertGreaterThanOrEqual(control.bounds.height, 38)
      }
      XCTAssertNil(try button("skinShortcut", in: controller).menu)
      XCTAssertTrue(try button("layoutVoiceShortcut", in: controller).isHidden)
      try button("moreShortcut", in: controller).sendActions(for: .primaryActionTriggered)
      try button("moreCard-繁体输出", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertTrue(ChineseOutputPreference.usesTraditional)
      try button("moreCard-繁体输出", in: controller).sendActions(for: .primaryActionTriggered)
      XCTAssertFalse(ChineseOutputPreference.usesTraditional)
      try button("closeMorePicker", in: controller).sendActions(for: .primaryActionTriggered)
      let key = try button("nineKey6", in: controller)
      let frame = key.convert(key.bounds, to: controller.view)
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Keyboard shortcuts \(Int(width))pt"
      attachment.lifetime = .keepAlways
      add(attachment)
      for digit in "64426" { try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered) }
      controller.view.layoutIfNeeded()
      XCTAssertTrue(toolbar.isHidden)
      // The chip carries the candidate and nothing else: a touch keyboard has no number row, so a
      // leading ordinal is noise that reads as part of the word.
      let chip = try XCTUnwrap(button("candidate-1", in: controller).configuration?.title)
      XCTAssertTrue(chip.contains("你好"))
      XCTAssertFalse(chip.contains(where: \.isNumber), chip)
      XCTAssertEqual(chip, chip.trimmingCharacters(in: .whitespaces), chip)
      XCTAssertEqual(key.convert(key.bounds, to: controller.view), frame)
      try clearComposition(in: controller)
      controller.view.layoutIfNeeded()
      XCTAssertFalse(toolbar.isHidden)
      XCTAssertEqual(key.convert(key.bounds, to: controller.view), frame)
    }
  }

  func testNewCandidatesAndPagesReturnToLeadingCandidate() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.nineKey, .quanpin] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 320, height: 260 + KeyboardViewController.stripExtraHeight)
      func type(_ text: String) throws {
        for character in text {
          let key: UIButton
          if scheme == .nineKey {
            key = try button("nineKey\(character)", in: controller)
          } else {
            key = try XCTUnwrap(descendants(controller.view).first {
              $0.accessibilityLabel == "字母 \(String(character).uppercased())"
            } as? UIButton)
          }
          key.sendActions(for: .primaryActionTriggered)
        }
        controller.view.layoutIfNeeded()
      }
      try type(scheme == .nineKey ? "6" : "n")
      let candidate = try button("candidate-1", in: controller)
      var ancestor = candidate.superview
      while ancestor != nil && !(ancestor is UIScrollView) { ancestor = ancestor?.superview }
      let scroll = try XCTUnwrap(ancestor as? UIScrollView)
      XCTAssertFalse(scroll.delaysContentTouches)
      XCTAssertTrue(scroll.canCancelContentTouches)
      XCTAssertTrue(scroll.touchesShouldCancel(in: candidate))
      XCTAssertGreaterThan(scroll.contentSize.width, scroll.bounds.width + 40)
      scroll.setContentOffset(CGPoint(x: 40, y: 0), animated: false)
      try type(scheme == .nineKey ? "4" : "i")
      XCTAssertEqual(scroll.contentOffset.x, 0, accuracy: 0.5)
      let leading = try button("candidate-1", in: controller)
      XCTAssertGreaterThanOrEqual(leading.convert(leading.bounds, to: scroll).minX, 0)

      // The strip shows a page (nine by default); everything past them is reached by expanding rather than by paging
      // nine at a time, which for a query answering with hundreds left the tail unreachable.
      let expand = try button("expandCandidates", in: controller)
      XCTAssertFalse(expand.isHidden)
      expand.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      let panel = try XCTUnwrap(
        descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
      let chips = descendants(panel).compactMap { $0.accessibilityIdentifier }
        .filter { $0.hasPrefix("panelCandidate-") }
      XCTAssertGreaterThan(chips.count, CandidatePageSizePreference.defaultSize)
      let close = try XCTUnwrap(
        descendants(panel).first { $0.accessibilityIdentifier == "closeCandidatePanel" } as? UIButton)
      close.sendActions(for: .primaryActionTriggered)
      controller.view.layoutIfNeeded()
      XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "candidatePanel" })
    }
  }

  func testSymbolKeyOpensAPanelInsteadOfAMenu() throws {
    let previous = InputSchemePreference.scheme
    // The panel leads with 最近 once anything was picked, which shifts every category index; this test itself records `@`, so a second run on the same device would otherwise start from its own leftovers.
    let defaults = KeyboardFeedbackPreference.defaults
    let previousRecents = defaults.stringArray(forKey: KeyboardSymbolRecents.key)
    defer {
      InputSchemePreference.scheme = previous
      if let previousRecents { defaults.set(previousRecents, forKey: KeyboardSymbolRecents.key) }
      else { defaults.removeObject(forKey: KeyboardSymbolRecents.key) }
    }
    defaults.removeObject(forKey: KeyboardSymbolRecents.key)
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 393, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()

    let key = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityLabel == "符号" } as? UIButton)
    XCTAssertNil(key.menu, "这颗键不再弹菜单")
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })

    key.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    let panel = try XCTUnwrap(
      descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
    XCTAssertEqual(panel.bounds.size, controller.view.bounds.size, "面板整块盖住键盘区")
    let last = KeyboardSymbolPanelView.categories.count - 1
    for identifier in ["symbolCategory_0", "symbolCategory_\(last)", "closeSymbolPanel", "symbolLockKey", "symbolDeleteKey"] {
      XCTAssertNotNil(descendants(panel).first { $0.accessibilityIdentifier == identifier }, identifier)
    }
    let grid = try XCTUnwrap(descendants(panel).first { $0.accessibilityIdentifier == "symbolGrid" } as? UIScrollView)
    XCTAssertGreaterThan(grid.contentSize.height, grid.bounds.height, "符号多到一屏放不下时要能往下滚")

    let network = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "symbolCategory_\(last)" } as? UIButton)
    network.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNotNil(descendants(panel).first { $0.accessibilityIdentifier == "symbolKey_http://" })

    let symbol = try XCTUnwrap(
      descendants(panel).first { $0.accessibilityIdentifier == "symbolKey_@" } as? UIButton)
    symbol.sendActions(for: .primaryActionTriggered)
    controller.view.layoutIfNeeded()
    XCTAssertNil(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardSymbolPanel" })
  }

  func testKeyLayoutsKeepNineKeyHeight() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for width in [320.0, 414.0] {
      InputSchemePreference.scheme = .nineKey
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      let reference = try button("nineKey6", in: controller).bounds.height
      // Handwriting has a taller canvas, covered by HandwritingTests. The kana nine-key panel
      // brings its own ⌫ / 空白 / 改行 and the action row is collapsed underneath it, so the
      // shared return key has no height to keep there; JapaneseNineKeyTests covers that layout.
      for scheme in ChineseInputScheme.allCases
      where scheme != .handwriting && scheme != .japaneseNineKey {
        InputSchemePreference.scheme = scheme
        controller.viewWillAppear(false)
        for symbols in [false, true] {
          if symbols { try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered) }
          controller.view.layoutIfNeeded()
          if scheme == .zhuyin && !symbols {
            // The Dachen layout fits four rows of keys above the action row into the same keyboard height, as the system Zhuyin keyboard does, so its rows are shorter than the three letter rows and all of them stay the same height.
            let returnHeight = try button("returnKey", in: controller).bounds.height
            XCTAssertLessThan(returnHeight, reference)
            XCTAssertGreaterThanOrEqual(returnHeight, 30)
            XCTAssertEqual(try button("zhuyinKeyq", in: controller).bounds.height, returnHeight, accuracy: 0.5)
          } else {
            XCTAssertEqual(try button("returnKey", in: controller).bounds.height, reference, accuracy: 0.5)
          }
          XCTAssertEqual(controller.view.constraints.first { $0.identifier == "keyboardHeight" }?.constant, 260 + KeyboardViewController.stripExtraHeight)
          if !symbols && [.nineKey, .quanpin].contains(scheme) {
            let selector = try button("schemeButton", in: controller)
            XCTAssertGreaterThanOrEqual(selector.bounds.width, 44)
            XCTAssertNil(selector.configuration?.title)
            XCTAssertNotNil(selector.configuration?.image)
            XCTAssertEqual(selector.accessibilityLabel, "选择输入方案")
            for id in ["layoutShortcut", "emojiShortcut", "skinShortcut", "moreShortcut", "dismissShortcut"] {
              XCTAssertGreaterThanOrEqual(try button(id, in: controller).bounds.width, 44)
            }
            if width == 320 {
              let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
                controller.view.layer.render(in: context.cgContext)
              })
              attachment.name = "Narrow \(scheme.rawValue) keyboard and toolbar"
              attachment.lifetime = .keepAlways
              add(attachment)
            }
          }
          let punctuation = try button("quickPunctuationKey", in: controller)
          XCTAssertEqual(punctuation.isHidden, symbols || [.nineKey, .japaneseNineKey, .handwriting].contains(scheme))
          if !punctuation.isHidden {
            XCTAssertEqual(punctuation.configuration?.title, scheme.isJapanese ? "、" : scheme.writesAsciiPunctuation ? "," : "，")
            XCTAssertEqual(punctuation.bounds.width, 44, accuracy: 0.5)
            XCTAssertGreaterThanOrEqual(try button("spaceKey", in: controller).bounds.width, 79.2)
            XCTAssertEqual(punctuation.menu?.children.count, 7)
          }
          // The Japanese nine-key owns its delete key inside the kana grid, including its digit
          // layer; the shared action-row delete remains hidden in both states.
          XCTAssertEqual(try button("symbolDeleteKey", in: controller).isHidden, !symbols || scheme == .japaneseNineKey)
          if !symbols && ![.nineKey, .japaneseNineKey, .handwriting, .zhuyin].contains(scheme) {
            let delete = try button("letterDeleteKey", in: controller)
            let shift = try button("shiftButton", in: controller)
            // Korean keys are named by the jamo they type.
            let letterLabel = { (letter: String) in
              "字母 \(scheme.isKorean ? DubeolsikKeyLayout.keycap(for: letter, shifted: false) ?? letter : letter)"
            }
            let m = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == letterLabel("M") } as? UIButton)
            XCTAssertEqual(delete.superview, m.superview)
            XCTAssertGreaterThan(delete.frame.minX, m.frame.maxX)
            XCTAssertEqual(delete.bounds.width, 44, accuracy: 0.5)
            XCTAssertEqual(shift.bounds.width, 44, accuracy: 0.5)
            XCTAssertEqual(delete.bounds.height, reference, accuracy: 0.5)
            XCTAssertLessThanOrEqual(delete.convert(delete.bounds, to: controller.view).maxX, width - 4.5)
            for label in ["Q", "A", "Z", "P", "L", "M"] {
              let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == letterLabel(label) } as? UIButton)
              XCTAssertEqual(key.bounds.height, reference, accuracy: 0.5)
              let frame = key.convert(key.bounds, to: controller.view)
              XCTAssertGreaterThanOrEqual(frame.minX, 4.5)
              XCTAssertLessThanOrEqual(frame.maxX, controller.view.bounds.width - 4.5, "\(scheme) \(label) frame \(frame)")
            }
          }
          if !symbols && scheme == .japaneseNineKey {
            XCTAssertEqual(try button("japaneseKana0", in: controller).bounds.height, reference, accuracy: 0.5)
          }
          if symbols { try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered) }
        }
      }
    }
  }

  func testLocalModeLayouts() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.nineKey, .shuangpin] {
      InputSchemePreference.scheme = scheme
      for trigger in ["U", "T", "J"] {
        let controller = KeyboardViewController()
        controller.loadViewIfNeeded()
        controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
        controller.openLocalInputMode(trigger)
        controller.view.layoutIfNeeded()
        let returnKey = try button("returnKey", in: controller)
        for label in ["Q", "A", "Z", "P", "L", "M"] {
          let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(label)" } as? UIButton)
          XCTAssertEqual(key.bounds.height, returnKey.bounds.height, accuracy: 0.5)
          XCTAssertGreaterThanOrEqual(key.bounds.height, 44)
          XCTAssertTrue(key.subviews.compactMap { $0 as? UILabel }.filter { $0.font.pointSize == 9 }.allSatisfy { $0.isHidden })
        }
        XCTAssertFalse(try button("exitLocalModeButton", in: controller).isHidden)
        let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
          controller.view.layer.render(in: context.cgContext)
        })
        attachment.name = "Local mode \(scheme) \(trigger)"
        attachment.lifetime = .keepAlways
        add(attachment)
        try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        XCTAssertNotNil(descendants(controller.view).first { $0.accessibilityLabel == "符号 \\" })
        XCTAssertNil(descendants(controller.view).first { $0.accessibilityLabel == "符号 、" })
        try button("exitLocalModeButton", in: controller).sendActions(for: .primaryActionTriggered)
        controller.view.layoutIfNeeded()
        XCTAssertTrue(try button("exitLocalModeButton", in: controller).isHidden)
        XCTAssertFalse(try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "keyboardShortcutBar" }).isHidden)
        XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, scheme.title)
        XCTAssertEqual(try XCTUnwrap(button("nineKey6", in: controller).superview).isHidden, scheme != .nineKey)
      }
    }
  }

  func testJapaneseSentenceConversionAndMemory() throws {
    func residentBytes() -> UInt64 {
      var info = mach_task_basic_info_data_t()
      var count = mach_msg_type_number_t(MemoryLayout.size(ofValue: info) / MemoryLayout<integer_t>.size)
      let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
          task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
        }
      }
      return result == KERN_SUCCESS ? info.resident_size : 0
    }
    func footprint() -> UInt64 {
      var info = task_vm_info_data_t()
      var count = mach_msg_type_number_t(MemoryLayout.size(ofValue: info) / MemoryLayout<integer_t>.size)
      let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
          task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
      }
      return result == KERN_SUCCESS ? info.phys_footprint : 0
    }
    let footprintBefore = footprint()
    let before = residentBytes()
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToJapanese()
    var snapshot = bridge.cancel()
    for letter in "watashihanihonjindesu" { snapshot = bridge.handleCharacter(String(letter)) }
    print("Japanese sentence candidates: \(snapshot.candidates.prefix(5))")
    XCTAssertTrue(snapshot.candidates.contains { $0.contains("日本人") && $0.contains("です") })
    XCTAssertNil(snapshot.diagnosticText)
    let after = residentBytes()
    let footprintAfter = footprint()
    print("Japanese physical footprint: before=\(footprintBefore), after=\(footprintAfter) bytes")
    XCTAssertGreaterThan(footprintBefore, 0)
    XCTAssertLessThan(footprintAfter > footprintBefore ? footprintAfter - footprintBefore : 0, 16 * 1024 * 1024,
      "The keyboard must not copy the full sentence model into private memory")
    var usage = rusage()
    getrusage(RUSAGE_SELF, &usage)
    print("Japanese model memory: before=\(before), after=\(after), peak=\(usage.ru_maxrss) bytes")
    let index = try XCTUnwrap(snapshot.candidates.firstIndex { $0.contains("日本人") && $0.contains("です") })
    XCTAssertEqual(bridge.selectCandidate(at: UInt(index)).commitText, snapshot.candidates[index])
    _ = bridge.switch(toShuangpin: false)
    _ = bridge.openLocalMode("R")
    for letter in "nihon" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("日本"))
    _ = bridge.cancel()
    XCTAssertFalse(bridge.isInLocalMode)
  }

  func testJapaneseSentenceCandidateStrip() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousScript = ChineseOutputPreference.usesTraditional
    InputSchemePreference.scheme = .japanese
    ChineseOutputPreference.usesTraditional = true
    defer {
      InputSchemePreference.scheme = previousScheme
      ChineseOutputPreference.usesTraditional = previousScript
    }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    for letter in "watashihanihonjindesu" {
      let key = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityLabel == "字母 \(String(letter).uppercased())" } as? UIButton)
      key.sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("私は日本人です"))
    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    })
    attachment.name = "Japanese full sentence keyboard"
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  // Putting the keyboard away must hand the engine's dictionary access back: iOS terminates an
  // extension suspended while holding a lock in the App Group container and reports it as
  // 0xdead10cc. Resuming has to rebuild a session that still converts.
  // The presentation cycle is what the device actually does: show, type, put away, show again.
  // Releasing the dictionary access on dismissal must not leave the next presentation unable to
  // convert, and a dropped keystroke would still have played its click.
  func testKeyboardStillConvertsAfterBeingPutAwayAndShownAgain() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.applyInputContext(keyboardType: .default, documentIdentifier: UUID())
    for round in 0..<2 {
      controller.viewWillAppear(false)
      controller.view.layoutIfNeeded()
      for letter in "nihao" {
        let key = try XCTUnwrap(descendants(controller.view).first {
          $0.accessibilityLabel == "字母 \(String(letter).uppercased())"
        } as? UIButton)
        key.sendActions(for: .primaryActionTriggered)
      }
      controller.view.layoutIfNeeded()
      XCTAssertEqual(try button("preeditButton", in: controller).configuration?.title, "nihao",
                     "round \(round): the keystrokes never reached the engine")
      XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"),
                    "round \(round): no Chinese candidate")
      controller.viewWillDisappear(false)
    }
  }

  func testSuspendReleasesDictionaryAccessAndResumeStillConverts() throws {
    let bridge = MetasequoiaInputSessionBridge()
    var snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"))
    XCTAssertFalse(bridge.suspendDictionarySession(), "an open composition still owns the session")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    XCTAssertTrue(bridge.suspendDictionarySession(), "suspending twice is not an error")
    // Pausing learning would leave the session, and its dictionary access, very much alive. Only a
    // destroyed session hands the access back, and a destroyed session cannot answer a keystroke.
    let whileSuspended = bridge.handleCharacter("n")
    XCTAssertNotNil(whileSuspended.diagnosticText, "the session is paused, not released")
    XCTAssertTrue(whileSuspended.candidates.isEmpty)
    try bridge.resumeDictionarySession()
    snapshot = bridge.cancel()
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertNil(snapshot.diagnosticText)
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session lost its dictionaries")
  }

  // Nine-key is engine-session state, not one of the preferences the prepared options carry, so a
  // session rebuilt after a dismissal starts back on the 26-key layout. The keyboard keeps sending
  // the digits its layout produces and the engine, expecting letters, answers with nothing at all:
  // no preedit, no candidates, no diagnostic.
  func testNineKeySurvivesTheSessionBeingReleasedAndRebuilt() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToNineKey()
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "nine-key never reached the engine")
    _ = bridge.cancel()
    XCTAssertTrue(bridge.suspendDictionarySession())
    try bridge.resumeDictionarySession()
    snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertFalse(snapshot.preedit.isEmpty, "the rebuilt session ignored the digits entirely")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot nine-key mode")
  }

  // A dismissal is not the only rebuild. Reading the personal dictionary hands the shared
  // dictionary lease back the same way, and the keyboard refreshes that list on every appearance:
  // the nine-key layout the host kept drawing was sitting on a 26-key engine from the first
  // refresh onwards, which is why typing only started working after a trip through 26 keys.
  func testNineKeySurvivesTheSessionRebuiltForDictionaryMaintenance() throws {
    let bridge = MetasequoiaInputSessionBridge()
    _ = bridge.switchToNineKey()
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "nine-key never reached the engine")
    _ = bridge.cancel()
    _ = try bridge.personalEntries(atOffset: 0)
    snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertFalse(snapshot.preedit.isEmpty, "the rebuilt session ignored the digits entirely")
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the rebuilt session forgot nine-key mode")
  }

  // A session is created from the shared document, so a scheme picked in an earlier session has to
  // reach it. Nothing on iOS ever wrote the touch layout there, so a cold keyboard created its
  // session on 26 keys no matter what the user had picked, and only the live session knew better.
  func testTouchSchemeReachesTheDocumentTheNextSessionIsCreatedFrom() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-scheme-persist-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    var first: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(try XCTUnwrap(first).setTouchKeyboardScheme(
      .nineKey, enabledSchemes: [.quanpin, .nineKey]))
    first = nil

    let next = MetasequoiaInputSessionBridge(stateRoot: state)
    var snapshot = next.cancel()
    for digit in "64426" { snapshot = next.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the new session did not start on nine-key")
    XCTAssertEqual(try XCTUnwrap(next.sharedPreferences)["touch_keyboard_layout"] as? String,
                   "nine_key")
  }

  // The scheme is not the only thing the keyboard itself can change. Every one of these used to
  // live on the session alone, so reloading the settings app's document put its older value back
  // over the choice the user had just made, and a new session never saw the choice at all.
  func testKeyboardSideSelectionsReachTheSharedDocument() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-selection-persist-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    var first: MetasequoiaInputSessionBridge? = MetasequoiaInputSessionBridge(stateRoot: state)
    let bridge = try XCTUnwrap(first)
    XCTAssertTrue(bridge.updateTheme(GlobalThemePreference.selecting("night")))
    XCTAssertTrue(bridge.setTraditionalChineseOutput(true))
    XCTAssertTrue(bridge.persistTouchKeyboardGeometry(
      keySpacing: 5, rowSpacing: 9, heightAdjustment: 12, voiceEnabled: true))
    // A drag reports on every gesture frame and only previews on the live session; taking a file
    // lock that often is what the separate commit above is for.
    XCTAssertTrue(bridge.setTouchKeyboardGeometry(
      keySpacing: 3, rowSpacing: 4, heightAdjustment: -5, voiceEnabled: false))
    first = nil

    let next = MetasequoiaInputSessionBridge(stateRoot: state)
    let preferences = try XCTUnwrap(next.sharedPreferences)
    XCTAssertEqual(preferences["global_theme"] as? String, "night")
    XCTAssertEqual(preferences["traditional_chinese_output"] as? Bool, true)
    XCTAssertEqual(preferences["touch_key_spacing_tenths"] as? Int, 50)
    XCTAssertEqual(preferences["touch_row_spacing_tenths"] as? Int, 90)
    XCTAssertEqual(preferences["touch_keyboard_height_adjustment"] as? Int, 12)
    XCTAssertEqual(preferences["touch_voice_shortcut"] as? Bool, true)
  }

  // Reloading the settings app's document replaced the session's preferences wholesale, dropping
  // the two values this host sets for itself along with them.
  func testSharedPreferenceReloadKeepsTheHostSessionContract() async throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-reload-overrides-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.nineKey, enabledSchemes: [.quanpin, .nineKey]))
    let reloaded = expectation(description: "shared preferences reloaded")
    bridge.reloadSharedPreferences { accepted in
      XCTAssertTrue(accepted)
      reloaded.fulfill()
    }
    await fulfillment(of: [reloaded], timeout: 15)

    let preferences = try XCTUnwrap(bridge.sharedPreferences)
    XCTAssertEqual(preferences["candidate_page_size"] as? Int, 9)
    XCTAssertEqual(preferences["default_ime_mode"] as? String, "chinese")
    XCTAssertEqual(preferences["touch_keyboard_layout"] as? String, "nine_key")
    var snapshot = bridge.cancel()
    for digit in "64426" { snapshot = bridge.handleCharacter(String(digit)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"), "the reloaded document lost nine-key")
  }

  /// The key face has to name what the keys actually produce.
  ///
  /// The hints used to come from a copy of the keymap kept in this target, and that copy
  /// had lost Xiaohe's `uai` from K - the key that types 乖 carried no sign of it. They now
  /// come from the Engine's own profile through the shared ABI, so this drives a real
  /// session per profile and checks the face against the key that produced candidates.
  func testShuangpinKeyHintsComeFromTheProfileTheSessionRuns() throws {
    let bridge = MetasequoiaInputSessionBridge()
    // The key each profile puts `uai` on, reached with the `g` initial.
    for (profile, key) in [("xiaohe", "K"), ("ziranma", "Y"), ("shoudao", "G"), ("microsoft", "Y")] {
      _ = bridge.switch(toShuangpinProfile: profile)
      _ = bridge.cancel()
      let hints = bridge.shuangpinKeyHints()
      XCTAssertGreaterThanOrEqual(hints.count, 26, "\(profile) labelled only \(hints.count) keys")
      XCTAssertEqual(hints[key]?.contains("uai"), true,
                     "\(profile) key \(key) reads \(hints[key] ?? "nothing") but types uai")
      _ = bridge.handleCharacter("g")
      XCTAssertFalse(bridge.handleCharacter(key.lowercased()).candidates.isEmpty,
                     "\(profile) g\(key.lowercased()) produced no candidates")
      _ = bridge.cancel()
    }
    // Both units K carries, not just the first one.
    _ = bridge.switch(toShuangpinProfile: "xiaohe")
    XCTAssertEqual(bridge.shuangpinKeyHints()["K"], "ing uai")
    // Initials and finals stay on their own side of the separator.
    XCTAssertEqual(bridge.shuangpinKeyHints()["V"], "zh / ui ü")
    // A session that is not running double pinyin gets no face rather than the default one's.
    _ = bridge.switch(toShuangpin: false)
    XCTAssertTrue(bridge.shuangpinKeyHints().isEmpty, "Quanpin was labelled with a double-pinyin face")
    _ = bridge.cancel()
  }

  func testAdditionalShuangpinProfilesAndKeyHints() throws {
    let bridge = MetasequoiaInputSessionBridge()
    for (profile, input) in [("ziranma", "nihk"), ("microsoft", "nihk"), ("shoudao", "nihd"), ("xiaohe", "nihc")] {
      _ = bridge.switch(toShuangpinProfile: profile)
      var snapshot = bridge.cancel()
      for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
      XCTAssertTrue(snapshot.candidates.contains("你好"), profile)
      _ = bridge.cancel()
    }
    _ = bridge.switch(toShuangpinProfile: "microsoft")
    XCTAssertTrue(bridge.shuangpinKeyHints()[";"]?.contains("ing") == true)
    _ = bridge.handleCharacter("n")
    XCTAssertFalse(bridge.handleCharacter(";").candidates.isEmpty)
    _ = bridge.cancel()
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    for scheme in [ChineseInputScheme.wubi, .japanese, .microsoft] {
      InputSchemePreference.scheme = scheme
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: 260 + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      XCTAssertEqual(try button("microsoftFinalKey", in: controller).isHidden, scheme != .microsoft)
      let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
        controller.view.layer.render(in: context.cgContext)
      })
      attachment.name = "Keyboard scheme \(scheme)"
      attachment.lifetime = .keepAlways
      add(attachment)
    }
  }

  func testAdditionalEngineSchemesAndLocalProviders() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-schemes-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    _ = bridge.switchToWubi()
    var snapshot = bridge.handleCharacter("a")
    XCTAssertTrue(snapshot.candidates.contains("工"))
    XCTAssertEqual(bridge.selectCandidate(at: UInt(snapshot.candidates.firstIndex(of: "工")!)).commitText, "工")
    _ = bridge.switchToJapanese()
    for letter in "nihon" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("にほん"))
    let japaneseRows = try XCTUnwrap(try bridge.allCandidates()["candidates"] as? [[String: Any]])
    XCTAssertTrue(japaneseRows.contains { $0["text"] as? String == "ニホン" })
    _ = bridge.cancel()
    snapshot = bridge.handleCharacter("a")
    XCTAssertTrue(snapshot.candidates.contains("亜"))
    _ = bridge.cancel()
    _ = bridge.switch(toShuangpin: false)
    for letter in "nihao" { snapshot = bridge.handleCharacter(String(letter)) }
    XCTAssertTrue(snapshot.candidates.contains("你好"))
    _ = bridge.cancel()
    for (trigger, input) in [("K", "yyds"), ("Y", "hello"), ("E", "smile"), ("M", "kaixin"), ("R", "nihon")] {
      _ = bridge.openLocalMode(trigger)
      XCTAssertTrue(bridge.isInLocalMode)
      for letter in input { snapshot = bridge.handleCharacter(String(letter)) }
      XCTAssertNil(snapshot.diagnosticText, "Provider \(trigger)")
      XCTAssertFalse(snapshot.candidates.isEmpty, "Provider \(trigger)")
      // Temporary English completes what was typed. This used to ask for more than one answer,
      // which counted rows in the pinned dictionary rather than describing the product: the
      // release `english.db` now holds exactly one word beginning with "hello", so the count
      // moved while the behaviour did not.
      if trigger == "Y" {
        XCTAssertTrue(snapshot.candidates.contains { $0.lowercased().hasPrefix(input) },
                      "Provider Y answered \(snapshot.candidates) for \(input)")
      }
      if trigger == "K" { XCTAssertTrue(snapshot.candidates.contains("永远滴神")) }
      _ = bridge.cancel()
    }
  }

  func testSpellingStripReusesItsButtonsBetweenKeystrokes() throws {
    let previousScheme = InputSchemePreference.scheme
    let previousEnabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = previousEnabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 292)
    controller.viewWillAppear(false)
    controller.view.layoutIfNeeded()

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    try button("nineKey4", in: controller).sendActions(for: .primaryActionTriggered)
    let strip = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "nineKeySpellingStrip"
    } as? UIScrollView)
    let first = descendants(strip).compactMap { $0 as? UIButton }.filter { !$0.isHidden }
    XCTAssertFalse(first.isEmpty)

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    let second = descendants(strip).compactMap { $0 as? UIButton }.filter { !$0.isHidden }
    XCTAssertFalse(second.isEmpty)
    for (before, after) in zip(first, second) {
      XCTAssertTrue(before === after)
    }
    XCTAssertTrue(second.contains { ($0.accessibilityIdentifier ?? "").hasPrefix("nineKeySpelling_") })
  }

  /// Nine-key digits reach the English candidates too.
  ///
  /// From a report: typing 65 on nine-key wanted `ok` and the strip had nothing. The digits are letter groups, so the mixed-English path has to see them the same way the Chinese one does; nothing in the host mapped them, and the failure looked like the word being missing from the dictionary rather than like the layout never asking.
  ///
  /// The shared `mixed_input.minimum_prefix` default is now 5, matching Windows, so a two-digit `ok` no longer reaches English at all; the check types the five digits of `hello` instead.
  func testNineKeyOffersEnglishForTheDigitsTyped() throws {
    let previousScheme = InputSchemePreference.scheme
    let enabled = InputSchemePreference.enabledSchemes
    defer {
      InputSchemePreference.enabledSchemes = enabled
      InputSchemePreference.scheme = previousScheme
    }
    InputSchemePreference.enabledSchemes = [.quanpin, .nineKey]
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 390, height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)
    for key in ["nineKey4", "nineKey3", "nineKey5", "nineKey5", "nineKey6"] {
      try button(key, in: controller).sendActions(for: .primaryActionTriggered)
    }
    let chips = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
      .compactMap { chip -> String? in
        chip.configuration?.attributedTitle.map { String($0.characters) } ?? chip.configuration?.title
      }
    XCTAssertTrue(chips.contains { $0.hasPrefix("hello") }, "nine-key 43556 offered no hello: \(chips)")
  }

  func testNineKeyInputAndLayoutSwitches() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .nineKey
    defer { InputSchemePreference.scheme = previous }
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 320, height: 216 + KeyboardViewController.stripExtraHeight)
    controller.view.layoutIfNeeded()

    let nine = try button("nineKey6", in: controller)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("schemeButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertEqual(descendants(controller.view).filter { $0.accessibilityIdentifier?.hasPrefix("schemeCard-") == true }.count, InputSchemePreference.offeredSchemes.count)
    try button("closeSchemePicker", in: controller).sendActions(for: .primaryActionTriggered)
    for digit in "64426" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
    try button("nineKeySpelling_ni", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
    controller.view.layoutIfNeeded()
    XCTAssertGreaterThanOrEqual(nine.bounds.height, 30)
    XCTAssertGreaterThan(nine.bounds.width, 60)
    let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
      controller.view.layer.render(in: context.cgContext)
    }
    let attachment = XCTAttachment(image: image)
    attachment.name = "Nine-key pinyin keyboard"
    attachment.lifetime = .keepAlways
    add(attachment)

    // The digit layer keeps the three-column grid and only swaps the legends, which is what
    // testNineKeyDigitLayerKeepsTheGridAndRestoresLetters pins down and what the Apple client
    // asserts here too. Switching language is the one that puts the grid away.
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("layoutToggleButton", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(try XCTUnwrap(nine.superview).isHidden)
    try button("bottomLanguageKey", in: controller).sendActions(for: .primaryActionTriggered)
    XCTAssertFalse(try XCTUnwrap(nine.superview).isHidden)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "全拼 9 键")

    InputSchemePreference.scheme = .shuangpin
    controller.viewWillAppear(false)
    XCTAssertTrue(try XCTUnwrap(nine.superview).isHidden)
    XCTAssertEqual(try button("schemeButton", in: controller).accessibilityValue, "小鹤双拼")
  }

  // 候选条能横向滚，所以放不下的候选应该滚出去，不是在 chip 里折成两行。
  func testCandidateChipsNeverWrapToASecondLine() throws {
    let previous = InputSchemePreference.scheme
    // Glosses are on by default and reserve a line of their own under the candidate, which is a
    // different question from the one this test asks: whether a chip too wide for the row breaks
    // instead of truncating. Pin the preference so the measurement is of wrapping alone.
    let previousGloss = CandidateGlossPreference.enabled
    defer {
      InputSchemePreference.scheme = previous
      CandidateGlossPreference.enabled = previousGloss
    }
    CandidateGlossPreference.enabled = false
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(
      x: 0, y: 0, width: 320, height: 260 + KeyboardViewController.stripExtraHeight)
    for digit in "926439" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    controller.view.layoutIfNeeded()
    let chips = descendants(controller.view).compactMap { view -> UIButton? in
      guard let button = view as? UIButton,
            let identifier = button.accessibilityIdentifier,
            identifier.hasPrefix("candidate-") else { return nil }
      return button
    }
    XCTAssertFalse(chips.isEmpty, "The strip offered no candidate to measure.")
    for chip in chips {
      let label = try XCTUnwrap(chip.titleLabel)
      XCTAssertEqual(label.numberOfLines, 1, "A candidate chip may take more than one line.")
      XCTAssertLessThan(
        label.bounds.height, label.font.lineHeight * 1.5,
        "Candidate \(chip.accessibilityIdentifier ?? "?") wrapped instead of keeping its width.")
    }
  }

  func testCandidateChipsReuseTheirSlotsBetweenKeystrokes() throws {
    let previous = InputSchemePreference.scheme
    defer { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .nineKey
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390,
                                   height: 260 + KeyboardViewController.stripExtraHeight)
    controller.viewWillAppear(false)

    for digit in "64426" {
      try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
    }
    let first = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
    XCTAssertFalse(first.isEmpty)

    try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
    let second = descendants(controller.view).compactMap { $0 as? UIButton }
      .filter { ($0.accessibilityIdentifier ?? "").hasPrefix("candidate-") && !$0.isHidden }
    XCTAssertFalse(second.isEmpty)
    for (before, after) in zip(first, second) {
      XCTAssertTrue(before === after)
    }
  }

  func testKeyPositionsStayFixedWhileComposingAndClearing() throws {
    let previous = InputSchemePreference.scheme
    InputSchemePreference.scheme = .nineKey
    defer { InputSchemePreference.scheme = previous }
    for width in [320.0, 414.0] {
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: width, height: CGFloat(260) + KeyboardViewController.stripExtraHeight)
      controller.view.layoutIfNeeded()
      let keys = try (1...9).map { try button("nineKey\($0)", in: controller) }
      let frames = keys.map { $0.convert($0.bounds, to: controller.view) }
      let delete = try button("nineKeyDelete", in: controller)
      let deleteFrame = delete.convert(delete.bounds, to: controller.view)
      XCTAssertGreaterThan(deleteFrame.minX, frames[2].maxX)
      XCTAssertEqual(deleteFrame.minY, frames[2].minY, accuracy: 0.5)
      XCTAssertGreaterThanOrEqual(frames[0].height, 44)
      let sidebar = try XCTUnwrap(descendants(controller.view).first {
        $0.accessibilityIdentifier == "nineKeySidebar"
      })
      XCTAssertLessThan(sidebar.convert(sidebar.bounds, to: controller.view).maxX, frames[0].minX)

      for phase in ["idle", "composing", "cleared"] {
        if phase == "composing" {
          for digit in "64426" {
            try button("nineKey\(digit)", in: controller).sendActions(for: .primaryActionTriggered)
          }
          XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
          // The hostless XCTest runner does not route target/action through UIApplication.
          // Invoke the registered release action to exercise the same callback as a key tap.
          let releaseAction = try XCTUnwrap(delete.actions(forTarget: controller, forControlEvent: .touchUpInside)?.first)
          controller.perform(NSSelectorFromString(releaseAction))
          try button("nineKey6", in: controller).sendActions(for: .primaryActionTriggered)
          XCTAssertTrue(try XCTUnwrap(button("candidate-1", in: controller).configuration?.title).contains("你好"))
        } else if phase == "cleared" {
          try clearComposition(in: controller)
          XCTAssertTrue(descendants(controller.view).allSatisfy {
            $0.accessibilityIdentifier?.hasPrefix("candidate-") != true || $0.isHidden
          })
        }
        controller.view.layoutIfNeeded()
        for (key, expected) in zip(keys, frames) {
          XCTAssertEqual(key.convert(key.bounds, to: controller.view), expected,
                         "Key moved during \(phase) at width \(width)")
        }
        let image = UIGraphicsImageRenderer(bounds: controller.view.bounds).image { context in
          controller.view.layer.render(in: context.cgContext)
        }
        let attachment = XCTAttachment(image: image)
        attachment.name = "Nine-key \(Int(width))pt \(phase)"
        attachment.lifetime = .keepAlways
        add(attachment)
      }
    }
  }

}
