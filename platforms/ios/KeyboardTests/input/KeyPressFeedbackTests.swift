import XCTest
import UIKit

/// 按键反馈在按下时发、松手不重复（#10768）：按键记下这次按下已经反馈过，松手时执行的动作在 `KeyboardKeyButton.releasing` 里就不再出声；没有经过按下的激活（`sendActions`、读屏）照常出声。
@MainActor
final class KeyPressFeedbackTests: XCTestCase {
  override func setUp() {
    super.setUp()
    enableAllInputSchemes()
  }

  private func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  private func key(labelled label: String, in controller: UIViewController) throws -> KeyboardKeyButton {
    try XCTUnwrap(nodes(controller.view).first { $0.accessibilityLabel == label } as? KeyboardKeyButton,
                  "No key labelled \(label).")
  }

  private func makeController(_ scheme: ChineseInputScheme) -> KeyboardViewController {
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = scheme
    let controller = KeyboardViewController()
    controller.loadViewIfNeeded()
    controller.view.frame = CGRect(x: 0, y: 0, width: 414, height: KeyboardViewController.defaultKeyboardHeight)
    controller.view.layoutIfNeeded()
    return controller
  }

  func testReleaseOfAPressThatFedBackIsSilent() {
    let key = KeyboardKeyButton(type: .system)
    key.markPressFedBack()
    var silenced: Bool?
    KeyboardKeyButton.releasing(key) { silenced = KeyboardKeyButton.releasingPressThatFedBack }
    XCTAssertEqual(silenced, true)
    XCTAssertFalse(KeyboardKeyButton.releasingPressThatFedBack, "the scope ends with the release")
  }

  /// 一次按下只抵一次松手：同一个键之后再有松手（例如这一下被手势接管后又在手势结束时上屏）不再被吞掉。
  func testReleaseConsumesThePress() {
    let key = KeyboardKeyButton(type: .system)
    key.markPressFedBack()
    KeyboardKeyButton.releasing(key) {}
    var silenced: Bool?
    KeyboardKeyButton.releasing(key) { silenced = KeyboardKeyButton.releasingPressThatFedBack }
    XCTAssertEqual(silenced, false)
  }

  /// 没有按下就执行的动作（`sendActions`、读屏激活）以及候选、面板这类不在按下时反馈的按钮，动作里照常出声。
  func testActivationWithoutAPressStillFeedsBack() {
    XCTAssertFalse(KeyboardKeyButton.releasingPressThatFedBack)
    var silenced: [Bool] = []
    KeyboardKeyButton.releasing(KeyboardKeyButton(type: .system)) { silenced.append(KeyboardKeyButton.releasingPressThatFedBack) }
    KeyboardKeyButton.releasing(UIButton(type: .system)) { silenced.append(KeyboardKeyButton.releasingPressThatFedBack) }
    KeyboardKeyButton.releasing(nil) { silenced.append(KeyboardKeyButton.releasingPressThatFedBack) }
    XCTAssertEqual(silenced, [false, false, false])
  }

  func testNestedReleaseRestoresTheOuterOne() {
    let pressed = KeyboardKeyButton(type: .system)
    pressed.markPressFedBack()
    var seen: [Bool] = []
    KeyboardKeyButton.releasing(pressed) {
      KeyboardKeyButton.releasing(KeyboardKeyButton(type: .system)) { seen.append(KeyboardKeyButton.releasingPressThatFedBack) }
      seen.append(KeyboardKeyButton.releasingPressThatFedBack)
    }
    XCTAssertEqual(seen, [false, true])
  }

  /// 能滚动的容器里按下可能是在开始滚动，反馈留到松手；放得下、滚不动的容器照样按下即反馈。
  func testKeysInScrollableContentWaitForTheRelease() {
    XCTAssertTrue(KeyboardKeyButton.scrolls(contentSize: CGSize(width: 40, height: 120), boundsSize: CGSize(width: 40, height: 100)))
    XCTAssertTrue(KeyboardKeyButton.scrolls(contentSize: CGSize(width: 140, height: 40), boundsSize: CGSize(width: 100, height: 40)))
    XCTAssertFalse(KeyboardKeyButton.scrolls(contentSize: CGSize(width: 40, height: 100), boundsSize: CGSize(width: 40, height: 100)))
    XCTAssertFalse(KeyboardKeyButton.scrolls(contentSize: CGSize(width: 40, height: 100.4), boundsSize: CGSize(width: 40, height: 100)))

    let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 40, height: 100))
    let key = KeyboardKeyButton(type: .system)
    scroll.addSubview(key)
    scroll.contentSize = CGSize(width: 40, height: 100)
    XCTAssertFalse(key.sitsInScrollableContent)
    scroll.contentSize = CGSize(width: 40, height: 200)
    XCTAssertTrue(key.sitsInScrollableContent)
    scroll.isScrollEnabled = false
    XCTAssertFalse(key.sitsInScrollableContent)
  }

  /// 键盘按键都在按下时反馈，空格、退格和回车按自己的声音类别出声。
  func testKeyboardKeysFeedBackOnPress() throws {
    let controller = makeController(.quanpin)
    let pressActions = { (button: UIButton) in button.actions(forTarget: controller, forControlEvent: .touchDown) ?? [] }
    for label in ["Q", "M", "大写", "空格", "删除", "换行"] {
      XCTAssertTrue(pressActions(try key(labelled: label, in: controller)).contains("keyPressFeedback:"), label)
    }
    XCTAssertEqual(try key(labelled: "Q", in: controller).pressSoundClass, .standard)
    XCTAssertEqual(try key(labelled: "空格", in: controller).pressSoundClass, .space)
    XCTAssertEqual(try key(labelled: "删除", in: controller).pressSoundClass, .backspace)
    XCTAssertEqual(try key(labelled: "换行", in: controller).pressSoundClass, .enter)
  }
}
