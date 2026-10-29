import CoreGraphics
import Foundation

/// 26 个字母键右上角的符号提示，取自设计稿：q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。沿设定方向滑动字母键或长按即输入这个符号。与 Android 的 `LetterHintTable` 是同一张表。
enum LetterHintTable {
  private static let hints: [Character: String] = [
    "q": "1", "w": "2", "e": "3", "r": "4", "t": "5", "y": "6", "u": "7", "i": "8", "o": "9", "p": "0",
    "a": "@", "s": "#", "d": "¥", "f": "%", "g": "&", "h": "*", "j": "(", "k": ")", "l": "\"",
    "z": "~", "x": "…", "c": "、", "v": "?", "b": "!", "n": "-", "m": "/",
  ]

  /// 字母键的提示符号，大小写字母都可传入；不是 26 个 ASCII 字母之一时为 nil。
  static func hint(for letter: String) -> String? {
    guard letter.count == 1, let character = letter.first, character.isASCII else { return nil }
    return hints[Character(character.lowercased())]
  }
}

/// 字母键沿设定方向滑动超过阈值时，松手输入角标；三端默认上滑，也可在设置里改成下滑。
enum SwipeHintPolicy {
  /// 手指需要移动的距离，必须严格大于这个值。
  static let threshold: CGFloat = 14

  /// 窗口纵坐标向下增大，严格超过所选方向的阈值才接管点按。
  static func swiped(direction: KeyboardLayoutPreference.SwipeDirection = .up,
                     downY: CGFloat, currentY: CGFloat) -> Bool {
    let distance = direction == .up ? downY - currentY : currentY - downY
    return distance > threshold
  }

  /// 「长空格」逗号的滑动，与 Android 的 `ImeBottomRow.bindCommaSwipe` 一致：上滑、下滑都算（逗号没有别的纵向手势，照字母键习惯上滑也要出句号），门槛与字母键相同，纵向位移严格大于 `threshold` 且大于横向位移，点按时手指带一下不算；按下后满 `holdDelay` 还没滑出去的触摸已经归长按，之后再挪手指不起滑动。坐标是窗口坐标，`elapsed` 是从按下起的秒数。
  static func swipedEitherWay(down: CGPoint, current: CGPoint, elapsed: TimeInterval, holdDelay: TimeInterval) -> Bool {
    guard elapsed < holdDelay else { return false }
    let vertical = abs(current.y - down.y)
    return vertical > threshold && vertical > abs(current.x - down.x)
  }
}

/// 拼音九键字母层 1–9 键的滑动（「九键滑动输入数字」，`NineKeySwipeDigits`，#10229），与 Android `NineKeySwipePolicy.gesture`、鸿蒙 `NineKeySwipePolicy` 逐条相同：只看纵向位移（窗口坐标向下为正，不看横向），沿设定方向严格超过 `threshold` 是输入数字，反方向严格超过、并且这个键有字母（2–9）是弹出数字和字母；设置关着时什么也不判。
enum NineKeySwipePolicy {
  enum Gesture: Equatable {
    /// 还没滑过阈值，或设置关着：点按、长按照旧。
    case none
    /// 沿设定方向滑过阈值：松手输入键上的数字。
    case digit
    /// 往反方向滑过阈值：弹出数字和字母，与长按相同。
    case letters
  }

  /// iOS 固定 24pt，等于 Android「九键滑动距离」的默认值和鸿蒙的 24vp。比 26 键的 `SwipeHintPolicy.threshold` 远：九键的键高，快打时手指往上带一下不该打出数字。
  static let threshold: CGFloat = 24

  static func gesture(_ mode: NineKeySwipeDigits, downY: CGFloat, currentY: CGFloat, hasLetters: Bool) -> Gesture {
    let upward = downY - currentY
    switch mode {
    case .off:
      return .none
    case .up:
      if upward > threshold { return .digit }
      return hasLetters && -upward > threshold ? .letters : .none
    case .down:
      if -upward > threshold { return .digit }
      return hasLetters && upward > threshold ? .letters : .none
    }
  }
}

/// 一次触摸上的九键滑动判定：按下时记纵坐标，每次移动判一次，判出来就锁住，之后手指再怎么动（包括滑回原处）结果都不变。设置在每次判定时由调用方现读现传。
struct NineKeySwipeTracker: Equatable {
  private(set) var gesture: NineKeySwipePolicy.Gesture = .none
  private var downY: CGFloat?

  mutating func begin(y: CGFloat) {
    downY = y
    gesture = .none
  }

  /// 这一次移动刚判出的结果；还没判出、早已判出或没有按下过时为 nil。
  mutating func move(y: CGFloat, mode: NineKeySwipeDigits, hasLetters: Bool) -> NineKeySwipePolicy.Gesture? {
    guard gesture == .none, let downY else { return nil }
    let next = NineKeySwipePolicy.gesture(mode, downY: downY, currentY: y, hasLetters: hasLetters)
    guard next != .none else { return nil }
    gesture = next
    return next
  }

  /// 这次触摸结束或被取消：交回判定结果，回到没按下的状态。
  mutating func finish() -> NineKeySwipePolicy.Gesture {
    defer { self = NineKeySwipeTracker() }
    return gesture
  }
}
