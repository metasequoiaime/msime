import UIKit

/// 「键盘调整」此刻哪些项生效，也就决定遮罩上显示哪些柄。高度（上沿柄）总是生效。
///
/// - 底部抬高和「移动」（`placement`）：iPhone 横屏、iPad 上手机形态的键盘（浮动键盘、Slide Over 和台前调度缩成手机尺寸的窗口）不生效，只留上沿柄、不显示「移动」。
/// - 宽度和水平位置（`width`，左右柄、「移动」的横向分量）：在 `placement` 之外，单手模式和分离式键盘画着时也不生效，隐藏左右柄，「移动」只改抬高。
///
/// 存下的值不因为此刻不生效而改变，回到生效的形态时照旧画出来。三端按同一份规则。
struct KeyboardAdjustCapabilities: Equatable {
  let placement: Bool
  let width: Bool

  static let all = KeyboardAdjustCapabilities(placement: true, width: true)

  static func resolve(idiom: UIUserInterfaceIdiom, formFactor: KeyboardFormFactor, landscape: Bool,
                      oneHanded: Bool, split: Bool) -> KeyboardAdjustCapabilities {
    // 全宽 iPad 键盘两个方向都生效；手机形态只有真 iPhone 竖屏生效，iPad 上的手机形态是浮动键盘或窄窗口。
    let placement = formFactor == .tablet || (idiom != .pad && !landscape)
    return KeyboardAdjustCapabilities(placement: placement, width: placement && !oneHanded && !split)
  }

  var showsBottomHandle: Bool { placement }
  var showsSideHandles: Bool { width }
  var showsMove: Bool { placement }
}

/// 「键盘调整」预览和保存的四项：高度（共享的 `touch_keyboard_height_adjustment`，点）和本机的摆放（`KeyboardPlacement`）。
struct KeyboardAdjustState: Equatable {
  let heightAdjustment: Int
  let placement: KeyboardPlacement

  /// 「重置」回到的值：高度 0、抬高 0、宽度 100%、居中。
  static let standard = KeyboardAdjustState(heightAdjustment: 0, placement: .standard)

  init(heightAdjustment: Int, placement: KeyboardPlacement) {
    self.heightAdjustment = min(max(heightAdjustment, KeyboardAdjustLayout.heightRange.lowerBound),
                                KeyboardAdjustLayout.heightRange.upperBound)
    self.placement = placement
  }
}

/// 键盘在键盘视图里实际占的横向位置和底部抬高，单位 pt。内边距（`KeyboardFormFactor.padding`）另算在这块里面。
struct KeyboardAdjustFrame: Equatable {
  let leading: CGFloat
  let width: CGFloat
  let bottomLift: CGFloat
}

/// 「键盘调整」的换算与拖动约束，键盘和它的测试共用。拖动量一律用屏幕上不动的坐标：`dx` 向右为正，`rise` 向上为正。
///
/// 每个柄只动它那条边，对边在屏幕上不动：上沿改高度；下沿改抬高，高度反向变化使上沿不动（往上拖到高度下限后整块继续上移）；左右沿改宽度，同时改水平位置使另一侧边不动。「移动」同时改抬高和水平位置，大小不变。任何一项到了范围的端点，这次拖动就停在那里。
enum KeyboardAdjustLayout {
  /// 拖动收窄时键盘至少这么宽；与 60% 下限取更宽的那个。
  static let minimumWidth: CGFloat = 240
  /// 高度沿用共享字段的范围（`KeyboardHeightPercent.adjustmentRange`）。
  static let heightRange = Int(KeyboardHeightPercent.adjustmentRange.lowerBound)...Int(KeyboardHeightPercent.adjustmentRange.upperBound)
  /// 读屏调节的步长：高度 5%（按键区的百分比）、抬高 4 pt、宽度 5%、位置 50‰。
  static let heightStepPercent = 5
  static let liftStep = 4
  static let widthStepPercent = 5
  static let positionStep = 50

  /// 宽度 `percent` 在宽 `containerWidth` 的键盘视图里画出的宽度：不窄于 `minimumWidth`，也不宽于视图本身。
  static func width(percent: Int, containerWidth: CGFloat) -> CGFloat {
    let container = max(0, containerWidth)
    return min(container, max(container * CGFloat(percent) / 100, minimumWidth))
  }

