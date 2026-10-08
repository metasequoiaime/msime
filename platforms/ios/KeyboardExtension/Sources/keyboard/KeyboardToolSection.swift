import UIKit

/// 键盘功能面板（「更多工具」）里的一项：图块画什么、是不是开关、点按和长按各做什么。
///
/// 面板是按显示顺序排列、分页的扁平列表；顺序、开关的当前值和动作都由键盘决定，面板只负责绘制。
struct KeyboardTool {
  /// 图块在标签上方画的内容：圆角框里的一个字符（全、，、≈、繁），或一个描边矢量图标。
  enum Face {
    case glyph(String)
    case icon(KeyboardIcon)
  }

  /// 面板据以复用图块视图的稳定键，这样更新时只重设已有图块的样式，不重建整页。
  let id: String
  /// 画在图形下方的短标签；也是图块无障碍标识符 `moreCard-<title>` 的后半段。
  let title: String
  /// 与标签不同时朗读的名称（「繁体」读作「繁体输出」，「振动」读作「按键振动」）；为 `nil` 时朗读标签。
  var accessibilityLabel: String? = nil
  let face: Face
  /// 图块是否为开关：开关会朗读「已开启」/「已关闭」。
  var isToggle = false
  /// 已开启的工具用强调色绘制，标签加粗为 semibold，并带一个勾选角标。
  var selected = false
  /// 不可用的工具显示为变暗，朗读「不可用」，并忽略点按。
  var enabled = true
  /// 动作类图块额外朗读的状态，例如当前的振动强度；不会画出来。
  var caption: String? = nil
  let run: () -> Void
  var longPress: (() -> Void)? = nil
}
