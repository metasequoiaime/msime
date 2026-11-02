import UIKit
import XCTest

/// 上屏后联想（#9433）在 iOS 上的规则：联想和普通候选一样选，轻触、数字键选本页对应的一条、空格选高亮的一条；回车和标点照常输入，关闭键送取消。联想时候选栏右端的按键按「联想栏按键」（`next_word_prediction_buttons`）的三种样式画，与 Android 的 `PredictionButtonsPolicySmoke` 钉住的规则相同。
@MainActor
final class NextWordPredictionTests: XCTestCase {
  func testButtonStyleReadsTheThreeWritingsAndFallsBackToExpandOnly() {
    XCTAssertEqual(NextWordPredictionButtons.documentKey, "next_word_prediction_buttons")
    XCTAssertEqual(NextWordPredictionButtons.enabledKey, "next_word_prediction")
    // 写法与 client-core 的 NextWordPredictionButtons 一一对应。
    for style in NextWordPredictionButtons.allCases {
      XCTAssertEqual(NextWordPredictionButtons.style(in: [NextWordPredictionButtons.documentKey: style.rawValue]), style)
    }
    XCTAssertEqual(NextWordPredictionButtons.allCases.map(\.rawValue), ["expand_only", "close_and_expand", "close_only"])
    XCTAssertEqual(NextWordPredictionButtons.style(in: nil), .expandOnly, "文档缺键时是现有样式")
    XCTAssertEqual(NextWordPredictionButtons.style(in: [NextWordPredictionButtons.documentKey: "close_left"]), .expandOnly,
                   "较新版本加的写法按现有样式读")
    XCTAssertEqual(NextWordPredictionButtons.style(in: [NextWordPredictionButtons.documentKey: 1]), .expandOnly)
    XCTAssertEqual(NextWordPredictionButtons.allCases.map(\.title), ["只有展开 ∨", "关闭 × 和展开 ∨", "只有关闭 ×"])
  }

  func testCloseKeyOnlyDuringPredictionAndCloseOnlyDropsTheArrow() {
    XCTAssertFalse(NextWordPredictionButtons.expandOnly.showsClose(prediction: true))
    XCTAssertTrue(NextWordPredictionButtons.expandOnly.allowsExpand(prediction: true))
    XCTAssertTrue(NextWordPredictionButtons.closeAndExpand.showsClose(prediction: true))
    XCTAssertTrue(NextWordPredictionButtons.closeAndExpand.allowsExpand(prediction: true))
    XCTAssertTrue(NextWordPredictionButtons.closeOnly.showsClose(prediction: true))
    XCTAssertFalse(NextWordPredictionButtons.closeOnly.allowsExpand(prediction: true))
    for style in NextWordPredictionButtons.allCases {
      XCTAssertFalse(style.showsClose(prediction: false), "组字时没有关闭键：\(style)")
      XCTAssertTrue(style.allowsExpand(prediction: false), "组字时展开键照旧：\(style)")
    }
  }

  func testDigitsGoToTheSessionOnlyWhilePredicting() {
    for digit in "0123456789" {
      XCTAssertTrue(KeyboardViewController.digitGoesToSessionDuringPrediction(String(digit), prediction: true))
      XCTAssertFalse(KeyboardViewController.digitGoesToSessionDuringPrediction(String(digit), prediction: false),
                     "不在联想时数字仍按候选编号处理")
    }
    XCTAssertFalse(KeyboardViewController.digitGoesToSessionDuringPrediction(",", prediction: true), "标点走自己的路线")
    XCTAssertFalse(KeyboardViewController.digitGoesToSessionDuringPrediction("12", prediction: true))
    // 越过词块的数字什么也不做只属于组字；联想时键盘把数字交给会话，由运行时决定选中还是照常输入。
    XCTAssertFalse(KeyboardViewController.digitHasNoChip("9", chips: 3, composing: false))
  }

