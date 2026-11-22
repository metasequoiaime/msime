import UIKit

extension KeyboardViewController {
  /// 手机键面第三排的七个字母与第一排的字母等宽，与 iOS 系统键盘相同：⇧ 和 ⌫ 仍是 `phoneLetterEdgeWidth`，多出来的宽度不再摊进字母，而是变成 ⇧ 与 Z、M 与 ⌫ 之间的空隙。以前七个字母平分 ⇧、⌫ 之外的整段，390pt 宽的 iPhone 上比第一排宽约 7%。
  ///
  /// 每段空隙由两格透明占位（`KeyRoutingSpacer`）撑开，靠外的一格把触摸交给 ⇧ 或 ⌫，靠里的一格交给 Z 或 M，空隙里没有死区。⇧ 与第一格、最后一格与 ⌫ 之间照常留键距，其余相邻处不留，所以空隙是一个键距加两格占位的宽度：宽度够时与不留键距、全由占位格撑开时一样宽，键盘窄到占位格缩成 0 时（单手模式、iPad 浮动键盘）也不会小于普通键距、让 ⇧ 贴着 Z。四格等宽，宽度不小于 0：放不下时（字母等宽约束低于 required）字母让步，照旧平分。
  func installPhoneThirdRowSpacers(in row: UIStackView, shift: UIView, letters: [UIView], delete: UIControl) {
    guard let first = letters.first, let last = letters.last else { return }
    let spacers = (0..<4).map { _ in KeyRoutingSpacer() }
    spacers[0].owner = shift as? UIControl
    spacers[1].owner = first as? UIControl
    spacers[2].owner = last as? UIControl
    spacers[3].owner = delete
    for spacer in spacers {
      spacer.isAccessibilityElement = false
      spacer.accessibilityIdentifier = "thirdRowSpacer"
      spacer.setContentHuggingPriority(.init(1), for: .horizontal)
      spacer.setContentCompressionResistancePriority(.init(1), for: .horizontal)
      spacer.widthAnchor.constraint(greaterThanOrEqualToConstant: 0).isActive = true
    }
    row.insertArrangedSubview(spacers[0], at: 1)
    row.insertArrangedSubview(spacers[1], at: 2)
    let deleteIndex = row.arrangedSubviews.firstIndex(of: delete) ?? row.arrangedSubviews.count
    row.insertArrangedSubview(spacers[2], at: deleteIndex)
    row.insertArrangedSubview(spacers[3], at: deleteIndex + 1)
    NSLayoutConstraint.activate(spacers.dropFirst().map { $0.widthAnchor.constraint(equalTo: spacers[0].widthAnchor) })
    if let firstRowLetter = letterButtons.first?.button, firstRowLetter !== first {
      let match = first.widthAnchor.constraint(equalTo: firstRowLetter.widthAnchor)
      match.priority = .init(999)
      phoneThirdRowLetterWidth = match
    }
    phoneThirdRowSpacers = spacers
    phoneThirdRowTightViews = [spacers[0], spacers[1], last, spacers[2]]
  }

  /// 按键面形态启用或停用第三排的等宽字母和两段空隙：手机键面启用；iPad 键面这一排是 ⇧ | 七个字母 | ，。| ⇧，按 `TabletLetterLayout` 的比例排，占位格隐藏、键距照常。
  func applyPhoneThirdRowSpacing(tablet: Bool) {
    // 先停用不属于这种键面的约束，再启用属于它的。
    if tablet {
      phoneThirdRowLetterWidth?.isActive = false
      NSLayoutConstraint.activate(tabletThirdRowWidths)
    } else {
      NSLayoutConstraint.deactivate(tabletThirdRowWidths)
      phoneThirdRowLetterWidth?.isActive = true
    }
    for spacer in phoneThirdRowSpacers where spacer.isHidden != tablet { spacer.isHidden = tablet }
    guard let row = phoneThirdRowSpacers.first?.superview as? UIStackView else { return }
    for view in phoneThirdRowTightViews {
      row.setCustomSpacing(tablet ? UIStackView.spacingUseDefault : 0, after: view)
    }
  }
}
