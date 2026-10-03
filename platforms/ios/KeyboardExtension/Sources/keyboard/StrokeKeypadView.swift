import UIKit

/// 笔画方案的键区：九键外框中间那块 3×3 网格换成 2×3 的笔画键（`StrokeKeyLayout.rows`）。左侧标点栏、右侧删除列和底部动作行仍是九键的，由键盘控制器摆放；这里只管笔画键本身。
///
/// 每个键画笔画字形，下方小字写笔画名，按下时把字母交给 `onStroke`。组字和查字全在 Engine。
@MainActor
final class StrokeKeypadView: UIStackView {
  /// 按下的笔画键，参数是它的字母（h s p n z x）。
  var onStroke: ((String) -> Void)?
  private(set) var keyButtons: [UIButton] = []
  private var wildcardKey: UIButton?

  /// `makeKey` 用来造一个与九键同样外观的键：标题、无障碍名称、按下动作。
  init(makeKey: (String, String, @escaping () -> Void) -> UIButton) {
    super.init(frame: .zero)
    axis = .vertical
    spacing = 7
    distribution = .fillEqually
    accessibilityIdentifier = "strokeKeypad"
    for keys in StrokeKeyLayout.rows {
      let row = UIStackView()
      row.axis = .horizontal
      row.alignment = .fill
      row.distribution = .fillEqually
      row.spacing = 6
      for key in keys {
        let button = makeKey(key.face, "笔画 \(key.name)") { [weak self] in self?.onStroke?(key.ascii) }
        button.accessibilityIdentifier = "strokeKey\(key.ascii)"
        Self.addName(key.name, to: button)
        if key.ascii == StrokeKeyLayout.wildcard {
          wildcardKey = button
          button.accessibilityHint = "已有笔画时匹配任意一笔"
        }
        keyButtons.append(button)
        row.addArrangedSubview(button)
      }
      addArrangedSubview(row)
    }
    setComposing(false)
  }

  @available(*, unavailable)
  required init(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

  /// 每行之间、每键之间的间距，跟九键网格走同一套几何。
  func applySpacing(row: CGFloat, key: CGFloat) {
    spacing = row
    for case let stack as UIStackView in arrangedSubviews { stack.spacing = key }
  }

  /// 空组合时通配键不起作用（Engine 不拿它开始组合），所以变灰，免得按了没反应却以为键坏了。
  func setComposing(_ composing: Bool) {
    guard let wildcardKey, wildcardKey.isEnabled != composing else { return }
    wildcardKey.isEnabled = composing
    wildcardKey.alpha = composing ? 1 : 0.45
  }

  /// 字形下方的笔画名，像九键键上的数字角标一样不单独读出（名字已在无障碍名称里）。
  private static func addName(_ name: String, to button: UIButton) {
    let label = UILabel()
    label.text = name
    label.font = .systemFont(ofSize: 10)
    label.textColor = KeyboardTheme.current.accent
    label.accessibilityIdentifier = "strokeKeyName"
    label.isAccessibilityElement = false
    label.translatesAutoresizingMaskIntoConstraints = false
    button.addSubview(label)
    NSLayoutConstraint.activate([
      label.bottomAnchor.constraint(equalTo: button.bottomAnchor, constant: -3),
      label.centerXAnchor.constraint(equalTo: button.centerXAnchor),
    ])
    if var configuration = button.configuration {
      configuration.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 0, bottom: 10, trailing: 0)
      button.configuration = configuration
    }
  }
}
