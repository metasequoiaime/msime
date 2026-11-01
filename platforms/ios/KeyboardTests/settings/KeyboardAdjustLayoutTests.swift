import UIKit
import XCTest

/// 「键盘调整」的换算与拖动约束（`KeyboardAdjustLayout`、`KeyboardAdjustSession`）、哪些柄显示（`KeyboardAdjustCapabilities`），以及摆放的本机存储（`KeyboardPlacement`）。三端按同一份契约：每个柄只动它那条边，对边不动；到限停住；拖动时宽度不小于 240pt 和 60% 中较宽的那个。
@MainActor
final class KeyboardAdjustLayoutTests: XCTestCase {
  private let placementKeys = [KeyboardLayoutPreference.bottomLiftKey, KeyboardLayoutPreference.widthPercentKey,
                               KeyboardLayoutPreference.positionXKey]
  private var saved: [String: Any] = [:]

  override func setUp() {
    super.setUp()
    for key in placementKeys { saved[key] = KeyboardLayoutPreference.defaults.object(forKey: key) }
  }

  override func tearDown() {
    for key in placementKeys {
      if let value = saved[key] { KeyboardLayoutPreference.defaults.set(value, forKey: key) }
      else { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    }
    super.tearDown()
  }

  private func state(height: Int = 0, lift: Int = 0, width: Int = 100, position: Int = 500) -> KeyboardAdjustState {
    KeyboardAdjustState(heightAdjustment: height, placement: KeyboardPlacement(bottomLift: lift, widthPercent: width, positionX: position))
  }

  private func edges(_ state: KeyboardAdjustState, _ container: CGFloat) -> (left: CGFloat, right: CGFloat) {
    let frame = KeyboardAdjustLayout.frame(containerWidth: container, placement: state.placement, capabilities: .all)
    return (frame.leading, frame.leading + frame.width)
  }

  func testStorageIsLocalWithTheSharedRangesAndDefaults() {
    XCTAssertEqual(KeyboardLayoutPreference.bottomLiftKey, "keyboard.bottomLift")
    XCTAssertEqual(KeyboardLayoutPreference.widthPercentKey, "keyboard.widthPercent")
    XCTAssertEqual(KeyboardLayoutPreference.positionXKey, "keyboard.positionX")
    for key in placementKeys { KeyboardLayoutPreference.defaults.removeObject(forKey: key) }
    XCTAssertEqual(KeyboardLayoutPreference.placement, KeyboardPlacement(bottomLift: 0, widthPercent: 100, positionX: 500))
    KeyboardLayoutPreference.placement = KeyboardPlacement(bottomLift: 24, widthPercent: 80, positionX: 250)
    XCTAssertEqual(KeyboardLayoutPreference.defaults.integer(forKey: KeyboardLayoutPreference.bottomLiftKey), 24)
    XCTAssertEqual(KeyboardLayoutPreference.placement, KeyboardPlacement(bottomLift: 24, widthPercent: 80, positionX: 250))
    // 超出范围的存储值限制到范围内，不是数字的按缺省。
    KeyboardLayoutPreference.defaults.set(500, forKey: KeyboardLayoutPreference.bottomLiftKey)
    KeyboardLayoutPreference.defaults.set(10, forKey: KeyboardLayoutPreference.widthPercentKey)
    KeyboardLayoutPreference.defaults.set("left", forKey: KeyboardLayoutPreference.positionXKey)
    XCTAssertEqual(KeyboardLayoutPreference.placement, KeyboardPlacement(bottomLift: 160, widthPercent: 60, positionX: 500))
    KeyboardLayoutPreference.defaults.set(true, forKey: KeyboardLayoutPreference.bottomLiftKey)
    XCTAssertEqual(KeyboardLayoutPreference.placement.bottomLift, 0, "a boolean is not a lift")
    // 超出 `Int` 的存储值不能让 `Int(Double)` 崩掉键盘，照样限制到范围内。
    KeyboardLayoutPreference.defaults.set(1e300, forKey: KeyboardLayoutPreference.bottomLiftKey)
    KeyboardLayoutPreference.defaults.set(-1e300, forKey: KeyboardLayoutPreference.widthPercentKey)
    XCTAssertEqual(KeyboardLayoutPreference.placement, KeyboardPlacement(bottomLift: 160, widthPercent: 60, positionX: 500))
  }

  func testCapabilitiesFollowTheFormFactorAndTheKeyboardMode() {
    let phonePortrait = KeyboardAdjustCapabilities.resolve(idiom: .phone, formFactor: .phone, landscape: false, oneHanded: false, split: false)
    XCTAssertEqual(phonePortrait, KeyboardAdjustCapabilities(placement: true, width: true))
    XCTAssertTrue(phonePortrait.showsBottomHandle && phonePortrait.showsSideHandles && phonePortrait.showsMove)
    let phoneLandscape = KeyboardAdjustCapabilities.resolve(idiom: .phone, formFactor: .phone, landscape: true, oneHanded: false, split: false)
    XCTAssertEqual(phoneLandscape, KeyboardAdjustCapabilities(placement: false, width: false), "iPhone landscape only adjusts the height")
    XCTAssertFalse(phoneLandscape.showsMove)
    let floating = KeyboardAdjustCapabilities.resolve(idiom: .pad, formFactor: .phone, landscape: false, oneHanded: false, split: false)
    XCTAssertEqual(floating, KeyboardAdjustCapabilities(placement: false, width: false), "nor does an iPad keyboard drawn at phone size")
    XCTAssertEqual(KeyboardAdjustCapabilities.resolve(idiom: .pad, formFactor: .tablet, landscape: true, oneHanded: false, split: false),
                   KeyboardAdjustCapabilities(placement: true, width: true))
    let oneHanded = KeyboardAdjustCapabilities.resolve(idiom: .phone, formFactor: .phone, landscape: false, oneHanded: true, split: false)
    XCTAssertEqual(oneHanded, KeyboardAdjustCapabilities(placement: true, width: false))
    XCTAssertFalse(oneHanded.showsSideHandles)
    XCTAssertTrue(oneHanded.showsMove, "move still lifts")
    XCTAssertEqual(KeyboardAdjustCapabilities.resolve(idiom: .pad, formFactor: .tablet, landscape: true, oneHanded: false, split: true),
                   KeyboardAdjustCapabilities(placement: true, width: false))
  }

  func testTheFrameIsTheWidthAtItsPositionInTheRemainingRoom() {
    let full = KeyboardAdjustLayout.frame(containerWidth: 400, placement: .standard, capabilities: .all)
    XCTAssertEqual(full, KeyboardAdjustFrame(leading: 0, width: 400, bottomLift: 0))
    let centred = KeyboardAdjustLayout.frame(
      containerWidth: 400, placement: KeyboardPlacement(bottomLift: 20, widthPercent: 80, positionX: 500), capabilities: .all)
    XCTAssertEqual(centred, KeyboardAdjustFrame(leading: 40, width: 320, bottomLift: 20))
    XCTAssertEqual(KeyboardAdjustLayout.frame(
      containerWidth: 400, placement: KeyboardPlacement(bottomLift: 0, widthPercent: 80, positionX: 0), capabilities: .all).leading, 0)
    XCTAssertEqual(KeyboardAdjustLayout.frame(
      containerWidth: 400, placement: KeyboardPlacement(bottomLift: 0, widthPercent: 80, positionX: 1000), capabilities: .all).leading, 80)
    // 不生效的项按缺省画；存下的值不变。
    let stored = KeyboardPlacement(bottomLift: 20, widthPercent: 80, positionX: 0)
    XCTAssertEqual(KeyboardAdjustLayout.frame(containerWidth: 400, placement: stored, capabilities: KeyboardAdjustCapabilities(placement: true, width: false)),
                   KeyboardAdjustFrame(leading: 0, width: 400, bottomLift: 20))
    XCTAssertEqual(KeyboardAdjustLayout.frame(containerWidth: 400, placement: stored, capabilities: KeyboardAdjustCapabilities(placement: false, width: false)),
                   KeyboardAdjustFrame(leading: 0, width: 400, bottomLift: 0))
    // 60% 画不到 240pt 时按 240 画，视图本身更窄时占满。
    XCTAssertEqual(KeyboardAdjustLayout.width(percent: 60, containerWidth: 375), 240)
    XCTAssertEqual(KeyboardAdjustLayout.width(percent: 60, containerWidth: 200), 200)
  }

  func testTheDraggingFloorIsTheWiderOf240PointsAndSixtyPercent() {
    XCTAssertEqual(KeyboardAdjustLayout.minimumWidthPercent(containerWidth: 390), 62)
    XCTAssertEqual(KeyboardAdjustLayout.minimumWidthPercent(containerWidth: 1024), 60)
    XCTAssertEqual(KeyboardAdjustLayout.minimumWidthPercent(containerWidth: 200), 100)
    let narrowest = KeyboardAdjustLayout.draggingRight(state(), dx: -1000, containerWidth: 390)
    XCTAssertEqual(narrowest.placement.widthPercent, 62)
    XCTAssertGreaterThanOrEqual(edges(narrowest, 390).right - edges(narrowest, 390).left, 240)
    XCTAssertEqual(KeyboardAdjustLayout.draggingLeft(state(), dx: 1000, containerWidth: 1024).placement.widthPercent, 60)
  }

  func testTheTopEdgeChangesTheHeightAndStopsAtTheSharedRange() {
    XCTAssertEqual(KeyboardAdjustLayout.draggingTop(state(), rise: 10.4).heightAdjustment, 10)
    XCTAssertEqual(KeyboardAdjustLayout.draggingTop(state(), rise: 500).heightAdjustment, 48)
    XCTAssertEqual(KeyboardAdjustLayout.draggingTop(state(), rise: -500).heightAdjustment, -12)
    XCTAssertEqual(KeyboardAdjustLayout.draggingTop(state(lift: 30), rise: 5).placement.bottomLift, 30, "the bottom edge stays put")
  }

  /// 往上拖下沿 u：抬高跟着手指走、到 160 停；高度反向让出同样多使上沿不动，到下限 −12 后不再让，整块键盘接着上移。往下拖：抬高减、高度加同样多，上沿不动，哪一项先到限就停住。
  func testTheBottomEdgeTradesHeightForLiftWithTheTopEdgeFixed() {
    let raised = KeyboardAdjustLayout.draggingBottom(state(height: 20), rise: 8)
    XCTAssertEqual(raised.placement.bottomLift, 8)
    XCTAssertEqual(raised.heightAdjustment, 12)
    // 高度还没到下限的那一段，两者之和（上沿的位置）不变。
    for rise: CGFloat in [-300, -20, -3, 0, 7, 20, 32] {
      for start in [state(height: 20), state(height: 48, lift: 150), state(height: 0, lift: 30)] {
        let next = KeyboardAdjustLayout.draggingBottom(start, rise: rise)
        guard next.heightAdjustment > KeyboardAdjustLayout.heightRange.lowerBound || rise <= 0 else { continue }
        XCTAssertEqual(next.heightAdjustment + next.placement.bottomLift, start.heightAdjustment + start.placement.bottomLift,
                       "rise \(rise) from \(start)")
      }
    }
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: 48, lift: 150), rise: 100), state(height: 38, lift: 160),
                   "the lift stops at 160")
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: 40, lift: 0), rise: -50), state(height: 40, lift: 0),
                   "no lift to give back")
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: 40, lift: 30), rise: -50), state(height: 48, lift: 22),
                   "the height reaches 48 first")
  }

  /// 高度已在下限时往上拖，抬高照样跟着手指增加，高度不再变；一直到 160。
  func testTheBottomEdgeKeepsLiftingOnceTheHeightIsAtItsFloor() {
    let floor = KeyboardAdjustLayout.heightRange.lowerBound
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: floor, lift: 30), rise: 25), state(height: floor, lift: 55))
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: floor, lift: 30), rise: 500), state(height: floor, lift: 160))
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(state(height: floor, lift: 160), rise: 20), state(height: floor, lift: 160))
    // 读屏的增加只要抬高没到上限就有效。
    var session = KeyboardAdjustSession(state: state(height: floor, lift: 150), capabilities: .all, containerWidth: 390, keyBlock: 229)
    session.step(.bottom, increment: true)
    XCTAssertEqual(session.state, state(height: floor, lift: 154))
    session.step(.bottom, increment: true)
    session.step(.bottom, increment: true)
    XCTAssertEqual(session.state, state(height: floor, lift: 160))
  }

  /// 一次拖动跨过高度下限：前 12 pt 拿高度换抬高、上沿不动，之后整块上移；往回拖时按同一次拖动从按下的值重算。
  func testOneBottomDragCrossesTheHeightFloor() {
    let start = state(height: 0, lift: 0)
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(start, rise: 12), state(height: -12, lift: 12))
    XCTAssertEqual(KeyboardAdjustLayout.draggingBottom(start, rise: 40), state(height: -12, lift: 40))
    var session = KeyboardAdjustSession(state: start, capabilities: .all, containerWidth: 390, keyBlock: 229)
    session.beginDrag()
    session.drag(.bottom, dx: 0, rise: 40)
    XCTAssertEqual(session.state, state(height: -12, lift: 40))
    session.drag(.bottom, dx: 0, rise: 5)
    XCTAssertEqual(session.state, state(height: -5, lift: 5), "dragging back within the same drag")
    session.endDrag()
  }

  func testTheLeftEdgeChangesTheWidthWithTheRightEdgeFixed() {
    let container: CGFloat = 390
    var current = state()
    for dx: CGFloat in [20, 37, 80, -15, 200] {
      let right = edges(current, container).right
      current = KeyboardAdjustLayout.draggingLeft(current, dx: dx, containerWidth: container)
      XCTAssertEqual(edges(current, container).right, right, accuracy: 0.5, "dx \(dx)")
    }
    let narrowed = KeyboardAdjustLayout.draggingLeft(state(), dx: 39, containerWidth: container)
    XCTAssertEqual(narrowed.placement.widthPercent, 90)
    XCTAssertEqual(narrowed.placement.positionX, 1000, "the narrowed keyboard keeps the right edge")
    // 右边不贴视图边时往左拖到底：左边停在视图边上（宽度取整到 1%，差不到 1%），右边仍不动。
    let moved = KeyboardAdjustLayout.moving(narrowed, dx: -20, rise: 0, containerWidth: container, capabilities: .all)
    XCTAssertLessThan(edges(moved, container).right, container - 10)
    let widest = KeyboardAdjustLayout.draggingLeft(moved, dx: -1000, containerWidth: container)
    XCTAssertGreaterThanOrEqual(edges(widest, container).left, 0)
    XCTAssertEqual(edges(widest, container).left, 0, accuracy: container / 100)
    XCTAssertEqual(edges(widest, container).right, edges(moved, container).right, accuracy: 0.5)
  }

  func testTheRightEdgeChangesTheWidthWithTheLeftEdgeFixed() {
    let container: CGFloat = 390
    var current = KeyboardAdjustLayout.draggingLeft(state(), dx: 60, containerWidth: container)
    for dx: CGFloat in [-30, -11, 25, 300] {
      let left = edges(current, container).left
      current = KeyboardAdjustLayout.draggingRight(current, dx: dx, containerWidth: container)
      XCTAssertEqual(edges(current, container).left, left, accuracy: 0.5, "dx \(dx)")
    }
    XCTAssertLessThanOrEqual(edges(current, container).right, container + 0.5, "never past the view")
    XCTAssertEqual(KeyboardAdjustLayout.draggingRight(state(), dx: 50, containerWidth: container), state(), "already full width")
  }

  func testMoveKeepsTheSizeAndStaysInsideTheView() {
    let container: CGFloat = 400
    let start = state(height: 6, lift: 10, width: 80, position: 500)
    let moved = KeyboardAdjustLayout.moving(start, dx: 20, rise: 15, containerWidth: container, capabilities: .all)
    XCTAssertEqual(moved, state(height: 6, lift: 25, width: 80, position: 750))
    XCTAssertEqual(KeyboardAdjustLayout.moving(start, dx: 999, rise: 999, containerWidth: container, capabilities: .all),
                   state(height: 6, lift: 160, width: 80, position: 1000))
    XCTAssertEqual(KeyboardAdjustLayout.moving(start, dx: -999, rise: -999, containerWidth: container, capabilities: .all),
                   state(height: 6, lift: 0, width: 80, position: 0))
    XCTAssertEqual(KeyboardAdjustLayout.moving(start, dx: 20, rise: 15, containerWidth: container,
                                               capabilities: KeyboardAdjustCapabilities(placement: true, width: false)),
                   state(height: 6, lift: 25, width: 80, position: 500), "without width only the lift moves")
    XCTAssertEqual(KeyboardAdjustLayout.moving(start, dx: 20, rise: 15, containerWidth: container,
                                               capabilities: KeyboardAdjustCapabilities(placement: false, width: false)), start)
  }

  /// 每一帧都从按下时的值重算：拖过了端点再往回拖，立刻跟着回来。不生效的柄不改任何值。
  func testTheSessionRecomputesEachDragFromWhereItStarted() {
    var session = KeyboardAdjustSession(state: state(), capabilities: .all, containerWidth: 390, keyBlock: 229)
    session.beginDrag()
    session.drag(.top, dx: 0, rise: 200)
    XCTAssertEqual(session.state.heightAdjustment, 48)
    session.drag(.top, dx: 0, rise: 10)
    XCTAssertEqual(session.state.heightAdjustment, 10)
    session.endDrag()
    session.beginDrag()
    session.drag(.top, dx: 0, rise: 5)
    XCTAssertEqual(session.state.heightAdjustment, 15)
    session.endDrag()

    session.capabilities = KeyboardAdjustCapabilities(placement: true, width: false)
    let before = session.state
    session.step(.left, increment: false)
    session.beginDrag()
    session.drag(.right, dx: -50, rise: 0)
    XCTAssertEqual(session.state, before, "the side handles do nothing while the width does not apply")
    session.endDrag()
    session.capabilities = KeyboardAdjustCapabilities(placement: false, width: false)
    session.step(.bottom, increment: true)
    session.step(.move, increment: true)
    XCTAssertEqual(session.state, before)

    session.reset()
    XCTAssertEqual(session.state, .standard)
    XCTAssertEqual(KeyboardAdjustState.standard, state(height: 0, lift: 0, width: 100, position: 500))
  }

  /// 读屏步长：高度 5%、抬高 4 pt、宽度 5%、位置 50‰；「移动」上下调抬高、左右调位置。
  func testScreenReaderSteps() {
    var session = KeyboardAdjustSession(state: state(), capabilities: .all, containerWidth: 400, keyBlock: 229)
    session.step(.top, increment: true)
    XCTAssertEqual(KeyboardHeightPercent.percent(adjustment: CGFloat(session.state.heightAdjustment), keyBlock: 229), 105)
    session.step(.bottom, increment: true)
    XCTAssertEqual(session.state.placement.bottomLift, 4)
    XCTAssertEqual(session.state.heightAdjustment, 11 - 4, "the height gives the lift back, so the top edge stays put")
    session.step(.left, increment: false)
    XCTAssertEqual(session.state.placement.widthPercent, 95)
    XCTAssertEqual(session.state.placement.positionX, 1000)
    session.step(.right, increment: false)
    XCTAssertEqual(session.state.placement.widthPercent, 90)
    session.stepPosition(right: false)
    XCTAssertEqual(session.state.placement.positionX, 450, "left and right on 移动 move by 50‰")
    session.step(.move, increment: true)
    XCTAssertEqual(session.state.placement.bottomLift, 8, "up and down on 移动 lift by 4 pt")
    XCTAssertEqual(session.state.heightAdjustment, 7, "without changing the size")
    session.capabilities = KeyboardAdjustCapabilities(placement: true, width: false)
    session.stepPosition(right: true)
    XCTAssertEqual(session.state.placement.positionX, 450, "no sideways move while the width does not apply")
    session.step(.move, increment: true)
    XCTAssertEqual(session.state.placement.bottomLift, 12)
  }
}
