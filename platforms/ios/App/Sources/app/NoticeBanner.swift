import SwiftUI

/// A notice as a dismissible card at the top of the app home. The home loads the list (AppNotices.load) when it appears or the app returns to the foreground; client-core never asks the server more than once a minute.
struct NoticeBanner: View {
  let notice: AppNotice
  let dismiss: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      HStack(alignment: .firstTextBaseline) {
        Label(notice.title, systemImage: "megaphone.fill").font(.subheadline.weight(.semibold))
        Spacer()
        Button(action: dismiss) { Image(systemName: "xmark").font(.caption.weight(.semibold)) }
          .buttonStyle(.borderless).foregroundStyle(.secondary)
          .accessibilityLabel("关闭公告").accessibilityIdentifier("dismissNotice")
      }
      Text(AppNotices.attributed(notice.body)).font(.footnote).fixedSize(horizontal: false, vertical: true)
    }
    .padding(.vertical, 4)
    .accessibilityIdentifier("appNotice")
  }
}
