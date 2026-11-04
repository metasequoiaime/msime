import UIKit

/// 混合滑行中含英文的词块留出一个备选的位置；候选本身与选词身份保持完整。
@MainActor
enum CandidateStripSizing {
  static func apply(to button: KeyboardKeyButton, word: String, viewport: UIScrollView,
                    enabled: Bool, hasAlternatives: Bool) {
    let english = word.utf8.contains {
      (65...90).contains($0) || (97...122).contains($0)
    }
    let limited = enabled && hasAlternatives && english
    let spacing = (button.superview as? UIStackView)?.spacing ?? 6
    let identifier = "candidateEnglishMaximumWidth"
    let maximum = button.constraints.first { $0.identifier == identifier }
      ?? viewport.constraints.first { $0.identifier == identifier && $0.firstItem === button }
    button.setContentCompressionResistancePriority(limited ? .defaultHigh : .required, for: .horizontal)
    guard limited else {
      maximum?.isActive = false
      button.constraints.first { $0.identifier == "candidateChipWidth" }?.priority = .init(999)
      return
    }
    // 保留内容宽度作为低一档的偏好：短词不会逐字母换行，长词仍服从半行上限；不依赖首次显示前尚为零的视口宽度。
    if let fixed = button.constraints.first(where: { $0.identifier == "candidateChipWidth" }) {
      fixed.priority = .init(998)
    }
    if let maximum {
      maximum.constant = -spacing / 2
      maximum.isActive = true
    } else {
      let constraint = button.widthAnchor.constraint(lessThanOrEqualTo: viewport.widthAnchor, multiplier: 0.5, constant: -spacing / 2)
      constraint.identifier = identifier
      constraint.priority = .init(999)
      constraint.isActive = true
    }
  }
}
