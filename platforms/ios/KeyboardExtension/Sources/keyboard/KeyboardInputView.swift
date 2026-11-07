import UIKit

/// 系统键盘宿主要求 `inputView` 是 `UIInputView`，键盘自己的布局则放在普通视图里，避免布局引擎跟着系统输入视图泄漏。
final class KeyboardInputContainerView: UIInputView, UIInputViewAudioFeedback {
  init(contentView: UIView) {
    super.init(frame: .zero, inputViewStyle: .keyboard)
    contentView.frame = bounds
    contentView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    addSubview(contentView)
  }

  required init?(coder: NSCoder) {
    fatalError("KeyboardInputContainerView 不支持从 coder 初始化")
  }

  var enableInputClicksWhenVisible: Bool { KeyboardFeedbackPreference.soundEnabled }

  /// 控制器释放时先摘掉普通视图，让系统保留下来的输入视图只剩空壳。
  func detachContentView() {
    subviews.filter { $0 is KeyboardInputView }.forEach { $0.removeFromSuperview() }
  }
}

/// 键盘控制器的布局视图。按键音协议由外层的 `KeyboardInputContainerView` 实现，因为 UIKit 检查可见的 `inputView`，不检查控制器的根视图。
///
/// 它是普通的 `UIView`，不是 `UIInputView`（#10944）。任何 `UIInputView` 都释放不掉：它和 UIKit 挂在它下面的两个 `_UIInputViewContent` 互相强引用（`_leftContentView` ↔ `_inputView`），iOS 18 和 27 都是这样，连一个刚建好、从没进过窗口的也是。控制器的根视图又是整套键盘约束所在布局引擎的宿主，引擎为四千来个变量长出来的存储表随根视图一起留下，摘掉子视图、停用约束也不缩回去：模拟器里每留一个根视图就多占 400–500 KB。系统每弹出一次键盘就新建一个控制器，iPhone XR 上这样弹出、收起 50 次，键盘扩展碰到上限被系统结束。普通视图随控制器释放，引擎和整棵键盘视图跟着一起释放。
///
/// 扩展里的 `UIInputView` 不画任何东西：键盘底板（iOS 26 起是 Liquid Glass）由系统在扩展窗口外面画。iOS 18.4 和 27.0 的模拟器里，原生皮肤下两种根视图的键盘截图除了光标和状态栏时钟逐像素相同，键盘高度也相同。
final class KeyboardInputView: UIView, UIInputViewAudioFeedback {
  var enableInputClicksWhenVisible: Bool { KeyboardFeedbackPreference.soundEnabled }
}
