import UIKit
import UIKit.UIGestureRecognizerSubclass

/// 18 键双拼双字母键上的左右横滑，挂在那个键自己身上。
///
/// 越过 `EighteenKeySwipePolicy` 的阈值之前一直停在 `.possible`，这时按键照常收到触摸，长按也照常弹出字母；一旦接手就开始识别并取消按键这次触摸（`cancelsTouchesInView`），这一下不再算点按。抬手时还在 `.possible` 就按抬手位置再判一次，与 Android 在 ACTION_UP 上也走一次横滑判定相同：快速一甩时 UIKit 可能只送一次没过阈值的移动，抬手那一点才过阈值，这时直接识别为选中那个字母；没过阈值才算一次普通点按。为此 `delaysTouchesEnded` 打开，按键的抬手要等这里判完才送达，识别了就换成取消，不会既输入字母又点一次整组；这里在抬手时当场判定，按键不会因此多等。接手后回到起点附近、第二根手指落在这个键上或系统取消，都不输入；反向拖过另一侧可以重选。同一个键上的长按先识别时，这个手势就不再开始，与 Android 长按后忽略横滑相同。是否在当前方案下接手由委托的 `gestureRecognizerShouldBegin` 在接手那一刻决定。
final class EighteenKeySwipeGestureRecognizer: UIGestureRecognizer {
  /// 这一键的两个字母，小写，按键面顺序。
  let letters: String
  /// 当前位移选中的字母，大写；还在阈值以内或纵向为主时为 nil。抬手时它就是要输入的字母。
  private(set) var selected: Character?
  private weak var touch: UITouch?
  private var origin: CGPoint = .zero

  init(letters: String, target: Any?, action: Selector?) {
    self.letters = letters
    super.init(target: target, action: action)
    name = "eighteenKeySwipe"
    cancelsTouchesInView = true
    delaysTouchesBegan = false
    delaysTouchesEnded = true
  }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
    guard touch == nil, touches.count == 1, let first = touches.first else {
      // 多指不是横滑：还没接手时放手让按键处理，已接手时取消，不补一次点按。
      selected = nil
      state = state == .possible ? .failed : .cancelled
      return
    }
    touch = first
    origin = first.location(in: view)
  }

  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
    guard let touch, touches.contains(touch) else { return }
    track(touch)
    if state == .possible {
      if selected != nil { state = .began }
    } else {
      state = .changed
    }
  }

  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
    guard let touch, touches.contains(touch) else { return }
    track(touch)
    if state == .possible && selected == nil {
      state = .failed
      return
    }
    state = .ended
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
    selected = nil
    state = state == .possible ? .failed : .cancelled
  }

  override func reset() {
    super.reset()
    selected = nil
    touch = nil
    origin = .zero
  }

  private func track(_ touch: UITouch) {
    let point = touch.location(in: view)
    selected = EighteenKeySwipePolicy.letter(letters, dx: point.x - origin.x, dy: point.y - origin.y)
  }
}
