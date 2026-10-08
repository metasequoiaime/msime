import SwiftUI

/// 在应用首页顶部以可关闭卡片显示的通知，样式同 设置 页的分组卡片。首页在出现或应用回到前台时加载列表（AppNotices.load）；client-core 每分钟最多向服务器请求一次。
struct NoticeBanner: View {
  let notice: AppNotice
  let dismiss: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      HStack(alignment: .firstTextBaseline) {
        Label(notice.title, systemImage: "megaphone.fill").font(.subheadline.weight(.semibold))
        Spacer()
        Button(action: dismiss) { Image(systemName: "xmark").font(.caption.weight(.semibold)) }
          .buttonStyle(.borderless).foregroundStyle(MetasequoiaTheme.sub)
          .accessibilityLabel("关闭公告").accessibilityIdentifier("dismissNotice")
      }
      Text(AppNotices.attributed(notice.body)).font(.footnote).fixedSize(horizontal: false, vertical: true)
    }
    .padding(.vertical, 18)
    .padding(.horizontal, 20)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(MetasequoiaTheme.surface, in: RoundedRectangle(cornerRadius: MetasequoiaTheme.cardRadius, style: .continuous))
    .accessibilityIdentifier("appNotice")
  }
}
