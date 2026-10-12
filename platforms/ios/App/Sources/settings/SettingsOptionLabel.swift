import SwiftUI

/// A setting title with its explanatory detail beneath it.
struct SettingsOptionLabel: View {
  let title: String
  let detail: String
  let titleFont: Font?
  let detailFont: Font
  let spacing: CGFloat

  init(title: String, detail: String, titleFont: Font? = nil, detailFont: Font = .footnote,
       spacing: CGFloat = 2) {
    self.title = title
    self.detail = detail
    self.titleFont = titleFont
    self.detailFont = detailFont
    self.spacing = spacing
  }

  var body: some View {
    VStack(alignment: .leading, spacing: spacing) {
      if let titleFont {
        Text(title).font(titleFont)
      } else {
        Text(title)
      }
      Text(detail).font(detailFont).foregroundStyle(.secondary)
    }
  }
}
