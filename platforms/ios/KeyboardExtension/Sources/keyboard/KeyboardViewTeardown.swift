import UIKit

/// 保留输入视图空壳的泄漏规避，同时避免把整套键盘约束一次迁到另一个布局引擎。
@MainActor
enum KeyboardViewTeardown {
  static func detachSubviews(from inputView: UIView?) {
    guard let inputView else { return }
    NSLayoutConstraint.deactivate(allConstraints(in: inputView))
    inputView.subviews.forEach { $0.removeFromSuperview() }
  }

  private static func allConstraints(in view: UIView) -> [NSLayoutConstraint] {
    view.constraints + view.subviews.flatMap { allConstraints(in: $0) }
  }
}