  func testSnapshotDecodesThePredictionFlagStrictly() throws {
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertTrue(try bridge.snapshot(from: try Self.predictionFixture(flag: "true")).isPrediction)
    XCTAssertFalse(try bridge.snapshot(from: try Self.predictionFixture(flag: "false")).isPrediction)
    XCTAssertFalse(try bridge.snapshot(from: try Self.predictionFixture(flag: nil)).isPrediction, "普通状态省略这个字段")
    XCTAssertThrowsError(try bridge.snapshot(from: try Self.predictionFixture(flag: "\"true\"")))
    XCTAssertTrue(try bridge.snapshot(from: try Self.predictionFixture(flag: "true")).replacingCommit("，").isPrediction)
  }

  /// 真实会话：打开偏好后选「你」上屏，会话给出联想、没有组字；数字键（键盘在联想时作为字符交给会话）选本页对应的一条，空格（`SelectHighlighted`）选高亮的一条，越过本页的数字以未处理返回、不带上屏；取消报已处理；轻触（按身份选择）只上屏联想本身；重新聚焦清掉联想。
  func testTheIOSSessionPredictsAfterACommitAndPicksLikeOrdinaryCandidates() throws {
    let state = FileManager.default.temporaryDirectory
      .appendingPathComponent("msime-next-word-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: state) }
    XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences(stateRoot: state) {
      $0[NextWordPredictionButtons.enabledKey] = true
    })
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)
    XCTAssertEqual(bridge.sharedPreferences?[NextWordPredictionButtons.enabledKey] as? Bool, true)

    func predictAfterNi() throws -> MetasequoiaInputSnapshot {
      var snapshot = bridge.cancel()
      for letter in "ni" { snapshot = bridge.handleCharacter(String(letter)) }
      let index = try XCTUnwrap(snapshot.candidates.firstIndex(of: "你"), "ni 没有「你」：\(snapshot.candidates)")
      let identity = try XCTUnwrap(snapshot.candidateIdentities[index])
      let committed = bridge.selectCandidate(generation: identity.generation, globalIndex: identity.index)
      XCTAssertEqual(committed.commitText, "你")
      return committed
    }

    let prediction = try predictAfterNi()
    XCTAssertTrue(prediction.isPrediction, "上屏后没有联想")
    XCTAssertFalse(prediction.candidates.isEmpty)
    XCTAssertEqual(prediction.preedit, "")
    XCTAssertEqual(prediction.editingText, "")

    let first = bridge.handleCharacter("1")
    XCTAssertTrue(first.isHandled, "本页内的数字由会话选中，不再写进文档")
    XCTAssertEqual(first.commitText, prediction.candidates.first)

    let again = try predictAfterNi()
    if again.candidates.count >= 2 {
      XCTAssertEqual(bridge.handleCharacter("2").commitText, again.candidates[1])
    }

    let spaced = try predictAfterNi()
    let space = bridge.commitCandidate()
    XCTAssertTrue(space.isHandled, "空格选中联想，不再插入空格")
    XCTAssertEqual(space.commitText, spaced.candidates.first, "空格选高亮的一条，即首条")

    // 联想最多八行，九个数字键总有越过本页的。
    let beyond = try predictAfterNi()
    XCTAssertLessThan(beyond.candidates.count, 9)
    let past = bridge.handleCharacter(String(min(beyond.candidates.count + 1, 9)))
    XCTAssertFalse(past.isHandled, "越过本页的数字交还编辑器照常输入")
    XCTAssertNil(past.commitText)
    XCTAssertFalse(past.isPrediction)

    XCTAssertTrue(try predictAfterNi().isPrediction)
    let cancelled = bridge.cancel()
    XCTAssertTrue(cancelled.isHandled, "关闭键送的取消报已处理，宿主不往文档写字")
    XCTAssertNil(cancelled.commitText)
    XCTAssertTrue(cancelled.candidates.isEmpty)

