import SwiftUI

/// A setting title with its explanatory detail beneath it.
struct SettingsOptionLabel: View {
  let title: String
  let detail: String

  var body: some View {
    VStack(alignment: .leading, spacing: 2) {
      Text(title)
      Text(detail).font(.footnote).foregroundStyle(.secondary)
    }
  }
}
