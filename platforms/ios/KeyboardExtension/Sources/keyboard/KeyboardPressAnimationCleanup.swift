import UIKit

extension UIView {
  /// 清理控件离开窗口时可能残留的按压动画状态。
  func resetKeyboardPressAnimationAfterWindowRemoval() {
    layer.removeAllAnimations()
    transform = .identity
  }
}
