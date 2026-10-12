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

/// A navigation row label with an optional value and trailing chevron.
struct SettingsNavigationLabel: View {
  let title: String
  let value: String?
  let showsChevron: Bool
  let valueTruncationMode: Text.TruncationMode

  init(title: String, value: String? = nil, showsChevron: Bool = true,
       valueTruncationMode: Text.TruncationMode = .tail) {
    self.title = title
    self.value = value
    self.showsChevron = showsChevron
    self.valueTruncationMode = valueTruncationMode
  }

  var body: some View {
    HStack(spacing: 8) {
      Text(title).font(.system(size: 16)).foregroundStyle(.primary)
      Spacer(minLength: 12)
      if let value {
        Text(value).font(.system(size: 15)).foregroundStyle(MetasequoiaTheme.sub)
          .lineLimit(1).truncationMode(valueTruncationMode)
      }
      if showsChevron {
        DesignChevron()
      }
    }
    .padding(.horizontal, 16)
    .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
    .contentShape(Rectangle())
    .accessibilityElement(children: .combine)
  }
}