  /// 存下的摆放在此刻画出的位置：不生效的项按缺省画（不抬高、全宽）。
  static func frame(containerWidth: CGFloat, placement: KeyboardPlacement,
                    capabilities: KeyboardAdjustCapabilities) -> KeyboardAdjustFrame {
    let container = max(0, containerWidth)
    let lift = capabilities.placement ? CGFloat(placement.bottomLift) : 0
    guard capabilities.width else { return KeyboardAdjustFrame(leading: 0, width: container, bottomLift: lift) }
    let width = Self.width(percent: placement.widthPercent, containerWidth: container)
    return KeyboardAdjustFrame(leading: (container - width) * CGFloat(placement.positionX) / 1000, width: width, bottomLift: lift)
  }

  /// 拖动时宽度百分比的下限：60%，或者 `minimumWidth` 占视图宽度的百分比（向上取整），取大的。
  static func minimumWidthPercent(containerWidth: CGFloat) -> Int {
    guard containerWidth > 0 else { return KeyboardPlacement.widthPercentRange.upperBound }
    let floor = Int((minimumWidth / containerWidth * 100).rounded(.up))
    return min(KeyboardPlacement.widthPercentRange.upperBound, max(KeyboardPlacement.widthPercentRange.lowerBound, floor))
  }

  /// 上沿：往上拖变高，下沿不动。
  static func draggingTop(_ start: KeyboardAdjustState, rise: CGFloat) -> KeyboardAdjustState {
    KeyboardAdjustState(heightAdjustment: start.heightAdjustment + Int(rise.rounded()), placement: start.placement)
  }

  /// 下沿。往上拖 u：抬高跟着手指走，到 160 才停；高度尽量反向让出同样多、上沿不动，高度到下限（−12）之后不再让，整块键盘接着往上移。往下拖：抬高减、高度加同样多，上沿不动，抬高到 0 或高度到上限就停住。
  static func draggingBottom(_ start: KeyboardAdjustState, rise: CGFloat) -> KeyboardAdjustState {
    let lift = start.placement.bottomLift, height = start.heightAdjustment
    let move = Int(rise.rounded())
    let nextLift: Int, nextHeight: Int
    if move >= 0 {
      nextLift = min(lift + move, KeyboardPlacement.bottomLiftRange.upperBound)
      nextHeight = height - min(nextLift - lift, max(0, height - heightRange.lowerBound))
    } else {
      let step = max(move, max(KeyboardPlacement.bottomLiftRange.lowerBound - lift, height - heightRange.upperBound))
      nextLift = lift + step
      nextHeight = height - step
    }
    return KeyboardAdjustState(
      heightAdjustment: nextHeight,
      placement: KeyboardPlacement(bottomLift: nextLift, widthPercent: start.placement.widthPercent,
                                   positionX: start.placement.positionX))
  }

  /// 左沿：往左拖变宽，右边不动。
  static func draggingLeft(_ start: KeyboardAdjustState, dx: CGFloat, containerWidth: CGFloat) -> KeyboardAdjustState {
    let frame = Self.frame(containerWidth: containerWidth, placement: start.placement, capabilities: .all)
    let right = frame.leading + frame.width
    let percent = widthPercent(target: frame.width - dx, room: right, containerWidth: containerWidth)
    let width = Self.width(percent: percent, containerWidth: containerWidth)
    return resized(start, percent: percent, leading: right - width, width: width, containerWidth: containerWidth)
  }

  /// 右沿：往右拖变宽，左边不动。
  static func draggingRight(_ start: KeyboardAdjustState, dx: CGFloat, containerWidth: CGFloat) -> KeyboardAdjustState {
    let frame = Self.frame(containerWidth: containerWidth, placement: start.placement, capabilities: .all)
    let percent = widthPercent(target: frame.width + dx, room: containerWidth - frame.leading, containerWidth: containerWidth)
    return resized(start, percent: percent, leading: frame.leading,
                   width: Self.width(percent: percent, containerWidth: containerWidth), containerWidth: containerWidth)
  }

