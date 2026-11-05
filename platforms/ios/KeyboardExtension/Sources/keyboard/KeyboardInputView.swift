import UIKit

/// 键盘控制器的根视图。UIKit checks the visible input view, not its controller, before playing input clicks, so the root adopts `UIInputViewAudioFeedback` itself.
///
/// 它是普通的 `UIView`，不是 `UIInputView`（#10944）。任何 `UIInputView` 都释放不掉：它和 UIKit 挂在它下面的两个 `_UIInputViewContent` 互相强引用（`_leftContentView` ↔ `_inputView`），iOS 18 和 27 都是这样，连一个刚建好、从没进过窗口的也是。控制器的根视图又是整套键盘约束所在布局引擎的宿主，引擎为四千来个变量长出来的存储表随根视图一起留下，摘掉子视图、停用约束也不缩回去：模拟器里每留一个根视图就多占 400–500 KB。系统每弹出一次键盘就新建一个控制器，iPhone XR 上这样弹出、收起 50 次，键盘扩展碰到上限被系统结束。普通视图随控制器释放，引擎和整棵键盘视图跟着一起释放。
///
/// 扩展里的 `UIInputView` 不画任何东西：键盘底板（iOS 26 起是 Liquid Glass）由系统在扩展窗口外面画。iOS 18.4 和 27.0 的模拟器里，原生皮肤下两种根视图的键盘截图除了光标和状态栏时钟逐像素相同，键盘高度也相同。
final class KeyboardInputView: UIView, UIInputViewAudioFeedback {
  var enableInputClicksWhenVisible: Bool { KeyboardFeedbackPreference.soundEnabled }
}
