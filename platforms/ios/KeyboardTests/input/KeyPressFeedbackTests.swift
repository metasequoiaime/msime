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
    XCTAssertTrue(key.beginPressFeedback())
    var silenced: Bool?
    KeyboardKeyButton.releasing(key) { silenced = KeyboardKeyButton.releasingPressThatFedBack }
    XCTAssertEqual(silenced, true)
    XCTAssertFalse(KeyboardKeyButton.releasingPressThatFedBack, "the scope ends with the release")
  }

  /// 一次按下只抵一次松手：同一个键之后再有松手（例如这一下被手势接管后又在手势结束时上屏）不再被吞掉。
  func testReleaseConsumesThePress() {
    let key = KeyboardKeyButton(type: .system)
    XCTAssertTrue(key.beginPressFeedback())
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
    XCTAssertTrue(pressed.beginPressFeedback())
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

  /// 每次按下都重写标记：滚得动的容器里按下不在当时反馈，也要清掉上一次被取消的触摸留下的标记，否则这次松手的动作不出声，这一下就什么反馈都没有。
  func testPressInScrollableContentClearsAStaleMark() {
    let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 40, height: 100))
    let key = KeyboardKeyButton(type: .system)
    scroll.addSubview(key)
    scroll.contentSize = CGSize(width: 40, height: 100)
    XCTAssertTrue(key.beginPressFeedback())
    // 这次触摸被取消（手势接管、系统取消），标记留在键上；之后容器变得滚得动，同一个键再按下。
    scroll.contentSize = CGSize(width: 40, height: 200)
    XCTAssertFalse(key.beginPressFeedback())
    XCTAssertFalse(key.pressFedBack)
    var silenced: Bool?
    KeyboardKeyButton.releasing(key) { silenced = KeyboardKeyButton.releasingPressThatFedBack }
    XCTAssertEqual(silenced, false)
  }

  /// 九键候选面板右栏的 ⌫ 也是 `makeKey` 造的、按下就反馈，要按退格类出声；它的动作里删笔画、删拼音时的退格声在松手里不再出声。
  func testNineKeyPanelDeleteSoundsAsBackspace() throws {
    let columns = KeyboardNineKeyPanelColumns(makeKey: { title, label, _ in
      let button = KeyboardKeyButton(type: .system)
      button.setTitle(title, for: .normal)
      button.accessibilityLabel = label
      return button
    })
    let classes = try Dictionary(uniqueKeysWithValues: columns.trailing.arrangedSubviews.map { view in
      let key = try XCTUnwrap(view as? KeyboardKeyButton)
      return (try XCTUnwrap(key.accessibilityIdentifier), key.pressSoundClass)
    })
    XCTAssertEqual(classes["candidatePanelDelete"], .backspace)
    for identifier in ["candidatePanelBack", "candidatePanelClear", "candidatePanelStrokeToggle", "candidatePanelSingleToggle"] {
      XCTAssertEqual(classes[identifier], .standard, identifier)
    }
  }

  /// 键盘按键都在按下时反馈，空格、退格和回车按自己的声音类别出声。
  func testKeyboardKeysFeedBackOnPress() throws {
    let controller = makeController(.quanpin)
    let pressActions = { (button: UIButton) in button.actions(forTarget: controller, forControlEvent: .touchDown) ?? [] }
    for label in ["字母 Q", "字母 M", "切换到英文大写", "空格", "删除", "换行"] {
      XCTAssertTrue(pressActions(try key(labelled: label, in: controller)).contains("keyPressFeedback:"), label)
    }
    XCTAssertEqual(try key(labelled: "字母 Q", in: controller).pressSoundClass, .standard)
    XCTAssertEqual(try key(labelled: "空格", in: controller).pressSoundClass, .space)
    XCTAssertEqual(try key(labelled: "删除", in: controller).pressSoundClass, .backspace)
    XCTAssertEqual(try key(labelled: "换行", in: controller).pressSoundClass, .enter)
  }
}