  /// 「移动」：大小不变，抬高跟着手指上下、水平位置跟着手指左右；宽度不生效时只改抬高。
  static func moving(_ start: KeyboardAdjustState, dx: CGFloat, rise: CGFloat, containerWidth: CGFloat,
                     capabilities: KeyboardAdjustCapabilities) -> KeyboardAdjustState {
    guard capabilities.placement else { return start }
    let lift = start.placement.bottomLift + Int(rise.rounded())
    var position = start.placement.positionX
    if capabilities.width {
      let frame = Self.frame(containerWidth: containerWidth, placement: start.placement, capabilities: .all)
      let room = max(0, containerWidth - frame.width)
      position = Self.position(leading: min(max(frame.leading + dx, 0), room), width: frame.width,
                               containerWidth: containerWidth, fallback: position)
    }
    return KeyboardAdjustState(
      heightAdjustment: start.heightAdjustment,
      placement: KeyboardPlacement(bottomLift: lift, widthPercent: start.placement.widthPercent, positionX: position))
  }

  /// 读屏的上沿柄：按键区百分比加减 `heightStepPercent`，与 App「键盘」页显示的百分比同一个换算。
  static func steppingHeight(_ start: KeyboardAdjustState, increment: Bool, keyBlock: CGFloat) -> KeyboardAdjustState {
    let percent = KeyboardHeightPercent.percent(adjustment: CGFloat(start.heightAdjustment), keyBlock: keyBlock)
      + (increment ? heightStepPercent : -heightStepPercent)
    return KeyboardAdjustState(
      heightAdjustment: Int(KeyboardHeightPercent.adjustment(percent: percent, keyBlock: keyBlock)), placement: start.placement)
  }

  /// 读屏的「移动」上下方向：抬高加减 `liftStep`，大小不变。
  static func steppingLift(_ start: KeyboardAdjustState, up: Bool, capabilities: KeyboardAdjustCapabilities) -> KeyboardAdjustState {
    guard capabilities.placement else { return start }
    let placement = start.placement
    return KeyboardAdjustState(
      heightAdjustment: start.heightAdjustment,
      placement: KeyboardPlacement(bottomLift: placement.bottomLift + (up ? liftStep : -liftStep),
                                   widthPercent: placement.widthPercent, positionX: placement.positionX))
  }

  /// 读屏的「移动」左右方向：水平位置加减 `positionStep`（向右为增）；宽度不生效时不能左右移。
  static func steppingPosition(_ start: KeyboardAdjustState, right: Bool, capabilities: KeyboardAdjustCapabilities) -> KeyboardAdjustState {
    guard capabilities.width else { return start }
    let placement = start.placement
    return KeyboardAdjustState(
      heightAdjustment: start.heightAdjustment,
      placement: KeyboardPlacement(bottomLift: placement.bottomLift, widthPercent: placement.widthPercent,
                                   positionX: placement.positionX + (right ? positionStep : -positionStep)))
  }

  /// 拖到 `target` 宽时存的百分比：取整到整数，限制在拖动下限和 100 之间，也不超过不动的那条边留给它的 `room`。
  private static func widthPercent(target: CGFloat, room: CGFloat, containerWidth: CGFloat) -> Int {
    guard containerWidth > 0 else { return KeyboardPlacement.widthPercentRange.upperBound }
    let lower = minimumWidthPercent(containerWidth: containerWidth)
    // 加一点余量再向下取整，免得 100% 宽、右边贴着视图边时因为浮点误差只剩 99%。
    let roomPercent = Int((room / containerWidth * 100 + 0.001).rounded(.down))
    let upper = max(lower, min(KeyboardPlacement.widthPercentRange.upperBound, roomPercent))
    return min(max(Int((target / containerWidth * 100).rounded()), lower), upper)
  }

  private static func resized(_ start: KeyboardAdjustState, percent: Int, leading: CGFloat, width: CGFloat,
                              containerWidth: CGFloat) -> KeyboardAdjustState {
    let position = Self.position(leading: leading, width: width, containerWidth: containerWidth, fallback: start.placement.positionX)
    return KeyboardAdjustState(
      heightAdjustment: start.heightAdjustment,
      placement: KeyboardPlacement(bottomLift: start.placement.bottomLift, widthPercent: percent, positionX: position))
  }