    let next = try predictAfterNi()
    let identity = try XCTUnwrap(next.candidateIdentities.first ?? nil)
    let selected = bridge.selectCandidate(generation: identity.generation, globalIndex: identity.index)
    XCTAssertEqual(selected.commitText, next.candidates.first, "轻触只上屏联想本身，不重复已上屏的「你」")

    XCTAssertTrue(try predictAfterNi().isPrediction)
    let refocused = bridge.refocus()
    XCTAssertNil(refocused.commitText, "重新获得焦点不提交联想")
    XCTAssertTrue(refocused.candidates.isEmpty)
  }

  /// 候选条画联想时，展开键和关闭键按样式出现；默认样式和 Android 现有的一样只有展开键。
  func testTheStripDrawsTheChosenButtonsForAPrediction() throws {
    let stored = MetasequoiaInputSessionBridge.loadSharedPreferences()?[NextWordPredictionButtons.documentKey]
    addTeardownBlock {
      _ = MetasequoiaInputSessionBridge.updateSharedPreferences { $0[NextWordPredictionButtons.documentKey] = stored }
    }
    let previous = InputSchemePreference.scheme
    addTeardownBlock { InputSchemePreference.scheme = previous }
    InputSchemePreference.scheme = .quanpin
    let state = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: state) }
    let bridge = MetasequoiaInputSessionBridge(stateRoot: state)

    func strip(_ style: NextWordPredictionButtons?, flag: String?) throws -> (close: Bool, expand: Bool, divider: Bool) {
      XCTAssertTrue(MetasequoiaInputSessionBridge.updateSharedPreferences {
        $0[NextWordPredictionButtons.documentKey] = style?.rawValue
      })
      let controller = KeyboardViewController()
      controller.loadViewIfNeeded()
      controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: KeyboardViewController.defaultKeyboardHeight)
      controller.viewWillAppear(false)
      defer { controller.viewDidDisappear(false) }
      controller.render(try bridge.snapshot(from: try Self.predictionFixture(flag: flag)))
      controller.view.layoutIfNeeded()
      func shown(_ identifier: String) throws -> Bool {
        !(try XCTUnwrap(Self.nodes(controller.view).first { $0.accessibilityIdentifier == identifier }, identifier).isHidden)
      }
      return (try shown("closePrediction"), try shown("expandCandidates"), try shown("expandCandidatesDivider"))
    }

    var buttons = try strip(nil, flag: "true")
    XCTAssertFalse(buttons.close)
    XCTAssertTrue(buttons.expand)
    buttons = try strip(.closeAndExpand, flag: "true")
    XCTAssertTrue(buttons.close && buttons.expand && buttons.divider)
    buttons = try strip(.closeOnly, flag: "true")
    XCTAssertTrue(buttons.close && buttons.divider)
    XCTAssertFalse(buttons.expand)
    // 不是联想的那一帧（这里没有组字也没有联想标记）不画关闭键，也不给不归会话的候选展开键。
    buttons = try strip(.closeOnly, flag: nil)
    XCTAssertFalse(buttons.close || buttons.expand || buttons.divider)
  }

  private static func nodes(_ view: UIView) -> [UIView] { [view] + view.subviews.flatMap(nodes) }

  /// host-api 上屏后联想的 View JSON：没有预编辑、拼写和读音，候选带身份；`flag` 为 nil 时不写 `prediction`。
  static func predictionFixture(flag: String?) throws -> [String: Any] {
    let field = flag.map { #", "prediction": \#($0)"# } ?? ""
    let json = """
    {"handled": true, "view": {"preedit": "", "reading": "", "editing_text": "", "generation": 7, "page_count": 1\(field), "candidates": [
      {"id": {"session": 1, "generation": 7, "index": 0}, "text": "们", "source": 0, "fixed_position": 0},
      {"id": {"session": 1, "generation": 7, "index": 1}, "text": "好", "source": 0, "fixed_position": 0}
    ]}}
    """
    return try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
  }
}
