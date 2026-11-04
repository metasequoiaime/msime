import UIKit
import XCTest

final class AppThemePaletteTests: XCTestCase {
  private var savedThemeID: Any?

  override func setUp() {
    super.setUp()
    savedThemeID = UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)?.object(forKey: AppThemePalette.storageKey)
  }

  override func tearDown() {
    let defaults = UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)
    if let savedThemeID { defaults?.set(savedThemeID, forKey: AppThemePalette.storageKey) }
    else { defaults?.removeObject(forKey: AppThemePalette.storageKey) }
    AppThemePalette.refresh()
    super.tearDown()
  }

  // 期望值取自设计稿的 token 表，用的是改色之前的秋杉种子（`#B5562B`、`#F6E9DC`）：它混出的按键色有一个通道恰好落在 249.5，现在的种子没有，所以用它钉住取整（键盘背景、按键和功能键、启动页背景）。
  func testMixMatchesTheDesignTokens() throws {
    let accent = try color("#B5562B"), background = try color("#F6E9DC")
    XCTAssertEqual(hex(AppThemePalette.mix(accent, 12, background)), "#EED7C7FF")
    // 绿色分量正好落在 249.5；Chrome 打印出的是 0.978431，即 249（F9），而不是普通四舍五入得到的 250。
    XCTAssertEqual(hex(AppThemePalette.mix(background, 25, try color("#FFFFFF"))), "#FDF9F6FF")
    XCTAssertEqual(hex(AppThemePalette.mix(accent, 24, background)), "#E6C6B2FF")
    XCTAssertEqual(hex(AppThemePalette.mix(accent, 20, try color("#0A0B0A"))), "#2C1A11FF")
  }

  func testMixBlendsAlpha() throws {
    XCTAssertEqual(hex(AppThemePalette.mix(try color("#B5562B33"), 50, try color("#B5562BFF"))), "#B5562B99")
  }

  func testHexColorReadsOnlyContractForms() {
    XCTAssertNotNil(AppThemePalette.hexColor("#B5562B"))
    XCTAssertEqual(AppThemePalette.hexColor("#B5562B33").map(hex), "#B5562B33")
    XCTAssertNil(AppThemePalette.hexColor("B5562B"))
    XCTAssertNil(AppThemePalette.hexColor("#B5562"))
    XCTAssertNil(AppThemePalette.hexColor("#B5562G"))
    XCTAssertNil(AppThemePalette.hexColor("#+5562B"))
  }

  func testFourSeasonsResolveAutumnInOctober() throws {
    UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)?.set("siji", forKey: AppThemePalette.storageKey)
    AppThemePalette.refresh(month: 10)
    let light = try XCTUnwrap(AppThemePalette.resolved(dark: false))
    let dark = try XCTUnwrap(AppThemePalette.resolved(dark: true))
    XCTAssertEqual(light.id, "siji")
    XCTAssertEqual(light.season, "autumn")
    XCTAssertEqual(hex(light.accent), "#B07800FF")
    XCTAssertEqual(hex(light.background), "#FAF5E8FF")
    XCTAssertEqual(hex(light.card), "#FFFDF6FF")
    XCTAssertEqual(hex(light.onAccent), "#1A1200FF")
    XCTAssertEqual(hex(dark.accent), "#F3C14FFF")
    XCTAssertEqual(hex(dark.background), "#1C1910FF")
    XCTAssertEqual(hex(dark.card), "#28241AFF")
  }

  func testChangingTheSeasonBumpsTheVersionAndPosts() {
    UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)?.set("siji", forKey: AppThemePalette.storageKey)
    AppThemePalette.refresh(month: 10)
    let before = AppThemePalette.version
    AppThemePalette.refresh(month: 11)
    XCTAssertEqual(AppThemePalette.version, before, "November is still autumn")
    expectation(forNotification: AppThemePalette.didChange, object: nil)
    AppThemePalette.refresh(month: 1)
    waitForExpectations(timeout: 1)
    XCTAssertEqual(AppThemePalette.resolved(dark: false)?.season, "winter")
    XCTAssertEqual(AppThemePalette.version, before + 1)
  }

  func testCatalogComesFromClientCore() {
    let catalog = AppThemePalette.catalog()
    XCTAssertEqual(catalog.first?.id, "siji")
    XCTAssertEqual(catalog.first?.seasonal, true)
    XCTAssertEqual(Set(catalog.map(\.id)), ["siji", "chunya", "xiayin", "qiushan", "dongxue"])
    XCTAssertTrue(catalog.allSatisfy { !$0.title.isEmpty })
  }

  func testAccentFallsBackToTheStoredDefaultForAnUnknownTheme() {
    UserDefaults(suiteName: MSIMEAppEdition.appGroupIdentifier)?.set("no-such-theme", forKey: AppThemePalette.storageKey)
    AppThemePalette.refresh(month: 10)
    XCTAssertEqual(AppThemePalette.resolved(dark: false)?.id, "siji")
  }

  private func color(_ value: String) throws -> UIColor {
    try XCTUnwrap(AppThemePalette.hexColor(value))
  }

  private func hex(_ color: UIColor) -> String {
    var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
    color.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
    let channel = { (value: CGFloat) in Int((value * 255).rounded()) }
    return String(format: "#%02X%02X%02X%02X", channel(red), channel(green), channel(blue), channel(alpha))
  }
}
