import UIKit

struct KeyboardCandidateAnnotation: Equatable {
  let text: String
  let accessibilityDescription: String

  static let none = KeyboardCandidateAnnotation(text: "", accessibilityDescription: "")
}

// Every candidate the engine returned, laid out over the keys.
//
// The strip shows nine at a time and the arrows advanced by nine, so a query answering with 351
// candidates -- `yi` does -- put the tail thirty-nine taps away. Nobody reaches it, which reads as
// the word not being in the dictionary. This shows the whole list at once instead.
final class KeyboardCandidatePanelView: UIView {
  private static let annotatedColumns: CGFloat = 3
  private static let rowSpacing: CGFloat = 6
  private let candidates: [String]
  private let candidateScale: CGFloat
  private let candidateFamilies: [String]
  private var annotations: [KeyboardCandidateAnnotation]
  private let markers: [[CandidateMarker]]
  private let display: (String) -> String
  private let onSelect: (Int) -> Void
  /// What a long press on a chip offers. Shared with the strip -- a press that manages an entry
  /// there should not stop working once the list is expanded.
  private let menuElements: (Int) -> [UIMenuElement]
  private let rows = UIStackView()
  private let scrollView = UIScrollView()
  private var laidOutWidth: CGFloat = 0

  init(candidates: [String], preedit: String, annotations: [KeyboardCandidateAnnotation] = [],
       markers: [[CandidateMarker]] = [],
       candidateScale: CGFloat = 1, preeditScale: CGFloat = 1, candidateFamilies: [String] = [],
       display: @escaping (String) -> String,
       menuElements: @escaping (Int) -> [UIMenuElement] = { _ in [] },
       onSelect: @escaping (Int) -> Void, onClose: @escaping () -> Void) {
    self.candidates = candidates
    self.candidateScale = candidateScale
    self.candidateFamilies = candidateFamilies
    self.annotations = annotations
    self.markers = markers
    self.display = display
    self.menuElements = menuElements
    self.onSelect = onSelect
    super.init(frame: .zero)
    accessibilityIdentifier = "candidatePanel"
    backgroundColor = KeyboardTheme.current.keyBackground.withAlphaComponent(0.98)

    let spelling = UILabel()
    spelling.text = preedit
    spelling.font = CandidateFontPreference.font(.subheadline, scale: preeditScale)
    spelling.adjustsFontForContentSizeCategory = true
    spelling.textColor = KeyboardTheme.current.accent
    spelling.accessibilityIdentifier = "candidatePanelSpelling"

    let count = UILabel()
    count.text = "\(candidates.count) 个候选"
    count.font = .preferredFont(forTextStyle: .footnote)
    count.adjustsFontForContentSizeCategory = true
    count.textColor = KeyboardTheme.current.keyForeground.withAlphaComponent(0.6)

    var closeConfiguration = UIButton.Configuration.plain()
    closeConfiguration.image = UIImage(systemName: "chevron.up")
    closeConfiguration.baseForegroundColor = KeyboardTheme.current.keyForeground
    let close = UIButton(
      configuration: closeConfiguration,
      primaryAction: UIAction { _ in onClose() })
    close.accessibilityIdentifier = "closeCandidatePanel"
    close.accessibilityLabel = "收起候选"
    close.setContentHuggingPriority(.required, for: .horizontal)

    let header = UIStackView(arrangedSubviews: [spelling, count, UIView(), close])
    header.axis = .horizontal
    header.alignment = .center
    header.spacing = 8
    header.translatesAutoresizingMaskIntoConstraints = false
    addSubview(header)

    rows.axis = .vertical
    rows.spacing = 6
    rows.alignment = .leading
    rows.translatesAutoresizingMaskIntoConstraints = false
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    scrollView.disableEdgeEffects()
    scrollView.addSubview(rows)
    addSubview(scrollView)

    NSLayoutConstraint.activate([
      header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
      header.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
      header.topAnchor.constraint(equalTo: topAnchor, constant: 6),
      header.heightAnchor.constraint(equalToConstant: 32),
      scrollView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
      scrollView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
      scrollView.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 4),
      scrollView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
      rows.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor),
      rows.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor),
      rows.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor),
      rows.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor),
      rows.widthAnchor.constraint(equalTo: scrollView.frameLayoutGuide.widthAnchor),
    ])
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

  func updateAnnotations(_ annotations: [KeyboardCandidateAnnotation]) {
    self.annotations = annotations
    guard laidOutWidth > 0 else { return }
    rebuildRows(within: laidOutWidth)
    // The rows were just replaced, so the chips that came back have no frame yet. Ask for a
    // layout pass rather than leaving them to whatever happens to dirty the panel next.
    setNeedsLayout()
  }

  // Rows are packed against a known width, so they are built here rather than in init.
  override func layoutSubviews() {
    super.layoutSubviews()
    let available = scrollView.bounds.width
    guard available > 0, available != laidOutWidth else { return }
    laidOutWidth = available
    rebuildRows(within: available)
  }

  private func rebuildRows(within available: CGFloat) {
    for row in rows.arrangedSubviews {
      rows.removeArrangedSubview(row)
      row.removeFromSuperview()
    }
    let spacing = Self.rowSpacing
    var row = makeRow(spacing: spacing)
    var used: CGFloat = 0
    for (offset, candidate) in candidates.enumerated() {
      let chip = makeChip(candidate: candidate, number: offset + 1)
      let width = min(naturalWidth(of: chip, available: available), available)
      chip.widthAnchor.constraint(equalToConstant: width).isActive = true
      if used > 0, used + spacing + width > available {
        rows.addArrangedSubview(row)
        row = makeRow(spacing: spacing)
        used = 0
      }
      row.addArrangedSubview(chip)
      used += (used > 0 ? spacing : 0) + width
    }
    if !row.arrangedSubviews.isEmpty { rows.addArrangedSubview(row) }
  }

  private func naturalWidth(of chip: UIButton, available: CGFloat) -> CGFloat {
    guard let title = chip.configuration?.attributedTitle.map({ NSAttributedString($0) }) else {
      return chip.intrinsicContentSize.width
    }
    let text = title.string as NSString
    var lineWidths: [CGFloat] = []
    text.enumerateSubstrings(in: NSRange(location: 0, length: text.length),
                             options: [.byLines, .substringNotRequired]) { _, range, _, _ in
      lineWidths.append(title.attributedSubstring(from: range).size().width)
    }
    let insets = chip.configuration?.contentInsets ?? .zero
    let titleWidth = ceil((lineWidths.first ?? 0) + insets.leading + insets.trailing)
    guard lineWidths.count > 1 else { return titleWidth }

    // A translation arrives asynchronously. Reserve a stable three-column width for annotated
    // candidates so a long answer cannot resize this page and shift every later candidate.
    let column = max(0, (available - Self.rowSpacing * (Self.annotatedColumns - 1))
      / Self.annotatedColumns)
    return max(titleWidth, ceil(column))
  }

  private func makeRow(spacing: CGFloat) -> UIStackView {
    let row = UIStackView()
    row.axis = .horizontal
    row.spacing = spacing
    row.alignment = .center
    return row
  }

  private func makeChip(candidate: String, number: Int) -> UIButton {
    let text = display(candidate)
    let annotation = annotations.indices.contains(number - 1) ? annotations[number - 1] : .none
    let marks = markers.indices.contains(number - 1) ? markers[number - 1] : []
    let secondary = KeyboardTheme.current.secondary
    var configuration = UIButton.Configuration.plain()
    let paragraph = NSMutableParagraphStyle()
    paragraph.lineBreakMode = .byTruncatingTail
    var title = AttributedString(text, attributes: AttributeContainer([
      .font: CandidateFontPreference.font(.body, scale: candidateScale, families: candidateFamilies),
      .paragraphStyle: paragraph,
    ]))
    title += KeyboardViewController.markerRun(marks, color: secondary, scale: candidateScale)
    if !annotation.text.isEmpty {
      let lines = annotation.text.split(separator: "\n", omittingEmptySubsequences: false)
      for line in lines {
        title += AttributedString("\n" + String(line), attributes: AttributeContainer([
          .font: UIFont.preferredFont(forTextStyle: .caption2), .paragraphStyle: paragraph,
          .foregroundColor: secondary,
        ]))
      }
    }
    configuration.attributedTitle = title
    configuration.baseForegroundColor = marks.contains { $0.symbol == "pin.fill" }
      ? KeyboardTheme.current.accent : KeyboardTheme.current.keyForeground
    configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 11, bottom: 6, trailing: 11)
    configuration.background.backgroundColor = KeyboardTheme.current.keyBackground
    configuration.background.strokeColor =
      KeyboardTheme.current.accent.withAlphaComponent(0.22)
    configuration.background.strokeWidth = 1
    configuration.background.cornerRadius = 9
    let index = number - 1
    let chip = KeyboardKeyButton(
      configuration: configuration,
      primaryAction: UIAction { [weak self] _ in self?.onSelect(index) })
    chip.titleLineCount = 1 + annotation.text.split(separator: "\n", omittingEmptySubsequences: false).count
    chip.accessibilityIdentifier = "panelCandidate-\(number)"
    chip.accessibilityLabel = annotation.accessibilityDescription.isEmpty
      ? "候选词 \(number)：\(text)"
      : "候选词 \(number)：\(text)，\(annotation.accessibilityDescription)"
    for marker in marks { chip.accessibilityLabel? += "，\(marker.spoken)" }
    // Built when the press opens it, not for every chip up front: this panel lays out the whole
    // list, which for a query like `yi` is several hundred of them.
    chip.menu = UIMenu(children: [
      UIDeferredMenuElement.uncached { [weak self] completion in
        completion(self?.menuElements(index) ?? [])
      }
    ])
    return chip
  }
}
