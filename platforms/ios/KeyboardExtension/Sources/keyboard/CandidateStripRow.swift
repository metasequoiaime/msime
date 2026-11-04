import UIKit

/// 候选按钮的内容宽度独立求解，横向栈只排列不带文字的容器。
@MainActor
final class CandidateStripRow {
  let stack = UIStackView()
  private(set) var buttons: [KeyboardKeyButton] = []
  private var trailingConstraints: [NSLayoutConstraint] = []

  func add(_ button: KeyboardKeyButton) {
    let container = UIView()
    container.translatesAutoresizingMaskIntoConstraints = false
    button.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(button)
    let trailing = button.trailingAnchor.constraint(equalTo: container.trailingAnchor)
    NSLayoutConstraint.activate([
      button.leadingAnchor.constraint(equalTo: container.leadingAnchor),
      button.topAnchor.constraint(equalTo: container.topAnchor),
      button.bottomAnchor.constraint(equalTo: container.bottomAnchor),
      trailing,
    ])
    buttons.append(button)
    trailingConstraints.append(trailing)
    stack.addArrangedSubview(container)
  }

  func setHidden(_ hidden: Bool, at index: Int) {
    let container = stack.arrangedSubviews[index]
    guard container.isHidden != hidden else { return }
    // 隐藏容器会被栈设为零宽；先断开右边界，按钮自己的固有宽度无需一并归零。
    trailingConstraints[index].isActive = !hidden
    buttons[index].isHidden = hidden
    container.isHidden = hidden
  }
}
