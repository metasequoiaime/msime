import SwiftUI
import UIKit

/// The candidate preview card that leads the 主题, 候选栏 and 输入 pages of the mobile design (dc.html L708-730, data L2157-2165): the spelling line, then six candidates on the keyboard's own background, the first in the accent and each with its gloss under it.
///
/// It draws what the keyboard strip would draw for the selected theme: the keyboard palette, or the resolved candidate palette when the iOS 候选栏使用主题配色 switch is on (see CandidatePalette). The words are fixed sample text, not engine output.
struct CandidatePreviewCard: View {
  let theme: KeyboardTheme
  /// The candidate palette the strip draws instead of the keyboard palette, nil while it follows the keyboard.
  var palette: CandidatePalette?
  var fontSize = CandidateFontPreference.defaultCandidateSize
  var families: [String] = []
  var showsGloss = true
  /// The keyboard's own light or dark setting; `.unspecified` follows this page.
  var keyboardStyle: UIUserInterfaceStyle = .unspecified

  @Environment(\.colorScheme) private var pageScheme

  /// The sample of dc.html L2163, the six a phone shows.
  private static let words = [("候选项", "candidate"), ("后选项", "choice"), ("侯选项", "option"),
                              ("2026年1月1日", "2026/1/1"), ("候選項", "candidate"), ("厚选项", "item")]

  /// The card for `document` as the keyboard reads it: the selected theme, the switch, the candidate font and the gloss setting.
  init(theme: KeyboardTheme, document: [String: Any]?, systemDark: Bool, fontSize: Int? = nil) {
    self.theme = theme
    let style = KeyboardAppearancePreference.style(KeyboardAppearancePreference.keyboardKey, in: document)
    keyboardStyle = style
    palette = CandidatePalette.active(in: document, systemDark: style == .unspecified ? systemDark : style == .dark)
    self.fontSize = fontSize ?? CandidateFontPreference.candidateSize(in: document, tablet: UIDevice.current.userInterfaceIdiom == .pad)
    families = CandidateFontPreference.families(in: document)
    showsGloss = InputHabitPreference.settings(in: document).glossEnabled
  }

  /// A theme with a fixed mode previews in it; `system` and an unbased custom theme take the keyboard's setting, then the page's.
  private var scheme: ColorScheme {
    switch theme.appearance ?? keyboardStyle {
    case .dark: .dark
    case .light: .light
    default: pageScheme
    }
  }

  private func color(_ value: UIColor) -> Color {
    Color(uiColor: value.resolvedColor(with: UITraitCollection(userInterfaceStyle: scheme == .dark ? .dark : .light)))
  }

  private var text: UIColor { palette?.text ?? theme.keyForeground }
  private var secondary: UIColor { palette?.number ?? theme.secondary }
  private var accent: UIColor { palette?.accent ?? theme.accent }

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      HStack(spacing: 10) {
        Text("中").font(.system(size: 12, weight: .semibold)).foregroundStyle(color(accent))
        Text("hou’xuan’xiang").font(.system(size: 14)).foregroundStyle(color(text))
          .frame(maxWidth: .infinity, alignment: .leading)
        Text("‹ ›").font(.system(size: 12)).foregroundStyle(color(secondary))
      }
      .padding(.horizontal, 4)
      CandidateFlowLayout(spacing: 2) {
        ForEach(Array(Self.words.enumerated()), id: \.offset) { index, word in
          candidate(index, word.0, word.1)
        }
      }
    }
    .padding(.vertical, 8).padding(.horizontal, 10)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background {
      if palette == nil {
        KeyboardSkinBackdrop(skin: theme).environment(\.colorScheme, scheme)
      } else {
        color(palette?.surface ?? theme.background)
      }
    }
    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("候选预览：" + Self.words.map(\.0).joined(separator: "，"))
    .accessibilityIdentifier("candidatePreviewCard")
  }

  private func candidate(_ index: Int, _ word: String, _ gloss: String) -> some View {
    HStack(alignment: .firstTextBaseline, spacing: 6) {
      Text("\(index + 1)").font(.system(size: 12)).foregroundStyle(color(secondary))
      VStack(alignment: .leading, spacing: 0) {
        Text(word)
          .font(Font(CandidateFontPreference.font(
            .body, scale: CGFloat(fontSize) / CGFloat(CandidateFontPreference.defaultCandidateSize), families: families)))
          .foregroundStyle(color(index == 0 ? accent : text))
          .lineLimit(1).fixedSize()
        if showsGloss {
          Text(gloss).font(.system(size: 12)).foregroundStyle(color(secondary)).lineLimit(1).fixedSize()
        }
      }
    }
    .padding(.leading, 8).padding(.trailing, 10).padding(.vertical, 4)
    .background {
      // The desktop palette fills the first candidate with its highlight; the keyboard palette leaves it bare (dc.html L2162).
      if index == 0, let hover = palette?.hover {
        RoundedRectangle(cornerRadius: 8, style: .continuous).fill(color(hover))
      }
    }
  }
}

/// Lays candidates out left to right and wraps them onto the next line, as the design's preview does with `flex-wrap`.
private struct CandidateFlowLayout: Layout {
  var spacing: CGFloat

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
    let width = rows.map(\.width).max() ?? 0
    let height = rows.reduce(0) { $0 + $1.height } + spacing * CGFloat(max(rows.count - 1, 0))
    return CGSize(width: proposal.width ?? width, height: height)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    var y = bounds.minY
    for row in arrange(width: bounds.width, subviews: subviews) {
      var x = bounds.minX
      for index in row.indices {
        let size = subviews[index].sizeThatFits(.unspecified)
        subviews[index].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
        x += size.width + spacing
      }
      y += row.height + spacing
    }
  }

  private struct Row {
    var indices: [Int] = []
    var width: CGFloat = 0
    var height: CGFloat = 0
  }

  private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
    var rows = [Row()]
    for index in subviews.indices {
      let size = subviews[index].sizeThatFits(.unspecified)
      let needed = rows[rows.count - 1].indices.isEmpty ? size.width : rows[rows.count - 1].width + spacing + size.width
      if needed > width, !rows[rows.count - 1].indices.isEmpty {
        rows.append(Row())
      }
      var row = rows[rows.count - 1]
      row.width = row.indices.isEmpty ? size.width : row.width + spacing + size.width
      row.height = max(row.height, size.height)
      row.indices.append(index)
      rows[rows.count - 1] = row
    }
    return rows
  }
}
