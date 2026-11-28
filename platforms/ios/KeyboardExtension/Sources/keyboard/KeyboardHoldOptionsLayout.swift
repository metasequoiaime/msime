import UIKit

/// 长按菜单始终贴在按键上方，不再翻到下面。
///
/// 键盘扩展画不出自己的窗口，菜单不能像系统键盘那样浮在键盘外面，只能盖住相邻的键行。原先顶排键上方放不下时
/// 会翻到按键下方：同一个长按，第 1 排和个别第 2 排的键在指下弹出，与其余键不一致。现在统一放上方——
/// 上方确实不够高时由容器的顶边约束把它压回键盘内，宁可盖住这一排自己，也不换到手指底下。
@MainActor
enum KeyboardHoldOptionsLayout {
  /// 气泡里每个选项格子的宽度；按住不放左右滑动时按这个宽度换算高亮到第几项。
  static let itemWidth: CGFloat = 36
  /// 相邻格子的间隔，与气泡里 UIStackView 的 spacing 一致。
  static let itemSpacing: CGFloat = 4

  /// 手指横向位置落在哪个选项上：取水平距离最近的那个选项中心。
  ///
  /// 与「相对按下点位移多少格」不同，这里直接用气泡里按钮的真实位置，手指到哪项就高亮哪项；
  /// 按下时也是同一条规则，于是默认高亮的是离手指最近的一项。并列时取靠前的项（左/上优先）。
  static func nearestIndex(to x: CGFloat, centers: [CGFloat]) -> Int {
    guard let first = centers.first else { return 0 }
    var best = 0
    var bestDistance = abs(first - x)
    for index in centers.indices.dropFirst() {
      let distance = abs(centers[index] - x)
      if distance < bestDistance {
        best = index
        bestDistance = distance
      }
    }
    return best
  }

  static func activate(_ options: UIView, from key: UIView, in container: UIView) {
    let anchor = options.bottomAnchor.constraint(equalTo: key.topAnchor, constant: -6)
    anchor.priority = .defaultHigh
    let centred = options.centerXAnchor.constraint(equalTo: key.centerXAnchor)
    centred.priority = .defaultHigh
    NSLayoutConstraint.activate([
      anchor, centred,
      options.topAnchor.constraint(greaterThanOrEqualTo: container.topAnchor, constant: 6),
      options.bottomAnchor.constraint(lessThanOrEqualTo: container.bottomAnchor, constant: -6),
      options.leadingAnchor.constraint(greaterThanOrEqualTo: container.leadingAnchor, constant: 6),
      options.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor, constant: -6),
    ])
  }
}
