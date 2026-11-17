import XCTest
import UIKit

/// 经真实 C ABI 核对普通 26 键双拼的表、错误位置和键位提示，设置页不复制 Engine 规则。
final class ShuangpinCustomTests: XCTestCase {
  func testDeletingCustomTablePreservesOtherSchemesAndPreferences() {
    for profile in ["custom", "microsoft"] {
      var document: [String: Any] = ["scheme": "quanpin", "shuangpin_profile": profile,
        "shuangpin_custom_profile": ["synthetic": [:]], "learning": false]
      ShuangpinCustomLayout.remove(from: &document)
      XCTAssertNil(document["shuangpin_custom_profile"])
      XCTAssertEqual(document["shuangpin_profile"] as? String, profile == "custom" ? "xiaohe" : profile)
      XCTAssertEqual(document["scheme"] as? String, "quanpin")
      XCTAssertEqual(document["learning"] as? Bool, false)
    }
  }
  @MainActor
  func testCustomSemicolonIsReachableAndProducesTheConfiguredCandidate() throws {
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()
    let mirror = (InputSchemePreference.enabledSchemes, InputSchemePreference.scheme)
    let keys = ["scheme", "last_chinese_scheme", "touch_keyboard_layout", "touch_keyboard_schemes",
                "shuangpin_profile", "shuangpin_custom_profile", "learning", "cloud_candidates",
                "touch_shuangpin_key_hints", "helpcode", "default_ime_mode"]
    defer {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
        for key in keys { document[key] = previous?[key] }
      }
      InputSchemePreference.mirror(.init(scheme: mirror.1, enabled: mirror.0))
    }
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences { document in
      document["learning"] = false
      document["cloud_candidates"] = false
      document["touch_shuangpin_key_hints"] = false
      document["helpcode"] = ["enabled": false]
      document["default_ime_mode"] = "chinese"
    })
    let table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "microsoft").table
    XCTAssertTrue(try ShuangpinCustomLayout.save(profile: "custom", table: table))
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
    controller.viewWillAppear(false)
    defer { controller.viewDidDisappear(false) }
    controller.view.layoutIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(descendants) }
    let semicolon = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityIdentifier == "microsoftFinalKey"
    } as? UIButton)
    XCTAssertEqual(ChineseInputScheme.shuangpin.shortLabel(shuangpinProfile: "custom"), "双拼 · 自定义")
    XCTAssertFalse(semicolon.isHidden, "关闭提示也必须能输入码表中的分号")
    XCTAssertEqual(semicolon.accessibilityLabel, "分号，自定义双拼 ing")
    let b = try XCTUnwrap(descendants(controller.view).first {
      $0.accessibilityLabel == "字母 B" || $0.accessibilityLabel == "大写 B"
    } as? UIButton)
    b.sendActions(for: .primaryActionTriggered)
    semicolon.sendActions(for: .primaryActionTriggered)
    XCTAssertTrue(descendants(controller.view).contains {
      guard let button = $0 as? UIButton else { return false }
      return button.accessibilityIdentifier?.hasPrefix("candidate-") == true &&
        button.configuration?.title?.contains("冰") == true
    }, "自定义 b; 必须按共享码表得到冰")
    let attachment = XCTAttachment(image: UIGraphicsImageRenderer(bounds: controller.view.bounds).image {
      controller.view.layer.render(in: $0.cgContext)
    })
    attachment.name = "custom-shuangpin-semicolon"
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  func testReturningToTheKeyboardKeepsCustomAndSmartABCProfiles() throws {
    let mapping = try XCTUnwrap(MetasequoiaInputSessionBridge.schemeMapping(.shuangpin, enabledSchemes: [.quanpin, .shuangpin]))
    for profile in ["custom", "smart_abc"] {
      var document: [String: Any] = ["shuangpin_profile": profile, "scheme": "quanpin"]
      ShuangpinCustomLayout.runtimeMapping(mapping, scheme: .shuangpin)(&document)
      XCTAssertEqual(document["shuangpin_profile"] as? String, profile)
      XCTAssertEqual(document["scheme"] as? String, "shuangpin")
      mapping(&document)
      XCTAssertEqual(document["shuangpin_profile"] as? String, "xiaohe")
    }
  }

  func testSavingProfileAlsoSelectsTheKeyboardWithoutOverwritingTheTableName() throws {
    let table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "microsoft").table
    for profile in ShuangpinCustomLayout.profiles {
      var document: [String: Any] = [
        "scheme": "quanpin",
        "touch_keyboard_schemes": ["selected": "quanpin", "enabled": ["quanpin"]],
      ]
      let selection = try XCTUnwrap(ShuangpinCustomLayout.write(profile: profile, table: table, into: &document))
      XCTAssertEqual(document["shuangpin_profile"] as? String, profile)
      XCTAssertEqual(document["scheme"] as? String, "shuangpin")
      XCTAssertEqual(document["touch_keyboard_layout"] as? String, "twenty_six_key")
      XCTAssertTrue(selection.enabled.contains(.quanpin))
      XCTAssertTrue(selection.enabled.contains(selection.scheme))
      XCTAssertEqual(InputSchemePreference.selectedScheme(in: document), selection.scheme)
      if profile == "custom" {
        XCTAssertEqual(document["shuangpin_custom_profile"] as? ShuangpinCustomLayout.Table, table)
      }
    }
  }

  func testBuiltinAndCustomLayoutsShareTheSameTableAndKeyFace() throws {
    let builtin = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe")
    XCTAssertNil(builtin.problem)
    XCTAssertEqual(builtin.effective, "xiaohe")
    XCTAssertEqual(builtin.table["finals"]?["ing"], "k")
    XCTAssertEqual(builtin.keys.first?.key, "Q")
    XCTAssertEqual(builtin.keys.first?.hint, "iu")

    let custom = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(
      profile: "custom", table: builtin.table)
    XCTAssertNil(custom.problem)
    XCTAssertEqual(custom.effective, "custom")
    XCTAssertEqual(custom.table, builtin.table)
    XCTAssertEqual(custom.keys.first?.hint, builtin.keys.first?.hint)
  }

  func testInvalidCustomTableReportsTheUnitAndUsesXiaoheForTheFace() throws {
    var table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe").table
    table["finals"]?.removeValue(forKey: "iong")
    let invalid = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(
      profile: "custom", table: table)
    XCTAssertEqual(invalid.effective, "xiaohe")
    XCTAssertEqual(invalid.problem?.kind, "missing_unit")
    XCTAssertEqual(invalid.problem?.part, "finals")
    XCTAssertEqual(invalid.problem?.unit, "iong")
    XCTAssertEqual(invalid.keys.first?.hint, "iu")
    XCTAssertThrowsError(try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "unknown"))
  }

  func testTableExportAndImportRoundTrip() throws {
    let table = try MetasequoiaInputSessionBridge.shuangpinCustomLayout(profile: "xiaohe").table
    XCTAssertEqual(try ShuangpinCustomLayout.importTable(ShuangpinCustomLayout.exportTable(table)), table)
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(Data("[]".utf8)))
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(Data(repeating: 32, count: 16_385)))
    var malformed = table
    malformed["unknown"] = ["synthetic": "q"]
    XCTAssertThrowsError(try ShuangpinCustomLayout.importTable(ShuangpinCustomLayout.exportTable(malformed)))
  }
}
