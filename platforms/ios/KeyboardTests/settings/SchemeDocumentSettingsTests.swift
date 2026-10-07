import XCTest
import UIKit

/// The app's scheme and output-form choices reach the shared document, which the keyboard copies over the App Group every time it appears.
final class SchemeDocumentSettingsTests: XCTestCase {
  private var state: URL!
  private var previousScheme = ChineseInputScheme.quanpin
  private var previousEnabled: [ChineseInputScheme] = []
  private var previousTraditional = false
  private var previousWubiProfile = "wubi86"

  override func setUp() {
    super.setUp()
    state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-scheme-document-\(UUID().uuidString)", isDirectory: true)
    previousEnabled = InputSchemePreference.enabledSchemes
    previousScheme = InputSchemePreference.scheme
    previousTraditional = ChineseOutputPreference.usesTraditional
    previousWubiProfile = WubiProfilePreference.profile
  }

  override func tearDown() {
    InputSchemePreference.enabledSchemes = previousEnabled
    InputSchemePreference.scheme = previousScheme
    ChineseOutputPreference.usesTraditional = previousTraditional
    WubiProfilePreference.profile = previousWubiProfile
    try? FileManager.default.removeItem(at: state)
    super.tearDown()
  }

  /// The keyboard's own switch wrote `touch_keyboard_schemes` first; a later choice in the app has to replace it there, not only in the App Group.
  func testAppSchemeChoiceReplacesTheKeyboardsEarlierSelection() throws {
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(bridge.setTouchKeyboardScheme(.nineKey, enabledSchemes: [.quanpin, .nineKey]))

    XCTAssertTrue(InputSchemePreference.save(scheme: .wubi, enabled: [.quanpin, .nineKey, .wubi], stateRoot: state))

    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    let schemes = try XCTUnwrap(document["touch_keyboard_schemes"] as? [String: Any])
    XCTAssertEqual(schemes["selected"] as? String, ChineseInputScheme.wubi.sharedIdentifier)
    XCTAssertEqual(schemes["enabled"] as? [String], [ChineseInputScheme.quanpin, .nineKey, .wubi].map(\.sharedIdentifier))
    XCTAssertEqual(document["scheme"] as? String, "wubi")
    XCTAssertEqual(document["touch_keyboard_layout"] as? String, "twenty_six_key")
    XCTAssertEqual(InputSchemePreference.scheme, .wubi)
  }

  /// 五笔版本写进文档的 `wubi_profile`，方案名和卡片角标跟着变；之后再切方案不会把它改回去。
  func testWubiProfileReachesTheDocumentAndSurvivesSchemeChanges() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(WubiProfilePreference.save("wubi98", stateRoot: state))
    XCTAssertEqual(WubiProfilePreference.profile, "wubi98")
    XCTAssertEqual(ChineseInputScheme.wubi.title, "98 五笔")
    XCTAssertEqual(WubiProfilePreference.badge(WubiProfilePreference.profile), "98")

    XCTAssertTrue(InputSchemePreference.save(scheme: .quanpin, enabled: [.quanpin, .wubi], stateRoot: state))
    XCTAssertTrue(InputSchemePreference.save(scheme: .wubi, enabled: [.quanpin, .wubi], stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["scheme"] as? String, "wubi")
    XCTAssertEqual(document["wubi_profile"] as? String, "wubi98")
    XCTAssertEqual(WubiProfilePreference.profile(in: document), "wubi98")

    XCTAssertFalse(WubiProfilePreference.save("wubi06", stateRoot: state))
    XCTAssertEqual(WubiProfilePreference.profile, "wubi98")
    XCTAssertEqual(WubiProfilePreference.profile(in: ["wubi_profile": "wubi06"]), "wubi86")
    WubiProfilePreference.mirror(["wubi_profile": "wubi86"])
    XCTAssertEqual(ChineseInputScheme.wubi.title, "86 五笔")
  }

  // ---- App Group 镜像落后于文档（#4288）：镜像记着双拼，文档里是用户后来选的全拼 ----

