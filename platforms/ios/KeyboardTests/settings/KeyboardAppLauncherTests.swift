import XCTest
import UIKit

/// 只应答 `openURL:` 的一节响应链,用来站在真实链条里 `UIApplication` 的位置上。
private final class OpeningResponder: UIResponder {
  private(set) var opened: [URL] = []
  @objc func openURL(_ url: URL) { opened.append(url) }
}

/// 链条中间那一节:什么都不应答,只负责把 `next` 往上接。
private final class PlainResponder: UIResponder {
  private let parent: UIResponder?
  init(next parent: UIResponder?) { self.parent = parent; super.init() }
  override var next: UIResponder? { parent }
}

final class KeyboardAppLauncherTests: XCTestCase {
  /// 键盘视图和应用对象之间隔着好几层,所以找的是链条,不是直接的 `next`。
  func testWalksPastResponderesThatCannotOpen() {
    let application = OpeningResponder()
    let chain = PlainResponder(next: PlainResponder(next: application))

    XCTAssertTrue(KeyboardAppLauncher.open(KeyboardAppLauncher.settingsURL, from: chain))
    XCTAssertEqual(application.opened, [KeyboardAppLauncher.settingsURL])
  }

  /// 链条上没有人能打开时要如实说没打开,而不是当成打开了 —— 调用方据此才有机会换个说法。
  func testReportsFailureWhenNothingOnTheChainOpens() {
    let chain = PlainResponder(next: PlainResponder(next: nil))

    XCTAssertFalse(KeyboardAppLauncher.open(KeyboardAppLauncher.settingsURL, from: chain))
  }

  /// scheme 在 project.yml 的 CFBundleURLTypes 里注册。两边对不上时应用不会被拉起,而键盘这边看不出区别,
  /// 所以把这个字面值钉在测试里。
  func testTargetsTheRegisteredScheme() {
    XCTAssertEqual(KeyboardAppLauncher.settingsURL.scheme, "msime")
    XCTAssertEqual(KeyboardAppLauncher.settingsURL.absoluteString, "msime://settings")
    XCTAssertEqual(KeyboardAppLauncher.voiceURL.absoluteString, "msime://voice")
  }

  /// 功能菜单的四个目的地使用应用 SettingsRouter 能处理的路由：`settings/<page>`、`feedback` 和 `about`。
  func testMenuDestinationsUseTheRoutesTheAppHandles() {
    XCTAssertEqual(KeyboardAppLauncher.dictionaryURL.absoluteString, "msime://settings/dictionary")
    XCTAssertEqual(KeyboardAppLauncher.inputSettingsURL.absoluteString, "msime://settings/input")
    XCTAssertEqual(KeyboardAppLauncher.feedbackURL.absoluteString, "msime://feedback")
    XCTAssertEqual(KeyboardAppLauncher.aboutURL.absoluteString, "msime://about")
    for url in [KeyboardAppLauncher.dictionaryURL, KeyboardAppLauncher.inputSettingsURL] {
      XCTAssertEqual(url.host, "settings")
    }
    XCTAssertEqual(KeyboardAppLauncher.dictionaryURL.pathComponents.last, "dictionary")
    XCTAssertEqual(KeyboardAppLauncher.inputSettingsURL.pathComponents.last, "input")
  }

  /// 常用语面板里的添加常用语打开应用的常用语页（`SettingsPage.phrases`），iOS 上只有那里能添加常用语。
  func testAddPhraseOpensThePhrasesPage() {
    XCTAssertEqual(KeyboardAppLauncher.phrasesURL.absoluteString, "msime://settings/phrases")
    XCTAssertEqual(KeyboardAppLauncher.phrasesURL.host, "settings")
    XCTAssertEqual(KeyboardAppLauncher.phrasesURL.pathComponents.last, "phrases")
  }

  /// 每个目的地都经由同一条响应链打开。
  func testEveryMenuDestinationOpensThroughTheChain() {
    let application = OpeningResponder()
    let chain = PlainResponder(next: application)
    let urls = [KeyboardAppLauncher.dictionaryURL, KeyboardAppLauncher.inputSettingsURL,
                KeyboardAppLauncher.feedbackURL, KeyboardAppLauncher.aboutURL]
    for url in urls { XCTAssertTrue(KeyboardAppLauncher.open(url, from: chain)) }
    XCTAssertEqual(application.opened, urls)
  }

  /// 每个版本各用一个 scheme，装了多个版本时键盘才不会拉起另一个版本的 App；full 仍是 msime。
  func testURLSchemeIsPerEdition() {
    XCTAssertEqual(MSIMEAppEdition.urlScheme(in: [:]), "msime")
    XCTAssertEqual(MSIMEAppEdition.urlScheme(in: ["MSIMEEdition": "full"]), "msime")
    XCTAssertEqual(MSIMEAppEdition.urlScheme(in: ["MSIMEEdition": "wubi"]), "msime-wubi")
    XCTAssertEqual(MSIMEAppEdition.urlScheme(in: ["MSIMEEdition": "pinyin"]), "msime-pinyin")
  }
}
