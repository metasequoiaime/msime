import UIKit

/// 长按菜单优先贴在按键上方；首行空间不足时留在扩展可命中的边界内。
@MainActor
enum KeyboardHoldOptionsLayout {
  static func activate(_ options: UIView, from key: UIView, in container: UIView) {
    let above = options.bottomAnchor.constraint(equalTo: key.topAnchor, constant: -6)
    above.priority = .defaultHigh
    let centred = options.centerXAnchor.constraint(equalTo: key.centerXAnchor)
    centred.priority = .defaultHigh
    NSLayoutConstraint.activate([
      above, centred,
      options.topAnchor.constraint(greaterThanOrEqualTo: container.topAnchor, constant: 6),
      options.bottomAnchor.constraint(lessThanOrEqualTo: container.bottomAnchor, constant: -6),
      options.leadingAnchor.constraint(greaterThanOrEqualTo: container.leadingAnchor, constant: 6),
      options.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor, constant: -6),
    ])
  }
}