  /// 左边在 `leading` 时的水平位置千分比；键盘占满整行、没有空出的宽度时位置无意义，保留原值。
  private static func position(leading: CGFloat, width: CGFloat, containerWidth: CGFloat, fallback: Int) -> Int {
    let room = containerWidth - width
    guard room > 0.5 else { return fallback }
    return Int((leading / room * 1000).rounded())
  }
}

/// 遮罩打开期间的预览：从打开时的值开始，拖动和读屏调节都改它，键盘按它画；「确认」或任何离开方式把它存下。
struct KeyboardAdjustSession {
  enum Part { case top, bottom, left, right, move }

  private(set) var state: KeyboardAdjustState
  /// 键盘视图此刻的能力、宽度和按键区高度；转屏、浮动与停靠切换后由键盘更新。
  var capabilities: KeyboardAdjustCapabilities
  var containerWidth: CGFloat
  var keyBlock: CGFloat
  private var dragStart: KeyboardAdjustState?

  init(state: KeyboardAdjustState, capabilities: KeyboardAdjustCapabilities, containerWidth: CGFloat, keyBlock: CGFloat) {
    self.state = state
    self.capabilities = capabilities
    self.containerWidth = containerWidth
    self.keyBlock = keyBlock
  }

  /// 此刻能动的柄：不生效的项既不显示也不接受调整。
  func allows(_ part: Part) -> Bool {
    switch part {
    case .top: true
    case .bottom: capabilities.showsBottomHandle
    case .left, .right: capabilities.showsSideHandles
    case .move: capabilities.showsMove
    }
  }

  mutating func beginDrag() { dragStart = state }

  /// 拖动 `part`，`dx`、`rise` 是从按下到现在的位移；每一帧都从按下时的值重算，所以停在端点后往回拖会立刻回来。
  mutating func drag(_ part: Part, dx: CGFloat, rise: CGFloat) {
    guard allows(part) else { return }
    let start = dragStart ?? state
    switch part {
    case .top: state = KeyboardAdjustLayout.draggingTop(start, rise: rise)
    case .bottom: state = KeyboardAdjustLayout.draggingBottom(start, rise: rise)
    case .left: state = KeyboardAdjustLayout.draggingLeft(start, dx: dx, containerWidth: containerWidth)
    case .right: state = KeyboardAdjustLayout.draggingRight(start, dx: dx, containerWidth: containerWidth)
    case .move:
      state = KeyboardAdjustLayout.moving(start, dx: dx, rise: rise, containerWidth: containerWidth, capabilities: capabilities)
    }
  }

  mutating func endDrag() { dragStart = nil }

  /// 读屏的增减：变高、抬高、变宽，以及「移动」向上移为增。
  mutating func step(_ part: Part, increment: Bool) {
    guard allows(part) else { return }
    let sign: CGFloat = increment ? 1 : -1
    let widthStep = containerWidth * CGFloat(KeyboardAdjustLayout.widthStepPercent) / 100
    switch part {
    case .top: state = KeyboardAdjustLayout.steppingHeight(state, increment: increment, keyBlock: keyBlock)
    case .bottom: state = KeyboardAdjustLayout.draggingBottom(state, rise: sign * CGFloat(KeyboardAdjustLayout.liftStep))
    case .left: state = KeyboardAdjustLayout.draggingLeft(state, dx: -sign * widthStep, containerWidth: containerWidth)
    case .right: state = KeyboardAdjustLayout.draggingRight(state, dx: sign * widthStep, containerWidth: containerWidth)
    case .move: state = KeyboardAdjustLayout.steppingLift(state, up: increment, capabilities: capabilities)
    }
  }

  /// 读屏让「移动」左右移一步（位置 `positionStep`）；宽度不生效时什么也不做。
  mutating func stepPosition(right: Bool) {
    state = KeyboardAdjustLayout.steppingPosition(state, right: right, capabilities: capabilities)
  }

  /// 「重置」：四项预览回缺省，不保存。
  mutating func reset() {
    dragStart = nil
    state = .standard
  }
}