  /// 文档选的是全拼、启用全拼双拼和手写；镜像还停在双拼，启用列表里没有手写。
  private func makeMirrorStale() throws {
    XCTAssertTrue(InputSchemePreference.save(scheme: .quanpin, enabled: [.quanpin, .shuangpin, .handwriting], stateRoot: state))
    InputSchemePreference.enabledSchemes = [.quanpin, .shuangpin]
    InputSchemePreference.scheme = .shuangpin
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(InputSchemePreference.selectedScheme(in: document), .quanpin)
  }

  private func assertDocumentSelection(_ scheme: ChineseInputScheme, enabled: [ChineseInputScheme],
                                       file: StaticString = #filePath, line: UInt = #line) throws {
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state), file: file, line: line)
    XCTAssertEqual(InputSchemePreference.selectedScheme(in: document), scheme, file: file, line: line)
    XCTAssertEqual(document["scheme"] as? String, scheme.engineScheme, file: file, line: line)
    XCTAssertEqual(InputSchemePreference.enabledSchemes(in: document).map(Set.init), Set(enabled), file: file, line: line)
    XCTAssertEqual(InputSchemePreference.scheme, scheme, "镜像没有跟上文档", file: file, line: line)
    XCTAssertEqual(InputSchemePreference.enabledSchemes, enabled, "镜像没有跟上文档", file: file, line: line)
  }

  /// 设置页开关一个入口（手写页的开关、输入页每个方案旁的开关）只改启用列表，不能把镜像里的双拼写回文档。
  func testTogglingASchemeKeepsTheDocumentsSelectionOverAStaleMirror() throws {
    try makeMirrorStale()
    XCTAssertTrue(InputSchemePreference.setEnabled(.handwriting, false, stateRoot: state))
    try assertDocumentSelection(.quanpin, enabled: [.quanpin, .shuangpin])
    XCTAssertTrue(InputSchemePreference.setEnabled(.wubi, true, stateRoot: state))
    try assertDocumentSelection(.quanpin, enabled: [.quanpin, .shuangpin, .wubi])
  }

  /// 停用文档里选中的入口时换成第一个启用的入口，与键盘和 `schemeMapping` 的回退一致；最后一个入口停不掉。
  func testDisablingTheSelectedSchemeFallsBackAndTheLastOneStays() throws {
    XCTAssertTrue(InputSchemePreference.save(scheme: .shuangpin, enabled: [.quanpin, .shuangpin], stateRoot: state))
    XCTAssertTrue(InputSchemePreference.setEnabled(.shuangpin, false, stateRoot: state))
    try assertDocumentSelection(.quanpin, enabled: [.quanpin])
    XCTAssertFalse(InputSchemePreference.setEnabled(.quanpin, false, stateRoot: state))
    try assertDocumentSelection(.quanpin, enabled: [.quanpin])
  }

  /// 点方案名只改选中的入口，文档里的启用列表不被镜像里过时的列表覆盖。
  func testSelectingASchemeKeepsTheDocumentsEnabledList() throws {
    try makeMirrorStale()
    XCTAssertTrue(InputSchemePreference.select(.shuangpin, stateRoot: state))
    try assertDocumentSelection(.shuangpin, enabled: [.quanpin, .shuangpin, .handwriting])
    // 还没启用的入口被选中时一并启用（欢迎页的方案卡片）。
    XCTAssertTrue(InputSchemePreference.select(.wubi, stateRoot: state))
    try assertDocumentSelection(.wubi, enabled: [.quanpin, .shuangpin, .wubi, .handwriting])
  }

  /// 云端同步：上传取文档里的方案；应用云端方案时保留文档里的启用列表。
  func testCloudSyncReadsAndWritesTheSchemeThroughTheDocument() throws {
    try makeMirrorStale()
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    let current = InputSchemePreference.current(in: document)
    XCTAssertEqual(current.scheme, .quanpin)
    XCTAssertEqual(Set(current.enabled), [.quanpin, .shuangpin, .handwriting])
    // 文档还没记过选择时，键盘按镜像行事，上传的也是镜像里的。
    XCTAssertEqual(InputSchemePreference.current(in: [:]).scheme, .shuangpin)

    var written: InputSchemePreference.Selection?
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) { document in
      written = InputSchemePreference.write({ $0.scheme = .shuangpin }, into: &document)
    })
    InputSchemePreference.mirror(try XCTUnwrap(written))
    try assertDocumentSelection(.shuangpin, enabled: [.quanpin, .shuangpin, .handwriting])
  }

  /// 新建的键盘先按文档里的方案画出来，并把它抄进镜像，不等后台重载。
  func testKeyboardStartsOnTheDocumentsSchemeNotTheStaleMirror() throws {
    // 键盘读的是模拟器里各用例共用的那份文档，用完把方案相关的字段放回原样。
    let keys = ["scheme", "last_chinese_scheme", "shuangpin_profile", "touch_keyboard_layout", "touch_keyboard_schemes"]
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences() ?? [:]
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { document in
        for key in keys { document[key] = previous[key] }
      }
    }
    XCTAssertTrue(InputSchemePreference.save(scheme: .quanpin, enabled: [.quanpin, .shuangpin]))
    InputSchemePreference.scheme = .shuangpin

    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    let schemeButton = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "schemeButton" })
    XCTAssertEqual(schemeButton.accessibilityValue, ChineseInputScheme.quanpin.title)
    XCTAssertEqual(InputSchemePreference.scheme, .quanpin)
  }

  // ---- 简繁的 App Group 镜像落后于文档：与上面的方案是同一类问题 ----

  /// 上传和新建键盘读的是模拟器里各用例共用的那份文档，用完把 `traditional_chinese_output` 放回原样。
  private func restoreSharedOutputFormAfterTest() {
    let key = ChineseOutputPreference.documentKey
    let previous = MetasequoiaInputSessionBridge.loadSharedPreferences()?[key]
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[key] = previous }
    }
  }

  /// 「上传本机设置」带上的是文档里的简繁，不是 App Group 镜像里的旧值；两个方向都要对。
  func testCloudUploadCarriesTheDocumentsOutputFormOverAStaleMirror() throws {
    restoreSharedOutputFormAfterTest()
    for traditional in [true, false] {
      XCTAssertTrue(ChineseOutputPreference.save(traditional))
      ChineseOutputPreference.usesTraditional = !traditional
      let settings = try IOSCloudSettings.snapshot()
      XCTAssertEqual(settings["input.character_set"], .string(traditional ? "traditional" : "simplified"))
    }
    // 文档没记过字形时，键盘按镜像行事，取的也是镜像。
    ChineseOutputPreference.usesTraditional = true
    XCTAssertTrue(ChineseOutputPreference.current(in: [:]))
    XCTAssertFalse(ChineseOutputPreference.current(in: [ChineseOutputPreference.documentKey: false]))
  }

  /// 新建的键盘按文档里的简繁转换上屏文字，并把它抄进镜像，不等后台重载。
  func testKeyboardStartsOnTheDocumentsOutputFormNotTheStaleMirror() throws {
    restoreSharedOutputFormAfterTest()
    XCTAssertTrue(ChineseOutputPreference.save(true))
    ChineseOutputPreference.usesTraditional = false

    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    func descendants(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap { descendants($0) } }
    let shortcut = try XCTUnwrap(descendants(controller.view).first { $0.accessibilityIdentifier == "characterSetShortcut" })
    XCTAssertEqual(shortcut.accessibilityValue, "繁体")
    XCTAssertTrue(ChineseOutputPreference.usesTraditional)
  }

  func testTraditionalOutputReachesTheDocument() throws {
    _ = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(ChineseOutputPreference.save(true, stateRoot: state))
    let document = try XCTUnwrap(MetasequoiaInputSessionBridge.loadSharedPreferences(stateRoot: state))
    XCTAssertEqual(document["traditional_chinese_output"] as? Bool, true)
    XCTAssertTrue(ChineseOutputPreference.usesTraditional)
  }
}
